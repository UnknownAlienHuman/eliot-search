//! Body-bound admission and terminal lifecycle for [`super::BoundSession`].

use search_contracts::{ProtocolVersion, RequestId};

use crate::error::ProtocolError;
use crate::grant::{
    AuthenticatedStandaloneGrantEnvelope, verify_standalone_grant_envelope_proof,
};
use crate::pairing::{ProofDigest, ServerNonce, verify_proof};
use crate::request::{
    AuthenticatedEnvelope, InFlightEntry, MonotonicMillis, RequestGuard, RequestStatus,
    verify_envelope_proof,
};
use crate::session::SessionState;
use crate::terminal::TerminalKind;

use super::BoundSession;

impl BoundSession {
    /// Admits one authenticated shell envelope and exact external body digest.
    ///
    /// The envelope proof is verified first. The adapter-observed digest of
    /// the exact body bytes must then equal the digest authenticated by the
    /// envelope before deadline, sequence, replay or in-flight state mutates.
    ///
    /// # Errors
    ///
    /// Returns [`ProtocolError`] when authentication, body binding, deadline,
    /// sequence, replay or finite-capacity checks fail.
    pub fn admit_body_bound(
        &mut self,
        envelope: &AuthenticatedEnvelope,
        expected_proof: &ProofDigest,
        observed_body_digest: &ProofDigest,
        sequence: u64,
        now: MonotonicMillis,
    ) -> Result<RequestGuard, ProtocolError> {
        self.admit_body_bound_with_deadline(
            envelope,
            expected_proof,
            observed_body_digest,
            sequence,
            now,
            None,
        )
    }

    /// Admits one authenticated shell envelope, exact body digest and optional
    /// relative deadline.
    ///
    /// Every check that can fail without recovery runs before session sequence
    /// or replay mutation. On success the connection-owned guard remains live
    /// until [`BoundSession::complete_request`], cancellation or disconnect.
    ///
    /// # Errors
    ///
    /// Returns [`ProtocolError`] when authentication, body binding, deadline,
    /// sequence, replay or finite-capacity checks fail.
    pub fn admit_body_bound_with_deadline(
        &mut self,
        envelope: &AuthenticatedEnvelope,
        expected_proof: &ProofDigest,
        observed_body_digest: &ProofDigest,
        sequence: u64,
        now: MonotonicMillis,
        relative_deadline_ms: Option<u64>,
    ) -> Result<RequestGuard, ProtocolError> {
        self.admit_envelope_internal(
            envelope,
            expected_proof,
            Some(observed_body_digest),
            sequence,
            now,
            relative_deadline_ms,
        )
    }

    /// Admits one dedicated authenticated standalone-grant envelope.
    ///
    /// The grant-specific proof domain and exact canonical body digest are
    /// both verified before mutable session state.
    ///
    /// # Errors
    ///
    /// Returns [`ProtocolError`] when authentication, body binding, deadline,
    /// sequence, replay or finite-capacity checks fail.
    pub fn admit_standalone_grant(
        &mut self,
        envelope: &AuthenticatedStandaloneGrantEnvelope,
        expected_proof: &ProofDigest,
        observed_body_digest: &ProofDigest,
        sequence: u64,
        now: MonotonicMillis,
    ) -> Result<RequestGuard, ProtocolError> {
        self.admit_standalone_grant_with_deadline(
            envelope,
            expected_proof,
            observed_body_digest,
            sequence,
            now,
            None,
        )
    }

    /// Admits one dedicated standalone-grant envelope with an optional
    /// relative deadline.
    ///
    /// # Errors
    ///
    /// Returns [`ProtocolError`] when authentication, body binding, deadline,
    /// sequence, replay or finite-capacity checks fail.
    pub fn admit_standalone_grant_with_deadline(
        &mut self,
        envelope: &AuthenticatedStandaloneGrantEnvelope,
        expected_proof: &ProofDigest,
        observed_body_digest: &ProofDigest,
        sequence: u64,
        now: MonotonicMillis,
        relative_deadline_ms: Option<u64>,
    ) -> Result<RequestGuard, ProtocolError> {
        self.validate_session_header(envelope.version(), envelope.server_nonce())?;
        verify_standalone_grant_envelope_proof(envelope, expected_proof)?;
        self.commit_authenticated_request(
            *envelope.request_id(),
            envelope.body_digest(),
            Some(observed_body_digest),
            sequence,
            now,
            relative_deadline_ms,
        )
    }

    pub(super) fn admit_envelope_internal(
        &mut self,
        envelope: &AuthenticatedEnvelope,
        expected_proof: &ProofDigest,
        observed_body_digest: Option<&ProofDigest>,
        sequence: u64,
        now: MonotonicMillis,
        relative_deadline_ms: Option<u64>,
    ) -> Result<RequestGuard, ProtocolError> {
        self.validate_session_header(envelope.version(), envelope.server_nonce())?;
        verify_envelope_proof(envelope, expected_proof)?;
        self.commit_authenticated_request(
            *envelope.request_id(),
            envelope.body_digest(),
            observed_body_digest,
            sequence,
            now,
            relative_deadline_ms,
        )
    }

    fn validate_session_header(
        &self,
        version: ProtocolVersion,
        nonce: &ServerNonce,
    ) -> Result<(), ProtocolError> {
        if !self.is_active() {
            return Err(match self.session.state() {
                SessionState::Draining => ProtocolError::SessionDraining,
                SessionState::Closed => ProtocolError::SessionClosed,
                SessionState::Quarantined => ProtocolError::Quarantined,
                _ => ProtocolError::AuthenticationRequired,
            });
        }
        if version != self.pairing.version() {
            return Err(ProtocolError::NoCompatibleVersion);
        }
        if nonce != &self.server_nonce {
            return Err(ProtocolError::AuthenticationFailed);
        }
        Ok(())
    }

    fn commit_authenticated_request(
        &mut self,
        request_id: RequestId,
        authenticated_body_digest: &ProofDigest,
        observed_body_digest: Option<&ProofDigest>,
        sequence: u64,
        now: MonotonicMillis,
        relative_deadline_ms: Option<u64>,
    ) -> Result<RequestGuard, ProtocolError> {
        if observed_body_digest
            .is_some_and(|observed| !verify_proof(authenticated_body_digest, observed))
        {
            return Err(ProtocolError::InvalidBody);
        }

        let guard = RequestGuard::new(
            request_id,
            sequence,
            now,
            relative_deadline_ms,
            self.limits,
        )?;
        if self.inflight.len() >= self.limits.max_in_flight_requests {
            return Err(ProtocolError::ResourceExhausted);
        }
        self.session.admit_request(request_id, sequence)?;
        if self
            .inflight
            .insert(
                request_id,
                InFlightEntry::new(sequence, now, guard.deadline()),
            )
            .is_err()
        {
            let _ = self.session.quarantine();
            return Err(ProtocolError::Quarantined);
        }
        self.guards.insert(request_id, guard.clone());
        Ok(guard)
    }

    /// Records the single terminal result and releases the in-flight slot.
    ///
    /// The connection-owned guard remains retained until disconnect so a
    /// repeated terminal can be rejected distinctly. A cancelled request may
    /// complete only as [`TerminalKind::Cancelled`].
    ///
    /// # Errors
    ///
    /// Returns [`ProtocolError::DuplicateTerminal`] for a repeated terminal,
    /// [`ProtocolError::InvalidSessionTransition`] for unknown or contradictory
    /// state, or the underlying progress/terminal error.
    pub fn complete_request(
        &mut self,
        request_id: &RequestId,
        terminal: TerminalKind,
    ) -> Result<RequestStatus, ProtocolError> {
        let cancelled = {
            let guard = self
                .guards
                .get(request_id)
                .ok_or(ProtocolError::InvalidSessionTransition)?;
            if guard
                .progress()
                .is_some_and(|progress| progress.terminal().is_some())
            {
                return Err(ProtocolError::DuplicateTerminal);
            }
            guard.is_cancelled()
        };
        if cancelled && terminal != TerminalKind::Cancelled {
            return Err(ProtocolError::InvalidSessionTransition);
        }

        self.guards
            .get_mut(request_id)
            .ok_or(ProtocolError::InvalidSessionTransition)?
            .finish(terminal, self.limits)?;

        let released = self.inflight.remove(request_id);
        if !released && !cancelled {
            let _ = self.session.quarantine();
            return Err(ProtocolError::Quarantined);
        }
        Ok(RequestStatus::from_terminal(terminal))
    }

    /// Whether an admitted request still occupies one in-flight slot.
    #[must_use]
    pub fn is_request_in_flight(&self, request_id: &RequestId) -> bool {
        self.inflight.contains(request_id)
    }
}

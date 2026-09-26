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

    pub(super) fn validate_session_header(
        &self,
        version: ProtocolVersion,
        nonce: &ServerNonce,
    ) -> Result<(), ProtocolError> {
        if !self.is_active() {
            return Err(match self.session_state() {
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

        self.commit_checked_request(request_id, sequence, now, relative_deadline_ms)
    }

    // Common mutation boundary after each envelope family has authenticated
    // its own exact bytes. Callers must not substitute a shell/grant envelope.
    pub(super) fn commit_checked_request(
        &mut self,
        request_id: RequestId,
        sequence: u64,
        now: MonotonicMillis,
        relative_deadline_ms: Option<u64>,
    ) -> Result<RequestGuard, ProtocolError> {
        let mut guard = RequestGuard::new(
            request_id,
            sequence,
            now,
            relative_deadline_ms,
            self.limits,
        )?;
        guard.attach_session_drain(&self.drain);
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
    /// repeated terminal can be rejected distinctly. Cancellation permits only
    /// [`TerminalKind::Cancelled`] or [`TerminalKind::OutcomeUnknown`] when
    /// possible external effects still require authoritative readback.
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
        Ok(self.prepare_request_terminal(request_id, terminal)?.commit())
    }

    /// Checks that this is the still-live guard issued by this exact session.
    ///
    /// Matching a request ID alone is insufficient: independently constructed
    /// guards or another connection's identically numbered request are refused.
    /// This proves admission/liveness, not a grant decision or source authority.
    /// The supplied clock must use the same monotonic origin as admission.
    ///
    /// # Errors
    ///
    /// Returns the session/header error, `InvalidSessionTransition` for a
    /// foreign, cancelled or inactive guard, or `DeadlineExpired` for expiry
    /// or a clock observation preceding admission. No state is changed.
    pub fn revalidate_request_guard(
        &self,
        request: &RequestGuard,
        now: MonotonicMillis,
    ) -> Result<(), ProtocolError> {
        self.validate_session_header(self.binding.version(), &self.server_nonce)?;
        let stored = self.guards.get(request.request_id())
            .ok_or(ProtocolError::InvalidSessionTransition)?;
        if stored.sequence() != request.sequence()
            || stored.admitted_at() != request.admitted_at()
            || stored.deadline() != request.deadline()
            || !stored.cancellation().same_signal(&request.cancellation())
            || !self.inflight.contains(request.request_id())
            || stored.is_cancelled()
            || stored.progress().is_some_and(|progress| progress.terminal().is_some())
        {
            return Err(ProtocolError::InvalidSessionTransition);
        }
        if now < stored.admitted_at() || stored.is_expired(now) {
            return Err(ProtocolError::DeadlineExpired);
        }
        Ok(())
    }

    /// Reserves a terminal without recording it or releasing its in-flight slot.
    ///
    /// All lifecycle checks are performed before the caller can write output.
    /// Dropping a successful preparation without delivery closes the session;
    /// a failed output or unwind cannot leave it reusable. The caller owns the
    /// real output barrier, clocks and effect classification. Selecting a
    /// terminal is not proof of delivery or cancellation before effects.
    ///
    /// # Errors
    ///
    /// Returns the terminal/session error before any output. Inconsistent slot
    /// ownership quarantines the connection without first finishing its guard.
    pub fn prepare_request_terminal(
        &mut self,
        request_id: &RequestId,
        terminal: TerminalKind,
    ) -> Result<PreparedRequestTerminal<'_>, ProtocolError> {
        let mut prepared = self.guards.get(request_id)
            .ok_or(ProtocolError::InvalidSessionTransition)?.clone();
        prepared.finish(terminal, self.limits)?;
        match self.session.state() {
            SessionState::Active | SessionState::Draining => {}
            SessionState::Closed => return Err(ProtocolError::SessionClosed),
            SessionState::Quarantined => return Err(ProtocolError::Quarantined),
            _ => return Err(ProtocolError::AuthenticationRequired),
        }
        if !self.inflight.contains(request_id) && !prepared.is_cancelled() {
            let _ = self.session.quarantine();
            return Err(ProtocolError::Quarantined);
        }
        Ok(PreparedRequestTerminal {
            session: self,
            request_id: *request_id,
            prepared: Some(prepared),
            status: RequestStatus::from_terminal(terminal),
            committed: false,
        })
    }

    /// Whether an admitted request still occupies one in-flight slot.
    #[must_use]
    pub fn is_request_in_flight(&self, request_id: &RequestId) -> bool {
        self.inflight.contains(request_id)
    }
}

/// Exclusive, single-use terminal output reservation for a bound session.
///
/// Obtain it through `BoundSession::prepare_request_terminal`. No source body,
/// grant or output buffer is retained here. The callback must encode, write and
/// flush the complete terminal and required transport acknowledgement under its
/// original budget/barrier; an error must never be reported as callback success.
#[must_use = "deliver the terminal or the session will be disconnected"]
pub struct PreparedRequestTerminal<'a> {
    session: &'a mut BoundSession,
    request_id: RequestId,
    prepared: Option<RequestGuard>,
    status: RequestStatus,
    committed: bool,
}

impl core::fmt::Debug for PreparedRequestTerminal<'_> {
    fn fmt(&self, formatter: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        formatter.debug_struct("PreparedRequestTerminal")
            .field("request_id", &self.request_id)
            .field("status", &self.status)
            .finish_non_exhaustive()
    }
}

impl PreparedRequestTerminal<'_> {
    /// Delivers the selected terminal, then records it without fallible work.
    ///
    /// Cancellation observed after terminal selection cannot rewrite bytes
    /// already being emitted. The transport still owns live output checks and
    /// must close on an unusable stream. Local I/O completion is not peer receipt.
    ///
    /// # Errors
    ///
    /// Returns the exact output error after disconnecting this session. Panic
    /// unwinding follows the same teardown, without claiming external rollback.
    pub fn deliver<E>(
        self,
        output: impl FnOnce(RequestStatus) -> Result<(), E>,
    ) -> Result<RequestStatus, E> {
        output(self.status)?;
        Ok(self.commit())
    }

    fn commit(mut self) -> RequestStatus {
        // Exclusive borrowing excludes registry mutation between preparation
        // and commit. The small finished guard was built before output.
        *self.session.guards.get_mut(&self.request_id)
            .expect("prepared request retained under exclusive session borrow") =
            self.prepared.take().expect("single-use prepared request guard");
        self.session.inflight.remove(&self.request_id);
        self.committed = true;
        self.status
    }
}

impl Drop for PreparedRequestTerminal<'_> {
    fn drop(&mut self) {
        if !self.committed {
            let _ = self.session.disconnect();
        }
    }
}

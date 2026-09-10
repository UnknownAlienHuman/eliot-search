//! Direction-local sequence tracking and the authenticated session machine.
//!
//! Sequences are monotonic per direction; duplicate, replayed, regressed or
//! gapped values fail closed with distinct errors. Admission additionally
//! requires an authenticated binding; the pairing prerequisite for envelope
//! admission is enforced by the [`crate::binding::BoundSession`] composer,
//! which holds both this machine and a mutually verified pairing.

use std::collections::BTreeSet;

use search_contracts::{ProtocolVersion, RequestId};

use crate::config::ProtocolLimits;
use crate::error::ProtocolError;
use crate::pairing::{ProofDigest, verify_proof};

/// Direction-local sequence observation.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum SequenceObservation {
    /// Exact next value accepted.
    Accepted,
    /// Immediately preceding value repeated.
    Duplicate,
    /// Value moved behind accepted history.
    Regression,
    /// Value skipped one or more expected values.
    Gap,
    /// Sequence space cannot advance beyond `u64::MAX`.
    Exhausted,
}

/// Strict contiguous sequence tracker for one direction.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct SequenceTracker {
    next_expected: Option<u64>,
    last_accepted: Option<u64>,
}

impl SequenceTracker {
    /// Creates a tracker beginning at `first_expected`.
    pub const fn new(first_expected: u64) -> Self {
        Self {
            next_expected: Some(first_expected),
            last_accepted: None,
        }
    }

    /// Exact next sequence, or `None` after exhaustion.
    pub const fn next_expected(self) -> Option<u64> {
        self.next_expected
    }

    /// Observes and conditionally advances one sequence.
    pub fn observe(&mut self, sequence: u64) -> SequenceObservation {
        if self.last_accepted == Some(sequence) {
            return SequenceObservation::Duplicate;
        }
        match self.next_expected {
            Some(expected) if sequence == expected => {
                self.last_accepted = Some(sequence);
                self.next_expected = expected.checked_add(1);
                SequenceObservation::Accepted
            }
            Some(expected) if sequence < expected => SequenceObservation::Regression,
            Some(_) => SequenceObservation::Gap,
            None => SequenceObservation::Exhausted,
        }
    }

    /// Converts an observation to a typed protocol result.
    pub const fn require_accepted(observation: SequenceObservation) -> Result<(), ProtocolError> {
        match observation {
            SequenceObservation::Accepted => Ok(()),
            SequenceObservation::Duplicate => Err(ProtocolError::DuplicateSequence),
            SequenceObservation::Regression => Err(ProtocolError::SequenceRegression),
            SequenceObservation::Gap => Err(ProtocolError::SequenceGap),
            SequenceObservation::Exhausted => Err(ProtocolError::SequenceExhausted),
        }
    }
}

/// Independent client-to-provider and provider-to-client sequence spaces.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct BidirectionalSequence {
    client: SequenceTracker,
    provider: SequenceTracker,
}

impl BidirectionalSequence {
    /// Creates independent sequence spaces.
    pub const fn new(client_first: u64, provider_first: u64) -> Self {
        Self {
            client: SequenceTracker::new(client_first),
            provider: SequenceTracker::new(provider_first),
        }
    }

    /// Mutable client-to-provider tracker.
    pub const fn client_mut(&mut self) -> &mut SequenceTracker {
        &mut self.client
    }

    /// Mutable provider-to-client tracker.
    pub const fn provider_mut(&mut self) -> &mut SequenceTracker {
        &mut self.provider
    }

    /// Client-to-provider state.
    pub const fn client(&self) -> SequenceTracker {
        self.client
    }

    /// Provider-to-client state.
    pub const fn provider(&self) -> SequenceTracker {
        self.provider
    }
}

/// Provider-session lifecycle.
#[derive(Clone, Copy, Debug, Eq, Ord, PartialEq, PartialOrd)]
pub enum SessionState {
    /// Transport exists but no version is selected.
    Offered,
    /// Version was selected; binding proof is still required.
    Negotiated,
    /// Authenticated session admits bounded requests.
    Active,
    /// New work is denied while accepted work drains.
    Draining,
    /// Session ended and cannot reopen.
    Closed,
    /// Contradictory state blocks all requests.
    Quarantined,
}

/// Deterministic authenticated session machine.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct SessionMachine {
    state: SessionState,
    version: Option<ProtocolVersion>,
    binding_verified: bool,
    sequences: BidirectionalSequence,
    replay: BTreeSet<RequestId>,
    limits: ProtocolLimits,
}

impl SessionMachine {
    /// Creates an offered unauthenticated session.
    pub fn new(
        limits: ProtocolLimits,
        client_first_sequence: u64,
        provider_first_sequence: u64,
    ) -> Result<Self, ProtocolError> {
        Ok(Self {
            state: SessionState::Offered,
            version: None,
            binding_verified: false,
            sequences: BidirectionalSequence::new(client_first_sequence, provider_first_sequence),
            replay: BTreeSet::new(),
            limits: limits.validate()?,
        })
    }

    /// Current session state.
    pub const fn state(&self) -> SessionState {
        self.state
    }

    /// Negotiated version when present.
    pub const fn version(&self) -> Option<ProtocolVersion> {
        self.version
    }

    /// Direction-local sequence state.
    pub const fn sequences(&self) -> &BidirectionalSequence {
        &self.sequences
    }

    /// Records exact version negotiation.
    pub fn negotiate(&mut self, version: ProtocolVersion) -> Result<(), ProtocolError> {
        if self.state != SessionState::Offered {
            return Err(ProtocolError::InvalidSessionTransition);
        }
        self.version = Some(version);
        self.state = SessionState::Negotiated;
        Ok(())
    }

    /// Activates after a constant-work binding-proof comparison.
    pub fn activate(
        &mut self,
        expected: &ProofDigest,
        observed: &ProofDigest,
    ) -> Result<(), ProtocolError> {
        if self.state != SessionState::Negotiated {
            return Err(ProtocolError::InvalidSessionTransition);
        }
        if !verify_proof(expected, observed) {
            return Err(ProtocolError::AuthenticationRequired);
        }
        self.binding_verified = true;
        self.state = SessionState::Active;
        Ok(())
    }

    /// Admits one exact client request after sequence and replay checks.
    pub fn admit_request(
        &mut self,
        request_id: RequestId,
        sequence: u64,
    ) -> Result<(), ProtocolError> {
        match self.state {
            SessionState::Active if self.binding_verified => {}
            SessionState::Offered | SessionState::Negotiated | SessionState::Active => {
                return Err(ProtocolError::AuthenticationRequired);
            }
            SessionState::Draining => return Err(ProtocolError::SessionDraining),
            SessionState::Closed => return Err(ProtocolError::SessionClosed),
            SessionState::Quarantined => return Err(ProtocolError::Quarantined),
        }
        SequenceTracker::require_accepted(self.sequences.client_mut().observe(sequence))?;
        if self.replay.contains(&request_id) {
            return Err(ProtocolError::ReplayDetected);
        }
        if self.replay.len() >= self.limits.max_replay_entries {
            return Err(ProtocolError::ReplayCapacityExceeded);
        }
        self.replay.insert(request_id);
        Ok(())
    }

    /// Accepts one provider-to-client sequence.
    pub fn accept_provider_sequence(&mut self, sequence: u64) -> Result<(), ProtocolError> {
        if !matches!(self.state, SessionState::Active | SessionState::Draining) {
            return Err(match self.state {
                SessionState::Closed => ProtocolError::SessionClosed,
                SessionState::Quarantined => ProtocolError::Quarantined,
                _ => ProtocolError::AuthenticationRequired,
            });
        }
        SequenceTracker::require_accepted(self.sequences.provider_mut().observe(sequence))
    }

    /// Begins graceful drain.
    pub fn begin_drain(&mut self) -> Result<(), ProtocolError> {
        if self.state != SessionState::Active || !self.binding_verified {
            return Err(ProtocolError::InvalidSessionTransition);
        }
        self.state = SessionState::Draining;
        Ok(())
    }

    /// Closes an active or draining session.
    pub fn close(&mut self) -> Result<(), ProtocolError> {
        if !matches!(self.state, SessionState::Active | SessionState::Draining) {
            return Err(ProtocolError::InvalidSessionTransition);
        }
        self.state = SessionState::Closed;
        self.binding_verified = false;
        Ok(())
    }

    /// Quarantines any non-closed session.
    pub fn quarantine(&mut self) -> Result<(), ProtocolError> {
        if self.state == SessionState::Closed {
            return Err(ProtocolError::SessionClosed);
        }
        self.state = SessionState::Quarantined;
        self.binding_verified = false;
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::config::DEFAULT_PROTOCOL_LIMITS;

    fn version(major: u16, minor: u16) -> ProtocolVersion {
        ProtocolVersion { major, minor }
    }

    #[test]
    fn sequence_errors_are_distinct() {
        let mut tracker = SequenceTracker::new(10);
        assert_eq!(tracker.observe(10), SequenceObservation::Accepted);
        assert_eq!(tracker.observe(10), SequenceObservation::Duplicate);
        assert_eq!(tracker.observe(9), SequenceObservation::Regression);
        assert_eq!(tracker.observe(12), SequenceObservation::Gap);
    }

    #[test]
    fn direction_sequences_are_independent() {
        let mut sequences = BidirectionalSequence::new(1, 100);
        assert_eq!(
            sequences.client_mut().observe(1),
            SequenceObservation::Accepted
        );
        assert_eq!(sequences.provider().next_expected(), Some(100));
    }

    #[test]
    fn unauthenticated_configuration_does_not_admit_requests() {
        let mut session = SessionMachine::new(DEFAULT_PROTOCOL_LIMITS, 1, 100).expect("session");
        session.negotiate(version(1, 0)).expect("negotiate");
        assert_eq!(
            session.admit_request(RequestId::from_bytes([1; 16]), 1),
            Err(ProtocolError::AuthenticationRequired)
        );
    }

    #[test]
    fn replay_is_rejected_after_authentication() {
        let mut session = SessionMachine::new(DEFAULT_PROTOCOL_LIMITS, 1, 100).expect("session");
        session.negotiate(version(1, 0)).expect("negotiate");
        let proof = ProofDigest::from_bytes([7; 32]);
        session.activate(&proof, &proof).expect("activate");
        let id = RequestId::from_bytes([1; 16]);
        session.admit_request(id, 1).expect("first");
        assert_eq!(
            session.admit_request(id, 2),
            Err(ProtocolError::ReplayDetected)
        );
    }

    #[test]
    fn drain_denies_new_work_and_close_is_terminal() {
        let mut session = SessionMachine::new(DEFAULT_PROTOCOL_LIMITS, 1, 100).expect("session");
        session.negotiate(version(1, 0)).expect("negotiate");
        let proof = ProofDigest::from_bytes([7; 32]);
        session.activate(&proof, &proof).expect("activate");
        session.begin_drain().expect("drain");
        assert_eq!(
            session.admit_request(RequestId::from_bytes([1; 16]), 1),
            Err(ProtocolError::SessionDraining)
        );
        session.close().expect("close");
        assert_eq!(session.quarantine(), Err(ProtocolError::SessionClosed));
    }
}

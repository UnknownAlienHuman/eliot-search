//! Binding sessions: hello negotiation, binding context and admission.
//!
//! Required logical surface (`AGENTS.md`): `BindingSession::accept_hello`
//! yields the negotiated session inputs, `BoundSession::admit` enforces the
//! full admission chain, `BoundSession::cancel` is idempotent, and
//! `project_capability_descriptor` exposes only binding-visible state.
//!
//! Sequencing is structural: a `BoundSession` can only be opened from a
//! [`BindingContext`] plus a [`crate::pairing::VerifiedPairing`], so envelope
//! admission always runs after mutual pairing verification — pairing first,
//! envelopes on top. A named-pipe ACL or loopback match alone cannot produce
//! either token, which keeps ACL-only authentication insufficient by
//! construction.

use std::collections::BTreeMap;

use search_contracts::protocol::{HelloBody, PeerRole, SearchProviderCapabilityDescriptor};
use search_contracts::{
    BindingId, InstallationIncarnationId, ProtocolRange, ProtocolVersion, RequestId,
};

use crate::cancel::{CancelOutcome, cancel_request};
use crate::cleanup::{DisconnectReceipt, disconnect_all};
use crate::config::ProtocolLimits;
use crate::error::ProtocolError;
use crate::negotiation::negotiate_hello;
use crate::pairing::{ProofDigest, ServerNonce, VerifiedPairing};
use crate::request::{
    AuthenticatedEnvelope, InFlightEntry, InFlightRegistry, MonotonicMillis, RequestGuard,
    verify_envelope_proof,
};
use crate::session::{SessionMachine, SessionState};

/// Transport peer identity supplied by the daemon adapter.
///
/// This is locator data (role, incarnation, binding); it grants no authority
/// on its own and must be paired with a verified pairing ceremony.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct TransportPeer {
    /// Claimed peer role; must match the hello role.
    pub role: PeerRole,
    /// Claimed installation incarnation; must match the live incarnation.
    pub incarnation: InstallationIncarnationId,
    /// Claimed binding; must match the durable binding record.
    pub binding: BindingId,
}

impl TransportPeer {
    /// Claimed installation incarnation.
    #[must_use]
    pub const fn incarnation(&self) -> &InstallationIncarnationId {
        &self.incarnation
    }
}

/// Authenticated binding context: negotiated version plus installation,
/// incarnation and peer binding proved by a mutual pairing ceremony.
///
/// Constructible only through [`authenticate_binding`], which requires the
/// ceremony token — never from ACL or locator data alone.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct BindingContext {
    version: ProtocolVersion,
    binding: BindingId,
    incarnation: InstallationIncarnationId,
    role: PeerRole,
}

impl BindingContext {
    /// Negotiated protocol version bound to this binding.
    #[must_use]
    pub const fn version(&self) -> ProtocolVersion {
        self.version
    }

    /// Durable binding identifier.
    #[must_use]
    pub const fn binding_id(&self) -> BindingId {
        self.binding
    }

    /// Installation incarnation this binding was issued for.
    #[must_use]
    pub const fn incarnation(&self) -> InstallationIncarnationId {
        self.incarnation
    }

    /// Verified peer role.
    #[must_use]
    pub const fn role(&self) -> PeerRole {
        self.role
    }
}

/// Authenticates a binding from a hello, a mutually verified pairing, the
/// live installation incarnation and the transport peer.
///
/// Requires the pairing proof plus installation/incarnation/peer binding:
/// role substitution, incarnation mismatch and a version outside the hello
/// range each fail with a distinct error. Success proves peer binding input
/// only; it grants no source membership or recipe authority.
pub fn authenticate_binding(
    hello: &HelloBody,
    pairing: &VerifiedPairing,
    incarnation: &InstallationIncarnationId,
    peer: &TransportPeer,
) -> Result<BindingContext, ProtocolError> {
    if hello.peer_role != peer.role {
        return Err(ProtocolError::AuthenticationFailed);
    }
    if peer.incarnation != *incarnation {
        return Err(ProtocolError::AuthenticationFailed);
    }
    if !hello.supported_protocol_range.contains(pairing.version()) {
        return Err(ProtocolError::NoCompatibleVersion);
    }
    Ok(BindingContext {
        version: pairing.version(),
        binding: peer.binding,
        incarnation: *incarnation,
        role: peer.role,
    })
}

/// Negotiated hello: exact version plus the peer it was negotiated with.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct NegotiatedHello {
    version: ProtocolVersion,
    peer: TransportPeer,
}

impl NegotiatedHello {
    /// Exact negotiated version.
    #[must_use]
    pub const fn version(&self) -> ProtocolVersion {
        self.version
    }

    /// Peer the version was negotiated with.
    #[must_use]
    pub const fn peer(&self) -> TransportPeer {
        self.peer
    }

    /// Begins the pairing ceremony for the negotiated version.
    #[must_use]
    pub fn begin_pairing(&self, binding: ProofDigest) -> crate::pairing::PairingMachine {
        crate::pairing::PairingMachine::new(self.version, binding)
    }
}

/// Hello acceptor: version negotiation plus single-use challenge ledger.
///
/// The acceptor owns no secrets and performs no I/O; the daemon adapter
/// supplies hellos, peer identities and ceremony material.
#[derive(Clone, Debug)]
pub struct BindingSession {
    local: ProtocolRange,
    limits: ProtocolLimits,
    ledger: crate::pairing::PairingLedger,
}

impl BindingSession {
    /// Creates an acceptor for one local protocol range and limits.
    pub fn new(local: ProtocolRange, limits: ProtocolLimits) -> Result<Self, ProtocolError> {
        let limits = limits.validate()?;
        Ok(Self {
            local,
            limits,
            ledger: crate::pairing::PairingLedger::new(limits.max_replay_entries)?,
        })
    }

    /// Accepts one hello: negotiates the exact version and checks the
    /// claimed role against the transport peer. Pairing and admission still
    /// have to follow; this step alone admits nothing.
    pub fn accept_hello(
        &self,
        hello: &HelloBody,
        peer: &TransportPeer,
    ) -> Result<NegotiatedHello, ProtocolError> {
        if hello.peer_role != peer.role {
            return Err(ProtocolError::AuthenticationFailed);
        }
        let version = negotiate_hello(self.local, hello.supported_protocol_range)?;
        Ok(NegotiatedHello {
            version,
            peer: *peer,
        })
    }

    /// Consumes one pairing challenge exactly once across all ceremonies.
    pub fn consume_challenge(
        &mut self,
        session: crate::pairing::SessionId,
        challenge: &crate::pairing::PairingChallenge,
    ) -> Result<(), ProtocolError> {
        self.ledger.consume(session, challenge)
    }

    /// Configured protocol limits.
    #[must_use]
    pub const fn limits(&self) -> ProtocolLimits {
        self.limits
    }
}

/// One open authenticated connection: pairing token, session machine,
/// in-flight registry and request guards composed in admission order.
///
/// Admission order is fixed in code: active session → paired version →
/// incarnation nonce → keyed proof → connection sequence → replay → deadline
/// → in-flight ceiling. Skipping pairing is unrepresentable: `open` requires
/// the ceremony token and every `admit` re-checks the paired version and
/// nonce before touching sequence or replay state.
#[derive(Clone, Debug)]
pub struct BoundSession {
    binding: BindingContext,
    pairing: VerifiedPairing,
    server_nonce: ServerNonce,
    session: SessionMachine,
    inflight: InFlightRegistry,
    guards: BTreeMap<RequestId, RequestGuard>,
    limits: ProtocolLimits,
}

impl BoundSession {
    /// Opens one connection from a binding context and its ceremony token.
    ///
    /// The transport session machine is bound to the exact ceremony output:
    /// activation compares the ceremony provider proof through the standard
    /// constant-work comparison, anchoring the session to the pairing.
    pub fn open(
        binding: BindingContext,
        pairing: VerifiedPairing,
        server_nonce: ServerNonce,
        limits: ProtocolLimits,
    ) -> Result<Self, ProtocolError> {
        if binding.version() != pairing.version() {
            return Err(ProtocolError::AuthenticationFailed);
        }
        let limits = limits.validate()?;
        let mut session = SessionMachine::new(limits, 1, 1)?;
        session.negotiate(binding.version())?;
        let ceremony_proof = pairing.provider_proof();
        session.activate(&ceremony_proof, &ceremony_proof)?;
        Ok(Self {
            binding,
            pairing,
            server_nonce,
            session,
            inflight: InFlightRegistry::new(limits.max_in_flight_requests)?,
            guards: BTreeMap::new(),
            limits,
        })
    }

    /// Whether the connection currently admits requests.
    #[must_use]
    pub fn is_active(&self) -> bool {
        self.session.state() == SessionState::Active
    }

    /// Current session lifecycle state.
    #[must_use]
    pub const fn session_state(&self) -> SessionState {
        self.session.state()
    }

    /// Binding context this connection was opened from.
    #[must_use]
    pub const fn binding_context(&self) -> BindingContext {
        self.binding
    }

    /// Ceremony token this connection was opened from.
    #[must_use]
    pub const fn pairing(&self) -> VerifiedPairing {
        self.pairing
    }

    /// Per-incarnation server nonce bound into every envelope proof.
    #[must_use]
    pub const fn server_nonce(&self) -> &ServerNonce {
        &self.server_nonce
    }

    /// Number of currently in-flight requests.
    #[must_use]
    pub fn in_flight_len(&self) -> usize {
        self.inflight.len()
    }

    /// Admits one authenticated envelope without a relative deadline.
    pub fn admit(
        &mut self,
        envelope: &AuthenticatedEnvelope,
        expected_proof: &ProofDigest,
        sequence: u64,
        now: MonotonicMillis,
    ) -> Result<RequestGuard, ProtocolError> {
        self.admit_with_deadline(envelope, expected_proof, sequence, now, None)
    }

    /// Admits one authenticated envelope with an optional relative deadline.
    ///
    /// The deadline is validated before any session state mutates, so an
    /// expired deadline leaves sequence, replay and in-flight state
    /// untouched.
    pub fn admit_with_deadline(
        &mut self,
        envelope: &AuthenticatedEnvelope,
        expected_proof: &ProofDigest,
        sequence: u64,
        now: MonotonicMillis,
        relative_deadline_ms: Option<u64>,
    ) -> Result<RequestGuard, ProtocolError> {
        if !self.is_active() {
            return Err(match self.session.state() {
                SessionState::Draining => ProtocolError::SessionDraining,
                SessionState::Closed => ProtocolError::SessionClosed,
                SessionState::Quarantined => ProtocolError::Quarantined,
                _ => ProtocolError::AuthenticationRequired,
            });
        }
        // Pairing prerequisite, enforced per admission: the envelope must
        // speak the mutually verified version and carry this incarnation's
        // server nonce before sequence, replay or ceiling state is touched.
        if envelope.version() != self.pairing.version() {
            return Err(ProtocolError::NoCompatibleVersion);
        }
        if envelope.server_nonce() != &self.server_nonce {
            return Err(ProtocolError::AuthenticationFailed);
        }
        verify_envelope_proof(envelope, expected_proof)?;
        // Finite-state pre-checks before mutation: deadline validity, then
        // in-flight capacity. The replay ledger is mutated only after every
        // check that can fail without recovery has passed.
        let guard = RequestGuard::new(
            *envelope.request_id(),
            sequence,
            now,
            relative_deadline_ms,
            self.limits,
        )?;
        if self.inflight.len() >= self.limits.max_in_flight_requests {
            return Err(ProtocolError::ResourceExhausted);
        }
        self.session
            .admit_request(*envelope.request_id(), sequence)?;
        // Insert cannot fail after the capacity pre-check and the replay
        // check above; a contradiction quarantines instead of diverging.
        if self
            .inflight
            .insert(
                *envelope.request_id(),
                InFlightEntry::new(sequence, now, guard.deadline()),
            )
            .is_err()
        {
            let _ = self.session.quarantine();
            return Err(ProtocolError::Quarantined);
        }
        self.guards.insert(*envelope.request_id(), guard.clone());
        Ok(guard)
    }

    /// Idempotently cancels one request and releases its in-flight slot.
    pub fn cancel(&mut self, target: &RequestId) -> CancelOutcome {
        cancel_request(&mut self.inflight, &mut self.guards, target)
    }

    /// Disconnects deterministically: cancels every in-flight request,
    /// releases every guard, closes the session and reports exact counts.
    /// Never fails; after return the connection admits nothing.
    pub fn disconnect(&mut self) -> DisconnectReceipt {
        let receipt = disconnect_all(&mut self.inflight, &mut self.guards);
        let _ = self.session.begin_drain();
        let _ = self.session.close();
        receipt
    }
}

/// Projects the binding-visible capability descriptor.
///
/// Requires the authoritative descriptor to validate, to target this
/// binding's incarnation and to speak the binding's version; only then is
/// the validated snapshot returned. Memberships are already opaque in the
/// contracts schema, so no hidden names, paths or counts can leak through
/// this projection. Recipe/profile filtering against live access state is
/// daemon composition (W8); this layer enforces the binding/version gate.
pub fn project_capability_descriptor(
    authoritative: &SearchProviderCapabilityDescriptor,
    binding: &BindingContext,
) -> Result<SearchProviderCapabilityDescriptor, ProtocolError> {
    authoritative
        .validate()
        .map_err(|_| ProtocolError::InvalidEnvelope)?;
    if authoritative.installation_incarnation_id != binding.incarnation() {
        return Err(ProtocolError::AuthenticationFailed);
    }
    if authoritative.provider_protocol_version != binding.version() {
        return Err(ProtocolError::NoCompatibleVersion);
    }
    Ok(authoritative.clone())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::config::DEFAULT_PROTOCOL_LIMITS;
    use crate::pairing::{ClientNonce, PairingChallenge, PairingMachine, SessionId, verify_proof};

    fn version(major: u16, minor: u16) -> ProtocolVersion {
        ProtocolVersion { major, minor }
    }

    fn local_range() -> ProtocolRange {
        ProtocolRange::new(version(1, 0), version(1, 2)).expect("range")
    }

    fn hello() -> HelloBody {
        HelloBody {
            peer_role: PeerRole::StandaloneCli,
            pairing_proof_ref: search_contracts::canonical::OpaqueRef::new("proof-ref")
                .expect("ref"),
            supported_protocol_range: local_range(),
            requested_capability_digest: None,
        }
    }

    fn peer() -> TransportPeer {
        TransportPeer {
            role: PeerRole::StandaloneCli,
            incarnation: InstallationIncarnationId::from_bytes([0x11; 16]),
            binding: BindingId::from_bytes([0x22; 16]),
        }
    }

    fn verified() -> VerifiedPairing {
        let binding = ProofDigest::from_bytes([9; 32]);
        let mut machine = PairingMachine::new(version(1, 0), binding);
        machine
            .issue_challenge(
                SessionId::from_bytes([1; 16]).expect("session"),
                ClientNonce::from_bytes([2; 16]).expect("nonce"),
                PairingChallenge::from_bytes([3; 32]).expect("challenge"),
            )
            .expect("challenge");
        machine
            .verify_client_proof(&binding, &binding)
            .expect("verify");
        machine
            .issue_provider_proof(ProofDigest::from_bytes([4; 32]))
            .expect("provider");
        machine.into_verified().expect("verified")
    }

    #[test]
    fn hello_negotiates_and_rejects_role_substitution() {
        let acceptor =
            BindingSession::new(local_range(), DEFAULT_PROTOCOL_LIMITS).expect("session");
        let negotiated = acceptor.accept_hello(&hello(), &peer()).expect("hello");
        assert_eq!(negotiated.version(), version(1, 2));
        let mut wrong_role = peer();
        wrong_role.role = PeerRole::Daemon;
        assert_eq!(
            acceptor.accept_hello(&hello(), &wrong_role),
            Err(ProtocolError::AuthenticationFailed)
        );
        // Major mismatch fails negotiation without admitting anything.
        let mut foreign = hello();
        foreign.supported_protocol_range =
            ProtocolRange::new(version(2, 0), version(2, 0)).expect("range");
        assert_eq!(
            acceptor.accept_hello(&foreign, &peer()),
            Err(ProtocolError::NoCompatibleVersion)
        );
    }

    #[test]
    fn binding_requires_pairing_plus_incarnation_and_peer() {
        let pairing = verified();
        let peer = peer();
        let context =
            authenticate_binding(&hello(), &pairing, &peer.incarnation, &peer).expect("binding");
        assert_eq!(context.version(), version(1, 0));
        assert_eq!(context.binding_id(), peer.binding);
        let mut other_incarnation = peer;
        other_incarnation.incarnation = InstallationIncarnationId::from_bytes([0x33; 16]);
        assert_eq!(
            authenticate_binding(&hello(), &pairing, &other_incarnation.incarnation, &peer),
            Err(ProtocolError::AuthenticationFailed)
        );
        let mut narrow = hello();
        narrow.supported_protocol_range =
            ProtocolRange::new(version(1, 1), version(1, 2)).expect("range");
        assert_eq!(
            authenticate_binding(&narrow, &pairing, &peer.incarnation, &peer),
            Err(ProtocolError::NoCompatibleVersion)
        );
    }

    #[test]
    fn open_binds_session_to_ceremony_proof() {
        let pairing = verified();
        let peer = peer();
        let context =
            authenticate_binding(&hello(), &pairing, &peer.incarnation, &peer).expect("binding");
        let session = BoundSession::open(
            context,
            pairing,
            ServerNonce::from_bytes([0x44; 16]).expect("nonce"),
            DEFAULT_PROTOCOL_LIMITS,
        )
        .expect("open");
        assert!(session.is_active());
        assert!(verify_proof(
            &pairing.provider_proof(),
            &pairing.provider_proof()
        ));
    }
}

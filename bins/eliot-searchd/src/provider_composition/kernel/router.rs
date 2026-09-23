//! Provider connection admission, replay, sequencing and terminal state.

use std::collections::{BTreeMap, BTreeSet, VecDeque};

use search_contracts::{ProtocolVersion, RequestId};
use search_provider_protocol::request::{
    AuthenticatedEnvelope, InFlightEntry, InFlightRegistry, MonotonicMillis, RequestGuard,
    RequestStatus,
};
use search_provider_protocol::{
    BindingKey, CancelOutcome, DisconnectReceipt, ProofDigest, ProtocolError, ProtocolLimits,
    SequenceTracker, ServerNonce, SessionMachine, TerminalKind, cancel_request, disconnect_all,
};

use super::pairing::verify_envelope;
use super::spec::PROVIDER_PROTOCOL_RANGE;

mod terminal;

pub use terminal::PreparedProviderTerminal;

/// Domain separating the session anchor from pairing material.
const SESSION_ANCHOR_DOMAIN: &[u8] = b"eliot-provider-session/v1\0";

/// One open provider connection: authenticated admission plus exactly-one
/// terminal per request.
///
/// Admission order is fixed in code: negotiated version, then incarnation
/// server nonce, then keyed proof, then connection sequence, then replay,
/// then in-flight ceiling. Terminal responses are emitted in admission
/// (FIFO) order; out-of-order completion fails closed. Cancellation is
/// idempotent and releases exactly one in-flight slot.
pub struct ProviderRouter {
    session: SessionMachine,
    inflight: InFlightRegistry,
    guards: BTreeMap<RequestId, RequestGuard>,
    pending: VecDeque<RequestId>,
    completed: BTreeSet<RequestId>,
    provider_sequence: SequenceTracker,
    nonce: ServerNonce,
    version: ProtocolVersion,
    limits: ProtocolLimits,
}

impl ProviderRouter {
    /// Opens one connection from the pairing key, negotiated version and a
    /// fresh server nonce.
    ///
    /// The session anchor proves key possession at open; connection
    /// authentication itself stays owned by the pairing ceremony that
    /// supplied the key. A zero key or an out-of-range version fails closed.
    pub fn open(
        key: &[u8; 32],
        version: ProtocolVersion,
        nonce: ServerNonce,
        limits: ProtocolLimits,
    ) -> Result<Self, ProtocolError> {
        let limits = limits.validate()?;
        if !PROVIDER_PROTOCOL_RANGE.contains(version) {
            return Err(ProtocolError::NoCompatibleVersion);
        }
        let binding = BindingKey::from_bytes(*key)?;
        let anchor = binding.with_bytes(|bytes| {
            let mut input = Vec::with_capacity(SESSION_ANCHOR_DOMAIN.len() + 2 + 2 + 16);
            input.extend_from_slice(SESSION_ANCHOR_DOMAIN);
            input.extend_from_slice(&version.major.to_le_bytes());
            input.extend_from_slice(&version.minor.to_le_bytes());
            input.extend_from_slice(nonce.as_bytes());
            ProofDigest::from_bytes(*blake3::keyed_hash(bytes, &input).as_bytes())
        });
        let mut session = SessionMachine::new(limits, 1, 1)?;
        session.negotiate(version)?;
        session.activate(&anchor, &anchor)?;
        Ok(Self {
            session,
            inflight: InFlightRegistry::new(limits.max_in_flight_requests)?,
            guards: BTreeMap::new(),
            pending: VecDeque::new(),
            completed: BTreeSet::new(),
            provider_sequence: SequenceTracker::new(1),
            nonce,
            version,
            limits,
        })
    }

    /// Whether the connection currently admits requests.
    #[must_use]
    pub fn is_active(&self) -> bool {
        self.session.state() == search_provider_protocol::SessionState::Active
    }

    /// Negotiated version bound to this connection.
    #[must_use]
    pub const fn version(&self) -> ProtocolVersion {
        self.version
    }

    /// Per-connection server nonce bound into every envelope proof.
    #[must_use]
    pub const fn server_nonce(&self) -> &ServerNonce {
        &self.nonce
    }

    /// Number of currently in-flight requests.
    #[must_use]
    pub fn in_flight_len(&self) -> usize {
        self.inflight.len()
    }

    /// Number of admitted requests awaiting their terminal response.
    #[must_use]
    pub fn pending_len(&self) -> usize {
        self.pending.len()
    }

    /// Admits one authenticated envelope with the pairing key.
    ///
    /// The deadline check runs before any session state mutates, so an
    /// expired deadline leaves sequence, replay and in-flight state
    /// untouched.
    pub fn admit(
        &mut self,
        envelope: &AuthenticatedEnvelope,
        key: &[u8; 32],
        sequence: u64,
        now: MonotonicMillis,
        relative_deadline_ms: Option<u64>,
    ) -> Result<RequestGuard, ProtocolError> {
        if !self.is_active() {
            return Err(match self.session.state() {
                search_provider_protocol::SessionState::Draining => ProtocolError::SessionDraining,
                search_provider_protocol::SessionState::Closed => ProtocolError::SessionClosed,
                search_provider_protocol::SessionState::Quarantined => ProtocolError::Quarantined,
                _ => ProtocolError::AuthenticationRequired,
            });
        }
        if envelope.version() != self.version {
            return Err(ProtocolError::NoCompatibleVersion);
        }
        if envelope.server_nonce() != &self.nonce {
            return Err(ProtocolError::AuthenticationFailed);
        }
        verify_envelope(key, envelope)?;
        let guard = RequestGuard::new(
            *envelope.request_id(),
            sequence,
            now,
            relative_deadline_ms,
            self.limits,
        )?;
        if self.in_flight_len() >= self.limits.max_in_flight_requests {
            return Err(ProtocolError::ResourceExhausted);
        }
        self.session
            .admit_request(*envelope.request_id(), sequence)?;
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
        self.pending.push_back(*envelope.request_id());
        Ok(guard)
    }

    /// Idempotently cancels one request and releases its in-flight slot.
    pub fn cancel(&mut self, target: &RequestId) -> CancelOutcome {
        let outcome = cancel_request(&mut self.inflight, &mut self.guards, target);
        if matches!(outcome, CancelOutcome::Cancelled { .. }) {
            self.pending.retain(|pending| pending != target);
            // The outcome already captured terminal state; only active guards
            // belong in this map. Replay protection remains in SessionMachine.
            self.guards.remove(target);
        }
        outcome
    }

    /// Records a terminal whose delivery is managed by the caller.
    ///
    /// All fallible lifecycle and sequence checks precede state changes. The
    /// live transport uses `prepare_terminal().deliver(...)` instead, so it
    /// cannot record completion before the response is fully written/flushed.
    pub fn note_terminal(
        &mut self,
        request_id: &RequestId,
        terminal: TerminalKind,
    ) -> Result<(RequestStatus, u64), ProtocolError> {
        Ok(self.prepare_terminal(request_id, terminal)?.commit())
    }

    /// Disconnects deterministically: cancels every in-flight request,
    /// releases every guard and closes the session, reporting exact counts.
    /// Never fails; afterwards the connection admits nothing.
    pub fn disconnect(&mut self) -> DisconnectReceipt {
        let receipt = disconnect_all(&mut self.inflight, &mut self.guards);
        self.pending.clear();
        let _ = self.session.begin_drain();
        let _ = self.session.close();
        receipt
    }
}

/// Process-local millisecond clock for admission instants.
///
/// Values are meaningful only inside this process incarnation and are never
/// serialized; the T06 child-request budget (not this clock) bounds work.
#[must_use]
pub fn monotonic_millis() -> MonotonicMillis {
    use std::sync::OnceLock;
    use std::time::Instant;

    static EPOCH: OnceLock<Instant> = OnceLock::new();
    let epoch = EPOCH.get_or_init(Instant::now);
    MonotonicMillis::new(u64::try_from(epoch.elapsed().as_millis()).unwrap_or(u64::MAX))
}

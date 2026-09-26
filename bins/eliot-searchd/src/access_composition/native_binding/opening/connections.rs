//! Bounded registry of native provider sessions for binding-generation fences.

use std::collections::BTreeMap;

use search_contracts::{BindingId, NonZeroRevision};
use search_provider_protocol::{SessionDrainHandle, SessionId};

use crate::provider_composition::CanonicalTcpConnection;
use super::super::{NativeBindingPin, ProviderBindingRecord, ProviderBindingStatus};

/// Hard ceiling matching the W8 client-edge schema maximum. Runtime configuration
/// may select a lower value, but cannot create an unbounded connection registry.
pub const MAX_REGISTERED_BINDING_CONNECTIONS: usize = 1_024;

/// Closed registry failure; no session, peer or credential material is rendered.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum BindingConnectionRegistryError {
    /// Configured capacity is zero or above the architectural ceiling.
    InvalidCapacity,
    /// The finite registry is full after pruning completed sessions.
    CapacityExceeded,
    /// The transport has already torn down its canonical session.
    ConnectionClosed,
    /// The transport session does not match the verified native binding pin.
    BindingMismatch,
    /// The same pairing session identity was registered more than once.
    DuplicateSession,
}

impl BindingConnectionRegistryError {
    /// Stable content-free failure code.
    #[must_use]
    pub const fn code(self) -> &'static str {
        match self {
            Self::InvalidCapacity => "BINDING_CONNECTION_CAPACITY_INVALID",
            Self::CapacityExceeded => "BINDING_CONNECTION_CAPACITY_EXCEEDED",
            Self::ConnectionClosed => "BINDING_CONNECTION_ALREADY_CLOSED",
            Self::BindingMismatch => "BINDING_CONNECTION_RECORD_MISMATCH",
            Self::DuplicateSession => "BINDING_CONNECTION_SESSION_DUPLICATE",
        }
    }
}

impl std::fmt::Display for BindingConnectionRegistryError {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter.write_str(self.code())
    }
}

impl std::error::Error for BindingConnectionRegistryError {}

struct RegisteredConnection {
    binding_id: BindingId,
    pairing_generation: NonZeroRevision,
    revocation_generation: NonZeroRevision,
    drain: SessionDrainHandle,
}

/// Executed admission/request fence for all registered older generations.
///
/// `still_open` is diagnostic: terminal cancellation output and socket cleanup may
/// continue, but every matched session already rejects new work and every request
/// cancellation probe observes the drain before this receipt is returned.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct BindingDrainReceipt {
    /// Binding whose older generations were fenced.
    pub binding_id: BindingId,
    /// Replacement pairing generation used as the strict upper fence.
    pub pairing_generation: NonZeroRevision,
    /// Replacement revocation generation used as the strict upper fence.
    pub revocation_generation: NonZeroRevision,
    /// Registered older sessions matched by this pass.
    pub matched: usize,
    /// Sessions that observed their first drain request in this pass.
    pub newly_requested: usize,
    /// Matched mutable owners that had not completed teardown at readback.
    pub still_open: usize,
}

/// Finite native connection index. It stores no socket, key, request, grant or
/// source state—only verified session identity, binding generations and one
/// monotonic drain capability owned by the canonical session.
///
/// Production native opening registers before returning a transport. Closed
/// entries are pruned lazily. A drain receipt is actual execution evidence for
/// admission fencing and cooperative request cancellation, not peer receipt or
/// proof that every backend side effect rolled back.
pub struct BindingConnectionRegistry {
    capacity: usize,
    sessions: BTreeMap<SessionId, RegisteredConnection>,
}

impl BindingConnectionRegistry {
    /// Creates a bounded registry within the W8 architectural ceiling.
    pub fn new(capacity: usize) -> Result<Self, BindingConnectionRegistryError> {
        if capacity == 0 || capacity > MAX_REGISTERED_BINDING_CONNECTIONS {
            return Err(BindingConnectionRegistryError::InvalidCapacity);
        }
        Ok(Self { capacity, sessions: BTreeMap::new() })
    }

    /// Number of retained entries, including drained owners not yet torn down.
    #[must_use]
    pub fn len(&self) -> usize { self.sessions.len() }

    /// Whether no entry is retained.
    #[must_use]
    pub fn is_empty(&self) -> bool { self.sessions.is_empty() }

    /// Registers one transport only against the exact pin produced by its native
    /// open. Failure occurs before the transport escapes to a serving owner.
    pub fn register(
        &mut self,
        pin: &NativeBindingPin,
        transport: &CanonicalTcpConnection,
    ) -> Result<(), BindingConnectionRegistryError> {
        self.prune_closed();
        if self.sessions.len() >= self.capacity {
            return Err(BindingConnectionRegistryError::CapacityExceeded);
        }
        let session = transport.session()
            .ok_or(BindingConnectionRegistryError::ConnectionClosed)?;
        let context = session.binding_context();
        let record = pin.record();
        if record.status != ProviderBindingStatus::Active
            || context.binding_id() != record.binding_id
            || context.incarnation() != record.installation_incarnation_id
            || context.role() != record.peer_role
        {
            return Err(BindingConnectionRegistryError::BindingMismatch);
        }
        let session_id = session.pairing().session();
        if self.sessions.contains_key(&session_id) {
            return Err(BindingConnectionRegistryError::DuplicateSession);
        }
        self.sessions.insert(session_id, RegisteredConnection {
            binding_id: record.binding_id,
            pairing_generation: record.pairing_generation,
            revocation_generation: record.revocation_generation,
            drain: session.drain_handle(),
        });
        Ok(())
    }

    /// Fences every registered session older than the replacement record.
    ///
    /// The replacement may be active, revoked or expired. A matched drain signal
    /// immediately denies new protocol requests and cancels all request observers.
    /// Calling this method repeatedly is idempotent and never reopens a session.
    pub fn fence_prior_generations(
        &mut self,
        replacement: &ProviderBindingRecord,
    ) -> BindingDrainReceipt {
        self.prune_closed();
        let mut matched = 0_usize;
        let mut newly_requested = 0_usize;
        for connection in self.sessions.values() {
            if connection.binding_id == replacement.binding_id
                && (connection.pairing_generation < replacement.pairing_generation
                    || connection.revocation_generation < replacement.revocation_generation)
            {
                matched = matched.saturating_add(1);
                newly_requested = newly_requested
                    .saturating_add(usize::from(connection.drain.request_drain()));
            }
        }
        let still_open = self.sessions.values().filter(|connection| {
            connection.binding_id == replacement.binding_id
                && (connection.pairing_generation < replacement.pairing_generation
                    || connection.revocation_generation < replacement.revocation_generation)
                && !connection.drain.is_closed()
        }).count();
        BindingDrainReceipt {
            binding_id: replacement.binding_id,
            pairing_generation: replacement.pairing_generation,
            revocation_generation: replacement.revocation_generation,
            matched,
            newly_requested,
            still_open,
        }
    }

    /// Removes entries whose canonical mutable session owner completed teardown.
    pub fn prune_closed(&mut self) {
        self.sessions.retain(|_, connection| !connection.drain.is_closed());
    }
}

impl std::fmt::Debug for BindingConnectionRegistry {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter
            .debug_struct("BindingConnectionRegistry")
            .field("capacity", &self.capacity)
            .field("sessions", &self.sessions.len())
            .finish_non_exhaustive()
    }
}

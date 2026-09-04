//! Deterministic, bounded ELIOT-facing request/session adapter.
//!
//! The adapter owns correlation, replay detection, request lifecycle and
//! fail-closed session transitions. It performs no transport, storage, search,
//! clock, credential or process I/O; callers supply already-captured inputs.

use std::collections::BTreeMap;
use std::fmt;
use std::num::{NonZeroU16, NonZeroUsize};

use search_contracts::{
    Blake3Digest32, GrantId, InstallationId, InstallationIncarnationId, RequestId, WorkspaceId,
};

/// Version of the ELIOT/Search adapter envelope.
#[derive(Clone, Copy, Debug, Eq, Ord, PartialEq, PartialOrd)]
pub struct AdapterProtocolVersion(NonZeroU16);

impl AdapterProtocolVersion {
    /// Current supported envelope version.
    pub const CURRENT: Self = Self(NonZeroU16::MIN);

    /// Builds a non-zero version.
    ///
    /// # Errors
    ///
    /// Returns [`AdapterError::UnsupportedVersion`] for zero or any version
    /// other than [`Self::CURRENT`].
    pub fn new(value: u16) -> Result<Self, AdapterError> {
        let value = NonZeroU16::new(value).ok_or(AdapterError::UnsupportedVersion)?;
        let version = Self(value);
        if version == Self::CURRENT {
            Ok(version)
        } else {
            Err(AdapterError::UnsupportedVersion)
        }
    }

    /// Numeric wire value.
    #[must_use]
    pub const fn get(self) -> u16 {
        self.0.get()
    }
}

/// Finite limits applied before a request enters the adapter ledger.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct AdapterLimits {
    max_in_flight: NonZeroUsize,
    max_retained_terminal: NonZeroUsize,
    max_payload_bytes: NonZeroUsize,
}

impl AdapterLimits {
    /// Creates non-zero finite limits.
    ///
    /// # Errors
    ///
    /// Returns [`AdapterError::InvalidLimits`] when any limit is zero.
    pub fn new(
        max_in_flight: usize,
        max_retained_terminal: usize,
        max_payload_bytes: usize,
    ) -> Result<Self, AdapterError> {
        Ok(Self {
            max_in_flight: NonZeroUsize::new(max_in_flight)
                .ok_or(AdapterError::InvalidLimits)?,
            max_retained_terminal: NonZeroUsize::new(max_retained_terminal)
                .ok_or(AdapterError::InvalidLimits)?,
            max_payload_bytes: NonZeroUsize::new(max_payload_bytes)
                .ok_or(AdapterError::InvalidLimits)?,
        })
    }

    /// Maximum requests that may be non-terminal simultaneously.
    #[must_use]
    pub const fn max_in_flight(self) -> usize {
        self.max_in_flight.get()
    }

    /// Maximum terminal records retained for replay classification.
    #[must_use]
    pub const fn max_retained_terminal(self) -> usize {
        self.max_retained_terminal.get()
    }

    /// Maximum encoded request payload accepted before dispatch.
    #[must_use]
    pub const fn max_payload_bytes(self) -> usize {
        self.max_payload_bytes.get()
    }
}

/// Exact ELIOT installation/workspace/grant binding for one session.
#[derive(Clone, Copy, Debug, Eq, Ord, PartialEq, PartialOrd)]
pub struct EliotBinding {
    /// Stable client installation.
    pub installation_id: InstallationId,
    /// Current client installation incarnation.
    pub installation_incarnation_id: InstallationIncarnationId,
    /// Workspace whose authority is being exercised.
    pub workspace_id: WorkspaceId,
    /// Grant authorizing the request.
    pub grant_id: GrantId,
}

/// Closed command set accepted by this adapter generation.
#[derive(Clone, Copy, Debug, Eq, Ord, PartialEq, PartialOrd)]
pub enum EliotCommand {
    /// Side-effect-free service health request.
    Health,
    /// Create and execute a bounded search plan.
    Search,
    /// Resume an existing continuation.
    Continue,
    /// Resolve an already-issued source handle.
    ReadHandle,
    /// Cancel one accepted request.
    Cancel,
    /// Request graceful adapter/daemon drain.
    Shutdown,
}

/// Captured request envelope. `payload` is opaque to this package.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct EliotRequest<P> {
    /// Adapter envelope version.
    pub version: AdapterProtocolVersion,
    /// Correlation and idempotency identity.
    pub request_id: RequestId,
    /// Exact client authority binding.
    pub binding: EliotBinding,
    /// Closed requested command.
    pub command: EliotCommand,
    /// Digest of exact canonical payload bytes.
    pub payload_digest: Blake3Digest32,
    /// Encoded payload byte count checked before dispatch.
    pub payload_bytes: usize,
    /// Opaque, already-decoded command payload.
    pub payload: P,
}

/// Lifecycle of one accepted request identity.
#[derive(Clone, Copy, Debug, Eq, Ord, PartialEq, PartialOrd)]
pub enum RequestLifecycle {
    /// Accepted but not yet dispatched to the search runtime.
    Accepted,
    /// Dispatched and still non-terminal.
    Running,
    /// Cancellation has been requested; terminal evidence is still required.
    CancellationRequested,
    /// Exactly one terminal outcome was recorded.
    Terminal(ResponseStatus),
}

/// Closed truthful response status.
#[derive(Clone, Copy, Debug, Eq, Ord, PartialEq, PartialOrd)]
pub enum ResponseStatus {
    /// Requested postcondition completed and was verified.
    Complete,
    /// Bounded useful output exists with explicit omissions or gaps.
    Partial,
    /// Request was rejected before a success postcondition.
    Rejected,
    /// Cancellation completed before an unresolved mutation boundary.
    Cancelled,
    /// Finite deadline elapsed.
    TimedOut,
    /// A possible mutation or external effect requires authoritative readback.
    OutcomeUnknown,
}

/// Immutable content-free terminal receipt retained for replay classification.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct TerminalReceipt {
    /// Request identity.
    pub request_id: RequestId,
    /// Digest of the exact original request payload.
    pub request_payload_digest: Blake3Digest32,
    /// Digest of the exact terminal response payload.
    pub response_payload_digest: Blake3Digest32,
    /// Truthful terminal status.
    pub status: ResponseStatus,
}

/// Result of request admission.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum Admission {
    /// A new request identity was accepted.
    Fresh,
    /// An identical non-terminal request was replayed.
    ReplayInFlight(RequestLifecycle),
    /// An identical terminal request was replayed.
    ReplayTerminal(TerminalReceipt),
}

/// Session lifecycle. Closed sessions never reopen.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum AdapterSessionState {
    /// Adapter exists but no authority binding has been installed.
    Unbound,
    /// Exact binding is installed and ordinary requests may be admitted.
    Active,
    /// New ordinary requests are rejected while accepted work drains.
    Draining,
    /// Session reached its terminal clean state.
    Closed,
    /// Contradictory state blocks all requests.
    Quarantined,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
struct RequestRecord {
    command: EliotCommand,
    payload_digest: Blake3Digest32,
    lifecycle: RequestLifecycle,
    terminal: Option<TerminalReceipt>,
}

/// In-memory deterministic ELIOT adapter state.
#[derive(Debug)]
pub struct EliotAdapter {
    limits: AdapterLimits,
    state: AdapterSessionState,
    binding: Option<EliotBinding>,
    requests: BTreeMap<RequestId, RequestRecord>,
    terminal_order: Vec<RequestId>,
}

impl EliotAdapter {
    /// Creates an unbound finite adapter.
    #[must_use]
    pub fn new(limits: AdapterLimits) -> Self {
        Self {
            limits,
            state: AdapterSessionState::Unbound,
            binding: None,
            requests: BTreeMap::new(),
            terminal_order: Vec::new(),
        }
    }

    /// Current session state.
    #[must_use]
    pub const fn state(&self) -> AdapterSessionState {
        self.state
    }

    /// Exact installed binding, when active or draining.
    #[must_use]
    pub const fn binding(&self) -> Option<EliotBinding> {
        self.binding
    }

    /// Number of accepted non-terminal requests.
    #[must_use]
    pub fn in_flight(&self) -> usize {
        self.requests
            .values()
            .filter(|record| !matches!(record.lifecycle, RequestLifecycle::Terminal(_)))
            .count()
    }

    /// Installs the exact client binding once.
    ///
    /// # Errors
    ///
    /// Rebinding or activating a closed/quarantined adapter is rejected.
    pub fn bind(&mut self, binding: EliotBinding) -> Result<(), AdapterError> {
        match self.state {
            AdapterSessionState::Unbound => {
                self.binding = Some(binding);
                self.state = AdapterSessionState::Active;
                Ok(())
            }
            AdapterSessionState::Active if self.binding == Some(binding) => Ok(()),
            AdapterSessionState::Active | AdapterSessionState::Draining => {
                Err(AdapterError::BindingMismatch)
            }
            AdapterSessionState::Closed => Err(AdapterError::SessionClosed),
            AdapterSessionState::Quarantined => Err(AdapterError::SessionQuarantined),
        }
    }

    /// Admits a request without interpreting its payload.
    ///
    /// # Errors
    ///
    /// Enforces active state, exact binding, supported version, payload and
    /// concurrency bounds, and conflict-free request-ID replay.
    pub fn admit<P>(&mut self, request: &EliotRequest<P>) -> Result<Admission, AdapterError> {
        if request.version != AdapterProtocolVersion::CURRENT {
            return Err(AdapterError::UnsupportedVersion);
        }
        match self.state {
            AdapterSessionState::Unbound => return Err(AdapterError::SessionUnbound),
            AdapterSessionState::Draining => {
                if !matches!(request.command, EliotCommand::Health | EliotCommand::Cancel) {
                    return Err(AdapterError::SessionDraining);
                }
            }
            AdapterSessionState::Closed => return Err(AdapterError::SessionClosed),
            AdapterSessionState::Quarantined => {
                return Err(AdapterError::SessionQuarantined);
            }
            AdapterSessionState::Active => {}
        }
        if self.binding != Some(request.binding) {
            return Err(AdapterError::BindingMismatch);
        }
        if request.payload_bytes > self.limits.max_payload_bytes() {
            return Err(AdapterError::PayloadTooLarge);
        }

        if let Some(existing) = self.requests.get(&request.request_id) {
            if existing.command != request.command
                || existing.payload_digest != request.payload_digest
            {
                return Err(AdapterError::RequestIdentityConflict);
            }
            return Ok(match existing.terminal {
                Some(receipt) => Admission::ReplayTerminal(receipt),
                None => Admission::ReplayInFlight(existing.lifecycle),
            });
        }

        if self.in_flight() >= self.limits.max_in_flight() {
            return Err(AdapterError::InFlightLimitExceeded);
        }
        self.requests.insert(
            request.request_id,
            RequestRecord {
                command: request.command,
                payload_digest: request.payload_digest,
                lifecycle: RequestLifecycle::Accepted,
                terminal: None,
            },
        );
        Ok(Admission::Fresh)
    }

    /// Marks one accepted request as dispatched.
    ///
    /// # Errors
    ///
    /// Unknown, terminal and cancellation-pending requests cannot be dispatched.
    pub fn mark_running(&mut self, request_id: RequestId) -> Result<(), AdapterError> {
        let record = self
            .requests
            .get_mut(&request_id)
            .ok_or(AdapterError::RequestNotFound)?;
        match record.lifecycle {
            RequestLifecycle::Accepted => {
                record.lifecycle = RequestLifecycle::Running;
                Ok(())
            }
            RequestLifecycle::Running => Ok(()),
            RequestLifecycle::CancellationRequested => Err(AdapterError::CancellationPending),
            RequestLifecycle::Terminal(_) => Err(AdapterError::DuplicateTerminal),
        }
    }

    /// Requests cancellation without inventing a terminal outcome.
    ///
    /// # Errors
    ///
    /// Unknown or terminal requests are rejected.
    pub fn request_cancellation(
        &mut self,
        request_id: RequestId,
    ) -> Result<(), AdapterError> {
        let record = self
            .requests
            .get_mut(&request_id)
            .ok_or(AdapterError::RequestNotFound)?;
        match record.lifecycle {
            RequestLifecycle::Accepted | RequestLifecycle::Running => {
                record.lifecycle = RequestLifecycle::CancellationRequested;
                Ok(())
            }
            RequestLifecycle::CancellationRequested => Ok(()),
            RequestLifecycle::Terminal(_) => Err(AdapterError::AlreadyTerminal),
        }
    }

    /// Records exactly one terminal response receipt.
    ///
    /// # Errors
    ///
    /// The request must exist, the original request digest must match, and a
    /// second terminal response is rejected.
    pub fn complete(
        &mut self,
        receipt: TerminalReceipt,
    ) -> Result<(), AdapterError> {
        let record = self
            .requests
            .get_mut(&receipt.request_id)
            .ok_or(AdapterError::RequestNotFound)?;
        if record.payload_digest != receipt.request_payload_digest {
            return Err(AdapterError::RequestIdentityConflict);
        }
        if record.terminal.is_some() {
            return Err(AdapterError::DuplicateTerminal);
        }
        record.lifecycle = RequestLifecycle::Terminal(receipt.status);
        record.terminal = Some(receipt);
        self.terminal_order.push(receipt.request_id);
        self.prune_terminal_history();
        Ok(())
    }

    /// Begins graceful drain. Existing requests remain addressable.
    ///
    /// # Errors
    ///
    /// An unbound, closed or quarantined session cannot enter drain.
    pub fn begin_drain(&mut self) -> Result<(), AdapterError> {
        match self.state {
            AdapterSessionState::Active => {
                self.state = AdapterSessionState::Draining;
                Ok(())
            }
            AdapterSessionState::Draining => Ok(()),
            AdapterSessionState::Unbound => Err(AdapterError::SessionUnbound),
            AdapterSessionState::Closed => Err(AdapterError::SessionClosed),
            AdapterSessionState::Quarantined => Err(AdapterError::SessionQuarantined),
        }
    }

    /// Closes a drained session only when no request remains non-terminal.
    ///
    /// # Errors
    ///
    /// Active sessions must drain first and in-flight work must reach terminal
    /// state before clean closure.
    pub fn close(&mut self) -> Result<(), AdapterError> {
        match self.state {
            AdapterSessionState::Draining if self.in_flight() == 0 => {
                self.state = AdapterSessionState::Closed;
                self.binding = None;
                Ok(())
            }
            AdapterSessionState::Draining => Err(AdapterError::InFlightRequestsRemain),
            AdapterSessionState::Active => Err(AdapterError::DrainRequired),
            AdapterSessionState::Unbound => Err(AdapterError::SessionUnbound),
            AdapterSessionState::Closed => Ok(()),
            AdapterSessionState::Quarantined => Err(AdapterError::SessionQuarantined),
        }
    }

    /// Quarantines contradictory state and denies future work.
    pub fn quarantine(&mut self) {
        self.state = AdapterSessionState::Quarantined;
    }

    /// Returns a content-free terminal receipt for exact replay.
    #[must_use]
    pub fn terminal_receipt(&self, request_id: RequestId) -> Option<TerminalReceipt> {
        self.requests.get(&request_id).and_then(|record| record.terminal)
    }

    fn prune_terminal_history(&mut self) {
        while self.terminal_order.len() > self.limits.max_retained_terminal() {
            let request_id = self.terminal_order.remove(0);
            if self
                .requests
                .get(&request_id)
                .is_some_and(|record| record.terminal.is_some())
            {
                self.requests.remove(&request_id);
            }
        }
    }
}

/// Closed adapter failure registry.
#[derive(Clone, Copy, Debug, Eq, Ord, PartialEq, PartialOrd)]
pub enum AdapterError {
    /// One or more configured limits are zero.
    InvalidLimits,
    /// Adapter envelope version is unsupported.
    UnsupportedVersion,
    /// Session has no installed authority binding.
    SessionUnbound,
    /// New ordinary work is denied while draining.
    SessionDraining,
    /// Session is cleanly closed.
    SessionClosed,
    /// Session is quarantined.
    SessionQuarantined,
    /// Request binding differs from the installed session binding.
    BindingMismatch,
    /// Encoded payload exceeds the configured byte ceiling.
    PayloadTooLarge,
    /// Non-terminal request capacity is exhausted.
    InFlightLimitExceeded,
    /// Request identity is unknown.
    RequestNotFound,
    /// Request ID was reused with another command or payload digest.
    RequestIdentityConflict,
    /// Request cancellation is pending.
    CancellationPending,
    /// Request already reached terminal state.
    AlreadyTerminal,
    /// A second terminal response was attempted.
    DuplicateTerminal,
    /// Graceful close requires draining state.
    DrainRequired,
    /// Accepted non-terminal requests still exist.
    InFlightRequestsRemain,
}

impl AdapterError {
    /// Stable machine-readable reason code.
    #[must_use]
    pub const fn code(self) -> &'static str {
        match self {
            Self::InvalidLimits => "ELIOT_ADAPTER_INVALID_LIMITS",
            Self::UnsupportedVersion => "ELIOT_ADAPTER_UNSUPPORTED_VERSION",
            Self::SessionUnbound => "ELIOT_ADAPTER_SESSION_UNBOUND",
            Self::SessionDraining => "ELIOT_ADAPTER_SESSION_DRAINING",
            Self::SessionClosed => "ELIOT_ADAPTER_SESSION_CLOSED",
            Self::SessionQuarantined => "ELIOT_ADAPTER_SESSION_QUARANTINED",
            Self::BindingMismatch => "ELIOT_ADAPTER_BINDING_MISMATCH",
            Self::PayloadTooLarge => "ELIOT_ADAPTER_PAYLOAD_TOO_LARGE",
            Self::InFlightLimitExceeded => "ELIOT_ADAPTER_IN_FLIGHT_LIMIT",
            Self::RequestNotFound => "ELIOT_ADAPTER_REQUEST_NOT_FOUND",
            Self::RequestIdentityConflict => "ELIOT_ADAPTER_REQUEST_IDENTITY_CONFLICT",
            Self::CancellationPending => "ELIOT_ADAPTER_CANCELLATION_PENDING",
            Self::AlreadyTerminal => "ELIOT_ADAPTER_ALREADY_TERMINAL",
            Self::DuplicateTerminal => "ELIOT_ADAPTER_DUPLICATE_TERMINAL",
            Self::DrainRequired => "ELIOT_ADAPTER_DRAIN_REQUIRED",
            Self::InFlightRequestsRemain => "ELIOT_ADAPTER_IN_FLIGHT_REMAINS",
        }
    }
}

impl fmt::Display for AdapterError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(self.code())
    }
}

impl std::error::Error for AdapterError {}

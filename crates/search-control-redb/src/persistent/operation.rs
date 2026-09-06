//! Cooperative operation control; no second journal or background worker.

use std::cell::Cell;
use std::fmt;
use std::time::{Duration, Instant};

use search_ports::{CancellationProbe, OperationContext, PortErrorKind, PortRetryability};

use crate::{ControlError, MutationId};

/// Why a context-controlled call stopped at a safe point.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ControlInterruption {
    /// The caller's cancellation capability was signalled.
    Cancelled,
    /// The single monotonic deadline for this call elapsed.
    DeadlineElapsed,
}

/// Content-free failure from a context-controlled journal operation.
///
/// The exact existing mutation identity is retained without hashing, renaming
/// or casting it into the shared port's different opaque identity type. This
/// lower-level API does not implement the entire `ControlJournalPort` contract.
/// An interruption after possible commit is still `CommitOutcomeUnknown`.
#[derive(Clone, Copy, Eq, PartialEq)]
pub struct ControlCallError {
    control: ControlError,
    interruption: Option<ControlInterruption>,
    operation_id: Option<MutationId>,
    before_start: bool,
    recovery: bool,
}

impl ControlCallError {
    /// Existing closed journal reason; no vendor exception is exposed.
    #[must_use]
    pub const fn control_error(self) -> ControlError { self.control }

    /// Cancellation/deadline cause, also retained for an unknown commit outcome.
    #[must_use]
    pub const fn interruption(self) -> Option<ControlInterruption> { self.interruption }

    /// Exact requested mutation identity, or `None` for a side-effect-free read.
    #[must_use]
    pub const fn operation_id(self) -> Option<MutationId> { self.operation_id }

    /// Shared failure classification, with uncertainty taking precedence.
    #[must_use]
    pub const fn kind(self) -> PortErrorKind {
        if matches!(self.control, ControlError::CommitOutcomeUnknown) {
            return PortErrorKind::OutcomeUnknown;
        }
        if matches!(self.control, ControlError::StoreQuarantined) {
            return PortErrorKind::Quarantined;
        }
        match self.interruption {
            Some(ControlInterruption::Cancelled) => PortErrorKind::CancelledBeforeSideEffect,
            Some(ControlInterruption::DeadlineElapsed) if self.before_start => PortErrorKind::DeadlineBeforeStart,
            Some(ControlInterruption::DeadlineElapsed) => PortErrorKind::DeadlineDuringOperation,
            None => match self.control {
                ControlError::StoreUnavailable => PortErrorKind::DependencyUnavailable,
                ControlError::StoreCorrupt | ControlError::IdentityMismatch
                | ControlError::SchemaUnsupported | ControlError::SchemaMismatch
                | ControlError::ForbiddenControlPayload => PortErrorKind::Quarantined,
                ControlError::TransactionConflict | ControlError::GenerationMismatch => PortErrorKind::StaleGeneration,
                ControlError::OperationConflict => PortErrorKind::Conflict,
                ControlError::BudgetExceeded | ControlError::GenerationExhausted
                | ControlError::IdempotencyCapacityExceeded => PortErrorKind::ResourceExhausted,
                ControlError::InvalidKey | ControlError::InvalidValue
                | ControlError::DuplicateMutationKey => PortErrorKind::InvalidInput,
                ControlError::ReadCancelled => PortErrorKind::CancelledBeforeSideEffect,
                _ => PortErrorKind::Internal,
            },
        }
    }

    /// Safe retry rule. Unknown outcomes require exact recovery first.
    #[must_use]
    pub const fn retryability(self) -> PortRetryability {
        if self.recovery { return PortRetryability::AfterReadback; }
        match self.kind() {
            PortErrorKind::OutcomeUnknown | PortErrorKind::Quarantined => PortRetryability::AfterReadback,
            PortErrorKind::CancelledBeforeSideEffect | PortErrorKind::DeadlineBeforeStart
            | PortErrorKind::DeadlineDuringOperation | PortErrorKind::DependencyUnavailable => {
                if self.operation_id.is_some() { PortRetryability::SameIdentity }
                else { PortRetryability::SameRequest }
            }
            PortErrorKind::StaleGeneration => PortRetryability::AfterRefresh,
            _ => PortRetryability::Never,
        }
    }
    pub(super) const fn for_recovery(mut self) -> Self {
        self.recovery = true;
        self
    }
}

impl fmt::Debug for ControlCallError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("ControlCallError")
            .field("control", &self.control)
            .field("interruption", &self.interruption)
            .field("operation_id", &self.operation_id.map(|_| "<opaque>"))
            .finish()
    }
}

impl fmt::Display for ControlCallError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{} ({:?})", self.control.code(), self.kind())
    }
}

impl std::error::Error for ControlCallError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> { Some(&self.control) }
}

// Points identify actual algorithm boundaries, not public failure-injection modes.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(super) enum Point {
    Start,
    Validated,
    ReadHeader,
    ReadRecord,
    ReadOperation,
    ReadComplete,
    PlanRecord,
    BeforeWrite,
    StageRecord,
    BeforeCommit,
    AfterCommit,
    Readback,
    MutationComplete,
    ReplayComplete,
    RecoveryComplete,
}

pub(super) trait Check {
    fn check(&self, point: Point) -> Result<(), ControlError>;
}

/// Only the pre-existing low-level entrypoints use this compatibility policy.
pub(super) struct Unscoped;
impl Check for Unscoped {
    fn check(&self, _point: Point) -> Result<(), ControlError> { Ok(()) }
}

pub(super) struct Budget<'a, C: CancellationProbe, F: Fn() -> Instant> {
    context: &'a OperationContext<C>,
    started: Instant,
    duration: Duration,
    clock: F,
    stopped: Cell<Option<(ControlInterruption, Point)>>,
}

impl<'a, C: CancellationProbe> Budget<'a, C, fn() -> Instant> {
    pub fn new(context: &'a OperationContext<C>) -> Self {
        Self::with_clock(context, Instant::now)
    }
}

impl<'a, C: CancellationProbe, F: Fn() -> Instant> Budget<'a, C, F> {
    // Internal fake-clock seam permits deterministic deadline tests, no sleeps.
    pub(super) fn with_clock(context: &'a OperationContext<C>, clock: F) -> Self {
        Self {
            started: clock(),
            duration: Duration::from_millis(context.relative_deadline_ms().get()),
            context,
            clock,
            stopped: Cell::new(None),
        }
    }

    pub(super) fn failure(&self, control: ControlError, operation_id: Option<MutationId>) -> ControlCallError {
        let stopped = self.stopped.get();
        ControlCallError {
            control,
            interruption: stopped.map(|(why, _)| why),
            operation_id,
            recovery: false,
            before_start: stopped.is_some_and(|(_, point)| point == Point::Start),
        }
    }

    pub(super) fn interrupted(&self) -> bool { self.stopped.get().is_some() }
}

impl<C: CancellationProbe, F: Fn() -> Instant> Check for Budget<'_, C, F> {
    fn check(&self, point: Point) -> Result<(), ControlError> {
        let stopped = if let Some(stopped) = self.stopped.get() {
            Some(stopped)
        } else if self.context.cancellation().is_cancelled() {
            Some((ControlInterruption::Cancelled, point))
        } else {
            // A backwards fake/platform clock is rejected, never a fresh budget.
            match (self.clock)().checked_duration_since(self.started) {
                Some(elapsed) if elapsed < self.duration => None,
                _ => Some((ControlInterruption::DeadlineElapsed, point)),
            }
        };
        self.stopped.set(stopped);
        match stopped {
            None => Ok(()),
            Some((ControlInterruption::Cancelled, _)) => Err(ControlError::ReadCancelled),
            Some((ControlInterruption::DeadlineElapsed, _)) => Err(ControlError::BudgetExceeded),
        }
    }
}

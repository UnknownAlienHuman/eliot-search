//! Typed, bounded, redacted port failures.

use core::fmt;

use search_contracts::OpaqueId;

/// Stable cross-port failure class.
#[derive(Clone, Copy, Debug, Eq, Ord, PartialEq, PartialOrd)]
pub enum PortErrorKind {
    /// Operation context is malformed.
    InvalidContext,
    /// Deadline elapsed before dispatch.
    DeadlineBeforeStart,
    /// Deadline elapsed while work was active.
    DeadlineDuringOperation,
    /// Cancellation occurred before any side effect.
    CancelledBeforeSideEffect,
    /// Cancellation occurred after an acknowledged external side effect.
    CancelledAfterSideEffect,
    /// Exact mutation outcome is unresolved.
    OutcomeUnknown,
    /// Required dependency generation is stale.
    StaleGeneration,
    /// Guarded compare-and-swap rejected the expected state.
    CompareAndSwapRejected,
    /// Operation completed only a bounded subset.
    Partial,
    /// Required dependency is unavailable.
    DependencyUnavailable,
    /// A finite resource limit was exhausted.
    ResourceExhausted,
    /// Input violates the accepted contract.
    InvalidInput,
    /// Existing state conflicts with the operation identity.
    Conflict,
    /// Current authority denies the operation.
    Unauthorized,
    /// Contradictory state requires quarantine.
    Quarantined,
    /// Private implementation failure with no stable public detail.
    Internal,
}

/// Retry classification attached to every port failure or receipt.
#[derive(Clone, Copy, Debug, Eq, Ord, PartialEq, PartialOrd)]
pub enum PortRetryability {
    /// Retry is forbidden.
    Never,
    /// The same read request may be retried.
    SameRequest,
    /// Mutation may retry only with the same immutable identity.
    SameIdentity,
    /// Refresh/reconcile state before retrying the same request.
    AfterRefresh,
    /// Create a new operation identity after refresh.
    NewOperationAfterRefresh,
    /// Perform exact authoritative readback before any retry decision.
    AfterReadback,
}

/// Maximum disclosure permitted for a failure surface.
#[derive(Clone, Copy, Debug, Eq, Ord, PartialEq, PartialOrd)]
pub enum DisclosureClass {
    /// Stable kind may be mapped to a public/provider reason.
    Public,
    /// Kind may be shown but implementation detail remains redacted.
    Redacted,
    /// Failure is package-local and must not cross an untrusted boundary.
    Private,
}

/// Required behavior for every concrete typed port error.
pub trait PortFailure: fmt::Debug + fmt::Display + Send + Sync + 'static {
    /// Stable cross-port failure class.
    fn kind(&self) -> PortErrorKind;
    /// Retry classification.
    fn retryability(&self) -> PortRetryability;
    /// Disclosure ceiling.
    fn disclosure(&self) -> DisclosureClass;
    /// Immutable operation identity when a mutation was involved.
    fn operation_id(&self) -> Option<&OpaqueId>;
}

/// Typed package-neutral wrapper around one package-local closed reason enum.
#[derive(Clone, Eq, PartialEq)]
pub struct PortError<R> {
    kind: PortErrorKind,
    retryability: PortRetryability,
    disclosure: DisclosureClass,
    reason: R,
    operation_id: Option<OpaqueId>,
}

impl<R> PortError<R> {
    /// Creates a bounded port error.
    #[must_use]
    pub const fn new(
        kind: PortErrorKind,
        retryability: PortRetryability,
        disclosure: DisclosureClass,
        reason: R,
        operation_id: Option<OpaqueId>,
    ) -> Self {
        Self {
            kind,
            retryability,
            disclosure,
            reason,
            operation_id,
        }
    }

    /// Package-local typed reason available to trusted code.
    #[must_use]
    pub const fn reason(&self) -> &R {
        &self.reason
    }
}

impl<R> fmt::Debug for PortError<R> {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("PortError")
            .field("kind", &self.kind)
            .field("retryability", &self.retryability)
            .field("disclosure", &self.disclosure)
            .field("reason", &"<typed-redacted>")
            .field(
                "operation_id",
                &self.operation_id.as_ref().map(|_| "<opaque>"),
            )
            .finish()
    }
}

impl<R> fmt::Display for PortError<R> {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(formatter, "port operation failed: {:?}", self.kind)
    }
}

impl<R> std::error::Error for PortError<R> where R: Send + Sync + 'static {}

impl<R> PortFailure for PortError<R>
where
    R: Send + Sync + 'static,
{
    fn kind(&self) -> PortErrorKind {
        self.kind
    }

    fn retryability(&self) -> PortRetryability {
        self.retryability
    }

    fn disclosure(&self) -> DisclosureClass {
        self.disclosure
    }

    fn operation_id(&self) -> Option<&OpaqueId> {
        self.operation_id.as_ref()
    }
}

#[cfg(test)]
mod tests {
    use search_contracts::OpaqueId;

    use super::{DisclosureClass, PortError, PortErrorKind, PortFailure, PortRetryability};

    #[derive(Clone, Eq, PartialEq)]
    struct Sensitive(&'static str);

    #[test]
    fn debug_and_display_do_not_expose_package_reason() {
        let error = PortError::new(
            PortErrorKind::Internal,
            PortRetryability::Never,
            DisclosureClass::Private,
            Sensitive("source body or secret"),
            Some(OpaqueId::new("operation:opaque").expect("id")),
        );
        let debug = format!("{error:?}");
        let display = format!("{error}");
        assert!(!debug.contains("source body or secret"));
        assert!(!display.contains("source body or secret"));
        assert_eq!(error.kind(), PortErrorKind::Internal);
    }
}

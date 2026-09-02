//! Operation context, opaque capabilities, mutation identity, and receipts.

use core::fmt;
use core::num::NonZeroU64;

use search_contracts::{Blake3Digest32, BoundedNonContentMetadata, OpaqueId, OpaqueRef, RequestId};

use crate::{DisclosureClass, PortError, PortErrorKind, PortFailure, PortRetryability};

/// Process-local capability type class.
///
/// Implementations remain private to the capability owner. Shared APIs receive
/// them through associated types and cannot serialize, canonicalize, or inspect
/// a native path, socket, channel, store, secret, or process handle.
pub trait PackageOpaque: fmt::Debug + Send + Sync + 'static {
    /// Static package owner identifier used only for diagnostics and conformance.
    fn owner_package(&self) -> &'static str;
}

/// Opaque cancellation capability observed by a port implementation.
pub trait CancellationProbe: PackageOpaque {
    /// Returns whether cancellation has been requested.
    fn is_cancelled(&self) -> bool;
}

/// Common associated types required by every port.
pub trait Port {
    /// Typed, bounded port failure.
    type Error: PortFailure;
    /// Process-local cancellation capability.
    type Cancellation: CancellationProbe;
}

/// Whether an operation may perform a durable or external mutation.
#[derive(Clone, Copy, Debug, Eq, Ord, PartialEq, PartialOrd)]
pub enum OperationClass {
    /// Side-effect-free read or pure descriptor operation.
    ReadOnly,
    /// Mutation that may retry only with the same immutable identity.
    IdempotentMutation,
    /// Mutation that is single-attempt until authoritative readback resolves it.
    NonIdempotentMutation,
}

/// Idempotency declaration attached to every mutation.
#[derive(Clone, Copy, Debug, Eq, Ord, PartialEq, PartialOrd)]
pub enum IdempotencyClass {
    /// Retry is permitted only with the exact same operation identity.
    RetrySameIdentity,
    /// One attempt is permitted; later action requires authoritative readback.
    SingleAttempt,
    /// The external dependency guarantees idempotency for this identity.
    ExternallyIdempotent,
}

/// Immutable identity required by every mutation operation.
#[derive(Clone, Debug, Eq, Ord, PartialEq, PartialOrd)]
pub struct MutationIdentity {
    /// Opaque operation identity.
    pub operation_id: OpaqueId,
    /// Explicit idempotency class.
    pub idempotency: IdempotencyClass,
}

impl MutationIdentity {
    /// Creates a mutation identity.
    #[must_use]
    pub const fn new(operation_id: OpaqueId, idempotency: IdempotencyClass) -> Self {
        Self {
            operation_id,
            idempotency,
        }
    }
}

/// Finite request context supplied to every potentially blocking operation.
#[derive(Debug)]
pub struct OperationContext<C>
where
    C: CancellationProbe,
{
    request_id: RequestId,
    relative_deadline_ms: NonZeroU64,
    cancellation_ref: C,
    budget_ref: OpaqueRef,
}

impl<C> OperationContext<C>
where
    C: CancellationProbe,
{
    /// Creates a validated finite operation context.
    ///
    /// # Errors
    ///
    /// A zero relative deadline is rejected before dispatch.
    pub fn new(
        request_id: RequestId,
        relative_deadline_ms: u64,
        cancellation_ref: C,
        budget_ref: OpaqueRef,
    ) -> Result<Self, PortError<ContextReason>> {
        let Some(relative_deadline_ms) = NonZeroU64::new(relative_deadline_ms) else {
            return Err(PortError::new(
                PortErrorKind::InvalidContext,
                PortRetryability::Never,
                DisclosureClass::Redacted,
                ContextReason::ZeroDeadline,
                None,
            ));
        };
        Ok(Self {
            request_id,
            relative_deadline_ms,
            cancellation_ref,
            budget_ref,
        })
    }

    /// Request identity.
    #[must_use]
    pub const fn request_id(&self) -> RequestId {
        self.request_id
    }

    /// Relative finite deadline in milliseconds.
    #[must_use]
    pub const fn relative_deadline_ms(&self) -> NonZeroU64 {
        self.relative_deadline_ms
    }

    /// Process-local cancellation capability.
    #[must_use]
    pub const fn cancellation(&self) -> &C {
        &self.cancellation_ref
    }

    /// Opaque request-budget reference.
    #[must_use]
    pub const fn budget_ref(&self) -> &OpaqueRef {
        &self.budget_ref
    }

    /// Checks cancellation before any side effect.
    ///
    /// # Errors
    ///
    /// Returns a typed cancellation failure when the capability is cancelled.
    pub fn preflight(&self) -> Result<(), PortError<ContextReason>> {
        if self.cancellation_ref.is_cancelled() {
            Err(PortError::new(
                PortErrorKind::CancelledBeforeSideEffect,
                PortRetryability::SameRequest,
                DisclosureClass::Public,
                ContextReason::Cancelled,
                None,
            ))
        } else {
            Ok(())
        }
    }
}

/// Internal closed context failure reason.
#[derive(Clone, Copy, Debug, Eq, Ord, PartialEq, PartialOrd)]
pub enum ContextReason {
    /// Relative deadline was zero.
    ZeroDeadline,
    /// Cancellation was already requested.
    Cancelled,
}

/// Closed terminal outcome carried by a port receipt.
#[derive(Clone, Copy, Debug, Eq, Ord, PartialEq, PartialOrd)]
pub enum PortOutcome {
    /// Required postcondition completed and was verified.
    Complete,
    /// Some bounded work completed with explicit omissions or gaps.
    Partial,
    /// Operation was rejected before success.
    Rejected,
    /// Operation was cancelled.
    Cancelled,
    /// Deadline expired.
    TimedOut,
}

/// Closed retryability field for [`PortReceipt`].
#[derive(Clone, Copy, Debug, Eq, Ord, PartialEq, PartialOrd)]
pub enum ReceiptRetryability {
    /// Retry is forbidden.
    Never,
    /// Retry is permitted only with the same mutation identity.
    SameIdentity,
    /// Refresh authoritative state and create a new operation identity.
    NewOperationAfterRefresh,
}

/// Content-free receipt shared across port implementations.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct PortReceipt {
    /// Immutable operation identity.
    pub operation_id: OpaqueId,
    /// Digest of exact dependency generations used by the operation.
    pub dependency_generation_digest: Blake3Digest32,
    /// Closed terminal outcome.
    pub outcome: PortOutcome,
    /// Retry rule implied by the outcome.
    pub retryability: ReceiptRetryability,
    /// Bounded non-content metadata only.
    pub bounded_metadata: BoundedNonContentMetadata,
}

#[cfg(test)]
mod tests {
    use core::fmt;

    use search_contracts::{OpaqueRef, RequestId};

    use super::{CancellationProbe, OperationContext, PackageOpaque};
    use crate::{PortErrorKind, PortFailure};

    #[derive(Clone, Copy)]
    struct Cancellation(bool);

    impl fmt::Debug for Cancellation {
        fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
            formatter.write_str("Cancellation(<opaque>)")
        }
    }

    impl PackageOpaque for Cancellation {
        fn owner_package(&self) -> &'static str {
            "search-ports"
        }
    }

    impl CancellationProbe for Cancellation {
        fn is_cancelled(&self) -> bool {
            self.0
        }
    }

    #[test]
    fn zero_deadline_fails_before_dispatch() {
        let error = OperationContext::new(
            RequestId::from_bytes([1; 16]),
            0,
            Cancellation(false),
            OpaqueRef::new("budget:test").expect("budget"),
        )
        .expect_err("zero is not a finite deadline");
        assert_eq!(error.kind(), PortErrorKind::InvalidContext);
    }

    #[test]
    fn cancellation_preflight_is_explicit() {
        let context = OperationContext::new(
            RequestId::from_bytes([1; 16]),
            1,
            Cancellation(true),
            OpaqueRef::new("budget:test").expect("budget"),
        )
        .expect("context");
        let error = context.preflight().expect_err("cancelled");
        assert_eq!(error.kind(), PortErrorKind::CancelledBeforeSideEffect);
    }
}

//! Fresh source and namespace identity allocation through the qualified OS CSPRNG.
//!
//! This adapter mints RFC 4122 version-4 UUIDs from the sole daemon entropy owner.
//! It performs no file reads, holds no native authority, and creates no new budget
//! or deadline. The original caller deadline and work callback are preserved.

use std::fmt;
use std::marker::PhantomData;
use std::time::Instant;

use search_contracts::{OpaqueId, SourceId, SourceNamespaceId};
use search_control_redb::MigrationBootstrapWork;
use search_ports::{
    CancellationProbe, ContextReason, DisclosureClass, OperationContext, Port, PortError,
    PortErrorKind, PortFailure, PortRetryability,
};
use search_runtime_owner::OwnerError;
use search_source_registry::source_ids::SourceIdentityIdPortV2;

/// Closed failure surface for fresh identity allocation.
#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) enum SourceIdentityIdError {
    /// Original operation context refused dispatch.
    Context(PortError<ContextReason>),
    /// Original native owner callback refused work.
    Owner(OwnerError),
    /// Original deadline elapsed before the typed identity was returned.
    Deadline,
    /// Qualified OS entropy source refused the allocation.
    EntropyRefused,
}

impl fmt::Display for SourceIdentityIdError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Context(_) => formatter.write_str("SOURCE_IDENTITY_CONTEXT_REFUSED"),
            Self::Owner(_) => formatter.write_str("SOURCE_IDENTITY_OWNER_REFUSED"),
            Self::Deadline => formatter.write_str("SOURCE_IDENTITY_DEADLINE_EXPIRED"),
            Self::EntropyRefused => formatter.write_str("SOURCE_IDENTITY_ENTROPY_REFUSED"),
        }
    }
}

impl std::error::Error for SourceIdentityIdError {}

impl PortFailure for SourceIdentityIdError {
    fn kind(&self) -> PortErrorKind {
        match self {
            Self::Context(error) => error.kind(),
            Self::Owner(OwnerError::OwnerCancelledBeforeMutation) => {
                PortErrorKind::CancelledBeforeSideEffect
            }
            Self::Owner(OwnerError::OwnerDeadlineInvalid) => PortErrorKind::DeadlineDuringOperation,
            Self::Owner(_) => PortErrorKind::DependencyUnavailable,
            Self::Deadline => PortErrorKind::DeadlineDuringOperation,
            Self::EntropyRefused => PortErrorKind::DependencyUnavailable,
        }
    }

    fn retryability(&self) -> PortRetryability {
        match self {
            Self::Context(error) => error.retryability(),
            Self::Owner(OwnerError::OwnerCancelledBeforeMutation) => PortRetryability::SameRequest,
            Self::Owner(_) => PortRetryability::Never,
            Self::Deadline => PortRetryability::Never,
            Self::EntropyRefused => PortRetryability::Never,
        }
    }

    fn disclosure(&self) -> DisclosureClass {
        DisclosureClass::Redacted
    }

    fn operation_id(&self) -> Option<&OpaqueId> {
        match self {
            Self::Context(error) => error.operation_id(),
            _ => None,
        }
    }
}

/// Adapter that mints fresh source and namespace IDs from qualified OS entropy.
///
/// The adapter borrows the original deadline and work callback. It performs no
/// I/O, holds no native authority, and creates no new budget or deadline.
pub(crate) struct SourceIdentityIdsV2<'a, C: CancellationProbe> {
    deadline: Instant,
    work: &'a dyn Fn(MigrationBootstrapWork) -> Result<(), OwnerError>,
    cancellation: PhantomData<fn() -> C>,
}

impl<'a, C: CancellationProbe> SourceIdentityIdsV2<'a, C> {
    /// Binds the adapter to the original caller deadline and work callback.
    pub(crate) fn bind(
        deadline: Instant,
        work: &'a dyn Fn(MigrationBootstrapWork) -> Result<(), OwnerError>,
    ) -> Self {
        Self {
            deadline,
            work,
            cancellation: PhantomData,
        }
    }
}

impl<C: CancellationProbe> Port for SourceIdentityIdsV2<'_, C> {
    type Error = SourceIdentityIdError;
    type Cancellation = C;
}

impl<C: CancellationProbe> SourceIdentityIdPortV2 for SourceIdentityIdsV2<'_, C> {
    fn fresh_source_id(
        &mut self,
        context: &OperationContext<C>,
    ) -> Result<SourceId, Self::Error> {
        mint_fresh_id(self.deadline, self.work, context).map(SourceId::from_bytes)
    }

    fn fresh_source_namespace_id(
        &mut self,
        context: &OperationContext<C>,
    ) -> Result<SourceNamespaceId, Self::Error> {
        mint_fresh_id(self.deadline, self.work, context).map(SourceNamespaceId::from_bytes)
    }
}

/// Mints 16 bytes of qualified OS entropy as an RFC 4122 version-4 UUID.
fn mint_fresh_id<C: CancellationProbe>(
    deadline: Instant,
    work: &dyn Fn(MigrationBootstrapWork) -> Result<(), OwnerError>,
    context: &OperationContext<C>,
) -> Result<[u8; 16], SourceIdentityIdError> {
    context.preflight().map_err(SourceIdentityIdError::Context)?;
    work(MigrationBootstrapWork::Checkpoint).map_err(SourceIdentityIdError::Owner)?;
    work(MigrationBootstrapWork::Allocate(16)).map_err(SourceIdentityIdError::Owner)?;
    work(MigrationBootstrapWork::Read(16)).map_err(SourceIdentityIdError::Owner)?;
    let mut bytes = [0_u8; 16];
    crate::qualified_entropy::fill_qualified_entropy(&mut bytes)
        .map_err(|_| SourceIdentityIdError::EntropyRefused)?;
    work(MigrationBootstrapWork::Process(16)).map_err(SourceIdentityIdError::Owner)?;
    bytes[6] = (bytes[6] & 0x0f) | 0x40;
    bytes[8] = (bytes[8] & 0x3f) | 0x80;
    if Instant::now() >= deadline {
        return Err(SourceIdentityIdError::Deadline);
    }
    context.preflight().map_err(SourceIdentityIdError::Context)?;
    Ok(bytes)
}

#[cfg(test)]
mod tests {
    use super::*;
    use search_contracts::{OpaqueRef, RequestId};
    use search_ports::FakeCancellation;
    use std::time::Duration;

    fn context(cancelled: bool) -> OperationContext<FakeCancellation> {
        OperationContext::new(
            RequestId::from_bytes([7; 16]),
            60_000,
            FakeCancellation::new(cancelled),
            OpaqueRef::new("source-ids-test").expect("budget"),
        )
        .expect("context")
    }

    #[test]
    fn pre_cancel_calls_no_work_or_entropy() {
        let deadline = Instant::now() + Duration::from_secs(30);
        let mut called = false;
        let work = |_w: MigrationBootstrapWork| -> Result<(), OwnerError> {
            called = true;
            Ok(())
        };
        let mut adapter = SourceIdentityIdsV2::<FakeCancellation>::bind(deadline, &work);
        let result = adapter.fresh_source_id(&context(true));
        assert!(matches!(result, Err(SourceIdentityIdError::Context(_))));
        assert!(!called);
    }

    #[test]
    fn owner_refusal_returns_no_id() {
        let deadline = Instant::now() + Duration::from_secs(30);
        let work = |_w: MigrationBootstrapWork| -> Result<(), OwnerError> {
            Err(OwnerError::OwnerOperationConflict)
        };
        let mut adapter = SourceIdentityIdsV2::<FakeCancellation>::bind(deadline, &work);
        let result = adapter.fresh_source_id(&context(false));
        assert!(matches!(result, Err(SourceIdentityIdError::Owner(_))));
    }

    #[test]
    fn actual_entropy_yields_v4_source_and_namespace_ids() {
        let deadline = Instant::now() + Duration::from_secs(30);
        let work = |_w: MigrationBootstrapWork| -> Result<(), OwnerError> { Ok(()) };
        let mut adapter = SourceIdentityIdsV2::<FakeCancellation>::bind(deadline, &work);
        let ctx = context(false);
        let source_id = adapter.fresh_source_id(&ctx).expect("source id");
        let bytes = source_id.as_bytes();
        assert_eq!(bytes[6] & 0xf0, 0x40, "version 4");
        assert_eq!(bytes[8] & 0xc0, 0x80, "RFC 4122 variant");
        let ns_id = adapter.fresh_source_namespace_id(&ctx).expect("namespace id");
        let ns_bytes = ns_id.as_bytes();
        assert_eq!(ns_bytes[6] & 0xf0, 0x40, "version 4");
        assert_eq!(ns_bytes[8] & 0xc0, 0x80, "RFC 4122 variant");
        assert_ne!(bytes, ns_bytes, "distinct allocations");
    }
}

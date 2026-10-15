//! Fresh source and namespace identity allocation through the qualified OS CSPRNG.
//!
//! This adapter mints RFC 4122 version-4 UUIDs from the sole daemon entropy owner.
//! It reads no source bytes, holds no native authority, and creates no new budget
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
use search_source_registry::error::{RegistryError, RegistryPortError, registry_port_error};
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
            Self::Owner(OwnerError::OwnerOperationCapacityExceeded) => {
                PortErrorKind::ResourceExhausted
            }
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
/// source reads, holds no native authority, and creates no new budget or deadline.
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
    fn fresh_source_id(&mut self, context: &OperationContext<C>) -> Result<SourceId, Self::Error> {
        mint_fresh_id(self.deadline, self.work, context).map(SourceId::from_bytes)
    }

    fn fresh_source_namespace_id(
        &mut self,
        context: &OperationContext<C>,
    ) -> Result<SourceNamespaceId, Self::Error> {
        mint_fresh_id(self.deadline, self.work, context).map(SourceNamespaceId::from_bytes)
    }
}

impl<C: CancellationProbe> SourceIdentityIdPortV2 for super::RegistryJournal<'_, C> {
    fn fresh_source_id(&mut self, context: &OperationContext<C>) -> Result<SourceId, Self::Error> {
        if self.requires_recovery() {
            return Err(recovery_fence());
        }
        let Some(original_v4) = self.original_v4.as_ref() else {
            return Err(original_v4_unavailable());
        };
        SourceIdentityIdsV2::<C>::bind(original_v4.deadline, original_v4.work)
            .fresh_source_id(context)
            .map_err(map_source_identity_error)
    }

    fn fresh_source_namespace_id(
        &mut self,
        context: &OperationContext<C>,
    ) -> Result<SourceNamespaceId, Self::Error> {
        if self.requires_recovery() {
            return Err(recovery_fence());
        }
        let Some(original_v4) = self.original_v4.as_ref() else {
            return Err(original_v4_unavailable());
        };
        SourceIdentityIdsV2::<C>::bind(original_v4.deadline, original_v4.work)
            .fresh_source_namespace_id(context)
            .map_err(map_source_identity_error)
    }
}

fn map_source_identity_error(error: SourceIdentityIdError) -> RegistryPortError {
    let kind = error.kind();
    let retryability = error.retryability();
    let reason = match kind {
        PortErrorKind::CancelledBeforeSideEffect => RegistryError::CancelledBeforeCommit,
        PortErrorKind::OutcomeUnknown => RegistryError::MutationOutcomeUnknown,
        PortErrorKind::ResourceExhausted => RegistryError::DurabilityExhausted,
        _ => RegistryError::DurabilityRejected,
    };
    registry_port_error(kind, retryability, reason, error.operation_id().cloned())
}

fn original_v4_unavailable() -> RegistryPortError {
    registry_port_error(
        PortErrorKind::DependencyUnavailable,
        PortRetryability::Never,
        RegistryError::AdmissionRestorationUnavailable,
        None,
    )
}

fn recovery_fence() -> RegistryPortError {
    registry_port_error(
        PortErrorKind::OutcomeUnknown,
        PortRetryability::AfterReadback,
        RegistryError::MutationOutcomeUnknown,
        None,
    )
}

/// Mints 16 bytes of qualified OS entropy as an RFC 4122 version-4 UUID.
fn mint_fresh_id<C: CancellationProbe>(
    deadline: Instant,
    work: &dyn Fn(MigrationBootstrapWork) -> Result<(), OwnerError>,
    context: &OperationContext<C>,
) -> Result<[u8; 16], SourceIdentityIdError> {
    context
        .preflight()
        .map_err(SourceIdentityIdError::Context)?;
    if Instant::now() >= deadline {
        return Err(SourceIdentityIdError::Deadline);
    }
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
    context
        .preflight()
        .map_err(SourceIdentityIdError::Context)?;
    Ok(bytes)
}

#[cfg(test)]
mod tests {
    use super::*;
    use search_contracts::{
        Blake3Digest32, DataRootId, InstallationIncarnationId, OpaqueRef, OwnerEpoch, RequestId,
    };
    use search_control_redb::{
        ControlSnapshotPublisher, JournalIdentity, JournalLimits, JournalProfileInitialization,
        MigrationBootstrapWork, MutationId, PersistentControlJournal,
    };
    use search_ports::{FakeCancellation, PortFailure};
    use search_runtime_owner::OwnerError;
    use search_source_admission::{AcceptedCapabilities, BASELINE_PROFILE, SectionPlatform};
    use std::fs::{self, File, OpenOptions};
    use std::path::PathBuf;
    use std::sync::atomic::{AtomicU64, Ordering};
    use std::time::{Duration, Instant};

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
        let called = std::cell::Cell::new(false);
        let work = |_w: MigrationBootstrapWork| -> Result<(), OwnerError> {
            called.set(true);
            Ok(())
        };
        let mut adapter = SourceIdentityIdsV2::<FakeCancellation>::bind(deadline, &work);
        let result = adapter.fresh_source_id(&context(true));
        assert!(matches!(result, Err(SourceIdentityIdError::Context(_))));
        assert!(!called.get());
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
    fn expired_original_deadline_refuses_before_work_or_entropy() {
        let called = std::cell::Cell::new(false);
        let work = |_| {
            called.set(true);
            Ok(())
        };
        let mut adapter = SourceIdentityIdsV2::<FakeCancellation>::bind(Instant::now(), &work);
        assert!(matches!(
            adapter.fresh_source_id(&context(false)),
            Err(SourceIdentityIdError::Deadline)
        ));
        assert!(!called.get());
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
        let ns_id = adapter
            .fresh_source_namespace_id(&ctx)
            .expect("namespace id");
        let ns_bytes = ns_id.as_bytes();
        assert_eq!(ns_bytes[6] & 0xf0, 0x40, "version 4");
        assert_eq!(ns_bytes[8] & 0xc0, 0x80, "RFC 4122 variant");
        assert_ne!(bytes, ns_bytes, "distinct allocations");
    }

    struct Scratch(PathBuf);

    impl Scratch {
        fn new() -> Self {
            static NEXT: AtomicU64 = AtomicU64::new(0);
            let path = std::env::temp_dir().join(format!(
                "search-registry-source-ids-{}-{}",
                std::process::id(),
                NEXT.fetch_add(1, Ordering::Relaxed)
            ));
            fs::create_dir(&path).expect("fresh scratch");
            Self(path)
        }

        fn file(&self, create: bool) -> File {
            OpenOptions::new()
                .read(true)
                .write(true)
                .create_new(create)
                .open(self.0.join("control.redb"))
                .expect("scratch journal")
        }
    }

    impl Drop for Scratch {
        fn drop(&mut self) {
            fs::remove_dir_all(&self.0).expect("closed scratch");
        }
    }

    fn journal_identity() -> JournalIdentity {
        JournalIdentity {
            installation_incarnation_id: InstallationIncarnationId::from_bytes([2; 16]),
            data_root_id: DataRootId::from_bytes([3; 16]),
            owner_epoch: OwnerEpoch::new(1).expect("epoch"),
            path_identity_digest: Blake3Digest32::from_bytes([4; 32]),
            schema_family_digest: Blake3Digest32::from_bytes([5; 32]),
            schema_version: search_control_redb::CURRENT_JOURNAL_SCHEMA_VERSION,
        }
    }

    fn initialized_journal(
        scratch: &Scratch,
        context: &OperationContext<FakeCancellation>,
    ) -> PersistentControlJournal {
        let mut journal = PersistentControlJournal::create(
            scratch.file(true),
            journal_identity(),
            JournalLimits::BASELINE,
        )
        .expect("actual redb initialization");
        journal
            .initialize_operating_profile(
                JournalProfileInitialization::Direct,
                MutationId([9; 32]),
                Blake3Digest32::from_bytes([10; 32]),
                context,
            )
            .expect("direct profile");
        journal
    }

    fn bind_v4<'a>(
        journal: &'a mut PersistentControlJournal,
        publisher: &'a mut ControlSnapshotPublisher,
        deadline: Instant,
        work: &'a dyn Fn(MigrationBootstrapWork) -> Result<(), OwnerError>,
    ) -> super::super::RegistryJournal<'a, FakeCancellation> {
        super::super::RegistryJournal::bind_original_v4(
            journal,
            publisher,
            journal_identity(),
            search_source_registry::api::DEFAULT_REGISTRY_LIMITS,
            deadline,
            work,
            SectionPlatform {
                os: "windows".into(),
                arch: "x64".into(),
            },
            AcceptedCapabilities {
                profiles: vec![BASELINE_PROFILE.into()],
            },
        )
        .expect("normal v4 journal binding")
    }

    fn bind_legacy<'a>(
        journal: &'a mut PersistentControlJournal,
        publisher: &'a mut ControlSnapshotPublisher,
        work: &'a dyn Fn(MigrationBootstrapWork) -> Result<(), OwnerError>,
    ) -> super::super::RegistryJournal<'a, FakeCancellation> {
        super::super::RegistryJournal::bind_original(
            journal,
            publisher,
            journal_identity(),
            search_source_registry::api::DEFAULT_REGISTRY_LIMITS,
            Instant::now() + Duration::from_secs(30),
            work,
        )
        .expect("legacy journal binding")
    }

    #[test]
    fn registry_journal_delegates_actual_entropy_to_original_v4() {
        let scratch = Scratch::new();
        let setup_context = context(false);
        let mut journal = initialized_journal(&scratch, &setup_context);
        let mut publisher = ControlSnapshotPublisher::new();
        let calls = std::cell::Cell::new(0_usize);
        let work = |_unit: MigrationBootstrapWork| {
            calls.set(calls.get() + 1);
            Ok::<_, OwnerError>(())
        };
        let mut port = bind_v4(
            &mut journal,
            &mut publisher,
            Instant::now() + Duration::from_secs(30),
            &work,
        );
        let source = port.fresh_source_id(&setup_context).expect("source id");
        let namespace = port
            .fresh_source_namespace_id(&setup_context)
            .expect("namespace id");
        assert_eq!(source.as_bytes()[6] & 0xf0, 0x40, "source version 4");
        assert_eq!(source.as_bytes()[8] & 0xc0, 0x80, "source variant");
        assert_eq!(namespace.as_bytes()[6] & 0xf0, 0x40, "namespace version 4");
        assert_eq!(namespace.as_bytes()[8] & 0xc0, 0x80, "namespace variant");
        assert_ne!(source.as_bytes(), namespace.as_bytes());
        assert!(calls.get() >= 8, "both allocations use the native budget");
    }

    #[test]
    fn registry_journal_cancelled_context_consumes_no_native_budget() {
        let scratch = Scratch::new();
        let live = context(false);
        let cancelled = context(true);
        let mut journal = initialized_journal(&scratch, &live);
        let mut publisher = ControlSnapshotPublisher::new();
        let calls = std::cell::Cell::new(0_usize);
        let work = |_unit: MigrationBootstrapWork| {
            calls.set(calls.get() + 1);
            Ok::<_, OwnerError>(())
        };
        let mut port = bind_v4(
            &mut journal,
            &mut publisher,
            Instant::now() + Duration::from_secs(30),
            &work,
        );
        let error = port
            .fresh_source_id(&cancelled)
            .expect_err("cancelled context");
        assert_eq!(error.kind(), PortErrorKind::CancelledBeforeSideEffect);
        assert_eq!(error.retryability(), PortRetryability::SameRequest);
        assert_eq!(*error.reason(), RegistryError::CancelledBeforeCommit);
        assert_eq!(
            calls.get(),
            0,
            "cancellation precedes native work and entropy"
        );
    }

    #[test]
    fn registry_journal_expired_original_deadline_consumes_no_native_budget() {
        let scratch = Scratch::new();
        let live = context(false);
        let mut journal = initialized_journal(&scratch, &live);
        let mut publisher = ControlSnapshotPublisher::new();
        let calls = std::cell::Cell::new(0_usize);
        let work = |_unit: MigrationBootstrapWork| {
            calls.set(calls.get() + 1);
            Ok::<_, OwnerError>(())
        };
        let mut port = bind_v4(&mut journal, &mut publisher, Instant::now(), &work);
        let error = port
            .fresh_source_namespace_id(&live)
            .expect_err("expired original deadline");
        assert_eq!(error.kind(), PortErrorKind::DeadlineDuringOperation);
        assert_eq!(error.retryability(), PortRetryability::Never);
        assert_eq!(*error.reason(), RegistryError::DurabilityRejected);
        assert_eq!(calls.get(), 0, "expiry precedes native work and entropy");
    }

    #[test]
    fn registry_journal_requires_actual_v4_context_and_healthy_recovery() {
        let scratch = Scratch::new();
        let live = context(false);
        let mut journal = initialized_journal(&scratch, &live);
        let mut publisher = ControlSnapshotPublisher::new();
        let calls = std::cell::Cell::new(0_usize);
        let work = |_unit: MigrationBootstrapWork| {
            calls.set(calls.get() + 1);
            Ok::<_, OwnerError>(())
        };
        let mut legacy = bind_legacy(&mut journal, &mut publisher, &work);
        let error = legacy
            .fresh_source_id(&live)
            .expect_err("legacy binding cannot allocate v2 IDs");
        assert_eq!(error.kind(), PortErrorKind::DependencyUnavailable);
        assert_eq!(
            *error.reason(),
            RegistryError::AdmissionRestorationUnavailable
        );
        assert_eq!(
            calls.get(),
            0,
            "missing v4 context consumes no native budget"
        );

        let scratch = Scratch::new();
        let mut journal = initialized_journal(&scratch, &live);
        let mut publisher = ControlSnapshotPublisher::new();
        let mut port = bind_v4(
            &mut journal,
            &mut publisher,
            Instant::now() + Duration::from_secs(30),
            &work,
        );
        port.original_uncertain = true;
        let error = port
            .fresh_source_namespace_id(&live)
            .expect_err("recovery fence");
        assert_eq!(error.kind(), PortErrorKind::OutcomeUnknown);
        assert_eq!(error.retryability(), PortRetryability::AfterReadback);
        assert_eq!(*error.reason(), RegistryError::MutationOutcomeUnknown);
        assert_eq!(
            calls.get(),
            0,
            "recovery fence precedes native work and entropy"
        );
    }
}

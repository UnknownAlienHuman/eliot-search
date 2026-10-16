//! Migration-only borrowing of the original native invocation and allowance.

use std::cell::{Cell, RefCell};
use std::rc::Rc;
use std::time::Instant;

use search_contracts::SourceNamespaceId;
use search_control_redb::migration::{SourceImportRow, SourceMappingHeader, SourceMappingSummary};
use search_os_secrets_windows::{LegacyRevisionDpapiContext, LegacyRevisionDpapiWork};
use search_revision_store::{LegacyRevisionObjectContext, LegacyRevisionObjectWork};
use search_runtime_owner::OwnerError;

use crate::owner_composition::NativeReadOperation;
use crate::plaintext_direct_store;
use crate::revision_protection::RevisionProtector;

pub struct NativeMigrationInput<'view, 'operation, 'owner> {
    pub(crate) namespace: &'view [u8],
    pub(crate) log: &'view [u8],
    pub(crate) protector: &'view RevisionProtector,
    pub(crate) context: &'view MigrationContext<'operation, 'owner>,
    pub(crate) inventory_owner: search_control_redb::migration::LegacyInventoryOwner,
    pub(crate) registration: Option<&'view [u8]>,
}

// Borrowed inputs only. Both native and residual legacy callers use the same
// staging pipeline/encoders; native replay never opens namespace/log locators.
pub struct MigrationSource<'view, 'operation, 'owner> {
    source: &'view plaintext_direct_store::DirectStore,
    native: Option<&'view NativeMigrationInput<'view, 'operation, 'owner>>,
    deadline: Instant,
}

impl<'view, 'operation, 'owner> MigrationSource<'view, 'operation, 'owner> {
    pub(crate) const fn new(
        source: &'view plaintext_direct_store::DirectStore,
        deadline: Instant,
        native: Option<&'view NativeMigrationInput<'view, 'operation, 'owner>>,
    ) -> Self {
        Self {
            source,
            native,
            deadline,
        }
    }

    pub(crate) const fn native(&self) -> Option<&NativeMigrationInput<'view, 'operation, 'owner>> {
        self.native
    }

    pub(crate) fn inventory_root(&self) -> &std::path::Path {
        self.source.inventory_root()
    }

    pub(crate) const fn inventory_namespace(&self) -> [u8; 32] {
        self.source.inventory_namespace()
    }

    pub(crate) fn checkpoint(&self) -> Result<(), String> {
        match self.native {
            Some(native) => native.context.checkpoint(),
            None if Instant::now() >= self.deadline => {
                Err("DIRECT_MIGRATION_DEADLINE_EXCEEDED".to_owned())
            }
            None => Ok(()),
        }
    }

    pub(crate) fn charge(&self, work: LegacyRevisionObjectWork) -> Result<(), String> {
        self.native
            .map_or_else(|| self.checkpoint(), |native| native.context.charge(work))
    }

    /// CPU-only progress through the same refusal/deadline/meter authority.
    /// It is not the post-I/O checkpoint and never refreshes external state.
    pub(crate) fn poll(&self) -> Result<(), String> {
        self.charge(LegacyRevisionObjectWork::Compare(0))
    }

    pub(crate) fn held_observer_dispatch(&self) -> Result<(), String> {
        self.poll()?;
        let Some(native) = self.native else { return Ok(()); };
        let mut operation = native.context.operation.try_borrow_mut()
            .map_err(|_| OwnerError::OwnerOperationConflict.code().to_owned())?;
        operation.migration_held_observer_dispatch().map_err(|error| error.code().to_owned())
    }

    pub(crate) fn root_observation(
        &self,
    ) -> Result<Option<eliot_searchd::native_file::Observation>, String> {
        let Some(native) = self.native else { return Ok(None); };
        native.context.checkpoint()?;
        let mut operation = native.context.operation.try_borrow_mut()
            .map_err(|_| OwnerError::OwnerOperationConflict.code().to_owned())?;
        operation.migration_root_observation().map(Some)
            .map_err(|error| error.code().to_owned())
    }

    pub(crate) fn source_mapping_header(
        &self,
        target: SourceNamespaceId,
    ) -> Result<SourceMappingHeader, String> {
        self.checkpoint()?;
        let header = self.source.source_mapping_header(target)?;
        self.checkpoint()?;
        Ok(header)
    }

    pub(crate) fn verify_migration_snapshot(&self, deadline: Instant) -> Result<[u8; 32], String> {
        self.checkpoint()?;
        let snapshot = self.native.map_or_else(
            || self.source.verify_migration_snapshot(deadline),
            |native| {
                self.source.verify_migration_snapshot_from_bytes(
                    native.namespace,
                    native.log,
                    || native.context.checkpoint(),
                )
            },
        )?;
        self.checkpoint()?;
        Ok(snapshot)
    }

    pub(crate) fn retained_revisions(
        &self,
    ) -> impl ExactSizeIterator<Item = plaintext_direct_store::RevisionMetadata> + '_ {
        self.source.retained_revisions()
    }

    pub(crate) fn retained_revision(
        &self,
        id: &str,
    ) -> Option<plaintext_direct_store::RevisionMetadata> {
        self.source.retained_revision(id)
    }

    pub(crate) fn inventory_heads(
        &self,
    ) -> impl Iterator<Item = &search_source_registry::LegacyDirectSourceRecord> + '_ {
        self.source.inventory_heads()
    }

    pub(crate) fn inventory_revisions(
        &self,
    ) -> impl ExactSizeIterator<Item = &search_source_registry::LegacyDirectSourceRecord> + '_ {
        self.source.inventory_revisions()
    }

    pub(crate) fn compile_source_mapping(
        &self,
        target: SourceNamespaceId,
        deadline: Instant,
        emit: impl FnMut(&[u8]) -> Result<(), String>,
    ) -> Result<SourceMappingSummary, String> {
        match self.native {
            Some(native) => self.source.compile_source_mapping_from_bytes(
                target,
                native.namespace,
                native.log,
                || native.context.checkpoint(),
                emit,
            ),
            None => self.source.compile_source_mapping(target, deadline, emit),
        }
    }

    pub(crate) fn compile_source_mapping_with_rows(
        &self,
        target: SourceNamespaceId,
        deadline: Instant,
        emit: impl FnMut(&[u8]) -> Result<(), String>,
        rows: impl FnMut(SourceImportRow) -> Result<(), String>,
    ) -> Result<SourceMappingSummary, String> {
        match self.native {
            Some(native) => self.source.compile_source_mapping_with_rows_from_bytes(
                target,
                native.namespace,
                native.log,
                || native.context.checkpoint(),
                emit,
                rows,
            ),
            None => self
                .source
                .compile_source_mapping_with_rows(target, deadline, emit, rows),
        }
    }
}

pub struct MigrationContext<'operation, 'owner> {
    operation: RefCell<&'operation mut NativeReadOperation<'owner>>,
    deadline: Instant,
    unexpected_source_effect: Cell<bool>,
    dpapi_first_refusal: Rc<Cell<Option<MigrationDpapiFirstRefusal>>>,
    dpapi_clear_accounting_failed: Rc<Cell<bool>>,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum MigrationDpapiFirstRefusal {
    NativeOwner(OwnerError),
    SourceWriteForbidden,
}

pub struct MigrationDpapiContextFailure {
    first_refusal: Rc<Cell<Option<MigrationDpapiFirstRefusal>>>,
    clear_accounting_failed: Rc<Cell<bool>>,
}

impl MigrationDpapiContextFailure {
    pub(crate) fn first_refusal(&self) -> Option<MigrationDpapiFirstRefusal> {
        self.first_refusal.get()
    }

    pub(crate) fn clear_accounting_failed(&self) -> bool {
        self.clear_accounting_failed.get()
    }

    pub(crate) fn public_reason(&self) -> &'static str {
        if self.clear_accounting_failed() {
            "DPAPI_LEGACY_REVISION_CONTEXT_AND_CLEAR_ACCOUNTING_FAILED"
        } else {
            match self.first_refusal() {
                Some(MigrationDpapiFirstRefusal::NativeOwner(error)) => error.code(),
                Some(MigrationDpapiFirstRefusal::SourceWriteForbidden) => {
                    "DIRECT_MIGRATION_SOURCE_WRITE_FORBIDDEN"
                }
                None => "DPAPI_LEGACY_REVISION_CONTEXT_REFUSED",
            }
        }
    }
}

impl<'operation, 'owner> MigrationContext<'operation, 'owner> {
    pub(crate) fn new(operation: &'operation mut NativeReadOperation<'owner>) -> Self {
        let deadline = operation.migration_deadline();
        Self {
            operation: RefCell::new(operation),
            deadline,
            unexpected_source_effect: Cell::new(false),
            dpapi_first_refusal: Rc::new(Cell::new(None)),
            dpapi_clear_accounting_failed: Rc::new(Cell::new(false)),
        }
    }

    pub(crate) const fn deadline(&self) -> Instant {
        self.deadline
    }

    pub(crate) fn inventory_reobserve(
        &self,
        input: &NativeMigrationInput<'_, '_, '_>,
    ) -> Result<(), String> {
        self.checkpoint()?;
        let mut operation = self
            .operation
            .try_borrow_mut()
            .map_err(|_| OwnerError::OwnerOperationConflict.code().to_owned())?;
        let namespace = operation
            .namespace_bytes()
            .map_err(|error| error.code().to_owned())?;
        let log = operation
            .catalog_bytes()
            .map_err(|error| error.code().to_owned())?;
        operation
            .migration_charge(LegacyRevisionObjectWork::Compare(
                namespace.len() + log.len(),
            ))
            .map_err(|error| error.code().to_owned())?;
        // These are private observations of the already admitted control
        // handles, not source-body reads. The owner observation performs the
        // complete locator/access/owner/key/barrier check after both reads and
        // before any compared metadata can escape this transaction.
        let owner = operation
            .migration_inventory_owner()
            .map_err(|error| error.code().to_owned())?;
        if owner != input.inventory_owner || namespace != input.namespace || log != input.log {
            return Err("DIRECT_MIGRATION_INVENTORY_CHANGED".to_owned());
        }
        Ok(())
    }

    pub(crate) fn checkpoint(&self) -> Result<(), String> {
        if self.dpapi_first_refusal.get().is_some() {
            return Err(self.dpapi_failure().public_reason().to_owned());
        }
        if self.unexpected_source_effect.get() {
            return Err("DIRECT_MIGRATION_SOURCE_WRITE_FORBIDDEN".to_owned());
        }
        self.operation
            .try_borrow_mut()
            .map_err(|_| OwnerError::OwnerOperationConflict.code().to_owned())?
            .migration_checkpoint()
            .map_err(|error| error.code().to_owned())
    }

    pub(crate) fn charge(&self, work: LegacyRevisionObjectWork) -> Result<(), String> {
        if self.dpapi_first_refusal.get().is_some() {
            return Err(self.dpapi_failure().public_reason().to_owned());
        }
        if self.unexpected_source_effect.get() {
            return Err("DIRECT_MIGRATION_SOURCE_WRITE_FORBIDDEN".to_owned());
        }
        self.operation
            .try_borrow_mut()
            .map_err(|_| OwnerError::OwnerOperationConflict.code().to_owned())?
            .migration_charge(work)
            .map_err(|error| error.code().to_owned())
    }

    fn dpapi_refusal(&self, cause: MigrationDpapiFirstRefusal) -> MigrationDpapiContextFailure {
        if self.dpapi_first_refusal.get().is_none() {
            self.dpapi_first_refusal.set(Some(cause));
        }
        self.dpapi_failure()
    }

    fn dpapi_failure(&self) -> MigrationDpapiContextFailure {
        MigrationDpapiContextFailure {
            first_refusal: Rc::clone(&self.dpapi_first_refusal),
            clear_accounting_failed: Rc::clone(&self.dpapi_clear_accounting_failed),
        }
    }

    fn dpapi_checkpoint(&self) -> Result<(), MigrationDpapiContextFailure> {
        if self.dpapi_first_refusal.get().is_some() {
            return Err(self.dpapi_failure());
        }
        if self.unexpected_source_effect.get() {
            return Err(self.dpapi_refusal(MigrationDpapiFirstRefusal::SourceWriteForbidden));
        }
        self.operation.try_borrow_mut().map_or_else(
            |_| {
                Err(self.dpapi_refusal(MigrationDpapiFirstRefusal::NativeOwner(
                    OwnerError::OwnerOperationConflict,
                )))
            },
            |mut operation| {
                operation.migration_checkpoint().map_err(|error| {
                    self.dpapi_refusal(MigrationDpapiFirstRefusal::NativeOwner(error))
                })
            },
        )
    }

    fn dpapi_charge(
        &self,
        work: LegacyRevisionObjectWork,
    ) -> Result<(), MigrationDpapiContextFailure> {
        if self.dpapi_first_refusal.get().is_some() {
            return Err(self.dpapi_failure());
        }
        if self.unexpected_source_effect.get() {
            return Err(self.dpapi_refusal(MigrationDpapiFirstRefusal::SourceWriteForbidden));
        }
        self.operation.try_borrow_mut().map_or_else(
            |_| {
                Err(self.dpapi_refusal(MigrationDpapiFirstRefusal::NativeOwner(
                    OwnerError::OwnerOperationConflict,
                )))
            },
            |mut operation| {
                operation.migration_charge(work).map_err(|error| {
                    self.dpapi_refusal(MigrationDpapiFirstRefusal::NativeOwner(error))
                })
            },
        )
    }

    fn dpapi_account_native_clear(
        &self,
        actual_bytes: usize,
    ) -> Result<(), MigrationDpapiContextFailure> {
        let was_refused = self.dpapi_first_refusal.get().is_some();
        let result = self.operation.try_borrow_mut().map_or(
            Err((OwnerError::OwnerOperationConflict, true)),
            |mut operation| operation.migration_account_native_clear(actual_bytes),
        );
        match result {
            Ok(()) if !was_refused => Ok(()),
            Ok(()) => Err(self.dpapi_failure()),
            Err((first_refusal, clear_accounting_failed)) => {
                if clear_accounting_failed {
                    self.dpapi_clear_accounting_failed.set(true);
                }
                Err(self.dpapi_refusal(MigrationDpapiFirstRefusal::NativeOwner(first_refusal)))
            }
        }
    }
}

// Object reads receive a shared adapter. Each method releases the native borrow
// before the object/platform kernel can call back; reentry refuses rather than
// panicking. External artifact publication remains with its existing owner.
impl LegacyRevisionObjectContext for &MigrationContext<'_, '_> {
    type Error = String;

    fn checkpoint(&mut self) -> Result<(), Self::Error> {
        MigrationContext::checkpoint(self)
    }

    fn progress(&mut self) -> Result<(), Self::Error> {
        MigrationContext::charge(self, LegacyRevisionObjectWork::Compare(0))
    }

    fn charge(&mut self, work: LegacyRevisionObjectWork) -> Result<(), Self::Error> {
        MigrationContext::charge(self, work)
    }

    fn held_observer_dispatch(&mut self) -> Result<(), Self::Error> {
        self.progress()?;
        self.operation.try_borrow_mut()
            .map_err(|_| OwnerError::OwnerOperationConflict.code().to_owned())?
            .migration_held_observer_dispatch()
            .map_err(|error| error.code().to_owned())
    }

    fn effect_attempted(&mut self) {
        self.unexpected_source_effect.set(true);
    }
}

#[cfg(all(test, windows))]
mod object_progress_tests {
    use super::*;
    use crate::revision_protection::TestCredentialGuard;
    use std::cell::Cell;
    use std::fs;

    #[test]
    fn object_progress_native_pending_after_abi_returns_no_body_and_stays_terminal() {
        use crate::owner_composition::{NativeRootAdmission, NativeRootLimits, NativeRootRequest,
            admit_existing, initialize_new};
        use search_runtime_owner::DataRootOpenMode;
        use std::ffi::OsString;
        use std::io::Write;
        use std::sync::Arc;
        use std::sync::atomic::{AtomicBool, Ordering};
        use std::time::{Duration, SystemTime, UNIX_EPOCH};

        let temp = fs::canonicalize(std::env::temp_dir()).unwrap();
        let stamp = SystemTime::now().duration_since(UNIX_EPOCH).unwrap().as_nanos();
        let base = temp.join(format!("search-object-progress-pending-{}-{stamp}", std::process::id()));
        fs::create_dir(&base).unwrap();
        let root = base.join("data");
        let output = base.join("output");
        let sources = base.join("sources");
        for path in [&root, &output, &sources] { fs::create_dir(path).unwrap(); }
        let limits = || NativeRootLimits::new([256, 256 * 1024, 512 * 1024,
            1024 * 1024 * 1024, 16384, 256]).unwrap();
        let deadline = Instant::now() + Duration::from_secs(120);
        let credentials = TestCredentialGuard::for_native_data_root(&root);
        let initialized = initialize_new(NativeRootRequest::new(DataRootOpenMode::InitializeNew,
            &[OsString::from("--initialize-data-root"), root.as_os_str().to_owned(),
                OsString::from("02690269026902690269026902690269")],
            deadline, Arc::new(AtomicBool::new(false)), limits()).unwrap())
            .unwrap();
        let namespace = crate::sha256::decode_digest(
            fs::read_to_string(root.join("control/namespace.id")).unwrap().trim()).unwrap();
        assert_eq!(credentials.capture_native_namespace().unwrap(), Some(namespace));
        initialized.emit(&mut Vec::new()).unwrap();
        let registration = search_source_registry::encode_legacy_source_root_catalog(
            [sources.to_str().unwrap()]).unwrap();
        {
            let mut catalog = fs::OpenOptions::new().write(true).create_new(true)
                .open(root.join("control/source-roots.v1")).unwrap();
            catalog.write_all(&registration).unwrap();
            catalog.sync_all().unwrap();
        }
        let evidence = || {
            [
                ".eliot-search-installation.v1",
                ".eliot-search-initialization-journal.v2",
                ".eliot-search-owner-state-a.v1",
                ".eliot-search-owner-state-b.v1",
                ".eliot-search-owner.lock",
                ".eliot-search-sealed-owner.lock",
                "control/namespace.id",
                "control/source-events.log",
                "control/source-roots.v1",
            ].map(|relative| fs::read(root.join(relative)).unwrap())
        };
        let before = evidence();
        let cancelled = Arc::new(AtomicBool::new(false));
        let (request, _) = NativeRootRequest::for_source_control_migration(
            &[OsString::from("--stage-source-control-migration"),
                root.as_os_str().to_owned(), OsString::from("10000000-0000-0000-0000-000000000269"),
                output.as_os_str().to_owned()], deadline, Arc::clone(&cancelled), limits()).unwrap();
        let NativeRootAdmission::Inspection(owner) = admit_existing(request).unwrap() else { panic!("native custody"); };
        let armed = Rc::new(Cell::new(false));
        let dispatches = Rc::new(Cell::new(0));
        let phases = Rc::new(RefCell::new(Vec::new()));
        let pending = Rc::new(RefCell::new(None::<std::path::PathBuf>));
        let hook_armed = Rc::clone(&armed);
        let hook_dispatches = Rc::clone(&dispatches);
        let hook_phases = Rc::clone(&phases);
        let hook_pending = Rc::clone(&pending);
        let outcome = owner.run_registration_one_shot(|operation| {
            assert_eq!(operation.migration_deadline(), deadline);
            let retained_registration = operation.registration_bytes()?;
            assert_eq!(retained_registration.as_deref(), Some(registration.as_slice()));
            operation.chain_migration_phase_hook_for_test(Box::new(move |phase| {
                if !hook_armed.get() { return; }
                hook_phases.borrow_mut().push(phase);
                if phase == "migration-held-observer-dispatch" {
                    let count = hook_dispatches.get() + 1;
                    hook_dispatches.set(count);
                    if count == 2 {
                        fs::write(hook_pending.borrow().as_ref().unwrap(), b"external pending adversary").unwrap();
                    }
                }
            }));
            {
                let context = MigrationContext::new(operation);
                let mut original = &context;
                armed.set(true);
                for _ in 0..3 { original.progress()?; }
                assert!(phases.borrow().is_empty(), "CPU progress dispatches no registration I/O");
                let path = root.join("control/source-roots.tmp");
                assert!(!path.exists());
                *pending.borrow_mut() = Some(path.clone());
                let result = super::super::storage_io::read_revision_object_detailed_with_context(
                    &root.join("control/namespace.id"), 256, &context);
                armed.set(false);
                fs::remove_file(&path).unwrap();
                let expected = OwnerError::OwnerRecoveryQuarantined.code().to_owned();
                assert!(matches!(result, Err(ref error) if error == &expected));
                assert_eq!(dispatches.get(), 3, "all three ABIs are inside the final pending fence");
                assert_eq!(original.progress(), Err(expected.clone()));
                assert_eq!(original.held_observer_dispatch(), Err(expected));
                assert_eq!(dispatches.get(), 3, "removing pending cannot renew the original context");
            }
            assert!(!operation.output_attempted());
            Ok(())
        });
        assert_eq!(outcome, Err(OwnerError::OwnerRecoveryQuarantined.code().to_owned()));
        assert_eq!(evidence(), before);
        assert_eq!(fs::read_dir(&output).unwrap().count(), 0);
        assert!(!root.join("control/source-roots.tmp").exists());
        assert!(!cancelled.load(Ordering::Acquire));
        assert!(credentials.cleanup_captured(), "exact fixture credential cleanup failed; artifacts retained");
        let current = fs::canonicalize(&base).unwrap();
        assert_eq!(current, base);
        assert_eq!(current.parent(), Some(temp.as_path()));
        assert!(current.file_name().unwrap().to_str().unwrap().starts_with("search-object-progress-pending-"));
        fs::remove_dir_all(current).unwrap();
    }

    #[test]
    fn object_progress_native_original_cancel_between_abis_is_sticky() {
        use crate::owner_composition::{NativeRootAdmission, NativeRootLimits, NativeRootRequest,
            admit_existing, initialize_new};
        use search_runtime_owner::DataRootOpenMode;
        use std::ffi::OsString;
        use std::sync::Arc;
        use std::sync::atomic::{AtomicBool, Ordering};
        use std::time::{Duration, SystemTime, UNIX_EPOCH};

        let temp = fs::canonicalize(std::env::temp_dir()).unwrap();
        let stamp = SystemTime::now().duration_since(UNIX_EPOCH).unwrap().as_nanos();
        let base = temp.join(format!("search-object-progress-{}-{stamp}", std::process::id()));
        fs::create_dir(&base).unwrap();
        let root = base.join("data");
        let output = base.join("output");
        fs::create_dir(&root).unwrap();
        fs::create_dir(&output).unwrap();
        let limits = || NativeRootLimits::new([256, 256 * 1024, 512 * 1024,
            1024 * 1024 * 1024, 16384, 256]).unwrap();
        let deadline = Instant::now() + Duration::from_secs(120);
        let credentials = TestCredentialGuard::for_native_data_root(&root);
        let initialized = initialize_new(NativeRootRequest::new(DataRootOpenMode::InitializeNew,
            &[OsString::from("--initialize-data-root"), root.as_os_str().to_owned(),
                OsString::from("02690269026902690269026902690269")],
            deadline, Arc::new(AtomicBool::new(false)), limits()).unwrap())
            .unwrap();
        let before = fs::read(root.join("control/namespace.id")).unwrap();
        let namespace = crate::sha256::decode_digest(std::str::from_utf8(&before).unwrap().trim()).unwrap();
        assert_eq!(credentials.capture_native_namespace().unwrap(), Some(namespace));
        initialized.emit(&mut Vec::new()).unwrap();
        let cancelled = Arc::new(AtomicBool::new(false));
        let (request, _) = NativeRootRequest::for_source_control_migration(
            &[OsString::from("--stage-source-control-migration"),
                root.as_os_str().to_owned(), OsString::from("10000000-0000-0000-0000-000000000269"),
                output.as_os_str().to_owned()], deadline, Arc::clone(&cancelled), limits()).unwrap();
        let NativeRootAdmission::Inspection(owner) = admit_existing(request).unwrap() else { panic!("native custody"); };
        let dispatches = Rc::new(Cell::new(0));
        let hook_dispatches = Rc::clone(&dispatches);
        let hook_cancelled = Arc::clone(&cancelled);
        let outcome = owner.run_registration_one_shot(|operation| {
            operation.chain_migration_phase_hook_for_test(Box::new(move |phase| {
                if phase == "migration-held-observer-dispatch" {
                    let count = hook_dispatches.get() + 1;
                    hook_dispatches.set(count);
                    if count == 2 { hook_cancelled.store(true, Ordering::Release); }
                }
            }));
            {
                let context = MigrationContext::new(operation);
                let result = super::super::storage_io::read_revision_object_detailed_with_context(
                    &root.join("control/namespace.id"), 256, &context);
                let expected = OwnerError::OwnerCancelledBeforeMutation.code().to_owned();
                assert!(matches!(result, Err(ref error) if error == &expected));
                assert_eq!(dispatches.get(), 2, "second admission refuses before its ABI");
                cancelled.store(false, Ordering::Release);
                let mut original = &context;
                assert_eq!(original.progress(), Err(expected.clone()));
                assert_eq!(original.held_observer_dispatch(), Err(expected));
                assert_eq!(dispatches.get(), 2);
            }
            assert!(!operation.output_attempted());
            Ok(())
        });
        assert_eq!(outcome, Err(OwnerError::OwnerCancelledBeforeMutation.code().to_owned()));
        assert_eq!(fs::read(root.join("control/namespace.id")).unwrap(), before);
        assert_eq!(fs::read_dir(&output).unwrap().count(), 0);
        assert!(credentials.cleanup_captured(), "exact fixture credential cleanup failed; artifacts retained");
        let current = fs::canonicalize(&base).unwrap();
        assert_eq!(current, base);
        assert_eq!(current.parent(), Some(temp.as_path()));
        assert!(current.file_name().unwrap().to_str().unwrap().starts_with("search-object-progress-"));
        fs::remove_dir_all(current).unwrap();
    }
}

impl LegacyRevisionDpapiContext for &MigrationContext<'_, '_> {
    type Error = MigrationDpapiContextFailure;

    fn checkpoint(&mut self) -> Result<(), Self::Error> {
        MigrationContext::dpapi_checkpoint(self)
    }

    fn charge(&mut self, work: LegacyRevisionDpapiWork) -> Result<(), Self::Error> {
        use LegacyRevisionObjectWork as Work;
        let work = match work {
            LegacyRevisionDpapiWork::NativeCall => Work::Observe,
            LegacyRevisionDpapiWork::Allocate(bytes) => Work::Allocate(bytes),
            LegacyRevisionDpapiWork::Process(bytes) => Work::Compare(bytes),
        };
        MigrationContext::dpapi_charge(self, work)
    }

    fn account_native_clear(&mut self, actual_bytes: usize) -> Result<(), Self::Error> {
        MigrationContext::dpapi_account_native_clear(self, actual_bytes)
    }
}

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

    fn charge(&mut self, work: LegacyRevisionObjectWork) -> Result<(), Self::Error> {
        MigrationContext::charge(self, work)
    }

    fn effect_attempted(&mut self) {
        self.unexpected_source_effect.set(true);
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

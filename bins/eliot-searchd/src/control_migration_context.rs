//! Migration-only borrowing of the original native invocation and allowance.

use std::cell::{Cell, RefCell};
use std::time::Instant;

use search_revision_store::{LegacyRevisionObjectContext, LegacyRevisionObjectWork};
use search_runtime_owner::OwnerError;
use search_contracts::SourceNamespaceId;
use search_control_redb::migration::{SourceImportRow, SourceMappingHeader, SourceMappingSummary};

use crate::owner_composition::NativeReadOperation;
use crate::plaintext_direct_store;
use crate::revision_protection::RevisionProtector;

pub struct NativeMigrationInput<'view, 'operation, 'owner> {
    pub(crate) namespace: &'view [u8],
    pub(crate) log: &'view [u8],
    pub(crate) protector: &'view RevisionProtector,
    pub(crate) context: &'view MigrationContext<'operation, 'owner>,
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
        Self { source, native, deadline }
    }

    pub(crate) const fn native(&self) -> Option<&NativeMigrationInput<'view, 'operation, 'owner>> {
        self.native
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
        self.native.map_or_else(|| self.checkpoint(), |native| native.context.charge(work))
    }

    pub(crate) fn source_mapping_header(&self, target: SourceNamespaceId) -> Result<SourceMappingHeader, String> {
        self.checkpoint()?;
        let header = self.source.source_mapping_header(target)?;
        self.checkpoint()?;
        Ok(header)
    }

    pub(crate) fn verify_migration_snapshot(&self, deadline: Instant) -> Result<[u8; 32], String> {
        self.checkpoint()?;
        let snapshot = self.native.map_or_else(
            || self.source.verify_migration_snapshot(deadline),
            |native| self.source.verify_migration_snapshot_from_bytes(
                native.namespace, native.log, || native.context.checkpoint(),
            ),
        )?;
        self.checkpoint()?;
        Ok(snapshot)
    }

    pub(crate) fn retained_revisions(&self) -> impl ExactSizeIterator<Item = plaintext_direct_store::RevisionMetadata> + '_ {
        self.source.retained_revisions()
    }

    pub(crate) fn compile_source_mapping(
        &self,
        target: SourceNamespaceId,
        deadline: Instant,
        emit: impl FnMut(&[u8]) -> Result<(), String>,
    ) -> Result<SourceMappingSummary, String> {
        match self.native {
            Some(native) => self.source.compile_source_mapping_from_bytes(
                target, native.namespace, native.log, || native.context.checkpoint(), emit,
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
                target, native.namespace, native.log, || native.context.checkpoint(), emit, rows,
            ),
            None => self.source.compile_source_mapping_with_rows(target, deadline, emit, rows),
        }
    }
}

pub struct MigrationContext<'operation, 'owner> {
    operation: RefCell<&'operation mut NativeReadOperation<'owner>>,
    deadline: Instant,
    unexpected_source_effect: Cell<bool>,
}

impl<'operation, 'owner> MigrationContext<'operation, 'owner> {
    pub(crate) fn new(operation: &'operation mut NativeReadOperation<'owner>) -> Self {
        let deadline = operation.migration_deadline();
        Self {
            operation: RefCell::new(operation),
            deadline,
            unexpected_source_effect: Cell::new(false),
        }
    }

    pub(crate) const fn deadline(&self) -> Instant {
        self.deadline
    }

    pub(crate) fn checkpoint(&self) -> Result<(), String> {
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
        if self.unexpected_source_effect.get() {
            return Err("DIRECT_MIGRATION_SOURCE_WRITE_FORBIDDEN".to_owned());
        }
        self.operation
            .try_borrow_mut()
            .map_err(|_| OwnerError::OwnerOperationConflict.code().to_owned())?
            .migration_charge(work)
            .map_err(|error| error.code().to_owned())
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

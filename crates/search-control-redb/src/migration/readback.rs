use std::fs::File;
use std::time::Instant;

use redb::{
    Database, ReadTransaction, ReadableTableMetadata, TableHandle,
};

use crate::ControlError;

use super::codec::{
    check, check_file, decode_progress, validate_counts,
};
use super::content::SourceContentManifest;
use super::model::{
    SourceImportBinding, SourceImportCounts, SourceImportRow,
};
use super::{EVENTS, META, REVISIONS, SOURCES};
use super::writer::{INERT_WORK, OWNER_MARKER, reserve};
use crate::MigrationBootstrapWork as Work;

/// A pinned comparison over the database retained by the original owner.
#[expect(clippy::redundant_pub_crate, reason = "The sibling persistent owner receives this state through a crate-private reexport.")]
pub(crate) struct SourceMappingComparison { prefix: PrefixReadback }

impl SourceMappingComparison {
    pub(crate) fn begin_in(database: &Database, binding: SourceImportBinding,
        content: Option<SourceContentManifest>, expected: SourceImportCounts,
        owner_profile: Option<&[u8]>, deadline: Instant,
        work: &dyn Fn(Work) -> Result<(), ControlError>,
    ) -> Result<Self, ControlError> {
        validate_counts(expected, binding)?;
        if let Some(content) = content { content.validate_counts(expected)?; }
        let (prefix, sealed) = read_mapping_in(database, binding, content, owner_profile, None, deadline, work)?;
        if !sealed || prefix.expected != expected { return Err(ControlError::MigrationUnverified); }
        Ok(Self { prefix })
    }

    pub(crate) fn begin_inventory_in(database: &Database, binding: SourceImportBinding,
        expected: SourceImportCounts, owner_profile: &[u8], inventory_profile: &[u8],
        deadline: Instant, work: &dyn Fn(Work) -> Result<(), ControlError>,
    ) -> Result<Self, ControlError> {
        validate_counts(expected, binding)?;
        let (prefix, sealed) = read_mapping_in(database, binding, None,
            Some(owner_profile), Some(inventory_profile), deadline, work)?;
        if !sealed || prefix.expected != expected { return Err(ControlError::MigrationUnverified); }
        Ok(Self { prefix })
    }

    pub(crate) fn compare(&mut self, row: &SourceImportRow, deadline: Instant,
        work: &dyn Fn(Work) -> Result<(), ControlError>,
    ) -> Result<(), ControlError> {
        self.prefix.compare_checked(row, deadline, work)
    }

    pub(crate) fn finish(self, deadline: Instant,
        work: &dyn Fn(Work) -> Result<(), ControlError>,
    ) -> Result<SourceImportCounts, ControlError> {
        reserve(deadline, work, Work::Checkpoint)?;
        let verified = self.prefix.compared;
        self.prefix.finish(deadline)?;
        Ok(verified)
    }
}

/// One reopened, coherent redb read transaction for exact compiler-to-target comparison.
/// It neither writes records nor exposes active-source or query admission operations.
pub struct SourceMappingReadback {
    prefix: PrefixReadback,
    _database: Database,
}

impl SourceMappingReadback {
    /// Open only a sealed existing target with the exact full binding and counts.
    /// Native redb recovery may update the target, never the source catalog.
    ///
    /// # Errors
    /// Returns `TransactionConflict` for rejected counts, `StoreCorrupt` for a
    /// contradictory target, `SchemaMismatch` for a foreign table layout,
    /// `IdentityMismatch` for a foreign binding, `MigrationUnverified` for an
    /// unsealed or short target, `StoreUnavailable` for I/O failure, or
    /// `ReadCancelled` for an expired deadline.
    pub fn open(
        file: File,
        binding: SourceImportBinding,
        expected: SourceImportCounts,
        deadline: Instant,
    ) -> Result<Self, ControlError> {
        Self::open_inner(file, binding, None, expected, deadline)
    }

    /// Compare a sealed target that also binds the exact verified content manifest.
    /// A missing or altered reference cannot be replaced by a successful source-only check.
    ///
    /// # Errors
    /// Returns `TransactionConflict` for rejected counts, `StoreCorrupt` for a
    /// contradictory target, `SchemaMismatch` for a foreign table layout,
    /// `IdentityMismatch` for a foreign binding or manifest,
    /// `MigrationUnverified` for an unsealed or short target,
    /// `StoreUnavailable` for I/O failure, or `ReadCancelled` for an expired
    /// deadline.
    pub fn open_with_content(
        file: File,
        binding: SourceImportBinding,
        content: SourceContentManifest,
        expected: SourceImportCounts,
        deadline: Instant,
    ) -> Result<Self, ControlError> {
        Self::open_inner(file, binding, Some(content), expected, deadline)
    }

    fn open_inner(
        file: File,
        binding: SourceImportBinding,
        content: Option<SourceContentManifest>,
        expected: SourceImportCounts,
        deadline: Instant,
    ) -> Result<Self, ControlError> {
        check(deadline)?;
        validate_counts(expected, binding)?;
        if let Some(content) = content {
            content.validate_counts(expected)?;
        }
        let (database, prefix, sealed) =
            open_import(file, binding, content, deadline)?;
        if !sealed || prefix.expected != expected {
            return Err(ControlError::MigrationUnverified);
        }
        Ok(Self {
            prefix,
            _database: database,
        })
    }

    /// Compare the next compiler row using the same checks as interrupted-import resume.
    ///
    /// # Errors
    /// Returns `TransactionConflict` for an out-of-order row, `StoreCorrupt`
    /// for a contradictory stored row, or `ReadCancelled` for an expired deadline.
    pub fn compare(
        &mut self,
        row: &SourceImportRow,
        deadline: Instant,
    ) -> Result<(), ControlError> {
        self.prefix.compare(row, deadline)
    }

    /// A complete readback requires all rows and all five accounting dimensions.
    ///
    /// # Errors
    /// Returns `MigrationUnverified` for a short readback or `ReadCancelled`
    /// for an expired deadline.
    pub fn finish(self, deadline: Instant) -> Result<(), ControlError> {
        self.prefix.finish(deadline)
    }
}

/// The read transaction pins one committed prefix until every row is checked.
/// Its memory footprint does not grow with the number of sources or events.
pub(super) struct PrefixReadback {
    read: ReadTransaction,
    pub(super) expected: SourceImportCounts,
    pub(super) compared: SourceImportCounts,
}

impl PrefixReadback {
    pub(super) fn compare(
        &mut self,
        row: &SourceImportRow,
        deadline: Instant,
    ) -> Result<(), ControlError> {
        self.compare_checked(row, deadline, &INERT_WORK)
    }

    pub(super) fn compare_checked(&mut self, row: &SourceImportRow, deadline: Instant,
        work: &dyn Fn(Work) -> Result<(), ControlError>,
    ) -> Result<(), ControlError> {
        reserve(deadline, work, Work::Allocate(super::ROW_BYTES))?;
        reserve(deadline, work, Work::Read(3 * super::ROW_BYTES + 96))?;
        check(deadline)?;
        if row.sequence != self.compared.events + 1
            || row.sequence > self.expected.events
        {
            return Err(ControlError::TransactionConflict);
        }
        let events = self
            .read
            .open_table(EVENTS)
            .map_err(|_| ControlError::StoreCorrupt)?;
        let value = events
            .get(row.sequence)
            .map_err(|_| ControlError::StoreCorrupt)?
            .ok_or(ControlError::StoreCorrupt)?;
        if value.value() != row.encode()? {
            return Err(ControlError::StoreCorrupt);
        }
        let sources = self
            .read
            .open_table(SOURCES)
            .map_err(|_| ControlError::StoreCorrupt)?;
        let head = sources
            .get(row.source.as_bytes().as_slice())
            .map_err(|_| ControlError::StoreCorrupt)?
            .ok_or(ControlError::StoreCorrupt)?
            .value();
        let last = events
            .get(head)
            .map_err(|_| ControlError::StoreCorrupt)?
            .ok_or(ControlError::StoreCorrupt)?;
        if head < row.sequence
            || head > self.expected.events
            || last.value().get(..16) != Some(row.source.as_bytes().as_slice())
        {
            return Err(ControlError::StoreCorrupt);
        }
        let revisions = self
            .read
            .open_table(REVISIONS)
            .map_err(|_| ControlError::StoreCorrupt)?;
        let first = revisions
            .get(row.revision.as_bytes().as_slice())
            .map_err(|_| ControlError::StoreCorrupt)?
            .ok_or(ControlError::StoreCorrupt)?
            .value();
        let opening = events
            .get(first)
            .map_err(|_| ControlError::StoreCorrupt)?
            .ok_or(ControlError::StoreCorrupt)?;
        if first > row.sequence
            || (row.lifecycle.opens_revision() && first != row.sequence)
            || opening.value().get(..16)
                != Some(row.source.as_bytes().as_slice())
            || opening.value().get(16..32)
                != Some(row.revision.as_bytes().as_slice())
            || opening
                .value()
                .get(81)
                .is_none_or(|flags| *flags & 2 == 0)
        {
            return Err(ControlError::StoreCorrupt);
        }
        reserve(deadline, work, Work::Checkpoint)?;
        self.compared.add(row);
        Ok(())
    }

    pub(super) fn finish(self, deadline: Instant) -> Result<(), ControlError> {
        check(deadline)?;
        if self.compared != self.expected {
            return Err(ControlError::MigrationUnverified);
        }
        Ok(())
    }
}

/// Shared reopen path: an incomplete header is inspectable for resume, not complete.
/// No absent table/header is created and no progress counter is inferred from row count.
pub(super) fn open_import(
    file: File,
    binding: SourceImportBinding,
    content: Option<SourceContentManifest>,
    deadline: Instant,
) -> Result<(Database, PrefixReadback, bool), ControlError> {
    check(deadline)?;
    binding.encode()?;
    content.map(|value| value.encode(binding)).transpose()?;
    check_file(&file, false)?;
    let database = Database::builder()
        .set_cache_size(8 * 1024 * 1024)
        .create_file(file)
        .map_err(|_| ControlError::StoreUnavailable)?;
    let (prefix, sealed) = read_mapping_in(&database, binding, content, None, None, deadline, &INERT_WORK)?;
    Ok((database, prefix, sealed))
}

fn read_mapping_in(database: &Database, binding: SourceImportBinding,
    content: Option<SourceContentManifest>, owner_profile: Option<&[u8]>, inventory_profile: Option<&[u8]>, deadline: Instant,
    work: &dyn Fn(Work) -> Result<(), ControlError>,
) -> Result<(PrefixReadback, bool), ControlError> {
    if owner_profile.is_some_and(|profile| profile.is_empty() || profile.len() > 4096) {
        return Err(ControlError::BudgetExceeded);
    }
    if inventory_profile.is_some_and(|profile| profile.is_empty() || profile.len() > 4096 || owner_profile.is_none()) {
        return Err(ControlError::BudgetExceeded);
    }
    reserve(deadline, work, Work::Allocate(8192))?;
    reserve(deadline, work, Work::Read(8192))?;
    let encoded = binding.encode()?;
    let content_bytes = content.map(|value| value.encode(binding)).transpose()?;
    let read = database
        .begin_read()
        .map_err(|_| ControlError::StoreUnavailable)?;
    let mut required = [META.name(), EVENTS.name(), SOURCES.name(), REVISIONS.name()]
        .into_iter()
        .map(str::to_owned)
        .collect::<std::collections::BTreeSet<_>>();
    if owner_profile.is_some() {
        required.extend(["eliot.control.meta.v1", "eliot.control.operations.v1", "eliot.control.records.v1"]
            .into_iter().map(str::to_owned));
    }
    if inventory_profile.is_some() {
        required.extend(["eliot.import.inventory.meta.v1", "eliot.import.inventory.rows.v1"]
            .into_iter().map(str::to_owned));
    }
    let names = read.list_tables().map_err(|_| ControlError::StoreCorrupt)?
        .take(required.len() + 1).map(|table| table.name().to_owned())
        .collect::<std::collections::BTreeSet<_>>();
    if names != required
        || read
            .list_multimap_tables()
            .map_err(|_| ControlError::StoreCorrupt)?
            .next()
            .is_some()
    {
        return Err(ControlError::SchemaMismatch);
    }
    if let Some(expected) = inventory_profile {
        let definition: redb::TableDefinition<&str, &[u8]> = redb::TableDefinition::new("eliot.import.inventory.meta.v1");
        let meta = read.open_table(definition).map_err(|_| ControlError::StoreCorrupt)?;
        if meta.get("profile").map_err(|_| ControlError::StoreCorrupt)?
            .is_none_or(|value| value.value() != expected) { return Err(ControlError::IdentityMismatch); }
    }
    let (counts, sealed) = {
        let meta = read
            .open_table(META)
            .map_err(|_| ControlError::StoreCorrupt)?;
        if meta.len().map_err(|_| ControlError::StoreCorrupt)?
            != 2 + u64::from(content.is_some()) + u64::from(owner_profile.is_some())
            || meta
                .get("binding")
                .map_err(|_| ControlError::StoreCorrupt)?
                .is_none_or(|value| value.value() != encoded)
        {
            return Err(ControlError::StoreCorrupt);
        }
        if meta.get(OWNER_MARKER).map_err(|_| ControlError::StoreCorrupt)?
            .as_ref().map(redb::AccessGuard::value) != owner_profile {
            return Err(ControlError::IdentityMismatch);
        }
        let stored_content = meta
            .get("content_manifest")
            .map_err(|_| ControlError::StoreCorrupt)?;
        if stored_content.as_ref().map(redb::AccessGuard::value)
            != content_bytes.as_deref()
        {
            return Err(ControlError::IdentityMismatch);
        }
        let progress = meta
            .get("progress")
            .map_err(|_| ControlError::StoreCorrupt)?
            .ok_or(ControlError::StoreCorrupt)?;
        decode_progress(progress.value(), binding)?
    };
    if read
        .open_table(EVENTS)
        .map_err(|_| ControlError::StoreCorrupt)?
        .len()
        .map_err(|_| ControlError::StoreCorrupt)?
        != counts.events
        || read
            .open_table(SOURCES)
            .map_err(|_| ControlError::StoreCorrupt)?
            .len()
            .map_err(|_| ControlError::StoreCorrupt)?
            != counts.sources
        || read
            .open_table(REVISIONS)
            .map_err(|_| ControlError::StoreCorrupt)?
            .len()
            .map_err(|_| ControlError::StoreCorrupt)?
            != counts.occurrences
    {
        return Err(ControlError::StoreCorrupt);
    }
    reserve(deadline, work, Work::Checkpoint)?;
    Ok((
        PrefixReadback {
            read,
            expected: counts,
            compared: SourceImportCounts::default(),
        },
        sealed,
    ))
}

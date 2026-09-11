use std::fs::File;
use std::time::Instant;

use redb::{
    Database, ReadTransaction, ReadableTable, ReadableTableMetadata, TableHandle,
};

use crate::ControlError;

use super::codec::{
    check, check_file, decode_progress, hash_field, validate_counts,
};
use super::content::SourceContentManifest;
use super::model::{
    SourceImportBinding, SourceImportCounts, SourceImportRow,
};
use super::{EVENTS, META, REVISIONS, SOURCES};

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
        check(deadline)?;
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
    let encoded = binding.encode()?;
    let content_bytes = content.map(|value| value.encode(binding)).transpose()?;
    check_file(&file, false)?;
    let database = Database::builder()
        .set_cache_size(8 * 1024 * 1024)
        .create_file(file)
        .map_err(|_| ControlError::StoreUnavailable)?;
    check(deadline)?;
    let read = database
        .begin_read()
        .map_err(|_| ControlError::StoreUnavailable)?;
    let names = read
        .list_tables()
        .map_err(|_| ControlError::StoreCorrupt)?
        .take(5)
        .map(|table| table.name().to_owned())
        .collect::<std::collections::BTreeSet<_>>();
    let required = [META.name(), EVENTS.name(), SOURCES.name(), REVISIONS.name()]
        .into_iter()
        .map(str::to_owned)
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
    let (counts, sealed) = {
        let meta = read
            .open_table(META)
            .map_err(|_| ControlError::StoreCorrupt)?;
        if meta.len().map_err(|_| ControlError::StoreCorrupt)?
            != 2 + u64::from(content.is_some())
            || meta
                .get("binding")
                .map_err(|_| ControlError::StoreCorrupt)?
                .is_none_or(|value| value.value() != encoded)
        {
            return Err(ControlError::StoreCorrupt);
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
    check(deadline)?;
    Ok((
        database,
        PrefixReadback {
            read,
            expected: counts,
            compared: SourceImportCounts::default(),
        },
        sealed,
    ))
}

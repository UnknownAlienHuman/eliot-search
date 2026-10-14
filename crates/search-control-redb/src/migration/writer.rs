use std::fs::File;
use std::time::Instant;

use redb::{Database, Durability, ReadableTable, TableHandle};

use crate::ControlError;

use super::codec::{
    check, check_file, hash_field, validate_counts, validate_successor,
};
use super::content::SourceContentManifest;
use super::model::{
    SourceImportBinding, SourceImportCounts, SourceImportRow,
};
use super::readback::{PrefixReadback, open_import};
use super::{BATCH_ROWS, EVENTS, META, REVISIONS, SOURCES};

/// Transactional staging writer. It cannot publish an active control snapshot.
/// Errors after dispatch block further use; drop it and resume by exact readback.
pub struct SourceMappingImport {
    state: SourceMappingWriteState,
    database: Database,
}

#[expect(clippy::redundant_pub_crate, reason = "The sibling persistent owner receives this state through a crate-private reexport.")]
pub(crate) struct SourceMappingWriteState {
    // Drop a retained read transaction before its native database owner.
    prefix: Option<PrefixReadback>,
    binding: SourceImportBinding,
    counts: SourceImportCounts,
    content: Option<SourceContentManifest>,
    pending: Vec<SourceImportRow>,
    blocked: bool,
    sealed: bool,
    owner_profile: Option<Vec<u8>>,
}

impl SourceMappingImport {
    /// Create only in an explicitly new empty regular file. The caller retains
    /// exclusive source/output ownership. Even failed initialization may leave bytes.
    /// Synchronous redb/OS calls are not interrupted by the cooperative deadline.
    ///
    /// # Errors
    /// Returns `BudgetExceeded` for a rejected binding, `StoreCorrupt` for a
    /// non-empty file, `StoreUnavailable` for I/O failure, `ReadCancelled` for
    /// an expired deadline, or `CommitOutcomeUnknown` once native creation may
    /// have started.
    pub fn create(
        file: File,
        binding: SourceImportBinding,
        deadline: Instant,
    ) -> Result<Self, ControlError> {
        Self::create_inner(file, binding, None, deadline)
    }

    /// Create an inactive target bound to a producer-verified content manifest.
    /// The extra immutable META record makes old unbound readers reject this
    /// envelope. The caller publishes it separately, never over a v1 target.
    ///
    /// # Errors
    /// Returns `BudgetExceeded` for a rejected binding, `IdentityMismatch` for
    /// a manifest bound to another import, `StoreCorrupt` for a non-empty
    /// file, `StoreUnavailable` for I/O failure, `ReadCancelled` for an
    /// expired deadline, or `CommitOutcomeUnknown` once native creation may
    /// have started.
    pub fn create_with_content(
        file: File,
        binding: SourceImportBinding,
        content: SourceContentManifest,
        deadline: Instant,
    ) -> Result<Self, ControlError> {
        Self::create_inner(file, binding, Some(content), deadline)
    }

    fn create_inner(
        file: File,
        binding: SourceImportBinding,
        content: Option<SourceContentManifest>,
        deadline: Instant,
    ) -> Result<Self, ControlError> {
        check(deadline)?;
        binding.encode()?;
        content.map(|value| value.encode(binding)).transpose()?;
        check_file(&file, true)?;
        let database = Database::builder()
            .set_cache_size(8 * 1024 * 1024)
            .create_file(file)
            .map_err(|_| ControlError::CommitOutcomeUnknown)?;
        let state = SourceMappingWriteState::initialize_in(
            &database, binding, content, None, deadline, &INERT_WORK,
        ).map_err(|_| ControlError::CommitOutcomeUnknown)?;
        Ok(Self { state, database })
    }

    /// Reopen an existing target for the same complete source/target/plan binding.
    /// Native redb recovery may run, but no application transaction is dispatched.
    /// Feed the compiler again from event one: `push` compares the committed prefix
    /// and all its source/occurrence indices before accepting any new suffix row.
    /// A sealed target can be verified this way but is never reopened for writes.
    /// Empty, partial-schema, foreign and contradictory targets fail without repair.
    ///
    /// # Errors
    /// Returns `StoreCorrupt` for an empty, partial or contradictory target,
    /// `SchemaMismatch` for a foreign table layout, `IdentityMismatch` for a
    /// foreign binding, `StoreUnavailable` for I/O failure, `ReadCancelled`
    /// for an expired deadline, or `MigrationUnverified` when the committed
    /// prefix cannot be adopted.
    pub fn resume(
        file: File,
        binding: SourceImportBinding,
        deadline: Instant,
    ) -> Result<Self, ControlError> {
        Self::resume_inner(file, binding, None, deadline)
    }

    /// Resume only the same source mapping AND exact content-manifest binding.
    /// Neither an old unbound target nor a different content profile is adopted.
    ///
    /// # Errors
    /// Returns `StoreCorrupt` for an empty, partial or contradictory target,
    /// `SchemaMismatch` for a foreign table layout, `IdentityMismatch` for a
    /// foreign binding or manifest, `StoreUnavailable` for I/O failure,
    /// `ReadCancelled` for an expired deadline, or `MigrationUnverified` when
    /// the committed prefix cannot be adopted.
    pub fn resume_with_content(
        file: File,
        binding: SourceImportBinding,
        content: SourceContentManifest,
        deadline: Instant,
    ) -> Result<Self, ControlError> {
        Self::resume_inner(file, binding, Some(content), deadline)
    }

    fn resume_inner(
        file: File,
        binding: SourceImportBinding,
        content: Option<SourceContentManifest>,
        deadline: Instant,
    ) -> Result<Self, ControlError> {
        let (database, prefix, sealed) =
            open_import(file, binding, content, deadline)?;
        let counts = prefix.expected;
        let prefix = if counts.events == 0 {
            prefix.finish(deadline)?;
            None
        } else {
            Some(prefix)
        };
        Ok(Self { database, state: SourceMappingWriteState {
            prefix,
            binding,
            counts,
            content,
            pending: Vec::with_capacity(BATCH_ROWS),
            blocked: false,
            sealed,
            owner_profile: None,
        } })
    }

    /// Append one exact mapping event using the shared bounded writer.
    /// # Errors
    /// Rejects malformed, out-of-order or blocked rows; possible writes stay unknown.
    pub fn push(&mut self, row: SourceImportRow, deadline: Instant) -> Result<(), ControlError> {
        self.state.push(&self.database, row, deadline, &INERT_WORK)
    }

    /// Seal the complete mapping using the shared durable engine.
    /// # Errors
    /// Rejects incomplete counts or an unverified prefix; possible writes stay unknown.
    pub fn finish(mut self, expected: SourceImportCounts, deadline: Instant) -> Result<(), ControlError> {
        self.state.seal(&self.database, expected, deadline, &INERT_WORK)
    }
}

pub(super) const OWNER_MARKER: &str = "owner_source_stage.v1";
pub(super) const INERT_WORK: fn(crate::MigrationBootstrapWork) -> Result<(), ControlError> = |_| Ok(());

pub(super) fn reserve(deadline: Instant,
    work: &dyn Fn(crate::MigrationBootstrapWork) -> Result<(), ControlError>,
    value: crate::MigrationBootstrapWork,
) -> Result<(), ControlError> {
    check(deadline)?;
    work(value)?;
    check(deadline)
}

impl SourceMappingWriteState {
    pub(crate) fn initialize_in(database: &Database, binding: SourceImportBinding,
        content: Option<SourceContentManifest>, owner_profile: Option<&[u8]>, deadline: Instant,
        work: &dyn Fn(crate::MigrationBootstrapWork) -> Result<(), ControlError>,
    ) -> Result<Self, ControlError> {
        use crate::MigrationBootstrapWork as Work;
        if owner_profile.is_some_and(|profile| profile.is_empty() || profile.len() > 4096) {
            return Err(ControlError::BudgetExceeded);
        }
        reserve(deadline, work, Work::Allocate(BATCH_ROWS * std::mem::size_of::<SourceImportRow>() + 8192))?;
        let header = binding.encode()?;
        let content_bytes = content.map(|value| value.encode(binding)).transpose()?;
        // Initialize only a new legacy store or the exact original control
        // database. Never adopt or overwrite an existing partial mapping.
        reserve(deadline, work, Work::Read(2048))?;
        let read = database.begin_read().map_err(|_| ControlError::StoreUnavailable)?;
        let names = read.list_tables().map_err(|_| ControlError::StoreCorrupt)?
            .take(8).map(|table| table.name().to_owned()).collect::<Vec<_>>();
        let expected: &[&str] = if owner_profile.is_some() {
            &["eliot.control.meta.v1", "eliot.control.operations.v1", "eliot.control.records.v1"]
        } else { &[] };
        if !names.iter().map(String::as_str).eq(expected.iter().copied())
            || read.list_multimap_tables().map_err(|_| ControlError::StoreCorrupt)?.next().is_some() {
            return Err(ControlError::SchemaMismatch);
        }
        drop(read);
        reserve(deadline, work, Work::Write(8192))?;
        let init = (|| {
            let mut write = database.begin_write().map_err(|_| ControlError::StoreUnavailable)?;
            write.set_durability(Durability::Immediate);
            {
                let mut meta = write.open_table(META).map_err(|_| ControlError::StoreCorrupt)?;
                meta.insert("binding", header.as_slice()).map_err(|_| ControlError::StoreUnavailable)?;
                if let Some(bytes) = &content_bytes {
                    meta.insert("content_manifest", bytes.as_slice()).map_err(|_| ControlError::StoreUnavailable)?;
                }
                if let Some(bytes) = owner_profile {
                    meta.insert(OWNER_MARKER, bytes).map_err(|_| ControlError::StoreUnavailable)?;
                }
                meta.insert("progress", SourceImportCounts::default().encode(false).as_slice())
                    .map_err(|_| ControlError::StoreUnavailable)?;
            }
            drop(write.open_table(EVENTS).map_err(|_| ControlError::StoreCorrupt)?);
            drop(write.open_table(SOURCES).map_err(|_| ControlError::StoreCorrupt)?);
            drop(write.open_table(REVISIONS).map_err(|_| ControlError::StoreCorrupt)?);
            reserve(deadline, work, Work::Write(0))?;
            write.commit().map_err(|_| ControlError::CommitOutcomeUnknown)?;
            reserve(deadline, work, Work::Checkpoint)
        })();
        init.map_err(|_| ControlError::CommitOutcomeUnknown)?;
        Ok(Self { prefix: None, binding, counts: SourceImportCounts::default(), content,
            pending: Vec::with_capacity(BATCH_ROWS), blocked: false, sealed: false,
            owner_profile: owner_profile.map(<[u8]>::to_vec) })
    }

    /// Append one exact mapping event; at most 256 rows are buffered. A batch
    /// atomically updates rows, source/revision references and progress counters.
    ///
    /// # Errors
    /// Returns `InvalidValue` for a malformed row, `TransactionConflict` for an
    /// out-of-order or duplicate sequence, `StoreQuarantined` after a rejected
    /// resume comparison, `MigrationUnverified` while a committed prefix is
    /// still being compared, `ReadCancelled` for an expired deadline, or
    /// `CommitOutcomeUnknown` after a possible batch write.
    pub(crate) fn push(
        &mut self,
        database: &Database,
        row: SourceImportRow,
        deadline: Instant,
        work: &dyn Fn(crate::MigrationBootstrapWork) -> Result<(), ControlError>,
    ) -> Result<(), ControlError> {
        check(deadline)?;
        if self.blocked {
            return Err(ControlError::StoreQuarantined);
        }
        if let Some(prefix) = self.prefix.as_mut() {
            // A rejected comparison cannot later turn into successful resume on
            // this handle. Existing rows are never overwritten or counted twice.
            self.blocked = true;
            prefix.compare_checked(&row, deadline, work)?;
            if prefix.compared.events == prefix.expected.events {
                self.prefix
                    .take()
                    .ok_or(ControlError::MigrationUnverified)?
                    .finish(deadline)?;
            }
            self.blocked = false;
            return Ok(());
        }
        if self.sealed {
            return Err(ControlError::TransactionConflict);
        }
        reserve(deadline, work, crate::MigrationBootstrapWork::Allocate(super::ROW_BYTES))?;
        row.encode()?;
        if row.sequence != self.counts.events + self.pending.len() as u64 + 1
            || row.sequence > self.binding.events
        {
            return Err(ControlError::TransactionConflict);
        }
        self.pending.push(row);
        if self.pending.len() == BATCH_ROWS {
            self.flush(database, deadline, work)?;
        }
        Ok(())
    }

    /// Seal only after the source compiler has validated its complete input.
    /// The temporary database still requires independent exact-row readback before publication.
    /// This marker completes source mapping only, never canonical H5 or owner cutover.
    ///
    /// # Errors
    /// Returns `TransactionConflict` for incomplete or surplus counts,
    /// `MigrationUnverified` while a prefix is uncompared or for a mismatched
    /// sealed readback, `StoreQuarantined` after a rejected comparison,
    /// `StoreCorrupt` for a contradictory progress record, `ReadCancelled` for
    /// an expired deadline, or `CommitOutcomeUnknown` after a possible seal write.
    pub(crate) fn seal(
        &mut self,
        database: &Database,
        expected: SourceImportCounts,
        deadline: Instant,
        work: &dyn Fn(crate::MigrationBootstrapWork) -> Result<(), ControlError>,
    ) -> Result<(), ControlError> {
        check(deadline)?;
        if self.blocked {
            return Err(ControlError::StoreQuarantined);
        }
        if self.prefix.is_some() {
            return Err(ControlError::MigrationUnverified);
        }
        validate_counts(expected, self.binding)?;
        if let Some(content) = self.content {
            content.validate_counts(expected)?;
        }
        if self.sealed {
            return if self.counts == expected {
                Ok(())
            } else {
                Err(ControlError::MigrationUnverified)
            };
        }
        self.flush(database, deadline, work)?;
        if self.counts != expected
            || expected.events != self.binding.events
            || expected.sources != self.binding.sources
            || expected.events
                != expected.occurrences
                    + expected.retained_events
                    + expected.retirements
        {
            return Err(ControlError::TransactionConflict);
        }
        check(deadline)?;
        self.blocked = true;
        reserve(deadline, work, crate::MigrationBootstrapWork::Allocate(8192))?;
        let content_bytes = self
            .content
            .map(|value| value.encode(self.binding))
            .transpose()?;
        let seal = (|| {
            reserve(deadline, work, crate::MigrationBootstrapWork::Write(8192))?;
            let mut write = database
                .begin_write()
                .map_err(|_| ControlError::StoreUnavailable)?;
            write.set_durability(Durability::Immediate);
            {
                reserve(deadline, work, crate::MigrationBootstrapWork::Read(8192))?;
                let mut meta = write
                    .open_table(META)
                    .map_err(|_| ControlError::StoreCorrupt)?;
                if meta
                    .get("progress")
                    .map_err(|_| ControlError::StoreUnavailable)?
                    .is_none_or(|value| value.value() != self.counts.encode(false))
                {
                    return Err(ControlError::StoreCorrupt);
                }
                if meta.get(OWNER_MARKER).map_err(|_| ControlError::StoreUnavailable)?
                    .as_ref().map(redb::AccessGuard::value) != self.owner_profile.as_deref() {
                    return Err(ControlError::IdentityMismatch);
                }
                let stored_content = meta
                    .get("content_manifest")
                    .map_err(|_| ControlError::StoreUnavailable)?;
                if stored_content.as_ref().map(redb::AccessGuard::value)
                    != content_bytes.as_deref()
                {
                    return Err(ControlError::IdentityMismatch);
                }
                drop(stored_content);
                meta.insert("progress", self.counts.encode(true).as_slice())
                    .map_err(|_| ControlError::StoreUnavailable)?;
            }
            reserve(deadline, work, crate::MigrationBootstrapWork::Write(0))?;
            write
                .commit()
                .map_err(|_| ControlError::CommitOutcomeUnknown)?;
            reserve(deadline, work, crate::MigrationBootstrapWork::Checkpoint)
        })();
        seal.map_err(|_| ControlError::CommitOutcomeUnknown)?;
        self.sealed = true;
        self.blocked = false;
        Ok(())
    }

    #[expect(
        clippy::too_many_lines,
        reason = "Keep the validated batch and its immediate durable commit within one redb transaction boundary."
    )]
    fn flush(&mut self, database: &Database, deadline: Instant,
        work: &dyn Fn(crate::MigrationBootstrapWork) -> Result<(), ControlError>,
    ) -> Result<(), ControlError> {
        check(deadline)?;
        if self.blocked {
            return Err(ControlError::StoreQuarantined);
        }
        if self.prefix.is_some() || self.sealed {
            return Err(ControlError::MigrationUnverified);
        }
        if self.pending.is_empty() {
            return Ok(());
        }
        reserve(deadline, work, crate::MigrationBootstrapWork::Allocate(8192))?;
        let binding = self.binding.encode()?;
        let content_bytes = self
            .content
            .map(|value| value.encode(self.binding))
            .transpose()?;
        self.blocked = true;
        let result = (|| {
            let mut counts = self.counts;
            reserve(deadline, work, crate::MigrationBootstrapWork::Write(8192))?;
            let mut write = database
                .begin_write()
                .map_err(|_| ControlError::StoreUnavailable)?;
            write.set_durability(Durability::Immediate);
            {
                reserve(deadline, work, crate::MigrationBootstrapWork::Read(8192))?;
                let mut meta = write
                    .open_table(META)
                    .map_err(|_| ControlError::StoreCorrupt)?;
                if meta
                    .get("binding")
                    .map_err(|_| ControlError::StoreUnavailable)?
                    .is_none_or(|value| value.value() != binding)
                    || meta
                        .get("progress")
                        .map_err(|_| ControlError::StoreUnavailable)?
                        .is_none_or(|value| value.value() != counts.encode(false))
                {
                    return Err(ControlError::StoreCorrupt);
                }
                if meta.get(OWNER_MARKER).map_err(|_| ControlError::StoreUnavailable)?
                    .as_ref().map(redb::AccessGuard::value) != self.owner_profile.as_deref() {
                    return Err(ControlError::IdentityMismatch);
                }
                let stored_content = meta
                    .get("content_manifest")
                    .map_err(|_| ControlError::StoreUnavailable)?;
                if stored_content.as_ref().map(redb::AccessGuard::value)
                    != content_bytes.as_deref()
                {
                    return Err(ControlError::IdentityMismatch);
                }
                drop(stored_content);
                let mut events = write
                    .open_table(EVENTS)
                    .map_err(|_| ControlError::StoreCorrupt)?;
                let mut sources = write
                    .open_table(SOURCES)
                    .map_err(|_| ControlError::StoreCorrupt)?;
                let mut revisions = write
                    .open_table(REVISIONS)
                    .map_err(|_| ControlError::StoreCorrupt)?;
                for row in &self.pending {
                    reserve(deadline, work, crate::MigrationBootstrapWork::Allocate(super::ROW_BYTES))?;
                    reserve(deadline, work, crate::MigrationBootstrapWork::Read(3 * super::ROW_BYTES + 96))?;
                    reserve(deadline, work, crate::MigrationBootstrapWork::Write(super::ROW_BYTES + 64))?;
                    check(deadline)?;
                    if row.sequence != counts.events + 1
                        || events
                            .get(row.sequence)
                            .map_err(|_| ControlError::StoreUnavailable)?
                            .is_some()
                    {
                        return Err(ControlError::TransactionConflict);
                    }
                    if counts.events == 0 {
                        if row.previous_event.as_bytes() != &[0; 32] {
                            return Err(ControlError::TransactionConflict);
                        }
                    } else {
                        let previous = events
                            .get(counts.events)
                            .map_err(|_| ControlError::StoreUnavailable)?
                            .ok_or(ControlError::StoreCorrupt)?;
                        if hash_field(previous.value(), 6)?
                            != row.previous_event.as_bytes()
                        {
                            return Err(ControlError::TransactionConflict);
                        }
                    }
                    let prior = sources
                        .get(row.source.as_bytes().as_slice())
                        .map_err(|_| ControlError::StoreUnavailable)?
                        .map(|value| value.value());
                    if prior.is_none() != row.lifecycle.opens_source() {
                        return Err(ControlError::TransactionConflict);
                    }
                    if let Some(prior) = prior {
                        let previous = events
                            .get(prior)
                            .map_err(|_| ControlError::StoreUnavailable)?
                            .ok_or(ControlError::StoreCorrupt)?;
                        validate_successor(previous.value(), row)?;
                    } else if row.previous_source_event.as_bytes() != &[0; 32] {
                        return Err(ControlError::TransactionConflict);
                    }
                    let revision = revisions
                        .get(row.revision.as_bytes().as_slice())
                        .map_err(|_| ControlError::StoreUnavailable)?
                        .map(|value| value.value());
                    if revision.is_none() != row.lifecycle.opens_revision() {
                        return Err(ControlError::TransactionConflict);
                    }
                    if row.lifecycle.opens_revision() {
                        revisions
                            .insert(row.revision.as_bytes().as_slice(), row.sequence)
                            .map_err(|_| ControlError::StoreUnavailable)?;
                    }
                    events
                        .insert(row.sequence, row.encode()?.as_slice())
                        .map_err(|_| ControlError::StoreUnavailable)?;
                    sources
                        .insert(row.source.as_bytes().as_slice(), row.sequence)
                        .map_err(|_| ControlError::StoreUnavailable)?;
                    counts.add(row);
                }
                meta.insert("progress", counts.encode(false).as_slice())
                    .map_err(|_| ControlError::StoreUnavailable)?;
            }
            reserve(deadline, work, crate::MigrationBootstrapWork::Write(0))?;
            write
                .commit()
                .map_err(|_| ControlError::CommitOutcomeUnknown)?;
            reserve(deadline, work, crate::MigrationBootstrapWork::Checkpoint)?;
            Ok(counts)
        })();
        match result {
            Ok(counts) => {
                self.counts = counts;
                self.pending.clear();
                self.blocked = false;
                Ok(())
            }
            Err(_) => Err(ControlError::CommitOutcomeUnknown),
        }
    }
}

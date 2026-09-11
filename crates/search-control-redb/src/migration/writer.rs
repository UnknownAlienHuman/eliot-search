use std::fs::File;
use std::time::Instant;

use redb::{Database, Durability, ReadableTable};

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
    // Drop a retained read transaction before its native database owner.
    prefix: Option<PrefixReadback>,
    database: Database,
    binding: SourceImportBinding,
    counts: SourceImportCounts,
    content: Option<SourceContentManifest>,
    pending: Vec<SourceImportRow>,
    blocked: bool,
    sealed: bool,
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
        let header = binding.encode()?;
        let content_bytes = content.map(|value| value.encode(binding)).transpose()?;
        check_file(&file, true)?;
        let database = Database::builder()
            .set_cache_size(8 * 1024 * 1024)
            .create_file(file)
            .map_err(|_| ControlError::CommitOutcomeUnknown)?;
        let init = (|| {
            let mut write = database
                .begin_write()
                .map_err(|_| ControlError::StoreUnavailable)?;
            write.set_durability(Durability::Immediate);
            {
                let mut meta = write
                    .open_table(META)
                    .map_err(|_| ControlError::StoreCorrupt)?;
                meta.insert("binding", header.as_slice())
                    .map_err(|_| ControlError::StoreUnavailable)?;
                if let Some(bytes) = &content_bytes {
                    meta.insert("content_manifest", bytes.as_slice())
                        .map_err(|_| ControlError::StoreUnavailable)?;
                }
                meta.insert(
                    "progress",
                    SourceImportCounts::default().encode(false).as_slice(),
                )
                .map_err(|_| ControlError::StoreUnavailable)?;
            }
            drop(
                write
                    .open_table(EVENTS)
                    .map_err(|_| ControlError::StoreCorrupt)?,
            );
            drop(
                write
                    .open_table(SOURCES)
                    .map_err(|_| ControlError::StoreCorrupt)?,
            );
            drop(
                write
                    .open_table(REVISIONS)
                    .map_err(|_| ControlError::StoreCorrupt)?,
            );
            check(deadline)?;
            write
                .commit()
                .map_err(|_| ControlError::CommitOutcomeUnknown)?;
            check(deadline)
        })();
        init.map_err(|_| ControlError::CommitOutcomeUnknown)?;
        Ok(Self {
            prefix: None,
            database,
            binding,
            counts: SourceImportCounts::default(),
            content,
            pending: Vec::with_capacity(BATCH_ROWS),
            blocked: false,
            sealed: false,
        })
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
        Ok(Self {
            prefix,
            database,
            binding,
            counts,
            content,
            pending: Vec::with_capacity(BATCH_ROWS),
            blocked: false,
            sealed,
        })
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
    pub fn push(
        &mut self,
        row: SourceImportRow,
        deadline: Instant,
    ) -> Result<(), ControlError> {
        check(deadline)?;
        if self.blocked {
            return Err(ControlError::StoreQuarantined);
        }
        if let Some(prefix) = self.prefix.as_mut() {
            // A rejected comparison cannot later turn into successful resume on
            // this handle. Existing rows are never overwritten or counted twice.
            self.blocked = true;
            prefix.compare(&row, deadline)?;
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
        row.encode()?;
        if row.sequence != self.counts.events + self.pending.len() as u64 + 1
            || row.sequence > self.binding.events
        {
            return Err(ControlError::TransactionConflict);
        }
        self.pending.push(row);
        if self.pending.len() == BATCH_ROWS {
            self.flush(deadline)?;
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
    pub fn finish(
        mut self,
        expected: SourceImportCounts,
        deadline: Instant,
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
        self.flush(deadline)?;
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
        let content_bytes = self
            .content
            .map(|value| value.encode(self.binding))
            .transpose()?;
        let seal = (|| {
            let mut write = self
                .database
                .begin_write()
                .map_err(|_| ControlError::StoreUnavailable)?;
            write.set_durability(Durability::Immediate);
            {
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
            check(deadline)?;
            write
                .commit()
                .map_err(|_| ControlError::CommitOutcomeUnknown)?;
            check(deadline)
        })();
        seal.map_err(|_| ControlError::CommitOutcomeUnknown)
    }

    fn flush(&mut self, deadline: Instant) -> Result<(), ControlError> {
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
        let binding = self.binding.encode()?;
        let content_bytes = self
            .content
            .map(|value| value.encode(self.binding))
            .transpose()?;
        self.blocked = true;
        let result = (|| {
            let mut counts = self.counts;
            let mut write = self
                .database
                .begin_write()
                .map_err(|_| ControlError::StoreUnavailable)?;
            write.set_durability(Durability::Immediate);
            {
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
            check(deadline)?;
            write
                .commit()
                .map_err(|_| ControlError::CommitOutcomeUnknown)?;
            check(deadline)?;
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

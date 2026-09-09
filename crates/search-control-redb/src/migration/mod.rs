//! Inactive, typed source-mapping imports. This file format is deliberately not
//! a `ControlJournal`: it carries no live owner, admission, visibility or H5 receipt.
//!
//! The caller owns input exclusion and the admitted output file. Resuming an
//! incomplete target verifies its entire committed prefix before appending rows.

use std::fs::File;
use std::time::Instant;

use redb::{Database, Durability, ReadTransaction, ReadableTable, ReadableTableMetadata,
    TableDefinition, TableHandle};
use search_contracts::{Sha256Digest32, SourceId, SourceNamespaceId, SourceRevisionId};
use crate::ControlError;

mod content;
pub use content::SourceContentManifest;

const META: TableDefinition<&str, &[u8]> = TableDefinition::new("eliot.import.source-map.meta.v1");
const EVENTS: TableDefinition<u64, &[u8]> = TableDefinition::new("eliot.import.source-map.events.v1");
const SOURCES: TableDefinition<&[u8], u64> = TableDefinition::new("eliot.import.source-map.sources.v1");
const REVISIONS: TableDefinition<&[u8], u64> = TableDefinition::new("eliot.import.source-map.revisions.v1");
const MAX_ROWS: u64 = 2_000_000;
const MAX_BYTES: u64 = 512 * 1024 * 1024;
const BATCH_ROWS: usize = 256;
const ROW_BYTES: usize = 83 + 9 * 32;
const HASH_BASE: usize = 83;

/// Identity of one inert mapping import, not installation or namespace ownership.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct SourceImportBinding {
    /// Explicit imported target namespace, never inferred from a source path.
    pub target_namespace: SourceNamespaceId,
    /// Original legacy namespace bytes, labelled SHA-256 rather than UUID/BLAKE3.
    pub legacy_namespace: Sha256Digest32,
    /// Complete verified source-event chain identity.
    pub catalog_snapshot: Sha256Digest32,
    /// Exact occurrence/identity mapping profile.
    pub mapping_profile: Sha256Digest32,
    /// Record-chain fingerprint of the corresponding canonical mapping artifact.
    pub plan_chain: Sha256Digest32,
    /// Expected input events, including retirement and path-only changes.
    pub events: u64,
    /// Expected distinct sources.
    pub sources: u64,
}

impl SourceImportBinding {
    fn encode(self) -> Result<Vec<u8>, ControlError> {
        if self.target_namespace.as_bytes() == &[0; 16] || self.events > MAX_ROWS
            || self.events.checked_mul(ROW_BYTES as u64).is_none_or(|n| n > MAX_BYTES)
            || self.sources > self.events || (self.events == 0) != (self.sources == 0)
        { return Err(ControlError::BudgetExceeded); }
        let mut out = b"ELSMAP01".to_vec();
        out.extend_from_slice(self.target_namespace.as_bytes());
        for digest in [self.legacy_namespace, self.catalog_snapshot, self.mapping_profile, self.plan_chain] {
            out.extend_from_slice(digest.as_bytes());
        }
        out.extend_from_slice(&self.events.to_be_bytes());
        out.extend_from_slice(&self.sources.to_be_bytes());
        Ok(out)
    }
}

/// Positional lifecycle flag bits for one mapping row (wire bytes 81-82).
///
/// Byte 81 packs `opens_source` (bit 0), `opens_revision` (bit 1) and
/// `retires_source` (bit 2); byte 82 carries `native_identity` as `0`/`1`.
/// The two-byte layout is frozen by fixture digests. Row-level validation
/// still rejects impossible combinations; this type only packs the bits.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct SourceLifecycleFlags {
    flags: u8,
    native: u8,
}

impl SourceLifecycleFlags {
    /// Packs four lifecycle observations into the frozen two-byte wire form.
    /// Order: `[opens_source, opens_revision, retires_source, native_identity]`.
    #[must_use]
    pub const fn new(flags: [bool; 4]) -> Self {
        Self {
            flags: (flags[0] as u8) | ((flags[1] as u8) << 1) | ((flags[2] as u8) << 2),
            native: flags[3] as u8,
        }
    }

    /// True only for the source's first activation.
    #[must_use]
    pub const fn opens_source(self) -> bool { self.flags & 1 != 0 }

    /// True only when a new occurrence is allocated.
    #[must_use]
    pub const fn opens_revision(self) -> bool { self.flags & 2 != 0 }

    /// True for a retirement rather than an activation.
    #[must_use]
    pub const fn retires_source(self) -> bool { self.flags & 4 != 0 }

    /// Legacy identity observation class; not a new native-identity qualification.
    #[must_use]
    pub const fn native_identity(self) -> bool { self.native != 0 }

    /// Exact two wire bytes in row order.
    #[must_use]
    pub const fn encode(self) -> [u8; 2] { [self.flags, self.native] }
}

/// Fully typed, content-free mapping row. No arbitrary bytes or source text field.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct SourceImportRow {
    /// Global order in the verified legacy journal.
    pub sequence: u64,
    /// Proposed imported source identity.
    pub source: SourceId,
    /// Proposed occurrence identity, distinct across A/B/A activations.
    pub revision: SourceRevisionId,
    /// Prior occurrence; retained for path-only changes and retirement.
    pub previous_revision: Option<SourceRevisionId>,
    /// Per-source occurrence count, independent of global event order.
    pub occurrence: u64,
    /// Per-source event count, including non-occurrence events.
    pub source_event: u64,
    /// Exact retained source-object length.
    pub source_bytes: u64,
    /// Packed lifecycle observations; see `SourceLifecycleFlags` for the wire layout.
    pub lifecycle: SourceLifecycleFlags,
    /// Original operation identity, not a newly generated receipt.
    pub operation: Sha256Digest32,
    /// Original source identity.
    pub legacy_source: Sha256Digest32,
    /// Original content-object identity, not the occurrence ID.
    pub legacy_revision: Sha256Digest32,
    /// Actual legacy content SHA-256, never relabelled as BLAKE3.
    pub content: Sha256Digest32,
    /// Original stable-object identity fingerprint.
    pub file_identity: Sha256Digest32,
    /// Original path fingerprint, not a disclosed locator.
    pub path: Sha256Digest32,
    /// Exact source-journal event fingerprint.
    pub event: Sha256Digest32,
    /// Global predecessor event fingerprint.
    pub previous_event: Sha256Digest32,
    /// Per-source predecessor event fingerprint.
    pub previous_source_event: Sha256Digest32,
}

impl SourceImportRow {
    fn encode(&self) -> Result<Vec<u8>, ControlError> {
        let lifecycle = self.lifecycle;
        if self.sequence == 0 || self.sequence > MAX_ROWS || self.occurrence == 0 || self.source_event == 0
            || self.source.as_bytes() == &[0; 16] || self.revision.as_bytes() == &[0; 16]
            || self.source_bytes > 64 * 1024 * 1024
            || (lifecycle.retires_source() && (lifecycle.opens_source() || lifecycle.opens_revision()))
            || (lifecycle.opens_source() && (!lifecycle.opens_revision() || self.occurrence != 1 || self.source_event != 1))
            || lifecycle.opens_source() != self.previous_revision.is_none()
            || (!lifecycle.opens_revision() && self.previous_revision != Some(self.revision))
            || (lifecycle.opens_revision() && self.previous_revision == Some(self.revision))
        { return Err(ControlError::InvalidValue); }
        let mut out = Vec::with_capacity(ROW_BYTES);
        out.extend_from_slice(self.source.as_bytes());
        out.extend_from_slice(self.revision.as_bytes());
        out.push(u8::from(self.previous_revision.is_some()));
        out.extend_from_slice(
            self.previous_revision
                .as_ref()
                .map_or(&[0; 16][..], |revision| revision.as_bytes().as_slice()),
        );
        for n in [self.occurrence, self.source_event, self.sequence, self.source_bytes] {
            out.extend_from_slice(&n.to_be_bytes());
        }
        out.extend_from_slice(&lifecycle.encode());
        for digest in [self.operation, self.legacy_source, self.legacy_revision, self.content,
            self.file_identity, self.path, self.event, self.previous_event, self.previous_source_event]
        { out.extend_from_slice(digest.as_bytes()); }
        Ok(out)
    }
}

/// Exact persisted accounting for all imported source events.
#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub struct SourceImportCounts {
    /// All source events.
    pub events: u64,
    /// Distinct mapped sources.
    pub sources: u64,
    /// New revision occurrences.
    pub occurrences: u64,
    /// Events that retain a live occurrence without retirement.
    pub retained_events: u64,
    /// Source retirements.
    pub retirements: u64,
}

impl SourceImportCounts {
    fn encode(self, sealed: bool) -> Vec<u8> {
        let mut out = Vec::with_capacity(41);
        out.push(u8::from(sealed));
        for n in [self.events, self.sources, self.occurrences, self.retained_events, self.retirements] {
            out.extend_from_slice(&n.to_be_bytes());
        }
        out
    }
    fn add(&mut self, row: &SourceImportRow) {
        // Validated sequence/count bounds are at most MAX_ROWS.
        let lifecycle = row.lifecycle;
        self.events += 1;
        self.sources += u64::from(lifecycle.opens_source());
        self.occurrences += u64::from(lifecycle.opens_revision());
        self.retained_events += u64::from(!lifecycle.opens_revision() && !lifecycle.retires_source());
        self.retirements += u64::from(lifecycle.retires_source());
    }
}

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
    pub fn create(file: File, binding: SourceImportBinding, deadline: Instant) -> Result<Self, ControlError> {
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
    pub fn create_with_content(file: File, binding: SourceImportBinding,
        content: SourceContentManifest, deadline: Instant,
    ) -> Result<Self, ControlError> {
        Self::create_inner(file, binding, Some(content), deadline)
    }

    fn create_inner(file: File, binding: SourceImportBinding,
        content: Option<SourceContentManifest>, deadline: Instant,
    ) -> Result<Self, ControlError> {
        check(deadline)?;
        let header = binding.encode()?;
        let content_bytes = content.map(|value| value.encode(binding)).transpose()?;
        check_file(&file, true)?;
        let database = Database::builder().set_cache_size(8 * 1024 * 1024).create_file(file)
            .map_err(|_| ControlError::CommitOutcomeUnknown)?;
        let init = (|| {
            let mut write = database.begin_write().map_err(|_| ControlError::StoreUnavailable)?;
            write.set_durability(Durability::Immediate);
            {
                let mut meta = write.open_table(META).map_err(|_| ControlError::StoreCorrupt)?;
                meta.insert("binding", header.as_slice()).map_err(|_| ControlError::StoreUnavailable)?;
                if let Some(bytes) = &content_bytes {
                    meta.insert("content_manifest", bytes.as_slice()).map_err(|_| ControlError::StoreUnavailable)?;
                }
                meta.insert("progress", SourceImportCounts::default().encode(false).as_slice())
                    .map_err(|_| ControlError::StoreUnavailable)?;
            }
            drop(write.open_table(EVENTS).map_err(|_| ControlError::StoreCorrupt)?);
            drop(write.open_table(SOURCES).map_err(|_| ControlError::StoreCorrupt)?);
            drop(write.open_table(REVISIONS).map_err(|_| ControlError::StoreCorrupt)?);
            check(deadline)?;
            write.commit().map_err(|_| ControlError::CommitOutcomeUnknown)?;
            check(deadline)
        })();
        init.map_err(|_| ControlError::CommitOutcomeUnknown)?;
        Ok(Self { prefix: None, database, binding, counts: SourceImportCounts::default(), content,
            pending: Vec::with_capacity(BATCH_ROWS), blocked: false, sealed: false })
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
    pub fn resume(file: File, binding: SourceImportBinding, deadline: Instant) -> Result<Self, ControlError> {
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
    pub fn resume_with_content(file: File, binding: SourceImportBinding,
        content: SourceContentManifest, deadline: Instant,
    ) -> Result<Self, ControlError> {
        Self::resume_inner(file, binding, Some(content), deadline)
    }

    fn resume_inner(file: File, binding: SourceImportBinding,
        content: Option<SourceContentManifest>, deadline: Instant,
    ) -> Result<Self, ControlError> {
        let (database, prefix, sealed) = open_import(file, binding, content, deadline)?;
        let counts = prefix.expected;
        let prefix = if counts.events == 0 {
            prefix.finish(deadline)?;
            None
        } else { Some(prefix) };
        Ok(Self { prefix, database, binding, counts, content, pending: Vec::with_capacity(BATCH_ROWS),
            blocked: false, sealed })
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
    pub fn push(&mut self, row: SourceImportRow, deadline: Instant) -> Result<(), ControlError> {
        check(deadline)?;
        if self.blocked { return Err(ControlError::StoreQuarantined); }
        if let Some(prefix) = self.prefix.as_mut() {
            // A rejected comparison cannot later turn into successful resume on
            // this handle. Existing rows are never overwritten or counted twice.
            self.blocked = true;
            prefix.compare(&row, deadline)?;
            if prefix.compared.events == prefix.expected.events {
                self.prefix.take().ok_or(ControlError::MigrationUnverified)?.finish(deadline)?;
            }
            self.blocked = false;
            return Ok(());
        }
        if self.sealed { return Err(ControlError::TransactionConflict); }
        row.encode()?;
        if row.sequence != self.counts.events + self.pending.len() as u64 + 1 || row.sequence > self.binding.events {
            return Err(ControlError::TransactionConflict);
        }
        self.pending.push(row);
        if self.pending.len() == BATCH_ROWS { self.flush(deadline)?; }
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
    pub fn finish(mut self, expected: SourceImportCounts, deadline: Instant) -> Result<(), ControlError> {
        check(deadline)?;
        if self.blocked { return Err(ControlError::StoreQuarantined); }
        if self.prefix.is_some() { return Err(ControlError::MigrationUnverified); }
        validate_counts(expected, self.binding)?;
        if let Some(content) = self.content { content.validate_counts(expected)?; }
        if self.sealed {
            return if self.counts == expected { Ok(()) } else { Err(ControlError::MigrationUnverified) };
        }
        self.flush(deadline)?;
        if self.counts != expected || expected.events != self.binding.events || expected.sources != self.binding.sources
            || expected.events != expected.occurrences + expected.retained_events + expected.retirements
        { return Err(ControlError::TransactionConflict); }
        check(deadline)?;
        self.blocked = true;
        let content_bytes = self.content.map(|value| value.encode(self.binding)).transpose()?;
        let seal = (|| {
            let mut write = self.database.begin_write().map_err(|_| ControlError::StoreUnavailable)?;
            write.set_durability(Durability::Immediate);
            {
                let mut meta = write.open_table(META).map_err(|_| ControlError::StoreCorrupt)?;
                if meta.get("progress").map_err(|_| ControlError::StoreUnavailable)?
                    .is_none_or(|v| v.value() != self.counts.encode(false))
                { return Err(ControlError::StoreCorrupt); }
                let stored_content = meta.get("content_manifest").map_err(|_| ControlError::StoreUnavailable)?;
                if stored_content.as_ref().map(redb::AccessGuard::value) != content_bytes.as_deref() {
                    return Err(ControlError::IdentityMismatch);
                }
                drop(stored_content);
                meta.insert("progress", self.counts.encode(true).as_slice()).map_err(|_| ControlError::StoreUnavailable)?;
            }
            check(deadline)?;
            write.commit().map_err(|_| ControlError::CommitOutcomeUnknown)?;
            check(deadline)
        })();
        seal.map_err(|_| ControlError::CommitOutcomeUnknown)
    }

    fn flush(&mut self, deadline: Instant) -> Result<(), ControlError> {
        check(deadline)?;
        if self.blocked { return Err(ControlError::StoreQuarantined); }
        if self.prefix.is_some() || self.sealed { return Err(ControlError::MigrationUnverified); }
        if self.pending.is_empty() { return Ok(()); }
        let binding = self.binding.encode()?;
        let content_bytes = self.content.map(|value| value.encode(self.binding)).transpose()?;
        self.blocked = true;
        let result = (|| {
            let mut counts = self.counts;
            let mut write = self.database.begin_write().map_err(|_| ControlError::StoreUnavailable)?;
            write.set_durability(Durability::Immediate);
            {
                let mut meta = write.open_table(META).map_err(|_| ControlError::StoreCorrupt)?;
                if meta.get("binding").map_err(|_| ControlError::StoreUnavailable)?
                    .is_none_or(|v| v.value() != binding)
                    || meta.get("progress").map_err(|_| ControlError::StoreUnavailable)?
                        .is_none_or(|v| v.value() != counts.encode(false))
                { return Err(ControlError::StoreCorrupt); }
                let stored_content = meta.get("content_manifest").map_err(|_| ControlError::StoreUnavailable)?;
                if stored_content.as_ref().map(redb::AccessGuard::value) != content_bytes.as_deref() {
                    return Err(ControlError::IdentityMismatch);
                }
                drop(stored_content);
                let mut events = write.open_table(EVENTS).map_err(|_| ControlError::StoreCorrupt)?;
                let mut sources = write.open_table(SOURCES).map_err(|_| ControlError::StoreCorrupt)?;
                let mut revisions = write.open_table(REVISIONS).map_err(|_| ControlError::StoreCorrupt)?;
                for row in &self.pending {
                    check(deadline)?;
                    if row.sequence != counts.events + 1 || events.get(row.sequence).map_err(|_| ControlError::StoreUnavailable)?.is_some() {
                        return Err(ControlError::TransactionConflict);
                    }
                    if counts.events == 0 {
                        if row.previous_event.as_bytes() != &[0; 32] { return Err(ControlError::TransactionConflict); }
                    } else {
                        let previous = events.get(counts.events).map_err(|_| ControlError::StoreUnavailable)?.ok_or(ControlError::StoreCorrupt)?;
                        if hash_field(previous.value(), 6)? != row.previous_event.as_bytes() { return Err(ControlError::TransactionConflict); }
                    }
                    let prior = sources.get(row.source.as_bytes().as_slice()).map_err(|_| ControlError::StoreUnavailable)?.map(|v| v.value());
                    if prior.is_none() != row.lifecycle.opens_source() { return Err(ControlError::TransactionConflict); }
                    if let Some(prior) = prior {
                        let previous = events.get(prior).map_err(|_| ControlError::StoreUnavailable)?.ok_or(ControlError::StoreCorrupt)?;
                        validate_successor(previous.value(), row)?;
                    } else if row.previous_source_event.as_bytes() != &[0; 32] {
                        return Err(ControlError::TransactionConflict);
                    }
                    let revision = revisions.get(row.revision.as_bytes().as_slice()).map_err(|_| ControlError::StoreUnavailable)?.map(|v| v.value());
                    if revision.is_none() != row.lifecycle.opens_revision() { return Err(ControlError::TransactionConflict); }
                    if row.lifecycle.opens_revision() {
                        revisions.insert(row.revision.as_bytes().as_slice(), row.sequence).map_err(|_| ControlError::StoreUnavailable)?;
                    }
                    events.insert(row.sequence, row.encode()?.as_slice()).map_err(|_| ControlError::StoreUnavailable)?;
                    sources.insert(row.source.as_bytes().as_slice(), row.sequence).map_err(|_| ControlError::StoreUnavailable)?;
                    counts.add(row);
                }
                meta.insert("progress", counts.encode(false).as_slice()).map_err(|_| ControlError::StoreUnavailable)?;
            }
            check(deadline)?;
            write.commit().map_err(|_| ControlError::CommitOutcomeUnknown)?;
            check(deadline)?;
            Ok(counts)
        })();
        match result {
            Ok(counts) => { self.counts = counts; self.pending.clear(); self.blocked = false; Ok(()) }
            Err(_) => Err(ControlError::CommitOutcomeUnknown),
        }
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
    pub fn open(file: File, binding: SourceImportBinding, expected: SourceImportCounts, deadline: Instant) -> Result<Self, ControlError> {
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
    pub fn open_with_content(file: File, binding: SourceImportBinding,
        content: SourceContentManifest, expected: SourceImportCounts, deadline: Instant,
    ) -> Result<Self, ControlError> {
        Self::open_inner(file, binding, Some(content), expected, deadline)
    }

    fn open_inner(file: File, binding: SourceImportBinding, content: Option<SourceContentManifest>,
        expected: SourceImportCounts, deadline: Instant,
    ) -> Result<Self, ControlError> {
        check(deadline)?;
        validate_counts(expected, binding)?;
        if let Some(content) = content { content.validate_counts(expected)?; }
        let (database, prefix, sealed) = open_import(file, binding, content, deadline)?;
        if !sealed || prefix.expected != expected { return Err(ControlError::MigrationUnverified); }
        Ok(Self { prefix, _database: database })
    }

    /// Compare the next compiler row using the same checks as interrupted-import resume.
    ///
    /// # Errors
    /// Returns `TransactionConflict` for an out-of-order row, `StoreCorrupt`
    /// for a contradictory stored row, or `ReadCancelled` for an expired deadline.
    pub fn compare(&mut self, row: &SourceImportRow, deadline: Instant) -> Result<(), ControlError> {
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
struct PrefixReadback {
    read: ReadTransaction,
    expected: SourceImportCounts,
    compared: SourceImportCounts,
}

impl PrefixReadback {
    fn compare(&mut self, row: &SourceImportRow, deadline: Instant) -> Result<(), ControlError> {
        check(deadline)?;
        if row.sequence != self.compared.events + 1 || row.sequence > self.expected.events {
            return Err(ControlError::TransactionConflict);
        }
        let events = self.read.open_table(EVENTS).map_err(|_| ControlError::StoreCorrupt)?;
        let value = events.get(row.sequence).map_err(|_| ControlError::StoreCorrupt)?.ok_or(ControlError::StoreCorrupt)?;
        if value.value() != row.encode()? { return Err(ControlError::StoreCorrupt); }
        let sources = self.read.open_table(SOURCES).map_err(|_| ControlError::StoreCorrupt)?;
        let head = sources.get(row.source.as_bytes().as_slice()).map_err(|_| ControlError::StoreCorrupt)?.ok_or(ControlError::StoreCorrupt)?.value();
        let last = events.get(head).map_err(|_| ControlError::StoreCorrupt)?.ok_or(ControlError::StoreCorrupt)?;
        if head < row.sequence || head > self.expected.events
            || last.value().get(..16) != Some(row.source.as_bytes().as_slice()) {
            return Err(ControlError::StoreCorrupt);
        }
        let revisions = self.read.open_table(REVISIONS).map_err(|_| ControlError::StoreCorrupt)?;
        let first = revisions.get(row.revision.as_bytes().as_slice()).map_err(|_| ControlError::StoreCorrupt)?.ok_or(ControlError::StoreCorrupt)?.value();
        let opening = events.get(first).map_err(|_| ControlError::StoreCorrupt)?.ok_or(ControlError::StoreCorrupt)?;
        if first > row.sequence || (row.lifecycle.opens_revision() && first != row.sequence)
            || opening.value().get(..16) != Some(row.source.as_bytes().as_slice())
            || opening.value().get(16..32) != Some(row.revision.as_bytes().as_slice())
            || opening.value().get(81).is_none_or(|flags| *flags & 2 == 0)
        { return Err(ControlError::StoreCorrupt); }
        check(deadline)?;
        self.compared.add(row);
        Ok(())
    }

    fn finish(self, deadline: Instant) -> Result<(), ControlError> {
        check(deadline)?;
        if self.compared != self.expected { return Err(ControlError::MigrationUnverified); }
        Ok(())
    }
}

/// Shared reopen path: an incomplete header is inspectable for resume, not complete.
/// No absent table/header is created and no progress counter is inferred from row count.
fn open_import(file: File, binding: SourceImportBinding, content: Option<SourceContentManifest>, deadline: Instant)
    -> Result<(Database, PrefixReadback, bool), ControlError> {
    check(deadline)?;
    let encoded = binding.encode()?;
    let content_bytes = content.map(|value| value.encode(binding)).transpose()?;
    check_file(&file, false)?;
    let database = Database::builder().set_cache_size(8 * 1024 * 1024).create_file(file)
        .map_err(|_| ControlError::StoreUnavailable)?;
    check(deadline)?;
    let read = database.begin_read().map_err(|_| ControlError::StoreUnavailable)?;
    let names = read.list_tables().map_err(|_| ControlError::StoreCorrupt)?
        .take(5).map(|table| table.name().to_owned()).collect::<std::collections::BTreeSet<_>>();
    let required = [META.name(), EVENTS.name(), SOURCES.name(), REVISIONS.name()]
        .into_iter().map(str::to_owned).collect::<std::collections::BTreeSet<_>>();
    if names != required || read.list_multimap_tables().map_err(|_| ControlError::StoreCorrupt)?.next().is_some() {
        return Err(ControlError::SchemaMismatch);
    }
    let (counts, sealed) = {
        let meta = read.open_table(META).map_err(|_| ControlError::StoreCorrupt)?;
        if meta.len().map_err(|_| ControlError::StoreCorrupt)? != 2 + u64::from(content.is_some())
            || meta.get("binding").map_err(|_| ControlError::StoreCorrupt)?.is_none_or(|v| v.value() != encoded)
        { return Err(ControlError::StoreCorrupt); }
        let stored_content = meta.get("content_manifest").map_err(|_| ControlError::StoreCorrupt)?;
        if stored_content.as_ref().map(redb::AccessGuard::value) != content_bytes.as_deref() {
            return Err(ControlError::IdentityMismatch);
        }
        let progress = meta.get("progress").map_err(|_| ControlError::StoreCorrupt)?.ok_or(ControlError::StoreCorrupt)?;
        decode_progress(progress.value(), binding)?
    };
    if read.open_table(EVENTS).map_err(|_| ControlError::StoreCorrupt)?.len().map_err(|_| ControlError::StoreCorrupt)? != counts.events
        || read.open_table(SOURCES).map_err(|_| ControlError::StoreCorrupt)?.len().map_err(|_| ControlError::StoreCorrupt)? != counts.sources
        || read.open_table(REVISIONS).map_err(|_| ControlError::StoreCorrupt)?.len().map_err(|_| ControlError::StoreCorrupt)? != counts.occurrences
    { return Err(ControlError::StoreCorrupt); }
    check(deadline)?;
    Ok((database, PrefixReadback { read, expected: counts, compared: SourceImportCounts::default() }, sealed))
}

fn decode_progress(bytes: &[u8], binding: SourceImportBinding) -> Result<(SourceImportCounts, bool), ControlError> {
    if bytes.len() != 41 || bytes[0] > 1 { return Err(ControlError::StoreCorrupt); }
    let counts = SourceImportCounts {
        events: number(bytes, 1)?, sources: number(bytes, 9)?, occurrences: number(bytes, 17)?,
        retained_events: number(bytes, 25)?, retirements: number(bytes, 33)?,
    };
    let sealed = bytes[0] == 1;
    if counts.events > binding.events || counts.sources > binding.sources
        || counts.sources > counts.occurrences || counts.occurrences > counts.events
        || (counts.events == 0) != (counts.sources == 0)
        || counts.occurrences.checked_add(counts.retained_events)
            .and_then(|n| n.checked_add(counts.retirements)) != Some(counts.events)
        || (sealed && validate_counts(counts, binding).is_err())
    { return Err(ControlError::StoreCorrupt); }
    Ok((counts, sealed))
}

fn validate_counts(value: SourceImportCounts, binding: SourceImportBinding) -> Result<(), ControlError> {
    if value.events != binding.events || value.sources != binding.sources
        || value.sources > value.occurrences || value.occurrences > value.events
        || value.occurrences.checked_add(value.retained_events).and_then(|n| n.checked_add(value.retirements)) != Some(value.events)
    { return Err(ControlError::TransactionConflict); }
    Ok(())
}

fn check(deadline: Instant) -> Result<(), ControlError> {
    if Instant::now() >= deadline { Err(ControlError::ReadCancelled) } else { Ok(()) }
}
fn check_file(file: &File, empty: bool) -> Result<(), ControlError> {
    let metadata = file.metadata().map_err(|_| ControlError::StoreUnavailable)?;
    if !metadata.is_file() || (metadata.len() == 0) != empty { return Err(ControlError::StoreCorrupt); }
    Ok(())
}
fn hash_field(value: &[u8], field: usize) -> Result<&[u8], ControlError> {
    if value.len() != ROW_BYTES { return Err(ControlError::StoreCorrupt); }
    value.get(HASH_BASE + field * 32..HASH_BASE + (field + 1) * 32).ok_or(ControlError::StoreCorrupt)
}
fn number(value: &[u8], start: usize) -> Result<u64, ControlError> {
    Ok(u64::from_be_bytes(value.get(start..start + 8).ok_or(ControlError::StoreCorrupt)?
        .try_into().map_err(|_| ControlError::StoreCorrupt)?))
}
fn validate_successor(previous: &[u8], next: &SourceImportRow) -> Result<(), ControlError> {
    if previous.len() != ROW_BYTES { return Err(ControlError::StoreCorrupt); }
    let lifecycle = next.lifecycle;
    let opens = !lifecycle.retires_source() && (previous[81] & 4 != 0 || hash_field(previous, 2)? != next.legacy_revision.as_bytes());
    if previous.get(..16) != Some(next.source.as_bytes().as_slice())
        || next.previous_revision.as_ref().map(SourceRevisionId::as_bytes).map(<[u8; 16]>::as_slice) != previous.get(16..32)
        || next.source_event != number(previous, 57)?.checked_add(1).ok_or(ControlError::GenerationExhausted)?
        || next.occurrence != number(previous, 49)?.checked_add(u64::from(opens)).ok_or(ControlError::GenerationExhausted)?
        || lifecycle.opens_revision() != opens || (lifecycle.retires_source() && previous[81] & 4 != 0)
        || hash_field(previous, 6)? != next.previous_source_event.as_bytes()
        || hash_field(previous, 1)? != next.legacy_source.as_bytes()
        || hash_field(previous, 4)? != next.file_identity.as_bytes()
        || previous[82] != u8::from(lifecycle.native_identity())
        || (hash_field(previous, 2)? == next.legacy_revision.as_bytes()
            && (hash_field(previous, 3)? != next.content.as_bytes() || number(previous, 73)? != next.source_bytes))
        || (lifecycle.retires_source() && (hash_field(previous, 2)? != next.legacy_revision.as_bytes()
            || hash_field(previous, 3)? != next.content.as_bytes() || hash_field(previous, 5)? != next.path.as_bytes()
            || number(previous, 73)? != next.source_bytes))
    { return Err(ControlError::TransactionConflict); }
    Ok(())
}

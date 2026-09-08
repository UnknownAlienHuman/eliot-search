//! Inactive, typed source-mapping imports. This file format is deliberately not
//! a ControlJournal: it carries no live owner, admission, visibility or H5 receipt.
//! The caller owns both input exclusion and the exclusively created output file.

use std::fs::File;
use std::time::Instant;

use redb::{Database, Durability, ReadTransaction, ReadableTable, ReadableTableMetadata,
    TableDefinition, TableHandle};
use search_contracts::{Sha256Digest32, SourceId, SourceNamespaceId, SourceRevisionId};
use crate::ControlError;

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
    /// True only for the source's first activation.
    pub opens_source: bool,
    /// True only when a new occurrence is allocated.
    pub opens_revision: bool,
    /// True for a retirement rather than an activation.
    pub retires_source: bool,
    /// Legacy identity observation class; not a new native-identity qualification.
    pub native_identity: bool,
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
        if self.sequence == 0 || self.sequence > MAX_ROWS || self.occurrence == 0 || self.source_event == 0
            || self.source.as_bytes() == &[0; 16] || self.revision.as_bytes() == &[0; 16]
            || self.source_bytes > 64 * 1024 * 1024
            || (self.retires_source && (self.opens_source || self.opens_revision))
            || (self.opens_source && (!self.opens_revision || self.occurrence != 1 || self.source_event != 1))
            || self.opens_source != self.previous_revision.is_none()
            || (!self.opens_revision && self.previous_revision != Some(self.revision))
            || (self.opens_revision && self.previous_revision == Some(self.revision))
        { return Err(ControlError::InvalidValue); }
        let mut out = Vec::with_capacity(ROW_BYTES);
        out.extend_from_slice(self.source.as_bytes());
        out.extend_from_slice(self.revision.as_bytes());
        out.push(u8::from(self.previous_revision.is_some()));
        out.extend_from_slice(self.previous_revision.as_ref().map_or(&[0; 16], SourceRevisionId::as_bytes));
        for n in [self.occurrence, self.source_event, self.sequence, self.source_bytes] {
            out.extend_from_slice(&n.to_be_bytes());
        }
        out.push(u8::from(self.opens_source) | (u8::from(self.opens_revision) << 1) | (u8::from(self.retires_source) << 2));
        out.push(u8::from(self.native_identity));
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
        self.events += 1;
        self.sources += u64::from(row.opens_source);
        self.occurrences += u64::from(row.opens_revision);
        self.retained_events += u64::from(!row.opens_revision && !row.retires_source);
        self.retirements += u64::from(row.retires_source);
    }
}

/// Transactional staging writer. It cannot publish an active control snapshot.
/// Errors after dispatch block further use; drop it and re-inspect/rebuild the target.
pub struct SourceMappingImport {
    database: Database,
    binding: SourceImportBinding,
    counts: SourceImportCounts,
    pending: Vec<SourceImportRow>,
    blocked: bool,
}

impl SourceMappingImport {
    /// Create only in an explicitly new empty regular file. The caller retains
    /// exclusive source/output ownership. Even failed initialization may leave bytes.
    /// Synchronous redb/OS calls are not interrupted by the cooperative deadline.
    pub fn create(file: File, binding: SourceImportBinding, deadline: Instant) -> Result<Self, ControlError> {
        check(deadline)?;
        let header = binding.encode()?;
        check_file(&file, true)?;
        let database = Database::builder().set_cache_size(8 * 1024 * 1024).create_file(file)
            .map_err(|_| ControlError::CommitOutcomeUnknown)?;
        let init = (|| {
            let mut write = database.begin_write().map_err(|_| ControlError::StoreUnavailable)?;
            write.set_durability(Durability::Immediate);
            {
                let mut meta = write.open_table(META).map_err(|_| ControlError::StoreCorrupt)?;
                meta.insert("binding", header.as_slice()).map_err(|_| ControlError::StoreUnavailable)?;
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
        Ok(Self { database, binding, counts: SourceImportCounts::default(), pending: Vec::with_capacity(BATCH_ROWS), blocked: false })
    }

    /// Append one exact mapping event; at most 256 rows are buffered. A batch
    /// atomically updates rows, source/revision references and progress counters.
    pub fn push(&mut self, row: SourceImportRow, deadline: Instant) -> Result<(), ControlError> {
        check(deadline)?;
        if self.blocked { return Err(ControlError::StoreQuarantined); }
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
    pub fn finish(mut self, expected: SourceImportCounts, deadline: Instant) -> Result<(), ControlError> {
        validate_counts(expected, self.binding)?;
        self.flush(deadline)?;
        if self.counts != expected || expected.events != self.binding.events || expected.sources != self.binding.sources
            || expected.events != expected.occurrences + expected.retained_events + expected.retirements
        { return Err(ControlError::TransactionConflict); }
        check(deadline)?;
        self.blocked = true;
        let seal = (|| {
            let mut write = self.database.begin_write().map_err(|_| ControlError::StoreUnavailable)?;
            write.set_durability(Durability::Immediate);
            {
                let mut meta = write.open_table(META).map_err(|_| ControlError::StoreCorrupt)?;
                if meta.get("progress").map_err(|_| ControlError::StoreUnavailable)?
                    .is_none_or(|v| v.value() != self.counts.encode(false))
                { return Err(ControlError::StoreCorrupt); }
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
        if self.pending.is_empty() { return Ok(()); }
        let binding = self.binding.encode()?;
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
                    if prior.is_none() != row.opens_source { return Err(ControlError::TransactionConflict); }
                    if let Some(prior) = prior {
                        let previous = events.get(prior).map_err(|_| ControlError::StoreUnavailable)?.ok_or(ControlError::StoreCorrupt)?;
                        validate_successor(previous.value(), row)?;
                    } else if row.previous_source_event.as_bytes() != &[0; 32] {
                        return Err(ControlError::TransactionConflict);
                    }
                    let revision = revisions.get(row.revision.as_bytes().as_slice()).map_err(|_| ControlError::StoreUnavailable)?.map(|v| v.value());
                    if revision.is_none() != row.opens_revision { return Err(ControlError::TransactionConflict); }
                    if row.opens_revision {
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
    read: ReadTransaction,
    _database: Database,
    expected: SourceImportCounts,
    compared: u64,
}

impl SourceMappingReadback {
    /// Open an existing nonempty staging database with the exact expected binding and
    /// terminal accounting. Incomplete imports are not accepted. Native redb recovery
    /// may update the staging file; this is not read-only inspection of the source root.
    pub fn open(file: File, binding: SourceImportBinding, expected: SourceImportCounts, deadline: Instant) -> Result<Self, ControlError> {
        check(deadline)?;
        let encoded = binding.encode()?;
        validate_counts(expected, binding)?;
        check_file(&file, false)?;
        // redb 2.6 uses create_file for an already-admitted nonempty File too.
        // check_file above forbids initializing a missing/empty target here.
        let database = Database::builder().set_cache_size(8 * 1024 * 1024).create_file(file)
            .map_err(|_| ControlError::StoreUnavailable)?;
        let read = database.begin_read().map_err(|_| ControlError::StoreUnavailable)?;
        let names = read.list_tables().map_err(|_| ControlError::StoreCorrupt)?
            .take(5).map(|table| table.name().to_owned()).collect::<std::collections::BTreeSet<_>>();
        let required = [META.name(), EVENTS.name(), SOURCES.name(), REVISIONS.name()]
            .into_iter().map(str::to_owned).collect::<std::collections::BTreeSet<_>>();
        if names != required || read.list_multimap_tables().map_err(|_| ControlError::StoreCorrupt)?.next().is_some() {
            return Err(ControlError::SchemaMismatch);
        }
        {
            let meta = read.open_table(META).map_err(|_| ControlError::StoreCorrupt)?;
            if meta.len().map_err(|_| ControlError::StoreCorrupt)? != 2
                || meta.get("binding").map_err(|_| ControlError::StoreCorrupt)?.is_none_or(|v| v.value() != encoded)
                || meta.get("progress").map_err(|_| ControlError::StoreCorrupt)?.is_none_or(|v| v.value() != expected.encode(true))
                || read.open_table(EVENTS).map_err(|_| ControlError::StoreCorrupt)?.len().map_err(|_| ControlError::StoreCorrupt)? != expected.events
                || read.open_table(SOURCES).map_err(|_| ControlError::StoreCorrupt)?.len().map_err(|_| ControlError::StoreCorrupt)? != expected.sources
                || read.open_table(REVISIONS).map_err(|_| ControlError::StoreCorrupt)?.len().map_err(|_| ControlError::StoreCorrupt)? != expected.occurrences
                || expected.events != binding.events || expected.sources != binding.sources
                || expected.events != expected.occurrences + expected.retained_events + expected.retirements
            { return Err(ControlError::StoreCorrupt); }
        }
        check(deadline)?;
        Ok(Self { read, _database: database, expected, compared: 0 })
    }

    /// Compare the next compiler-generated row and its source/occurrence index entries.
    /// Processing every expected row proves that no skipped, extra or altered row hides in the target.
    pub fn compare(&mut self, row: &SourceImportRow, deadline: Instant) -> Result<(), ControlError> {
        check(deadline)?;
        if row.sequence != self.compared + 1 || row.sequence > self.expected.events { return Err(ControlError::TransactionConflict); }
        let events = self.read.open_table(EVENTS).map_err(|_| ControlError::StoreCorrupt)?;
        let value = events.get(row.sequence).map_err(|_| ControlError::StoreCorrupt)?.ok_or(ControlError::StoreCorrupt)?;
        if value.value() != row.encode()? { return Err(ControlError::StoreCorrupt); }
        let sources = self.read.open_table(SOURCES).map_err(|_| ControlError::StoreCorrupt)?;
        let head = sources.get(row.source.as_bytes().as_slice()).map_err(|_| ControlError::StoreCorrupt)?.ok_or(ControlError::StoreCorrupt)?.value();
        let last = events.get(head).map_err(|_| ControlError::StoreCorrupt)?.ok_or(ControlError::StoreCorrupt)?;
        if head < row.sequence || last.value().get(..16) != Some(row.source.as_bytes().as_slice()) { return Err(ControlError::StoreCorrupt); }
        let revisions = self.read.open_table(REVISIONS).map_err(|_| ControlError::StoreCorrupt)?;
        let first = revisions.get(row.revision.as_bytes().as_slice()).map_err(|_| ControlError::StoreCorrupt)?.ok_or(ControlError::StoreCorrupt)?.value();
        let opening = events.get(first).map_err(|_| ControlError::StoreCorrupt)?.ok_or(ControlError::StoreCorrupt)?;
        if first > row.sequence || (row.opens_revision && first != row.sequence)
            || opening.value().get(..16) != Some(row.source.as_bytes().as_slice())
            || opening.value().get(16..32) != Some(row.revision.as_bytes().as_slice())
            || opening.value().get(81).is_none_or(|flags| *flags & 2 == 0)
        { return Err(ControlError::StoreCorrupt); }
        self.compared += 1;
        check(deadline)
    }

    /// Complete exact-row readback. A prefix, even with a valid database header, fails.
    pub fn finish(self, deadline: Instant) -> Result<(), ControlError> {
        check(deadline)?;
        if self.compared != self.expected.events { return Err(ControlError::MigrationUnverified); }
        Ok(())
    }
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
    let opens = !next.retires_source && (previous[81] & 4 != 0 || hash_field(previous, 2)? != next.legacy_revision.as_bytes());
    if previous.get(..16) != Some(next.source.as_bytes().as_slice())
        || next.previous_revision.as_ref().map(SourceRevisionId::as_bytes).map(|v| v.as_slice()) != previous.get(16..32)
        || next.source_event != number(previous, 57)?.checked_add(1).ok_or(ControlError::GenerationExhausted)?
        || next.occurrence != number(previous, 49)?.checked_add(u64::from(opens)).ok_or(ControlError::GenerationExhausted)?
        || next.opens_revision != opens || (next.retires_source && previous[81] & 4 != 0)
        || hash_field(previous, 6)? != next.previous_source_event.as_bytes()
        || hash_field(previous, 1)? != next.legacy_source.as_bytes()
        || hash_field(previous, 4)? != next.file_identity.as_bytes()
        || previous[82] != u8::from(next.native_identity)
        || (hash_field(previous, 2)? == next.legacy_revision.as_bytes()
            && (hash_field(previous, 3)? != next.content.as_bytes() || number(previous, 73)? != next.source_bytes))
        || (next.retires_source && (hash_field(previous, 2)? != next.legacy_revision.as_bytes()
            || hash_field(previous, 3)? != next.content.as_bytes() || hash_field(previous, 5)? != next.path.as_bytes()
            || number(previous, 73)? != next.source_bytes))
    { return Err(ControlError::TransactionConflict); }
    Ok(())
}

//! Fixed corpus inputs, upload pages, evidence and conditional generation visibility.

mod codec;

use std::path::Path;
use std::sync::atomic::{AtomicBool, Ordering};
use std::time::Instant;

use redb_plain::{
    Database, ReadOnlyDatabase, ReadTransaction, ReadableDatabase, ReadableTable,
    ReadableTableMetadata, TableDefinition, TableError, TableHandle, WriteTransaction,
};
use search_contracts::{SourceId, SourceNamespaceId};
use search_source_registry::{CurrentSourceRecord, ObjectRef};
use serde::{Deserialize, Serialize};

use crate::current_preparation::{CurrentPreparation, PreparationOutcome};
use crate::current_source::{
    SOURCE_ROWS, SOURCE_STATE, catalogue_names, decode_record, read_namespace,
};

pub(super) const TABLE_NAMES: [&str; 5] = [
    "publication_state",
    "generation_sources",
    "generation_pages",
    "generation_evidence",
    "retired_generations",
];
const STATE: TableDefinition<&[u8], &[u8]> = TableDefinition::new(TABLE_NAMES[0]);
const SOURCES: TableDefinition<&[u8], &[u8]> = TableDefinition::new(TABLE_NAMES[1]);
const PAGES: TableDefinition<&[u8], &[u8]> = TableDefinition::new(TABLE_NAMES[2]);
const EVIDENCE: TableDefinition<&[u8], &[u8]> = TableDefinition::new(TABLE_NAMES[3]);
const RETIRED: TableDefinition<&[u8], &[u8]> = TableDefinition::new(TABLE_NAMES[4]);
const PREPARATIONS: TableDefinition<&[u8], &[u8]> = TableDefinition::new("current_preparations");
const STATE_KEY: &[u8] = b"v1";
const MAX_SOURCES: usize = 4096;
const MAX_CAPTURE_BYTES: usize = 16 * 1024 * 1024;
const MAX_STAGED_BYTES: u64 = 512 * 1024 * 1024;
const MAX_POINTS: u64 = 1_000_000;
const MAX_RETIRED: u64 = 8;

/// Closed publication/control failures.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum PublicationError {
    /// A source lacks a usable outcome for its selected input/settings.
    Unprepared,
    /// No generation has been committed.
    NotPublished,
    /// A source, current pointer or pending capture changed.
    Stale,
    /// A possibly dispatched mutation has not been settled.
    Unresolved,
    /// Persisted metadata, a page or a binding is corrupt.
    Corrupt,
    /// The ordinary current catalogue is unsupported or partial.
    UnsupportedSchema,
    /// A finite byte/count/revision allowance was exhausted.
    Limit,
    /// A bounded allocation failed.
    Allocation,
    /// Database I/O failed.
    Database,
    /// The OS random source failed.
    Random,
    /// Cancellation was observed.
    Cancelled,
    /// The original deadline expired.
    Deadline,
}
impl PublicationError {
    /// Content-free public error code.
    #[must_use]
    pub const fn code(self) -> &'static str {
        match self {
            Self::Unprepared => "PUBLICATION_SOURCE_NOT_PREPARED",
            Self::NotPublished => "PUBLICATION_NOT_PUBLISHED",
            Self::Stale => "PUBLICATION_STALE",
            Self::Unresolved => "PUBLICATION_EFFECT_UNRESOLVED",
            Self::Corrupt => "PUBLICATION_CORRUPT",
            Self::UnsupportedSchema => "PUBLICATION_SCHEMA_UNSUPPORTED",
            Self::Limit => "PUBLICATION_LIMIT",
            Self::Allocation => "PUBLICATION_ALLOCATION_FAILED",
            Self::Database => "PUBLICATION_CONTROL_IO",
            Self::Random => "PUBLICATION_RANDOM_UNAVAILABLE",
            Self::Cancelled => "PUBLICATION_CANCELLED",
            Self::Deadline => "PUBLICATION_DEADLINE",
        }
    }
}
type Result<T> = std::result::Result<T, PublicationError>;

/// One exact current source and preparation selection captured together.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct CapturedSource {
    /// The real persisted source row.
    pub source: CurrentSourceRecord,
    /// Exact selected settings and bound outcome, including its revisions.
    pub preparation: CurrentPreparation,
}

/// Immutable generation identity and actual complete content dimensions.
#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Generation {
    /// Persisted random publication namespace, independent of source identity.
    pub namespace: [u8; 16],
    /// Never-reset generation reservation sequence.
    pub sequence: u64,
    /// Captured wanted-corpus revision.
    pub wanted_revision: u64,
    /// Number of captured sources, including empty and Gap outcomes.
    pub source_count: u64,
    /// Number of explicit captured Gaps.
    pub gap_count: u64,
    /// Number of actual staged points.
    pub point_count: u64,
    /// Number of actual complete upload pages.
    pub page_count: u64,
}
impl Generation {
    /// Exact collection name fixed before any remote effects.
    #[must_use]
    pub fn collection(&self) -> String {
        const DIGITS: &[u8; 16] = b"0123456789abcdef";
        let mut collection = String::with_capacity(64);
        collection.push_str("eliot_");
        for byte in self.namespace {
            collection.push(char::from(DIGITS[usize::from(byte >> 4)]));
            collection.push(char::from(DIGITS[usize::from(byte & 15)]));
        }
        collection.push_str("_g");
        collection.push_str(&self.sequence.to_string());
        collection
    }
}

/// The single mutation which may still be targetable on an unserved build.
#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub enum PendingOperation {
    /// Creating the exact collection and scoring index.
    Create,
    /// Inserting one immutable saved page.
    Insert(u64),
}

/// Durable bounded build progress; no remote operation is inferred from a receipt.
#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct PendingGeneration {
    /// Exact unserved generation.
    pub generation: Generation,
    /// Whether all sources and complete pages were persisted before effects.
    pub capture_complete: bool,
    /// Whether collection/index creation returned terminal success.
    pub created: bool,
    /// Number of terminally acknowledged saved upload pages.
    pub uploaded_pages: u64,
    /// A dispatched mutation retained across uncertain results or process exit.
    pub in_flight: Option<PendingOperation>,
    /// Non-reset attempt revision preventing late callbacks from settling a newer dispatch.
    pub operation_revision: u64,
    completed_sources: u64,
    staged_bytes: u64,
    source_namespace: Option<[u8; 16]>,
    expected_current_revision: u64,
    expected_current: Option<Generation>,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct State {
    version: u16,
    namespace: Option<[u8; 16]>,
    next_sequence: u64,
    current_revision: u64,
    current: Option<Generation>,
    pending: Option<PendingGeneration>,
}
impl Default for State {
    fn default() -> Self {
        Self {
            version: 1,
            namespace: None,
            next_sequence: 0,
            current_revision: 0,
            current: None,
            pending: None,
        }
    }
}

/// Bounded owned metadata from one native read-only snapshot; it owns no file bodies.
pub struct CorpusCapture {
    /// Every retained selected source in key order.
    pub sources: Vec<CapturedSource>,
    /// Current committed generation, if any.
    pub current: Option<Generation>,
    /// The existing pending build, if any; it must not be silently replaced.
    pub pending: Option<PendingGeneration>,
    source_namespace: Option<[u8; 16]>,
    wanted_revision: u64,
    state: State,
}
impl CorpusCapture {
    /// Whether current was built from this exact unchanged wanted-corpus guard.
    #[must_use]
    pub fn current_matches(&self) -> bool {
        self.current
            .as_ref()
            .is_some_and(|g| g.wanted_revision == self.wanted_revision)
    }
    /// Whether the existing pending build still captures the entire wanted corpus/current pointer.
    #[must_use]
    pub fn pending_matches(&self) -> bool {
        self.pending.as_ref().is_some_and(|p| {
            p.source_namespace == self.source_namespace
                && p.generation.wanted_revision == self.wanted_revision
                && p.expected_current_revision == self.state.current_revision
                && p.expected_current == self.current
        })
    }
}

/// Actual retained-object and range binding for one backend point.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct PointEvidence {
    /// Real persisted source namespace.
    pub source_namespace_id: SourceNamespaceId,
    /// Real persisted logical source identity.
    pub source_id: SourceId,
    /// Exact retained source input.
    pub input: ObjectRef,
    /// Exact complete retained prepared artifact.
    pub prepared: ObjectRef,
    /// Position within that artifact's ordered descriptors.
    pub unit: u64,
    /// Inclusive representation/source byte start.
    pub start: u64,
    /// Exclusive representation/source byte end.
    pub end: u64,
    /// Inclusive logical-line start.
    pub line_start: u64,
    /// Exclusive logical-line end.
    pub line_end: u64,
    /// Whether the start is a complete line boundary.
    pub starts_at_line_boundary: bool,
    /// Whether the end is a complete line boundary.
    pub ends_at_line_boundary: bool,
}

/// One actual complete unit in an upload page.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct UploadPoint {
    /// Generation-bound collision-checked backend identifier.
    pub id: [u8; 16],
    /// Complete retained evidence binding.
    pub evidence: PointEvidence,
    /// Actual prepared unit text, never aliases, vectors or a digest substitute.
    pub retrieval_text: String,
}

/// One bounded complete durable upload page.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct UploadPage {
    /// Ordered actual points.
    pub points: Vec<UploadPoint>,
}

struct SourceEntry {
    captured: CapturedSource,
    staged_units: u64,
    complete: bool,
}

fn check(deadline: Instant, cancel: &AtomicBool) -> Result<()> {
    if cancel.load(Ordering::Relaxed) {
        Err(PublicationError::Cancelled)
    } else if Instant::now() >= deadline {
        Err(PublicationError::Deadline)
    } else {
        Ok(())
    }
}
const fn source_error(error: crate::current_source::SourceControlError) -> PublicationError {
    use crate::current_source::SourceControlError as E;
    match error {
        E::Database => PublicationError::Database,
        E::UnsupportedSchema => PublicationError::UnsupportedSchema,
        E::Stale => PublicationError::Stale,
        E::Deadline => PublicationError::Deadline,
        E::Cancelled => PublicationError::Cancelled,
        E::RevisionExhausted => PublicationError::Limit,
        E::RandomUnavailable => PublicationError::Random,
        E::Corrupt | E::InvalidLocator | E::UnsupportedPlatform => PublicationError::Corrupt,
    }
}
fn key(sequence: u64, suffix: &[u8]) -> Vec<u8> {
    let mut out = Vec::with_capacity(8 + suffix.len());
    out.extend_from_slice(&sequence.to_be_bytes());
    out.extend_from_slice(suffix);
    out
}
fn state_bytes(state: &State) -> Result<Vec<u8>> {
    let bytes = serde_json::to_vec(state).map_err(|_| PublicationError::Corrupt)?;
    if bytes.len() > 16 * 1024 {
        return Err(PublicationError::Limit);
    }
    Ok(bytes)
}
fn decode_state(bytes: &[u8]) -> Result<State> {
    if bytes.len() > 16 * 1024 {
        return Err(PublicationError::Corrupt);
    }
    let state: State = serde_json::from_slice(bytes).map_err(|_| PublicationError::Corrupt)?;
    if state.version != 1 || state.namespace.is_none() || state.next_sequence == 0 {
        return Err(PublicationError::Corrupt);
    }
    for g in state
        .current
        .iter()
        .chain(state.pending.iter().map(|p| &p.generation))
    {
        if Some(g.namespace) != state.namespace
            || g.sequence == 0
            || g.sequence > state.next_sequence
            || g.source_count > MAX_SOURCES as u64
            || g.gap_count > g.source_count
            || g.point_count > MAX_POINTS
            || g.page_count > MAX_POINTS
        {
            return Err(PublicationError::Corrupt);
        }
    }
    if state.current.is_some() != (state.current_revision != 0) {
        return Err(PublicationError::Corrupt);
    }
    if let Some(p) = &state.pending
        && (p.completed_sources > p.generation.source_count
            || p.uploaded_pages > p.generation.page_count
            || p.staged_bytes > MAX_STAGED_BYTES
            || state
                .current
                .as_ref()
                .is_some_and(|current| current.sequence == p.generation.sequence)
            || (p.capture_complete && p.completed_sources != p.generation.source_count))
    {
        return Err(PublicationError::Corrupt);
    }
    Ok(state)
}
fn read_state(tx: &ReadTransaction) -> Result<State> {
    match tx.open_table(STATE) {
        Ok(table) => {
            let value = table
                .get(STATE_KEY)
                .map_err(|_| PublicationError::Database)?
                .ok_or(PublicationError::Corrupt)?;
            decode_state(value.value())
        }
        Err(TableError::TableDoesNotExist(_)) => Ok(State::default()),
        Err(_) => Err(PublicationError::Database),
    }
}
fn observe<T>(
    path: &Path,
    deadline: Instant,
    cancel: &AtomicBool,
    read: impl FnOnce(&ReadTransaction) -> Result<T>,
) -> Result<T> {
    check(deadline, cancel)?;
    let db = ReadOnlyDatabase::open(path).map_err(|_| PublicationError::Database)?;
    let tx = db.begin_read().map_err(|_| PublicationError::Database)?;
    let result = read(&tx)?;
    check(deadline, cancel)?;
    drop(tx);
    db.close().map_err(|_| PublicationError::Database)?;
    Ok(result)
}
fn edit<T>(
    path: &Path,
    deadline: Instant,
    cancel: &AtomicBool,
    write: impl FnOnce(&WriteTransaction, &mut State) -> Result<T>,
) -> Result<T> {
    check(deadline, cancel)?;
    let db = Database::open(path).map_err(|_| PublicationError::Database)?;
    let tx = db.begin_write().map_err(|_| PublicationError::Database)?;
    let existed = tx
        .list_tables()
        .map_err(|_| PublicationError::Database)?
        .any(|name| name.name() == TABLE_NAMES[0]);
    for table in [SOURCES, PAGES, EVIDENCE, RETIRED] {
        drop(
            tx.open_table(table)
                .map_err(|_| PublicationError::Database)?,
        );
    }
    let mut state = {
        let table = tx
            .open_table(STATE)
            .map_err(|_| PublicationError::Database)?;
        let value = table
            .get(STATE_KEY)
            .map_err(|_| PublicationError::Database)?;
        match value {
            Some(value) => decode_state(value.value())?,
            None if !existed => State::default(),
            None => return Err(PublicationError::Corrupt),
        }
    };
    let result = write(&tx, &mut state)?;
    let bytes = state_bytes(&state)?;
    tx.open_table(STATE)
        .map_err(|_| PublicationError::Database)?
        .insert(STATE_KEY, bytes.as_slice())
        .map_err(|_| PublicationError::Database)?;
    check(deadline, cancel)?;
    let committed = tx.commit();
    drop(db);
    if committed.is_err() {
        return Err(PublicationError::Database);
    }
    Ok(result)
}
fn pending_mut<'a>(
    state: &'a mut State,
    expected: &PendingGeneration,
) -> Result<&'a mut PendingGeneration> {
    let pending = state.pending.as_mut().ok_or(PublicationError::Stale)?;
    if pending.generation.namespace != expected.generation.namespace
        || pending.generation.sequence != expected.generation.sequence
    {
        return Err(PublicationError::Stale);
    }
    Ok(pending)
}
fn check_wanted(tx: &WriteTransaction, namespace: Option<[u8; 16]>, revision: u64) -> Result<()> {
    let present = tx
        .list_tables()
        .map_err(|_| PublicationError::Database)?
        .any(|name| name.name() == "source_state");
    let (actual_namespace, actual_revision) = if present {
        let ns: [u8; 16] = {
            let table = tx
                .open_table(SOURCE_STATE)
                .map_err(|_| PublicationError::Database)?;
            let value = table
                .get(crate::current_source::NAMESPACE_KEY)
                .map_err(|_| PublicationError::Database)?
                .ok_or(PublicationError::Corrupt)?;
            value
                .value()
                .try_into()
                .map_err(|_| PublicationError::Corrupt)?
        };
        (
            Some(ns),
            crate::current_source::write_wanted_revision(tx).map_err(source_error)?,
        )
    } else {
        (None, 0)
    };
    if (actual_namespace, actual_revision) != (namespace, revision) {
        return Err(PublicationError::Stale);
    }
    Ok(())
}

/// Captures every retained source and selected outcome from one read-only snapshot.
///
/// # Errors
/// Refuses missing/stale preparation, partial schemas, corruption, bounds and I/O without writing.
pub fn capture_corpus(
    path: &Path,
    deadline: Instant,
    cancel: &AtomicBool,
) -> Result<CorpusCapture> {
    observe(path, deadline, cancel, |tx| {
        let names = tx
            .list_tables()
            .map_err(|_| PublicationError::Database)?
            .map(|name| name.name().to_owned())
            .collect();
        let multi = tx
            .list_multimap_tables()
            .map_err(|_| PublicationError::Database)?
            .next()
            .is_some();
        let has_sources = catalogue_names(names, multi).map_err(source_error)?;
        let state = read_state(tx)?;
        let mut sources = Vec::new();
        let (namespace, revision) = if has_sources {
            let ns = read_namespace(tx)
                .map_err(source_error)?
                .ok_or(PublicationError::Corrupt)?;
            let revision = crate::current_source::wanted_revision(tx).map_err(source_error)?;
            let rows = tx
                .open_table(SOURCE_ROWS)
                .map_err(|_| PublicationError::Database)?;
            let preparations = match tx.open_table(PREPARATIONS) {
                Ok(table) => Some(table),
                Err(TableError::TableDoesNotExist(_)) => None,
                Err(_) => return Err(PublicationError::Database),
            };
            let mut charged = 0_usize;
            for row in rows.iter().map_err(|_| PublicationError::Database)? {
                check(deadline, cancel)?;
                let (id, bytes) = row.map_err(|_| PublicationError::Database)?;
                charged = charged
                    .checked_add(bytes.value().len())
                    .ok_or(PublicationError::Limit)?;
                if sources.len() >= MAX_SOURCES || charged > MAX_CAPTURE_BYTES {
                    return Err(PublicationError::Limit);
                }
                let source = decode_record(bytes.value()).map_err(source_error)?;
                if id.value() != source.source_id.as_bytes() || source.source_namespace_id != ns {
                    return Err(PublicationError::Corrupt);
                }
                let value = preparations
                    .as_ref()
                    .ok_or(PublicationError::Unprepared)?
                    .get(id.value())
                    .map_err(|_| PublicationError::Database)?
                    .ok_or(PublicationError::Unprepared)?;
                charged = charged
                    .checked_add(value.value().len())
                    .ok_or(PublicationError::Limit)?;
                if charged > MAX_CAPTURE_BYTES {
                    return Err(PublicationError::Limit);
                }
                let preparation = crate::current_preparation::decode(value.value())
                    .map_err(|_| PublicationError::Corrupt)?;
                if preparation.source_namespace_id != ns
                    || preparation.source_id != source.source_id
                {
                    return Err(PublicationError::Corrupt);
                }
                if !preparation
                    .outcome
                    .as_ref()
                    .is_some_and(|outcome| outcome.matches_input(&source, preparation.settings))
                {
                    return Err(PublicationError::Unprepared);
                }
                sources
                    .try_reserve(1)
                    .map_err(|_| PublicationError::Allocation)?;
                sources.push(CapturedSource {
                    source,
                    preparation,
                });
            }
            (Some(*ns.as_bytes()), revision)
        } else {
            (None, 0)
        };
        Ok(CorpusCapture {
            sources,
            current: state.current.clone(),
            pending: state.pending.clone(),
            source_namespace: namespace,
            wanted_revision: revision,
            state,
        })
    })
}

/// Reserves a never-reused generation and durably saves its exact source input rows.
///
/// # Errors
/// Refuses changed corpus/current/pending guards, bounds, RNG failure or database failure.
pub fn start_build(
    path: &Path,
    capture: &CorpusCapture,
    deadline: Instant,
    cancel: &AtomicBool,
) -> Result<PendingGeneration> {
    let mut namespace = capture.state.namespace.unwrap_or([0; 16]);
    if capture.state.namespace.is_none() {
        getrandom::getrandom(&mut namespace).map_err(|_| PublicationError::Random)?;
    }
    let pending = edit(path, deadline, cancel, |tx, state| {
        check_wanted(tx, capture.source_namespace, capture.wanted_revision)?;
        if state.current_revision != capture.state.current_revision
            || state.current != capture.current
            || state.pending.is_some()
            || capture.pending.is_some()
            || state.namespace != capture.state.namespace
            || state.next_sequence != capture.state.next_sequence
        {
            return Err(PublicationError::Stale);
        }
        if tx
            .open_table(RETIRED)
            .map_err(|_| PublicationError::Database)?
            .len()
            .map_err(|_| PublicationError::Database)?
            >= MAX_RETIRED
        {
            return Err(PublicationError::Limit);
        }
        state.namespace = Some(namespace);
        state.next_sequence = state
            .next_sequence
            .checked_add(1)
            .ok_or(PublicationError::Limit)?;
        let pending = PendingGeneration {
            generation: Generation {
                namespace,
                sequence: state.next_sequence,
                wanted_revision: capture.wanted_revision,
                source_count: capture.sources.len() as u64,
                gap_count: capture
                    .sources
                    .iter()
                    .filter(|s| {
                        matches!(
                            s.preparation.outcome.as_ref().map(|o| o.result),
                            Some(PreparationOutcome::Gap { .. })
                        )
                    })
                    .count() as u64,
                point_count: 0,
                page_count: 0,
            },
            capture_complete: false,
            created: false,
            uploaded_pages: 0,
            in_flight: None,
            operation_revision: 0,
            completed_sources: 0,
            staged_bytes: 0,
            source_namespace: capture.source_namespace,
            expected_current_revision: state.current_revision,
            expected_current: state.current.clone(),
        };
        state.pending = Some(pending.clone());
        Ok(pending)
    })?;
    persist_captured_inputs(path, &pending, &capture.sources, deadline, cancel)?;
    Ok(pending)
}

fn persist_captured_inputs(
    path: &Path,
    pending: &PendingGeneration,
    sources: &[CapturedSource],
    deadline: Instant,
    cancel: &AtomicBool,
) -> Result<()> {
    for chunk in sources.chunks(32) {
        check(deadline, cancel)?;
        let encoded = chunk
            .iter()
            .map(|source| {
                Ok((
                    key(
                        pending.generation.sequence,
                        source.source.source_id.as_bytes(),
                    ),
                    codec::encode_source(&SourceEntry {
                        captured: source.clone(),
                        staged_units: 0,
                        complete: false,
                    })?,
                ))
            })
            .collect::<Result<Vec<_>>>()?;
        edit(path, deadline, cancel, |tx, state| {
            pending_mut(state, pending)?;
            let mut table = tx
                .open_table(SOURCES)
                .map_err(|_| PublicationError::Database)?;
            for (key, bytes) in encoded {
                if table
                    .get(key.as_slice())
                    .map_err(|_| PublicationError::Database)?
                    .is_some()
                {
                    return Err(PublicationError::Corrupt);
                }
                table
                    .insert(key.as_slice(), bytes.as_slice())
                    .map_err(|_| PublicationError::Database)?;
            }
            Ok(())
        })?;
    }
    Ok(())
}

/// Measures the complete durable page encoding, independently of SDK protobuf size.
///
/// # Errors
/// Rejects invalid units or a page exceeding its point/byte allowance.
pub fn upload_page_bytes(page: &UploadPage) -> Result<usize> {
    codec::page_size(page)
}

/// Persists actual upload text and matching evidence in one bounded transaction.
///
/// # Errors
/// Rejects changed/captured-complete builds, duplicate IDs, invalid source bindings and limits.
pub fn stage_page(
    path: &Path,
    pending: &PendingGeneration,
    page: &UploadPage,
    deadline: Instant,
    cancel: &AtomicBool,
) -> Result<()> {
    let bytes = codec::encode_page(page)?;
    edit(path, deadline, cancel, |tx, state| {
        let p = pending_mut(state, pending)?;
        if p.capture_complete || p.in_flight.is_some() {
            return Err(PublicationError::Stale);
        }
        p.staged_bytes = p
            .staged_bytes
            .checked_add(bytes.len() as u64)
            .filter(|n| *n <= MAX_STAGED_BYTES)
            .ok_or(PublicationError::Limit)?;
        let mut evidence = tx
            .open_table(EVIDENCE)
            .map_err(|_| PublicationError::Database)?;
        let mut sources = tx
            .open_table(SOURCES)
            .map_err(|_| PublicationError::Database)?;
        for point in &page.points {
            check(deadline, cancel)?;
            let index = p.generation.point_count;
            if point.id[..8] != p.generation.sequence.to_be_bytes()
                || point.id[8..] != index.to_be_bytes()
            {
                return Err(PublicationError::Corrupt);
            }
            let source_key = key(p.generation.sequence, point.evidence.source_id.as_bytes());
            let mut entry = {
                let value = sources
                    .get(source_key.as_slice())
                    .map_err(|_| PublicationError::Database)?
                    .ok_or(PublicationError::Corrupt)?;
                codec::decode_source(value.value())?
            };
            let c = &entry.captured;
            if entry.complete
                || point.evidence.source_namespace_id != c.source.source_namespace_id
                || point.evidence.input != c.source.object_ref
                || point.evidence.unit != entry.staged_units
                || !matches!(c.preparation.outcome.as_ref().map(|o| o.result), Some(PreparationOutcome::Ready { artifact }) if artifact == point.evidence.prepared)
            {
                return Err(PublicationError::Corrupt);
            }
            entry.staged_units = entry
                .staged_units
                .checked_add(1)
                .ok_or(PublicationError::Limit)?;
            let encoded_source = codec::encode_source(&entry)?;
            sources
                .insert(source_key.as_slice(), encoded_source.as_slice())
                .map_err(|_| PublicationError::Database)?;
            let evidence_key = key(p.generation.sequence, &point.id);
            if evidence
                .get(evidence_key.as_slice())
                .map_err(|_| PublicationError::Database)?
                .is_some()
            {
                return Err(PublicationError::Corrupt);
            }
            let encoded = codec::encode_evidence(&point.evidence);
            evidence
                .insert(evidence_key.as_slice(), encoded.as_slice())
                .map_err(|_| PublicationError::Database)?;
            p.generation.point_count = index
                .checked_add(1)
                .filter(|n| *n <= MAX_POINTS)
                .ok_or(PublicationError::Limit)?;
        }
        let page_key = key(
            p.generation.sequence,
            &p.generation.page_count.to_be_bytes(),
        );
        tx.open_table(PAGES)
            .map_err(|_| PublicationError::Database)?
            .insert(page_key.as_slice(), bytes.as_slice())
            .map_err(|_| PublicationError::Database)?;
        p.generation.page_count = p
            .generation
            .page_count
            .checked_add(1)
            .ok_or(PublicationError::Limit)?;
        Ok(())
    })
}

/// Completes one captured source after all its decoded units have been staged.
///
/// # Errors
/// Rejects duplicate completion, wrong unit counts or changed builds.
pub fn finish_source(
    path: &Path,
    pending: &PendingGeneration,
    id: SourceId,
    units: u64,
    deadline: Instant,
    cancel: &AtomicBool,
) -> Result<()> {
    edit(path, deadline, cancel, |tx, state| {
        let p = pending_mut(state, pending)?;
        if p.capture_complete {
            return Err(PublicationError::Stale);
        }
        let source_key = key(p.generation.sequence, id.as_bytes());
        let mut table = tx
            .open_table(SOURCES)
            .map_err(|_| PublicationError::Database)?;
        let mut entry = {
            let value = table
                .get(source_key.as_slice())
                .map_err(|_| PublicationError::Database)?
                .ok_or(PublicationError::Corrupt)?;
            codec::decode_source(value.value())?
        };
        if entry.complete || entry.staged_units != units {
            return Err(PublicationError::Corrupt);
        }
        entry.complete = true;
        let bytes = codec::encode_source(&entry)?;
        table
            .insert(source_key.as_slice(), bytes.as_slice())
            .map_err(|_| PublicationError::Database)?;
        p.completed_sources = p
            .completed_sources
            .checked_add(1)
            .ok_or(PublicationError::Limit)?;
        Ok(())
    })
}

/// Seals complete durable inputs before any backend mutation.
///
/// # Errors
/// Rejects incomplete sources, changed corpus/current guards or a changed pending build.
pub fn complete_capture(
    path: &Path,
    pending: &PendingGeneration,
    deadline: Instant,
    cancel: &AtomicBool,
) -> Result<PendingGeneration> {
    edit(path, deadline, cancel, |tx, state| {
        let p = pending_mut(state, pending)?;
        check_wanted(tx, p.source_namespace, p.generation.wanted_revision)?;
        if p.completed_sources != p.generation.source_count {
            return Err(PublicationError::Corrupt);
        }
        p.capture_complete = true;
        Ok(p.clone())
    })
}

/// Loads the exact persisted pending build without writes.
///
/// # Errors
/// Returns stale/corrupt/I/O errors, never a fabricated empty build.
pub fn load_pending(
    path: &Path,
    deadline: Instant,
    cancel: &AtomicBool,
) -> Result<Option<PendingGeneration>> {
    observe(path, deadline, cancel, |tx| Ok(read_state(tx)?.pending))
}

/// Reads one immutable saved upload page, including actual text.
///
/// # Errors
/// Refuses missing/corrupt pages and invalid point bindings.
pub fn read_upload_page(
    path: &Path,
    generation: &Generation,
    page: u64,
    deadline: Instant,
    cancel: &AtomicBool,
) -> Result<UploadPage> {
    observe(path, deadline, cancel, |tx| {
        let state = read_state(tx)?;
        if page >= generation.page_count
            || !(state.current.as_ref() == Some(generation)
                || state
                    .pending
                    .as_ref()
                    .is_some_and(|p| &p.generation == generation))
        {
            return Err(PublicationError::Stale);
        }
        let table = tx
            .open_table(PAGES)
            .map_err(|_| PublicationError::Corrupt)?;
        let page_key = key(generation.sequence, &page.to_be_bytes());
        let value = table
            .get(page_key.as_slice())
            .map_err(|_| PublicationError::Database)?
            .ok_or(PublicationError::Corrupt)?;
        codec::decode_page(value.value())
    })
}

/// Records the single mutation BEFORE dispatch; a crash retains its uncertainty.
///
/// # Errors
/// Refuses unsealed/stale builds, wrong progress or an already unresolved mutation.
pub fn record_dispatch(
    path: &Path,
    pending: &PendingGeneration,
    operation: PendingOperation,
    deadline: Instant,
    cancel: &AtomicBool,
) -> Result<u64> {
    edit(path, deadline, cancel, |tx, state| {
        let p = pending_mut(state, pending)?;
        check_wanted(tx, p.source_namespace, p.generation.wanted_revision)?;
        if p.in_flight.is_some() {
            return Err(PublicationError::Unresolved);
        }
        if p.operation_revision != pending.operation_revision
            || p.uploaded_pages != pending.uploaded_pages
            || p.created != pending.created
        {
            return Err(PublicationError::Stale);
        }
        if !p.capture_complete
            || !match operation {
                PendingOperation::Create => !p.created,
                PendingOperation::Insert(page) => {
                    p.created && page == p.uploaded_pages && page < p.generation.page_count
                }
            }
        {
            return Err(PublicationError::Stale);
        }
        p.operation_revision = p
            .operation_revision
            .checked_add(1)
            .ok_or(PublicationError::Limit)?;
        p.in_flight = Some(operation);
        Ok(p.operation_revision)
    })
}

/// Records terminal success or definite pre-dispatch refusal, never an ambiguous mutation.
///
/// # Errors
/// Rejects a changed build or operation. Database failure leaves the saved obligation conservative.
pub fn finish_dispatch(
    path: &Path,
    pending: &PendingGeneration,
    operation: PendingOperation,
    operation_revision: u64,
    applied: bool,
    deadline: Instant,
    cancel: &AtomicBool,
) -> Result<()> {
    edit(path, deadline, cancel, |_tx, state| {
        let p = pending_mut(state, pending)?;
        if p.in_flight != Some(operation) || p.operation_revision != operation_revision {
            return Err(PublicationError::Stale);
        }
        if applied {
            match operation {
                PendingOperation::Create => p.created = true,
                PendingOperation::Insert(_) => {
                    p.uploaded_pages = p
                        .uploaded_pages
                        .checked_add(1)
                        .ok_or(PublicationError::Limit)?;
                }
            }
        }
        p.in_flight = None;
        Ok(())
    })
}

/// Atomically switches only the current pointer, predecessor cleanup and pending header.
///
/// # Errors
/// Refuses stale corpus/current/expected-pending state, incomplete uploads or unresolved effects.
pub fn commit_generation(
    path: &Path,
    pending: &PendingGeneration,
    deadline: Instant,
    cancel: &AtomicBool,
) -> Result<Generation> {
    let committed = edit(path, deadline, cancel, |tx, state| {
        let p = pending_mut(state, pending)?.clone();
        if &p != pending {
            return Err(PublicationError::Stale);
        }
        check_wanted(tx, p.source_namespace, p.generation.wanted_revision)?;
        if p.in_flight.is_some() {
            return Err(PublicationError::Unresolved);
        }
        if !p.capture_complete
            || !p.created
            || p.uploaded_pages != p.generation.page_count
            || state.current_revision != p.expected_current_revision
            || state.current != p.expected_current
        {
            return Err(PublicationError::Stale);
        }
        if let Some(old) = &state.current {
            let retired = RetiredGeneration {
                generation: old.clone(),
                remote_owned: true,
            };
            let bytes = serde_json::to_vec(&retired).map_err(|_| PublicationError::Corrupt)?;
            tx.open_table(RETIRED)
                .map_err(|_| PublicationError::Database)?
                .insert(old.sequence.to_be_bytes().as_slice(), bytes.as_slice())
                .map_err(|_| PublicationError::Database)?;
        }
        state.current_revision = state
            .current_revision
            .checked_add(1)
            .ok_or(PublicationError::Limit)?;
        state.current = Some(p.generation.clone());
        state.pending = None;
        Ok(p.generation)
    });
    match committed {
        Err(PublicationError::Database) => observe(path, deadline, cancel, |tx| {
            let state = read_state(tx)?;
            if state.current.as_ref() == Some(&pending.generation)
                && state.pending.is_none()
                && pending.expected_current_revision.checked_add(1) == Some(state.current_revision)
            {
                Ok(pending.generation.clone())
            } else {
                Err(PublicationError::Database)
            }
        }),
        result => result,
    }
}

/// Abandons only a known unserved target; an unresolved mutation always blocks this operation.
///
/// # Errors
/// Preserves unknown effects and refuses changed pending identity.
pub fn abandon_build(
    path: &Path,
    pending: &PendingGeneration,
    deadline: Instant,
    cancel: &AtomicBool,
) -> Result<()> {
    edit(path, deadline, cancel, |tx, state| {
        let p = pending_mut(state, pending)?.clone();
        if &p != pending {
            return Err(PublicationError::Stale);
        }
        if p.in_flight.is_some() {
            return Err(PublicationError::Unresolved);
        }
        let retired = RetiredGeneration {
            generation: p.generation,
            remote_owned: p.created,
        };
        let bytes = serde_json::to_vec(&retired).map_err(|_| PublicationError::Corrupt)?;
        tx.open_table(RETIRED)
            .map_err(|_| PublicationError::Database)?
            .insert(
                retired.generation.sequence.to_be_bytes().as_slice(),
                bytes.as_slice(),
            )
            .map_err(|_| PublicationError::Database)?;
        state.pending = None;
        Ok(())
    })
}

/// One exact retired generation with ownership proven by terminal creation.
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct RetiredGeneration {
    /// Exact immutable generation to retire.
    pub generation: Generation,
    /// Whether this application received terminal creation success.
    pub remote_owned: bool,
}

/// Reads bounded whole-generation obligations before the caller opens any view.
///
/// # Errors
/// Rejects malformed/foreign bindings or a target still used by current/pending state.
pub fn retired_generations(
    path: &Path,
    deadline: Instant,
    cancel: &AtomicBool,
) -> Result<Vec<RetiredGeneration>> {
    observe(path, deadline, cancel, |tx| {
        let state = read_state(tx)?;
        if state.namespace.is_none() {
            return Ok(Vec::new());
        }
        let table = tx
            .open_table(RETIRED)
            .map_err(|_| PublicationError::Corrupt)?;
        if table.len().map_err(|_| PublicationError::Database)? > MAX_RETIRED {
            return Err(PublicationError::Limit);
        }
        let mut out = Vec::new();
        for row in table.iter().map_err(|_| PublicationError::Database)? {
            let (id, value) = row.map_err(|_| PublicationError::Database)?;
            if value.value().len() > 4096 {
                return Err(PublicationError::Corrupt);
            }
            let retired: RetiredGeneration =
                serde_json::from_slice(value.value()).map_err(|_| PublicationError::Corrupt)?;
            let g = &retired.generation;
            if id.value() != g.sequence.to_be_bytes()
                || state.namespace != Some(g.namespace)
                || g.sequence == 0
                || g.sequence > state.next_sequence
                || state
                    .current
                    .as_ref()
                    .is_some_and(|c| c.sequence == g.sequence)
                || state
                    .pending
                    .as_ref()
                    .is_some_and(|p| p.generation.sequence == g.sequence)
            {
                return Err(PublicationError::Corrupt);
            }
            out.push(retired);
        }
        Ok(out)
    })
}

/// Removes obsolete generation metadata in bounded chunks after terminal remote absence.
///
/// # Errors
/// Every chunk rechecks current/pending/cleanup identity; uncertainty must be retained by the caller.
pub fn finish_retirement(
    path: &Path,
    retired: &RetiredGeneration,
    deadline: Instant,
    cancel: &AtomicBool,
) -> Result<()> {
    let sequence = retired.generation.sequence;
    let prefix = sequence.to_be_bytes();
    for definition in [SOURCES, PAGES, EVIDENCE] {
        loop {
            let keys = observe(path, deadline, cancel, |tx| {
                let table = tx
                    .open_table(definition)
                    .map_err(|_| PublicationError::Corrupt)?;
                let mut keys = Vec::new();
                for row in table
                    .range(prefix.as_slice()..)
                    .map_err(|_| PublicationError::Database)?
                    .take(32)
                {
                    let (key, _) = row.map_err(|_| PublicationError::Database)?;
                    if !key.value().starts_with(&prefix) {
                        break;
                    }
                    keys.push(key.value().to_vec());
                }
                Ok(keys)
            })?;
            if keys.is_empty() {
                break;
            }
            edit(path, deadline, cancel, |tx, state| {
                check_retired(tx, state, retired)?;
                let mut table = tx
                    .open_table(definition)
                    .map_err(|_| PublicationError::Database)?;
                for key in keys {
                    table
                        .remove(key.as_slice())
                        .map_err(|_| PublicationError::Database)?;
                }
                Ok(())
            })?;
        }
    }
    edit(path, deadline, cancel, |tx, state| {
        check_retired(tx, state, retired)?;
        tx.open_table(RETIRED)
            .map_err(|_| PublicationError::Database)?
            .remove(prefix.as_slice())
            .map_err(|_| PublicationError::Database)?;
        Ok(())
    })
}
fn check_retired(tx: &WriteTransaction, state: &State, retired: &RetiredGeneration) -> Result<()> {
    let g = &retired.generation;
    if state.namespace != Some(g.namespace)
        || state
            .current
            .as_ref()
            .is_some_and(|c| c.sequence == g.sequence)
        || state
            .pending
            .as_ref()
            .is_some_and(|p| p.generation.sequence == g.sequence)
    {
        return Err(PublicationError::Stale);
    }
    let table = tx
        .open_table(RETIRED)
        .map_err(|_| PublicationError::Database)?;
    let value = table
        .get(g.sequence.to_be_bytes().as_slice())
        .map_err(|_| PublicationError::Database)?
        .ok_or(PublicationError::Stale)?;
    let actual: RetiredGeneration =
        serde_json::from_slice(value.value()).map_err(|_| PublicationError::Corrupt)?;
    if actual.generation != *g || actual.remote_owned != retired.remote_owned {
        return Err(PublicationError::Stale);
    }
    Ok(())
}

/// Matching committed pointer and owned native read-only evidence snapshot.
pub struct PublishedView {
    /// Exact immutable physical generation to query.
    pub generation: Generation,
    transaction: ReadTransaction,
    database: ReadOnlyDatabase,
}
impl PublishedView {
    /// Resolves a real returned point through this generation's pinned metadata.
    ///
    /// # Errors
    /// Missing/corrupt metadata is an error, never an invented hit or empty result.
    pub fn evidence(&self, id: [u8; 16]) -> Result<PointEvidence> {
        if id[..8] != self.generation.sequence.to_be_bytes() {
            return Err(PublicationError::Corrupt);
        }
        let table = self
            .transaction
            .open_table(EVIDENCE)
            .map_err(|_| PublicationError::Corrupt)?;
        let evidence_key = key(self.generation.sequence, &id);
        let value = table
            .get(evidence_key.as_slice())
            .map_err(|_| PublicationError::Database)?
            .ok_or(PublicationError::Corrupt)?;
        let evidence = codec::decode_evidence(value.value())?;
        let sources = self
            .transaction
            .open_table(SOURCES)
            .map_err(|_| PublicationError::Corrupt)?;
        let source_key = key(self.generation.sequence, evidence.source_id.as_bytes());
        let source = sources
            .get(source_key.as_slice())
            .map_err(|_| PublicationError::Database)?
            .ok_or(PublicationError::Corrupt)?;
        let entry = codec::decode_source(source.value())?;
        if !entry.complete
            || evidence.unit >= entry.staged_units
            || evidence.source_namespace_id != entry.captured.source.source_namespace_id
            || evidence.input != entry.captured.source.object_ref
            || !matches!(entry.captured.preparation.outcome.as_ref().map(|o| o.result),
                Some(PreparationOutcome::Ready { artifact }) if artifact == evidence.prepared)
        {
            return Err(PublicationError::Corrupt);
        }
        Ok(evidence)
    }

    /// Closes the snapshot before any subsequent writer is opened.
    ///
    /// # Errors
    /// Reports database close failure.
    pub fn close(self) -> Result<()> {
        drop(self.transaction);
        self.database
            .close()
            .map_err(|_| PublicationError::Database)
    }
}

/// Opens the current pointer and evidence tables from the same native read-only snapshot.
///
/// # Errors
/// Fresh unpublished roots, partial metadata, corruption and I/O are distinct failures.
pub fn open_published(
    path: &Path,
    deadline: Instant,
    cancel: &AtomicBool,
) -> Result<PublishedView> {
    check(deadline, cancel)?;
    let database = ReadOnlyDatabase::open(path).map_err(|_| PublicationError::Database)?;
    let transaction = database
        .begin_read()
        .map_err(|_| PublicationError::Database)?;
    let generation = read_state(&transaction)?
        .current
        .ok_or(PublicationError::NotPublished)?;
    drop(
        transaction
            .open_table(SOURCES)
            .map_err(|_| PublicationError::Corrupt)?,
    );
    drop(
        transaction
            .open_table(EVIDENCE)
            .map_err(|_| PublicationError::Corrupt)?,
    );
    drop(
        transaction
            .open_table(PAGES)
            .map_err(|_| PublicationError::Corrupt)?,
    );
    check(deadline, cancel)?;
    Ok(PublishedView {
        generation,
        transaction,
        database,
    })
}

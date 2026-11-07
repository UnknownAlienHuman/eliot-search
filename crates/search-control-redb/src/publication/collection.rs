//! Typed reachability from complete, building and partially retired generations.

use std::sync::atomic::AtomicBool;
use std::time::Instant;

use redb_plain::{ReadTransaction, ReadableTable, ReadableTableMetadata, TableError};
use search_contracts::SourceNamespaceId;

use super::{
    EVIDENCE, Generation, MAX_POINTS, MAX_RETIRED, MAX_SOURCES, MAX_STAGED_BYTES, PAGES,
    PendingGeneration, PendingOperation, PointEvidence, PreparationOutcome, RETIRED, SOURCES,
    STATE, STATE_KEY, codec, key, read_retirements, read_state,
};
use crate::object_marks::{MarkError as E, ObjectMarks, check};

enum Kind {
    Complete,
    Building { completed_sources: u64 },
    Retiring,
}

struct Root {
    generation: Generation,
    kind: Kind,
    source_rows: u64,
    complete_rows: u64,
    gaps: u64,
    page_rows: u64,
    points: u64,
    evidence_rows: u64,
    page_bytes: u64,
    last_page: Option<u64>,
    last_point: Option<u64>,
    last_evidence: Option<u64>,
    staged_units: u64,
    expected_staged_bytes: Option<u64>,
}

impl Root {
    const fn new(generation: Generation, kind: Kind) -> Self {
        Self {
            generation,
            kind,
            source_rows: 0,
            complete_rows: 0,
            gaps: 0,
            page_rows: 0,
            points: 0,
            evidence_rows: 0,
            page_bytes: 0,
            last_page: None,
            last_point: None,
            last_evidence: None,
            staged_units: 0,
            expected_staged_bytes: None,
        }
    }

    fn charge_page(&mut self, bytes: usize) -> Result<(), E> {
        self.page_bytes = self
            .page_bytes
            .checked_add(u64::try_from(bytes).map_err(|_| E::Limit)?)
            .filter(|bytes| *bytes <= MAX_STAGED_BYTES)
            .ok_or(E::Limit)?;
        Ok(())
    }
}

fn valid_generation(g: &Generation, namespace: Option<[u8; 16]>, sequence: u64) -> Result<(), E> {
    if namespace != Some(g.namespace)
        || g.sequence == 0
        || g.sequence > sequence
        || g.source_count > MAX_SOURCES as u64
        || g.gap_count > g.source_count
        || g.point_count > MAX_POINTS
        || g.page_count > MAX_POINTS
    {
        return Err(E::Corrupt);
    }
    Ok(())
}

const fn valid_pending(p: &PendingGeneration) -> Result<(), E> {
    if p.completed_sources > p.generation.source_count
        || p.staged_bytes > MAX_STAGED_BYTES
        || p.uploaded_pages > p.generation.page_count
        || (!p.created && p.uploaded_pages != 0)
        || (p.created && p.operation_revision == 0)
        || (p.capture_complete && p.completed_sources != p.generation.source_count)
        || (!p.capture_complete
            && (p.created
                || p.uploaded_pages != 0
                || p.in_flight.is_some()
                || p.operation_revision != 0))
    {
        return Err(E::Corrupt);
    }
    if let Some(operation) = p.in_flight
        && (!p.capture_complete
            || p.operation_revision == 0
            || !match operation {
                PendingOperation::Create => !p.created && p.uploaded_pages == 0,
                PendingOperation::Insert(page) => {
                    p.created && page == p.uploaded_pages && page < p.generation.page_count
                }
            })
    {
        return Err(E::Corrupt);
    }
    Ok(())
}

fn locate<'a>(roots: &'a mut [Root], row_key: &[u8], size: usize) -> Result<&'a mut Root, E> {
    if row_key.len() != size {
        return Err(E::Corrupt);
    }
    let sequence = u64::from_be_bytes(row_key[..8].try_into().map_err(|_| E::Corrupt)?);
    roots
        .iter_mut()
        .find(|root| root.generation.sequence == sequence)
        .ok_or(E::Corrupt)
}

fn point_binding(
    tx: &ReadTransaction,
    root: &Root,
    id: &[u8; 16],
    point: &PointEvidence,
    namespace: Option<SourceNamespaceId>,
) -> Result<u64, E> {
    let ordinal = u64::from_be_bytes(id[8..].try_into().map_err(|_| E::Corrupt)?);
    if id[..8] != root.generation.sequence.to_be_bytes()
        || ordinal >= root.generation.point_count
        || point.unit >= MAX_POINTS
        || Some(point.source_namespace_id) != namespace
    {
        return Err(E::Corrupt);
    }
    let sources = tx.open_table(SOURCES).map_err(|_| E::Database)?;
    let source_key = key(root.generation.sequence, point.source_id.as_bytes());
    match sources
        .get(source_key.as_slice())
        .map_err(|_| E::Database)?
    {
        Some(value) => {
            let entry = codec::decode_source(value.value()).map_err(|_| E::Corrupt)?;
            if point.unit >= entry.staged_units {
                return Err(E::Corrupt);
            }
            let source = entry.captured;
            if source.source.source_id != point.source_id
                || source.source.source_namespace_id != point.source_namespace_id
                || source.source.object_ref != point.input
                || !matches!(source.preparation.outcome.as_ref().map(|o| o.result),
                    Some(PreparationOutcome::Ready { artifact }) if artifact == point.prepared)
            {
                return Err(E::Corrupt);
            }
        }
        None if matches!(root.kind, Kind::Retiring) => {}
        None => return Err(E::Corrupt),
    }
    Ok(ordinal)
}

fn mark_sources(
    tx: &ReadTransaction,
    roots: &mut [Root],
    marks: &mut ObjectMarks,
    namespace: Option<SourceNamespaceId>,
    deadline: Instant,
    cancel: &AtomicBool,
) -> Result<(), E> {
    let table = tx.open_table(SOURCES).map_err(|_| E::Database)?;
    for row in table.iter().map_err(|_| E::Database)? {
        check(deadline, cancel)?;
        let (row_key, value) = row.map_err(|_| E::Database)?;
        let root = locate(roots, row_key.value(), 24)?;
        root.source_rows += 1;
        if root.source_rows > root.generation.source_count {
            return Err(E::Corrupt);
        }
        let entry = codec::decode_source(value.value()).map_err(|_| E::Corrupt)?;
        let source = &entry.captured.source;
        if row_key.value()[8..] != *source.source_id.as_bytes()
            || Some(source.source_namespace_id) != namespace
            || entry.staged_units > MAX_POINTS
        {
            return Err(E::Corrupt);
        }
        root.complete_rows += u64::from(entry.complete);
        root.staged_units = root
            .staged_units
            .checked_add(entry.staged_units)
            .filter(|units| *units <= root.generation.point_count)
            .ok_or(E::Corrupt)?;
        marks.charge(value.value().len())?;
        marks.insert(source.object_ref, deadline, cancel)?;
        let outcome = entry.captured.preparation.outcome.ok_or(E::Corrupt)?;
        marks.insert(outcome.input, deadline, cancel)?;
        match outcome.result {
            PreparationOutcome::Ready { artifact } => marks.prepared(
                artifact,
                crate::object_marks::PreparedExpectation {
                    source_namespace_id: *source.source_namespace_id.as_bytes(),
                    source_id: *source.source_id.as_bytes(),
                    input: outcome.input,
                    revision_settings: Some((outcome.registration_revision, outcome.settings)),
                },
                deadline,
                cancel,
            )?,
            PreparationOutcome::Gap { .. } => {
                root.gaps += 1;
                if entry.staged_units != 0 {
                    return Err(E::Corrupt);
                }
            }
        }
        if root.gaps > root.generation.gap_count {
            return Err(E::Corrupt);
        }
    }
    Ok(())
}

fn mark_pages(
    tx: &ReadTransaction,
    roots: &mut [Root],
    marks: &mut ObjectMarks,
    namespace: Option<SourceNamespaceId>,
    deadline: Instant,
    cancel: &AtomicBool,
) -> Result<(), E> {
    let table = tx.open_table(PAGES).map_err(|_| E::Database)?;
    let evidence = tx.open_table(EVIDENCE).map_err(|_| E::Database)?;
    for row in table.iter().map_err(|_| E::Database)? {
        check(deadline, cancel)?;
        let (row_key, value) = row.map_err(|_| E::Database)?;
        let root = locate(roots, row_key.value(), 16)?;
        root.charge_page(value.value().len())?;
        let number = u64::from_be_bytes(row_key.value()[8..].try_into().map_err(|_| E::Corrupt)?);
        if number >= root.generation.page_count
            || root
                .last_page
                .is_some_and(|last| last.checked_add(1) != Some(number))
            || (!matches!(root.kind, Kind::Retiring) && number != root.page_rows)
        {
            return Err(E::Corrupt);
        }
        root.last_page = Some(number);
        root.page_rows += 1;
        let page = codec::decode_page(value.value()).map_err(|_| E::Corrupt)?;
        marks.charge(value.value().len())?;
        for point in page.points {
            let ordinal = point_binding(tx, root, &point.id, &point.evidence, namespace)?;
            if root
                .last_point
                .is_some_and(|last| last.checked_add(1) != Some(ordinal))
                || (!matches!(root.kind, Kind::Retiring) && ordinal != root.points)
            {
                return Err(E::Corrupt);
            }
            root.last_point = Some(ordinal);
            root.points += 1;
            let evidence_key = key(root.generation.sequence, &point.id);
            let stored = evidence
                .get(evidence_key.as_slice())
                .map_err(|_| E::Database)?
                .ok_or(E::Corrupt)?;
            if codec::decode_evidence(stored.value()).map_err(|_| E::Corrupt)? != point.evidence {
                return Err(E::Corrupt);
            }
            marks.insert(point.evidence.input, deadline, cancel)?;
            marks.prepared(
                point.evidence.prepared,
                crate::object_marks::PreparedExpectation {
                    source_namespace_id: *point.evidence.source_namespace_id.as_bytes(),
                    source_id: *point.evidence.source_id.as_bytes(),
                    input: point.evidence.input,
                    revision_settings: None,
                },
                deadline,
                cancel,
            )?;
        }
    }
    Ok(())
}

fn mark_evidence(
    tx: &ReadTransaction,
    roots: &mut [Root],
    marks: &mut ObjectMarks,
    namespace: Option<SourceNamespaceId>,
    deadline: Instant,
    cancel: &AtomicBool,
) -> Result<(), E> {
    let table = tx.open_table(EVIDENCE).map_err(|_| E::Database)?;
    for row in table.iter().map_err(|_| E::Database)? {
        check(deadline, cancel)?;
        let (row_key, value) = row.map_err(|_| E::Database)?;
        let root = locate(roots, row_key.value(), 24)?;
        let id: [u8; 16] = row_key.value()[8..].try_into().map_err(|_| E::Corrupt)?;
        let evidence = codec::decode_evidence(value.value()).map_err(|_| E::Corrupt)?;
        let ordinal = point_binding(tx, root, &id, &evidence, namespace)?;
        if root
            .last_evidence
            .is_some_and(|last| last.checked_add(1) != Some(ordinal))
            || (!matches!(root.kind, Kind::Retiring) && ordinal != root.evidence_rows)
        {
            return Err(E::Corrupt);
        }
        root.last_evidence = Some(ordinal);
        root.evidence_rows += 1;
        if root.evidence_rows > root.generation.point_count {
            return Err(E::Corrupt);
        }
        marks.charge(value.value().len())?;
        marks.insert(evidence.input, deadline, cancel)?;
        marks.prepared(
            evidence.prepared,
            crate::object_marks::PreparedExpectation {
                source_namespace_id: *evidence.source_namespace_id.as_bytes(),
                source_id: *evidence.source_id.as_bytes(),
                input: evidence.input,
                revision_settings: None,
            },
            deadline,
            cancel,
        )?;
    }
    Ok(())
}

pub fn mark_objects(
    tx: &ReadTransaction,
    marks: &mut ObjectMarks,
    deadline: Instant,
    cancel: &AtomicBool,
) -> Result<(), E> {
    let table = match tx.open_table(STATE) {
        Ok(table) => table,
        Err(TableError::TableDoesNotExist(_)) => return Ok(()),
        Err(_) => return Err(E::Database),
    };
    if table.len().map_err(|_| E::Database)? != 1
        || table.get(STATE_KEY).map_err(|_| E::Database)?.is_none()
    {
        return Err(E::Corrupt);
    }
    drop(table);
    let state = read_state(tx).map_err(|_| E::Corrupt)?;
    let namespace = match tx.open_table(crate::current_source::SOURCE_STATE) {
        Ok(_) => crate::current_source::read_namespace(tx).map_err(|_| E::Corrupt)?,
        Err(TableError::TableDoesNotExist(_)) => None,
        Err(_) => return Err(E::Database),
    };
    let mut roots = Vec::new();
    roots
        .try_reserve_exact(usize::try_from(MAX_RETIRED).map_err(|_| E::Limit)? + 2)
        .map_err(|_| E::Limit)?;
    if let Some(g) = &state.current {
        roots.push(Root::new(g.clone(), Kind::Complete));
    }
    if let Some(pending) = &state.pending {
        valid_pending(pending)?;
        if pending.expected_current != state.current
            || pending.expected_current_revision != state.current_revision
        {
            return Err(E::Corrupt);
        }
        let kind = if pending.capture_complete {
            Kind::Complete
        } else {
            Kind::Building {
                completed_sources: pending.completed_sources,
            }
        };
        let mut root = Root::new(pending.generation.clone(), kind);
        root.expected_staged_bytes = Some(pending.staged_bytes);
        roots.push(root);
    }
    for retired in read_retirements(tx).map_err(|_| E::Corrupt)? {
        let expected_staged_bytes = retired
            .unresolved_build
            .as_ref()
            .map(|pending| pending.staged_bytes);
        let kind = if let Some(pending) = retired.unresolved_build {
            valid_pending(&pending)?;
            Kind::Complete
        } else {
            Kind::Retiring
        };
        let mut root = Root::new(retired.generation, kind);
        root.expected_staged_bytes = expected_staged_bytes;
        roots.push(root);
    }
    // Even an empty retirement table must have the supported declared type.
    drop(tx.open_table(RETIRED).map_err(|_| E::Corrupt)?);
    for root in &roots {
        valid_generation(&root.generation, state.namespace, state.next_sequence)?;
    }
    mark_sources(tx, &mut roots, marks, namespace, deadline, cancel)?;
    mark_pages(tx, &mut roots, marks, namespace, deadline, cancel)?;
    mark_evidence(tx, &mut roots, marks, namespace, deadline, cancel)?;
    for root in roots {
        check(deadline, cancel)?;
        if root
            .expected_staged_bytes
            .is_some_and(|expected| expected != root.page_bytes)
        {
            return Err(E::Corrupt);
        }
        match root.kind {
            Kind::Complete => super::verify::generation(tx, &root.generation, deadline, cancel)
                .map_err(|_| E::Corrupt)?,
            Kind::Building { completed_sources } => {
                if root.complete_rows != completed_sources
                    || root.staged_units != root.generation.point_count
                    || root.page_rows != root.generation.page_count
                    || root.points != root.generation.point_count
                    || root.evidence_rows != root.generation.point_count
                    || (root.source_rows == root.generation.source_count
                        && root.gaps != root.generation.gap_count)
                {
                    return Err(E::Corrupt);
                }
            }
            Kind::Retiring => {}
        }
    }
    Ok(())
}

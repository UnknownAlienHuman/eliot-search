//! Complete bounded graph validation in one read-only generation snapshot.

use std::sync::atomic::AtomicBool;
use std::time::Instant;

use redb_plain::ReadTransaction;
use search_contracts::{SourceId, SourceNamespaceId};
use search_source_registry::ObjectRef;

use super::{
    EVIDENCE, Generation, MAX_POINTS, MAX_SOURCES, MAX_STAGED_BYTES, PAGES, PointEvidence,
    PublicationError as E, Result, SOURCES, SourceEntry, UploadPoint, check, codec, key,
};
use crate::current_preparation::PreparationOutcome;

struct SourceProgress {
    id: SourceId,
    namespace: SourceNamespaceId,
    input: ObjectRef,
    prepared: Option<ObjectRef>,
    units: u64,
    seen: u64,
    byte_end: u64,
}
impl SourceProgress {
    fn from_entry(entry: SourceEntry) -> Result<Self> {
        if !entry.complete || entry.staged_units > MAX_POINTS {
            return Err(E::Corrupt);
        }
        let source = entry.captured.source;
        let prepared = match entry.captured.preparation.outcome.ok_or(E::Corrupt)?.result {
            PreparationOutcome::Ready { artifact } => {
                if (source.object_ref.byte_length == 0) != (entry.staged_units == 0) {
                    return Err(E::Corrupt);
                }
                Some(artifact)
            }
            PreparationOutcome::Gap { .. } => {
                if entry.staged_units != 0 {
                    return Err(E::Corrupt);
                }
                None
            }
        };
        Ok(Self {
            id: source.source_id,
            namespace: source.source_namespace_id,
            input: source.object_ref,
            prepared,
            units: entry.staged_units,
            seen: 0,
            byte_end: 0,
        })
    }

    fn accept(&mut self, evidence: &PointEvidence) -> Result<()> {
        if evidence.source_namespace_id != self.namespace
            || evidence.source_id != self.id
            || evidence.input != self.input
            || Some(evidence.prepared) != self.prepared
            || evidence.unit != self.seen
            || evidence.start != self.byte_end
            || evidence.end > self.input.byte_length
            || self.seen >= self.units
        {
            return Err(E::Corrupt);
        }
        self.seen = self.seen.checked_add(1).ok_or(E::Limit)?;
        self.byte_end = evidence.end;
        Ok(())
    }

    const fn complete(&self) -> bool {
        self.seen == self.units
            && (self.prepared.is_none() || self.byte_end == self.input.byte_length)
    }
}

fn sources(
    tx: &ReadTransaction,
    generation: &Generation,
    deadline: Instant,
    cancel: &AtomicBool,
) -> Result<Vec<SourceProgress>> {
    let count = usize::try_from(generation.source_count).map_err(|_| E::Corrupt)?;
    if count > MAX_SOURCES {
        return Err(E::Corrupt);
    }
    let table = tx.open_table(SOURCES).map_err(|_| E::Corrupt)?;
    let prefix = generation.sequence.to_be_bytes();
    let mut progress = Vec::new();
    progress
        .try_reserve_exact(count)
        .map_err(|_| E::Allocation)?;
    let mut gaps = 0_u64;
    let mut units = 0_u64;
    let mut namespace = None;
    for row in table.range(prefix.as_slice()..).map_err(|_| E::Database)? {
        check(deadline, cancel)?;
        let (row_key, value) = row.map_err(|_| E::Database)?;
        if !row_key.value().starts_with(&prefix) {
            break;
        }
        if row_key.value().len() != 24 || progress.len() >= count {
            return Err(E::Corrupt);
        }
        let source = SourceProgress::from_entry(codec::decode_source(value.value())?)?;
        if row_key.value()[8..] != *source.id.as_bytes()
            || namespace.is_some_and(|expected| expected != source.namespace)
        {
            return Err(E::Corrupt);
        }
        namespace = Some(source.namespace);
        gaps += u64::from(source.prepared.is_none());
        units = units
            .checked_add(source.units)
            .filter(|n| *n <= MAX_POINTS)
            .ok_or(E::Corrupt)?;
        progress.push(source);
    }
    if progress.len() != count || gaps != generation.gap_count || units != generation.point_count {
        return Err(E::Corrupt);
    }
    Ok(progress)
}

fn point(
    generation: &Generation,
    point: &UploadPoint,
    ordinal: u64,
    saved_evidence: &[u8],
    progress: &mut [SourceProgress],
) -> Result<()> {
    if point.id[..8] != generation.sequence.to_be_bytes() || point.id[8..] != ordinal.to_be_bytes()
    {
        return Err(E::Corrupt);
    }
    if codec::decode_evidence(saved_evidence)? != point.evidence {
        return Err(E::Corrupt);
    }
    let source = progress
        .binary_search_by_key(&point.evidence.source_id, |source| source.id)
        .map_err(|_| E::Corrupt)?;
    progress[source].accept(&point.evidence)
}

fn evidence_count(
    tx: &ReadTransaction,
    generation: &Generation,
    deadline: Instant,
    cancel: &AtomicBool,
) -> Result<()> {
    let table = tx.open_table(EVIDENCE).map_err(|_| E::Corrupt)?;
    let prefix = generation.sequence.to_be_bytes();
    let mut count = 0_u64;
    for row in table.range(prefix.as_slice()..).map_err(|_| E::Database)? {
        check(deadline, cancel)?;
        let (row_key, _) = row.map_err(|_| E::Database)?;
        let bytes = row_key.value();
        if !bytes.starts_with(&prefix) {
            break;
        }
        if bytes.len() != 24 || bytes[8..16] != prefix {
            return Err(E::Corrupt);
        }
        let ordinal = u64::from_be_bytes(bytes[16..24].try_into().map_err(|_| E::Corrupt)?);
        if ordinal >= generation.point_count {
            return Err(E::Corrupt);
        }
        count = count
            .checked_add(1)
            .filter(|n| *n <= generation.point_count)
            .ok_or(E::Corrupt)?;
    }
    if count != generation.point_count {
        return Err(E::Corrupt);
    }
    Ok(())
}

pub(super) fn generation(
    tx: &ReadTransaction,
    generation: &Generation,
    deadline: Instant,
    cancel: &AtomicBool,
) -> Result<()> {
    let mut progress = sources(tx, generation, deadline, cancel)?;
    let pages = tx.open_table(PAGES).map_err(|_| E::Corrupt)?;
    let evidence = tx.open_table(EVIDENCE).map_err(|_| E::Corrupt)?;
    let prefix = generation.sequence.to_be_bytes();
    let mut page_count = 0_u64;
    let mut point_count = 0_u64;
    let mut charged = 0_u64;
    for row in pages.range(prefix.as_slice()..).map_err(|_| E::Database)? {
        check(deadline, cancel)?;
        let (row_key, value) = row.map_err(|_| E::Database)?;
        if !row_key.value().starts_with(&prefix) {
            break;
        }
        if page_count >= generation.page_count
            || row_key.value() != key(generation.sequence, &page_count.to_be_bytes()).as_slice()
        {
            return Err(E::Corrupt);
        }
        charged = charged
            .checked_add(u64::try_from(value.value().len()).map_err(|_| E::Corrupt)?)
            .filter(|n| *n <= MAX_STAGED_BYTES)
            .ok_or(E::Corrupt)?;
        let page = codec::decode_page(value.value())?;
        for saved in &page.points {
            check(deadline, cancel)?;
            if point_count >= generation.point_count {
                return Err(E::Corrupt);
            }
            let evidence_key = key(generation.sequence, &saved.id);
            let value = evidence
                .get(evidence_key.as_slice())
                .map_err(|_| E::Database)?
                .ok_or(E::Corrupt)?;
            point(generation, saved, point_count, value.value(), &mut progress)?;
            point_count = point_count.checked_add(1).ok_or(E::Corrupt)?;
        }
        page_count = page_count.checked_add(1).ok_or(E::Corrupt)?;
    }
    if page_count != generation.page_count
        || point_count != generation.point_count
        || progress.iter().any(|source| !source.complete())
    {
        return Err(E::Corrupt);
    }
    evidence_count(tx, generation, deadline, cancel)
}

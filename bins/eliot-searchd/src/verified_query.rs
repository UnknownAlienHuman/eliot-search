//! Read-only nominations resolved against one pinned published source binding.

use std::sync::atomic::{AtomicBool, Ordering};
use std::time::Instant;

use search_control_redb::publication::{
    PointEvidence, PublicationError, PublishedView, ResolvedPoint,
};
use search_provider_protocol::{
    EvidenceMetadata, FailureCode, ObjectKindDto, ObjectRefDto, SearchGap, SearchHit, SearchItem,
    SearchRequest,
};
use search_qdrant_bridge::{Backend, BackendError, BackendLimits, Hit};
use search_revision_store::{ObjectStoreError, read_object};
use search_runtime_owner::{PlainRootError, PlainRootGuard};
use search_source_registry::{MAX_SOURCE_OBJECT_BYTES, ObjectKind, ObjectRef};
use search_unitizer::{PreparationError, PreparationLimits, PreparedUtf8, decode_prepared_utf8};

#[derive(Clone, Copy)]
pub enum QueryFailure {
    Cancelled,
    Deadline,
    Code(&'static str),
}
impl QueryFailure {
    pub(super) const fn publication(error: PublicationError) -> Self {
        match error {
            PublicationError::Cancelled => Self::Cancelled,
            PublicationError::Deadline => Self::Deadline,
            other => Self::Code(other.code()),
        }
    }
    pub(super) const fn root(error: PlainRootError) -> Self {
        match error {
            PlainRootError::Cancelled => Self::Cancelled,
            PlainRootError::Deadline => Self::Deadline,
            PlainRootError::Busy => Self::Code("ROOT_BUSY"),
            PlainRootError::Absent => Self::Code("ROOT_ABSENT"),
            PlainRootError::InvalidPath => Self::Code("ROOT_INVALID_PATH"),
            PlainRootError::Unsupported => Self::Code("ROOT_UNSUPPORTED"),
            PlainRootError::Corrupt => Self::Code("ROOT_CORRUPT"),
            PlainRootError::Archive => Self::Code("ROOT_ARCHIVE"),
            PlainRootError::IncompleteImport => Self::Code("ROOT_INCOMPLETE_IMPORT"),
            PlainRootError::WrongState => Self::Code("ROOT_WRONG_STATE"),
            PlainRootError::AlreadyExists => Self::Code("ROOT_ALREADY_EXISTS"),
            PlainRootError::Io => Self::Code("ROOT_IO"),
            PlainRootError::TooLarge => Self::Code("ROOT_TOO_LARGE"),
            PlainRootError::Unavailable => Self::Code("ROOT_UNAVAILABLE"),
            PlainRootError::ProbeLimit => Self::Code("ROOT_PROBE_LIMIT"),
        }
    }
    const fn backend(error: BackendError) -> Self {
        match error {
            BackendError::Cancelled => Self::Cancelled,
            BackendError::Deadline => Self::Deadline,
            other => Self::Code(other.code()),
        }
    }
    const fn object(error: ObjectStoreError) -> Self {
        match error {
            ObjectStoreError::Cancelled => Self::Cancelled,
            ObjectStoreError::Deadline => Self::Deadline,
            other => Self::Code(other.code()),
        }
    }
    const fn prepared(error: PreparationError) -> Self {
        match error {
            PreparationError::Cancelled => Self::Cancelled,
            PreparationError::Deadline => Self::Deadline,
            other => Self::Code(other.code()),
        }
    }
}
type Result<T> = std::result::Result<T, QueryFailure>;

struct RankedPoint {
    rank: u8,
    hit: Hit,
    resolved: ResolvedPoint,
}

fn check(deadline: Instant, cancel: &AtomicBool) -> Result<()> {
    if cancel.load(Ordering::Relaxed) {
        Err(QueryFailure::Cancelled)
    } else if Instant::now() >= deadline {
        Err(QueryFailure::Deadline)
    } else {
        Ok(())
    }
}
const fn limits() -> PreparationLimits {
    PreparationLimits {
        max_input_bytes: 8 * 1024 * 1024,
        max_output_bytes: 8 * 1024 * 1024,
        max_lines: 1_000_000,
        max_units: 1_000_000,
        max_encoded_bytes: 64 * 1024 * 1024,
    }
}
fn hex(bytes: &[u8]) -> String {
    const DIGITS: &[u8; 16] = b"0123456789abcdef";
    let mut text = String::with_capacity(bytes.len() * 2);
    for byte in bytes {
        text.push(char::from(DIGITS[usize::from(byte >> 4)]));
        text.push(char::from(DIGITS[usize::from(byte & 15)]));
    }
    text
}
fn object_metadata(reference: ObjectRef) -> ObjectRefDto {
    ObjectRefDto {
        version: reference.format_version,
        kind: match reference.kind {
            ObjectKind::Source => ObjectKindDto::Source,
            ObjectKind::Prepared => ObjectKindDto::Prepared,
        },
        digest: hex(reference.digest.as_bytes()),
        bytes: reference.byte_length,
    }
}
fn metadata(generation: u64, point: &RankedPoint) -> EvidenceMetadata {
    let e = &point.resolved.evidence;
    EvidenceMetadata {
        generation,
        point_id: hex(&point.hit.id),
        source_namespace_id: e.source_namespace_id.to_string(),
        source_id: e.source_id.to_string(),
        input: object_metadata(e.input),
        prepared: object_metadata(e.prepared),
        unit: e.unit,
        start: e.start,
        end: e.end,
        line_start: e.line_start,
        line_end: e.line_end,
        starts_at_line_boundary: e.starts_at_line_boundary,
        ends_at_line_boundary: e.ends_at_line_boundary,
    }
}
fn gap(generation: u64, point: &RankedPoint, code: &'static str) -> Result<SearchItem> {
    Ok(SearchItem::Gap(SearchGap {
        rank: point.rank,
        score: point.hit.score,
        evidence: metadata(generation, point),
        code: FailureCode::new(code).map_err(|_| QueryFailure::Code("QUERY_INTERNAL_ERROR"))?,
    }))
}
fn same_group(left: &RankedPoint, right: &RankedPoint) -> bool {
    left.resolved.captured == right.resolved.captured
        && left.resolved.evidence.prepared == right.resolved.evidence.prepared
}
fn verify_unit<'a>(
    source: &[u8],
    prepared: &'a PreparedUtf8<'_>,
    e: &PointEvidence,
) -> Result<&'a str> {
    let ordinal =
        usize::try_from(e.unit).map_err(|_| QueryFailure::Code("EVIDENCE_RANGE_INVALID"))?;
    let unit = prepared
        .units()
        .get(ordinal)
        .ok_or(QueryFailure::Code("EVIDENCE_RANGE_INVALID"))?;
    if u64::try_from(unit.source_start).ok() != Some(e.start)
        || u64::try_from(unit.source_end).ok() != Some(e.end)
        || unit.logical_line_start != e.line_start
        || unit.logical_line_end != e.line_end
        || unit.starts_at_line_boundary != e.starts_at_line_boundary
        || unit.ends_at_line_boundary != e.ends_at_line_boundary
    {
        return Err(QueryFailure::Code("EVIDENCE_RANGE_MISMATCH"));
    }
    let text = prepared
        .text()
        .get(unit.source_start..unit.source_end)
        .ok_or(QueryFailure::Code("EVIDENCE_RANGE_INVALID"))?;
    let original = source
        .get(unit.source_start..unit.source_end)
        .ok_or(QueryFailure::Code("EVIDENCE_RANGE_INVALID"))?;
    if original != text.as_bytes() || text.len() > 65_536 {
        return Err(QueryFailure::Code("EVIDENCE_TEXT_MISMATCH"));
    }
    Ok(text)
}

fn fill_group(
    root: &PlainRootGuard,
    generation: u64,
    first: usize,
    points: &[RankedPoint],
    slots: &mut [Option<SearchItem>],
    deadline: Instant,
    cancel: &AtomicBool,
) -> Result<()> {
    let binding = &points[first].resolved;
    let indices: Vec<_> = points
        .iter()
        .enumerate()
        .filter(|(_, p)| same_group(&points[first], p))
        .map(|(i, _)| i)
        .collect();
    let source = match read_object(
        root,
        binding.evidence.input,
        MAX_SOURCE_OBJECT_BYTES,
        deadline,
        cancel,
    ) {
        Ok(bytes) => bytes,
        Err(ObjectStoreError::Missing) => {
            for index in indices {
                slots[index] = Some(gap(generation, &points[index], "SOURCE_OBJECT_MISSING")?);
            }
            return Ok(());
        }
        Err(error) => return Err(QueryFailure::object(error)),
    };
    let artifact = match read_object(
        root,
        binding.evidence.prepared,
        64 * 1024 * 1024,
        deadline,
        cancel,
    ) {
        Ok(bytes) => bytes,
        Err(ObjectStoreError::Missing) => {
            for index in indices {
                slots[index] = Some(gap(generation, &points[index], "PREPARED_OBJECT_MISSING")?);
            }
            return Ok(());
        }
        Err(error) => return Err(QueryFailure::object(error)),
    };
    let prepared = decode_prepared_utf8(
        &artifact,
        &binding.captured.source,
        &binding.captured.preparation.settings,
        limits(),
        deadline,
        cancel,
    )
    .map_err(QueryFailure::prepared)?;
    if prepared.text().as_bytes() != source.as_slice() {
        return Err(QueryFailure::Code("EVIDENCE_TEXT_MISMATCH"));
    }
    for index in indices {
        check(deadline, cancel)?;
        let point = &points[index];
        let text = verify_unit(&source, &prepared, &point.resolved.evidence)?;
        let mut unit_text = String::new();
        unit_text
            .try_reserve_exact(text.len())
            .map_err(|_| QueryFailure::Code("QUERY_ALLOCATION_FAILED"))?;
        unit_text.push_str(text);
        slots[index] = Some(SearchItem::Hit(SearchHit {
            rank: point.rank,
            score: point.hit.score,
            evidence: metadata(generation, point),
            unit_text,
        }));
    }
    Ok(())
}

pub fn execute(
    root: &PlainRootGuard,
    view: &PublishedView,
    endpoint: &str,
    request: &SearchRequest,
    deadline: Instant,
    cancel: &AtomicBool,
) -> Result<Vec<SearchItem>> {
    check(deadline, cancel)?;
    request
        .validate()
        .map_err(|e| QueryFailure::Code(e.code()))?;
    let key = crate::published_corpus::api_key().map_err(QueryFailure::Code)?;
    let backend = Backend::connect(
        endpoint,
        key.as_deref(),
        BackendLimits::default(),
        deadline,
        cancel,
    )
    .map_err(QueryFailure::backend)?;
    let collection = view.generation.collection();
    backend
        .verify_schema(&collection, deadline, cancel)
        .map_err(QueryFailure::backend)?;
    let hits = backend
        .query_text(
            &collection,
            &request.query,
            usize::from(request.limit),
            deadline,
            cancel,
        )
        .map_err(QueryFailure::backend)?;
    let mut points = Vec::new();
    points
        .try_reserve_exact(hits.len())
        .map_err(|_| QueryFailure::Code("QUERY_ALLOCATION_FAILED"))?;
    for (index, hit) in hits.into_iter().enumerate() {
        check(deadline, cancel)?;
        let resolved = view
            .resolve_point(hit.id)
            .map_err(QueryFailure::publication)?;
        if hit.source_id != resolved.evidence.source_id || !hit.score.is_finite() {
            return Err(QueryFailure::Code("QUERY_POINT_ROUTE_MISMATCH"));
        }
        points.push(RankedPoint {
            rank: u8::try_from(index + 1).map_err(|_| QueryFailure::Code("QUERY_LIMIT"))?,
            hit,
            resolved,
        });
    }
    let mut slots: Vec<Option<SearchItem>> = (0..points.len()).map(|_| None).collect();
    let mut items = Vec::new();
    items
        .try_reserve_exact(points.len())
        .map_err(|_| QueryFailure::Code("QUERY_ALLOCATION_FAILED"))?;
    for index in 0..points.len() {
        if slots[index].is_none() {
            fill_group(
                root,
                view.generation.sequence,
                index,
                &points,
                &mut slots,
                deadline,
                cancel,
            )?;
        }
        items.push(
            slots[index]
                .take()
                .ok_or(QueryFailure::Code("QUERY_INTERNAL_ERROR"))?,
        );
    }
    check(deadline, cancel)?;
    Ok(items)
}

//! Literal Exact over verified saved UTF-8 with an independently checked identity map.

use std::sync::atomic::AtomicBool;
use std::time::Instant;

use search_control_redb::current_preparation::PreparationOutcome;
use search_control_redb::publication::{CapturedSource, PublishedView};
use search_provider_protocol::{
    DecodedUtf8Gap, DecodedUtf8Item, DecodedUtf8Match, DecodedUtf8Request, DecodedUtf8Result,
    FailureCode, StopReason,
};
use search_revision_store::{ObjectStoreError, read_object};
use search_runtime_owner::PlainRootGuard;
use search_source_registry::{MAX_SOURCE_OBJECT_BYTES, ObjectRef};
use search_unitizer::{PreparationError, PreparationLimits, PreparedUtf8, decode_prepared_utf8};

use crate::exact_literal::{Literal, check, object_ref, select_sources, source_ref};
use crate::verified_query::QueryFailure;

type Result<T> = std::result::Result<T, QueryFailure>;

pub struct RepresentationGap {
    pub prepared: Option<ObjectRef>,
    pub code: &'static str,
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

const fn object_failure(error: ObjectStoreError) -> QueryFailure {
    match error {
        ObjectStoreError::Cancelled => QueryFailure::Cancelled,
        ObjectStoreError::Deadline => QueryFailure::Deadline,
        other => QueryFailure::Code(other.code()),
    }
}

// The callback borrows one verified artifact. No self-referential owner, second
// decoding pass or materialization is needed; source bytes die before scanning.
pub fn visit_representation(
    root: &PlainRootGuard,
    captured: &CapturedSource,
    deadline: Instant,
    cancel: &AtomicBool,
    visit: impl FnOnce(ObjectRef, &PreparedUtf8<'_>) -> Result<()>,
) -> Result<Option<RepresentationGap>> {
    check(deadline, cancel)?;
    let outcome = captured
        .preparation
        .outcome
        .as_ref()
        .ok_or(QueryFailure::Code("EVIDENCE_BINDING_INVALID"))?;
    let artifact = match outcome.result {
        PreparationOutcome::Ready { artifact } => artifact,
        PreparationOutcome::Gap { .. } => {
            return Ok(Some(RepresentationGap {
                prepared: None,
                code: "PREPARATION_GAP",
            }));
        }
    };
    let source = match read_object(
        root,
        captured.source.object_ref,
        MAX_SOURCE_OBJECT_BYTES,
        deadline,
        cancel,
    ) {
        Ok(source) => source,
        Err(ObjectStoreError::Missing) => {
            return Ok(Some(RepresentationGap {
                prepared: Some(artifact),
                code: "SOURCE_OBJECT_MISSING",
            }));
        }
        Err(error) => return Err(object_failure(error)),
    };
    let bytes = match read_object(root, artifact, 64 * 1024 * 1024, deadline, cancel) {
        Ok(bytes) => bytes,
        Err(ObjectStoreError::Missing) => {
            return Ok(Some(RepresentationGap {
                prepared: Some(artifact),
                code: "PREPARED_OBJECT_MISSING",
            }));
        }
        Err(error) => return Err(object_failure(error)),
    };
    let view = decode_prepared_utf8(
        &bytes,
        &captured.source,
        &captured.preparation.settings,
        limits(),
        deadline,
        cancel,
    )
    .map_err(|error| match error {
        PreparationError::Cancelled => QueryFailure::Cancelled,
        PreparationError::Deadline => QueryFailure::Deadline,
        other => QueryFailure::Code(other.code()),
    })?;
    if view.text().as_bytes() != source.as_slice() {
        return Err(QueryFailure::Code("EVIDENCE_TEXT_MISMATCH"));
    }
    drop(source);
    check(deadline, cancel)?;
    visit(artifact, &view)?;
    Ok(None)
}

fn scan_source(
    root: &PlainRootGuard,
    captured: &CapturedSource,
    literal: &Literal<'_>,
    result: &mut DecodedUtf8Result,
    limit: usize,
    deadline: Instant,
    cancel: &AtomicBool,
) -> Result<()> {
    let binding = source_ref(&captured.source)?;
    let gap = visit_representation(root, captured, deadline, cancel, |artifact, view| {
        let prepared = object_ref(artifact)?;
        // Scan the full representation, not individual unit descriptors.
        literal.scan(view.text().as_bytes(), deadline, cancel, |start, end| {
            result.matches = result
                .matches
                .checked_add(1)
                .ok_or(QueryFailure::Code("EXACT_INPUT_LIMIT"))?;
            if result.items.len() < limit {
                let matched = view
                    .text()
                    .get(start..end)
                    .ok_or(QueryFailure::Code("EVIDENCE_RANGE_INVALID"))?;
                let mut text = String::new();
                text.try_reserve_exact(matched.len())
                    .map_err(|_| QueryFailure::Code("QUERY_ALLOCATION_FAILED"))?;
                text.push_str(matched);
                let start = u64::try_from(start)
                    .map_err(|_| QueryFailure::Code("EVIDENCE_RANGE_INVALID"))?;
                let end =
                    u64::try_from(end).map_err(|_| QueryFailure::Code("EVIDENCE_RANGE_INVALID"))?;
                result.items.push(DecodedUtf8Item::Match(DecodedUtf8Match {
                    source: binding.clone(),
                    prepared: prepared.clone(),
                    representation_start: start,
                    representation_end: end,
                    source_start: start,
                    source_end: end,
                    text,
                    source_info: None,
                }));
            }
            Ok(())
        })?;
        result.scanned_sources += 1;
        result.scanned_bytes = result
            .scanned_bytes
            .checked_add(captured.source.object_ref.byte_length)
            .ok_or(QueryFailure::Code("EXACT_INPUT_LIMIT"))?;
        Ok(())
    })?;
    if let Some(gap) = gap {
        result.gap_sources += 1;
        if result.items.len() < limit {
            result.items.push(DecodedUtf8Item::Gap(DecodedUtf8Gap {
                source: binding,
                prepared: gap.prepared.map(object_ref).transpose()?,
                code: FailureCode::new(gap.code)
                    .map_err(|error| QueryFailure::Code(error.code()))?,
                source_info: None,
            }));
        }
    }
    Ok(())
}

pub fn execute(
    root: &PlainRootGuard,
    view: &PublishedView,
    request: &DecodedUtf8Request,
    include_source_info: bool,
    deadline: Instant,
    cancel: &AtomicBool,
) -> Result<(DecodedUtf8Result, u64)> {
    check(deadline, cancel)?;
    request
        .validate()
        .map_err(|error| QueryFailure::Code(error.code()))?;
    let literal = Literal::new(request.literal.as_bytes(), deadline, cancel)?;
    let inventory = view
        .source_inventory(deadline, cancel)
        .map_err(QueryFailure::publication)?;
    let inventory = select_sources(inventory, request.source_ids.as_deref(), deadline, cancel)?;
    let mut result = DecodedUtf8Result {
        generation: view.generation.sequence,
        selected_sources: u64::try_from(inventory.len())
            .map_err(|_| QueryFailure::Code("EXACT_INPUT_LIMIT"))?,
        scanned_sources: 0,
        gap_sources: 0,
        scanned_bytes: 0,
        matches: 0,
        items: Vec::new(),
        scan_complete: false,
        complete_negative: false,
        stop: StopReason::Complete,
    };
    result
        .items
        .try_reserve_exact(usize::from(request.limit))
        .map_err(|_| QueryFailure::Code("QUERY_ALLOCATION_FAILED"))?;
    let mut preparation_gaps = 0_u64;
    for entry in inventory {
        if matches!(
            entry
                .preparation
                .outcome
                .as_ref()
                .map(|outcome| outcome.result),
            Some(PreparationOutcome::Gap { .. })
        ) {
            preparation_gaps += 1;
        }
        let first = result.items.len();
        scan_source(
            root,
            &entry,
            &literal,
            &mut result,
            usize::from(request.limit),
            deadline,
            cancel,
        )?;
        if first != result.items.len() {
            let info = crate::exact_literal::source_info(&entry.source, include_source_info);
            for item in &mut result.items[first..] {
                match item {
                    DecodedUtf8Item::Match(hit) => hit.source_info.clone_from(&info),
                    DecodedUtf8Item::Gap(gap) => gap.source_info.clone_from(&info),
                }
            }
        }
    }
    check(deadline, cancel)?;
    result.scan_complete = result.gap_sources == 0;
    // An empty scan population proves nothing; see raw_exact::execute.
    result.complete_negative =
        result.scan_complete && result.matches == 0 && result.selected_sources != 0;
    let total = result
        .matches
        .checked_add(result.gap_sources)
        .ok_or(QueryFailure::Code("EXACT_INPUT_LIMIT"))?;
    if u64::try_from(result.items.len()).ok() != Some(total) {
        result.stop = StopReason::OutputLimit;
    }
    result
        .validate_request(request)
        .map_err(|error| QueryFailure::Code(error.code()))?;
    Ok((result, preparation_gaps))
}

//! Literal raw-byte matching over one pinned committed source inventory.

use std::sync::atomic::AtomicBool;
use std::time::Instant;

use search_control_redb::current_preparation::PreparationOutcome;
use search_control_redb::publication::PublishedView;
use search_provider_protocol::{
    FailureCode, RawExactGap, RawExactItem, RawExactMatch, RawExactRequest, RawExactResult,
    StopReason,
};
use search_revision_store::{ObjectStoreError, read_object};
use search_runtime_owner::PlainRootGuard;
use search_source_registry::{CurrentSourceRecord, MAX_SOURCE_OBJECT_BYTES};

use crate::exact_literal::{Literal, check, hex, select_sources, source_ref};
use crate::verified_query::QueryFailure;

type Result<T> = std::result::Result<T, QueryFailure>;

fn scan_source(
    root: &PlainRootGuard,
    source: &CurrentSourceRecord,
    literal: &Literal<'_>,
    result: &mut RawExactResult,
    limit: usize,
    deadline: Instant,
    cancel: &AtomicBool,
) -> Result<()> {
    check(deadline, cancel)?;
    let binding = source_ref(source)?;
    let bytes = match read_object(
        root,
        source.object_ref,
        MAX_SOURCE_OBJECT_BYTES,
        deadline,
        cancel,
    ) {
        Ok(bytes) => bytes,
        Err(ObjectStoreError::Missing) => {
            result.missing_sources += 1;
            if result.items.len() < limit {
                result.items.push(RawExactItem::Gap(RawExactGap {
                    source: binding,
                    code: FailureCode::new("SOURCE_OBJECT_MISSING")
                        .map_err(|error| QueryFailure::Code(error.code()))?,
                    source_info: None,
                }));
            }
            return Ok(());
        }
        Err(ObjectStoreError::Cancelled) => return Err(QueryFailure::Cancelled),
        Err(ObjectStoreError::Deadline) => return Err(QueryFailure::Deadline),
        Err(error) => return Err(QueryFailure::Code(error.code())),
    };
    // read_object accepts length/hash before any occurrence can become evidence.
    // The buffer dies on return, before the next selected object is acquired.
    literal.scan(&bytes, deadline, cancel, |start, end| {
        result.matches = result
            .matches
            .checked_add(1)
            .ok_or(QueryFailure::Code("EXACT_INPUT_LIMIT"))?;
        if result.items.len() < limit {
            let matched = bytes
                .get(start..end)
                .ok_or(QueryFailure::Code("EVIDENCE_RANGE_INVALID"))?;
            result.items.push(RawExactItem::Match(RawExactMatch {
                source: binding.clone(),
                start: u64::try_from(start)
                    .map_err(|_| QueryFailure::Code("EVIDENCE_RANGE_INVALID"))?,
                end: u64::try_from(end)
                    .map_err(|_| QueryFailure::Code("EVIDENCE_RANGE_INVALID"))?,
                bytes_hex: hex(matched)?,
                source_info: None,
            }));
        }
        Ok(())
    })?;
    result.scanned_sources += 1;
    result.scanned_bytes = result
        .scanned_bytes
        .checked_add(source.object_ref.byte_length)
        .ok_or(QueryFailure::Code("EXACT_INPUT_LIMIT"))?;
    Ok(())
}

pub fn execute(
    root: &PlainRootGuard,
    view: &PublishedView,
    request: &RawExactRequest,
    include_source_info: bool,
    deadline: Instant,
    cancel: &AtomicBool,
) -> Result<(RawExactResult, u64)> {
    check(deadline, cancel)?;
    request
        .validate()
        .map_err(|error| QueryFailure::Code(error.code()))?;
    let literal = Literal::new(request.literal.as_bytes(), deadline, cancel)?;
    let inventory = view
        .source_inventory(deadline, cancel)
        .map_err(QueryFailure::publication)?;
    let inventory = select_sources(inventory, request.source_ids.as_deref(), deadline, cancel)?;
    let mut result = RawExactResult {
        generation: view.generation.sequence,
        selected_sources: u64::try_from(inventory.len())
            .map_err(|_| QueryFailure::Code("EXACT_INPUT_LIMIT"))?,
        scanned_sources: 0,
        missing_sources: 0,
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
    // Existing generation/source-size ceilings bound input, including integrity
    // work; presentation limits never end scanning or change the denominator.
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
            &entry.source,
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
                    RawExactItem::Match(hit) => hit.source_info.clone_from(&info),
                    RawExactItem::Gap(gap) => gap.source_info.clone_from(&info),
                }
            }
        }
    }
    check(deadline, cancel)?;
    result.scan_complete = result.missing_sources == 0;
    // An empty scan population proves nothing. A vacuous negative over zero
    // selected sources is never reported as a complete negative; the caller
    // still receives the exact zero-source denominator.
    result.complete_negative =
        result.scan_complete && result.matches == 0 && result.selected_sources != 0;
    let total = result
        .matches
        .checked_add(result.missing_sources)
        .ok_or(QueryFailure::Code("EXACT_INPUT_LIMIT"))?;
    if u64::try_from(result.items.len()).ok() != Some(total) {
        result.stop = StopReason::OutputLimit;
    }
    result
        .validate_request(request)
        .map_err(|error| QueryFailure::Code(error.code()))?;
    Ok((result, preparation_gaps))
}

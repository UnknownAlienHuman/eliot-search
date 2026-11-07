//! Literal raw-byte matching over one pinned committed source inventory.

use std::sync::atomic::{AtomicBool, Ordering};
use std::time::Instant;

use search_contracts::SourceId;
use search_control_redb::publication::{CapturedSource, PublishedView};
use search_provider_protocol::{
    FailureCode, ObjectKindDto, ObjectRefDto, RawExactGap, RawExactItem, RawExactMatch,
    RawExactRequest, RawExactResult, RawSourceRef, StopReason,
};
use search_revision_store::{ObjectStoreError, read_object};
use search_runtime_owner::PlainRootGuard;
use search_source_registry::{CurrentSourceRecord, MAX_SOURCE_OBJECT_BYTES};

use crate::verified_query::QueryFailure;

type Result<T> = std::result::Result<T, QueryFailure>;
const BLOCK_BYTES: usize = 8192;

fn check(deadline: Instant, cancel: &AtomicBool) -> Result<()> {
    if cancel.load(Ordering::Relaxed) {
        Err(QueryFailure::Cancelled)
    } else if Instant::now() >= deadline {
        Err(QueryFailure::Deadline)
    } else {
        Ok(())
    }
}

// The prefix table is compiled once per request. Each scan has independent
// source-local state that survives block boundaries and preserves overlaps.
struct Literal<'a> {
    bytes: &'a [u8],
    prefix: Vec<usize>,
}
impl<'a> Literal<'a> {
    fn new(bytes: &'a [u8], deadline: Instant, cancel: &AtomicBool) -> Result<Self> {
        check(deadline, cancel)?;
        if bytes.is_empty() {
            return Err(QueryFailure::Code("EXACT_LITERAL_INVALID"));
        }
        let mut prefix = Vec::new();
        prefix
            .try_reserve_exact(bytes.len())
            .map_err(|_| QueryFailure::Code("QUERY_ALLOCATION_FAILED"))?;
        prefix.resize(bytes.len(), 0);
        let mut matched = 0;
        for index in 1..bytes.len() {
            while matched != 0 && bytes[index] != bytes[matched] {
                matched = prefix[matched - 1];
            }
            if bytes[index] == bytes[matched] {
                matched += 1;
            }
            prefix[index] = matched;
        }
        check(deadline, cancel)?;
        Ok(Self { bytes, prefix })
    }

    fn scan(
        &self,
        input: &[u8],
        deadline: Instant,
        cancel: &AtomicBool,
        mut found: impl FnMut(usize, usize) -> Result<()>,
    ) -> Result<()> {
        let mut matched = 0;
        let mut consumed = 0;
        for block in input.chunks(BLOCK_BYTES) {
            check(deadline, cancel)?;
            for (index, byte) in block.iter().enumerate() {
                while matched != 0 && *byte != self.bytes[matched] {
                    matched = self.prefix[matched - 1];
                }
                if *byte == self.bytes[matched] {
                    matched += 1;
                }
                if matched == self.bytes.len() {
                    let end = consumed + index + 1;
                    found(end - self.bytes.len(), end)?;
                    matched = self.prefix[matched - 1];
                }
            }
            consumed += block.len();
        }
        check(deadline, cancel)
    }
}

fn hex(bytes: &[u8]) -> Result<String> {
    const DIGITS: &[u8; 16] = b"0123456789abcdef";
    let mut text = String::new();
    text.try_reserve_exact(bytes.len() * 2)
        .map_err(|_| QueryFailure::Code("QUERY_ALLOCATION_FAILED"))?;
    for byte in bytes {
        text.push(char::from(DIGITS[usize::from(*byte >> 4)]));
        text.push(char::from(DIGITS[usize::from(*byte & 15)]));
    }
    Ok(text)
}

fn source_ref(source: &CurrentSourceRecord) -> Result<RawSourceRef> {
    Ok(RawSourceRef {
        source_namespace_id: source.source_namespace_id.to_string(),
        source_id: source.source_id.to_string(),
        row_revision: source.row_revision,
        registration_revision: source.registration_revision,
        input: ObjectRefDto {
            version: source.object_ref.format_version,
            kind: ObjectKindDto::Source,
            digest: hex(source.object_ref.digest.as_bytes())?,
            bytes: source.object_ref.byte_length,
        },
    })
}

fn select_sources(
    mut inventory: Vec<CapturedSource>,
    ids: Option<&[String]>,
    deadline: Instant,
    cancel: &AtomicBool,
) -> Result<Vec<CapturedSource>> {
    if let Some(ids) = ids {
        let mut selected = Vec::new();
        selected
            .try_reserve_exact(ids.len())
            .map_err(|_| QueryFailure::Code("QUERY_ALLOCATION_FAILED"))?;
        for id in ids {
            check(deadline, cancel)?;
            let id =
                SourceId::parse(id).map_err(|_| QueryFailure::Code("EXACT_SELECTION_INVALID"))?;
            if inventory
                .binary_search_by_key(&id, |entry| entry.source.source_id)
                .is_err()
            {
                return Err(QueryFailure::Code("EXACT_SOURCE_NOT_PUBLISHED"));
            }
            selected.push(id);
        }
        inventory.retain(|entry| selected.contains(&entry.source.source_id));
    }
    check(deadline, cancel)?;
    Ok(inventory)
}

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
    deadline: Instant,
    cancel: &AtomicBool,
) -> Result<RawExactResult> {
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
    for entry in inventory {
        scan_source(
            root,
            &entry.source,
            &literal,
            &mut result,
            usize::from(request.limit),
            deadline,
            cancel,
        )?;
    }
    check(deadline, cancel)?;
    result.scan_complete = result.missing_sources == 0;
    result.complete_negative = result.scan_complete && result.matches == 0;
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
    Ok(result)
}

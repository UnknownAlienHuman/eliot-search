//! Shared bounded literal matching and pinned source selection for raw and UTF-8 Exact.

use std::sync::atomic::{AtomicBool, Ordering};
use std::time::Instant;

use search_contracts::SourceId;
use search_control_redb::publication::CapturedSource;
use search_provider_protocol::{ObjectKindDto, ObjectRefDto, RawSourceRef};
use search_source_registry::{CurrentSourceRecord, ObjectKind, ObjectRef};

use crate::verified_query::QueryFailure;
type Result<T> = std::result::Result<T, QueryFailure>;
const BLOCK_BYTES: usize = 8192;

pub fn check(deadline: Instant, cancel: &AtomicBool) -> Result<()> {
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
pub struct Literal<'a> {
    bytes: &'a [u8],
    prefix: Vec<usize>,
}
impl<'a> Literal<'a> {
    pub fn new(bytes: &'a [u8], deadline: Instant, cancel: &AtomicBool) -> Result<Self> {
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

    pub fn scan(
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

pub fn hex(bytes: &[u8]) -> Result<String> {
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

pub fn object_ref(reference: ObjectRef) -> Result<ObjectRefDto> {
    Ok(ObjectRefDto {
        version: reference.format_version,
        kind: match reference.kind {
            ObjectKind::Source => ObjectKindDto::Source,
            ObjectKind::Prepared => ObjectKindDto::Prepared,
        },
        digest: hex(reference.digest.as_bytes())?,
        bytes: reference.byte_length,
    })
}

pub fn source_ref(source: &CurrentSourceRecord) -> Result<RawSourceRef> {
    Ok(RawSourceRef {
        source_namespace_id: source.source_namespace_id.to_string(),
        source_id: source.source_id.to_string(),
        row_revision: source.row_revision,
        registration_revision: source.registration_revision,
        input: object_ref(source.object_ref)?,
    })
}

pub fn source_info(
    source: &CurrentSourceRecord,
    include: bool,
) -> Option<search_provider_protocol::SourceInfo> {
    use search_control_redb::current_source::{SourceControlError, source_path_from_locator};
    use search_provider_protocol::{
        SourceInfo, SourceInfoUnavailableReason as Reason, SourceLiveStatus,
    };
    if !include {
        return None;
    }
    let unavailable = |reason| {
        Some(SourceInfo::Unavailable {
            reason,
            live_status: SourceLiveStatus::NotChecked,
        })
    };
    let path = match source_path_from_locator(&source.locator) {
        Ok(path) => path,
        Err(SourceControlError::UnsupportedPlatform) => {
            return unavailable(Reason::UnsupportedPlatform);
        }
        Err(_) => return unavailable(Reason::InvalidLocator),
    };
    let Some(text) = path.to_str() else {
        return unavailable(Reason::NonUnicodeLocator);
    };
    if text.len() > search_provider_protocol::MAX_UNIT_TEXT_BYTES {
        return unavailable(Reason::OutputLimit);
    }
    let mut saved_path_hint = String::new();
    if saved_path_hint.try_reserve_exact(text.len()).is_err() {
        return unavailable(Reason::AllocationFailed);
    }
    saved_path_hint.push_str(text);
    Some(SourceInfo::Available {
        saved_path_hint,
        live_status: SourceLiveStatus::NotChecked,
    })
}

pub fn select_sources(
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

use std::fs::File;
use std::time::Instant;

use search_contracts::SourceRevisionId;

use crate::ControlError;

use super::model::{
    SourceImportBinding, SourceImportCounts, SourceImportRow,
};
use super::{HASH_BASE, ROW_BYTES};

pub(super) fn decode_progress(
    bytes: &[u8],
    binding: SourceImportBinding,
) -> Result<(SourceImportCounts, bool), ControlError> {
    if bytes.len() != 41 || bytes[0] > 1 {
        return Err(ControlError::StoreCorrupt);
    }
    let counts = SourceImportCounts {
        events: number(bytes, 1)?,
        sources: number(bytes, 9)?,
        occurrences: number(bytes, 17)?,
        retained_events: number(bytes, 25)?,
        retirements: number(bytes, 33)?,
    };
    let sealed = bytes[0] == 1;
    if counts.events > binding.events
        || counts.sources > binding.sources
        || counts.sources > counts.occurrences
        || counts.occurrences > counts.events
        || (counts.events == 0) != (counts.sources == 0)
        || counts
            .occurrences
            .checked_add(counts.retained_events)
            .and_then(|value| value.checked_add(counts.retirements))
            != Some(counts.events)
        || (sealed && validate_counts(counts, binding).is_err())
    {
        return Err(ControlError::StoreCorrupt);
    }
    Ok((counts, sealed))
}

pub(super) fn validate_counts(
    value: SourceImportCounts,
    binding: SourceImportBinding,
) -> Result<(), ControlError> {
    if value.events != binding.events
        || value.sources != binding.sources
        || value.sources > value.occurrences
        || value.occurrences > value.events
        || value
            .occurrences
            .checked_add(value.retained_events)
            .and_then(|count| count.checked_add(value.retirements))
            != Some(value.events)
    {
        return Err(ControlError::TransactionConflict);
    }
    Ok(())
}

pub(super) fn check(deadline: Instant) -> Result<(), ControlError> {
    if Instant::now() >= deadline {
        Err(ControlError::ReadCancelled)
    } else {
        Ok(())
    }
}

pub(super) fn check_file(file: &File, empty: bool) -> Result<(), ControlError> {
    let metadata = file
        .metadata()
        .map_err(|_| ControlError::StoreUnavailable)?;
    if !metadata.is_file() || (metadata.len() == 0) != empty {
        return Err(ControlError::StoreCorrupt);
    }
    Ok(())
}

pub(super) fn hash_field(
    value: &[u8],
    field: usize,
) -> Result<&[u8], ControlError> {
    if value.len() != ROW_BYTES {
        return Err(ControlError::StoreCorrupt);
    }
    value
        .get(HASH_BASE + field * 32..HASH_BASE + (field + 1) * 32)
        .ok_or(ControlError::StoreCorrupt)
}

fn number(value: &[u8], start: usize) -> Result<u64, ControlError> {
    Ok(u64::from_be_bytes(
        value
            .get(start..start + 8)
            .ok_or(ControlError::StoreCorrupt)?
            .try_into()
            .map_err(|_| ControlError::StoreCorrupt)?,
    ))
}

pub(super) fn validate_successor(
    previous: &[u8],
    next: &SourceImportRow,
) -> Result<(), ControlError> {
    if previous.len() != ROW_BYTES {
        return Err(ControlError::StoreCorrupt);
    }
    let lifecycle = next.lifecycle;
    let opens = !lifecycle.retires_source()
        && (previous[81] & 4 != 0
            || hash_field(previous, 2)? != next.legacy_revision.as_bytes());
    if previous.get(..16) != Some(next.source.as_bytes().as_slice())
        || next
            .previous_revision
            .as_ref()
            .map(SourceRevisionId::as_bytes)
            .map(<[u8; 16]>::as_slice)
            != previous.get(16..32)
        || next.source_event
            != number(previous, 57)?
                .checked_add(1)
                .ok_or(ControlError::GenerationExhausted)?
        || next.occurrence
            != number(previous, 49)?
                .checked_add(u64::from(opens))
                .ok_or(ControlError::GenerationExhausted)?
        || lifecycle.opens_revision() != opens
        || (lifecycle.retires_source() && previous[81] & 4 != 0)
        || hash_field(previous, 6)? != next.previous_source_event.as_bytes()
        || hash_field(previous, 1)? != next.legacy_source.as_bytes()
        || hash_field(previous, 4)? != next.file_identity.as_bytes()
        || previous[82] != u8::from(lifecycle.native_identity())
        || (hash_field(previous, 2)? == next.legacy_revision.as_bytes()
            && (hash_field(previous, 3)? != next.content.as_bytes()
                || number(previous, 73)? != next.source_bytes))
        || (lifecycle.retires_source()
            && (hash_field(previous, 2)? != next.legacy_revision.as_bytes()
                || hash_field(previous, 3)? != next.content.as_bytes()
                || hash_field(previous, 5)? != next.path.as_bytes()
                || number(previous, 73)? != next.source_bytes))
    {
        return Err(ControlError::TransactionConflict);
    }
    Ok(())
}

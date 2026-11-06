//! Line-preferred ranges over the original materializer-owned inventory.

use std::sync::atomic::AtomicBool;
use std::time::Instant;

use search_materializer::{LineSpan, MaterializedText};
use search_source_registry::PreparationSettings;

use crate::PreparationError;
use crate::PreparationLimits;
use crate::prepared::check;

/// A text descriptor in exact source and identical representation byte coordinates.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct UnitSpan {
    /// Inclusive source/representation byte start.
    pub source_start: usize,
    /// Exclusive source/representation byte end.
    pub source_end: usize,
    /// Inclusive zero-based logical line index.
    pub logical_line_start: u64,
    /// Exclusive zero-based logical line index.
    pub logical_line_end: u64,
    /// Whether the start is a complete logical-line boundary.
    pub starts_at_line_boundary: bool,
    /// Whether the end is a complete logical-line boundary.
    pub ends_at_line_boundary: bool,
}

pub fn unitize_text(
    product: &MaterializedText,
    settings: PreparationSettings,
    limits: PreparationLimits,
    deadline: Instant,
    cancelled: &AtomicBool,
) -> Result<Vec<UnitSpan>, PreparationError> {
    let text = product.text();
    let lines = product.lines();
    let preferred = usize::try_from(settings.preferred_unit_bytes)
        .map_err(|_| PreparationError::OffsetOverflow)?;
    let hard =
        usize::try_from(settings.max_unit_bytes).map_err(|_| PreparationError::OffsetOverflow)?;
    let mut units = Vec::new();
    let mut start = 0;
    while start < text.len() {
        check(deadline, cancelled)?;
        if units.len() >= limits.max_units {
            return Err(PreparationError::TooManyUnits);
        }
        let end = choose_end(text, lines, start, preferred, hard)?;
        if end <= start || end - start > hard {
            return Err(PreparationError::UnitTooLarge);
        }
        let start64 = u64::try_from(start).map_err(|_| PreparationError::OffsetOverflow)?;
        let end64 = u64::try_from(end).map_err(|_| PreparationError::OffsetOverflow)?;
        let first = lines
            .get(lines.partition_point(|line| line.source_end <= start64))
            .ok_or(PreparationError::InvalidArtifact)?;
        let last = lines
            .get(lines.partition_point(|line| line.source_end < end64))
            .ok_or(PreparationError::InvalidArtifact)?;
        if units.len() == units.capacity() {
            let additional = units.capacity().max(16).min(limits.max_units - units.len());
            units
                .try_reserve_exact(additional)
                .map_err(|_| PreparationError::AllocationFailed)?;
        }
        units.push(UnitSpan {
            source_start: start,
            source_end: end,
            logical_line_start: first.line_index,
            logical_line_end: last
                .line_index
                .checked_add(1)
                .ok_or(PreparationError::OffsetOverflow)?,
            starts_at_line_boundary: first.source_start == start64,
            ends_at_line_boundary: last.source_end == end64,
        });
        start = end;
    }
    check(deadline, cancelled)?;
    Ok(units)
}

fn choose_end(
    text: &str,
    lines: &[LineSpan],
    start: usize,
    preferred_bytes: usize,
    hard_bytes: usize,
) -> Result<usize, PreparationError> {
    let preferred = start
        .checked_add(preferred_bytes)
        .ok_or(PreparationError::OffsetOverflow)?
        .min(text.len());
    let hard = start
        .checked_add(hard_bytes)
        .ok_or(PreparationError::OffsetOverflow)?
        .min(text.len());
    if hard == text.len() && text.len() - start <= preferred_bytes {
        return Ok(text.len());
    }
    let preferred64 = u64::try_from(preferred).map_err(|_| PreparationError::OffsetOverflow)?;
    let start64 = u64::try_from(start).map_err(|_| PreparationError::OffsetOverflow)?;
    let hard64 = u64::try_from(hard).map_err(|_| PreparationError::OffsetOverflow)?;
    let after = lines.partition_point(|line| line.source_end <= preferred64);
    if let Some(line) = after.checked_sub(1).and_then(|index| lines.get(index))
        && line.source_end > start64
    {
        return usize::try_from(line.source_end).map_err(|_| PreparationError::OffsetOverflow);
    }
    if let Some(line) = lines.get(after)
        && line.source_end <= hard64
    {
        return usize::try_from(line.source_end).map_err(|_| PreparationError::OffsetOverflow);
    }
    let mut end = preferred;
    while end > start && !text.is_char_boundary(end) {
        end -= 1;
    }
    if end <= start {
        end = start
            .checked_add(1)
            .ok_or(PreparationError::OffsetOverflow)?;
        while end <= hard && !text.is_char_boundary(end) {
            end += 1;
        }
    }
    if end <= start || end > hard {
        return Err(PreparationError::UnitTooLarge);
    }
    Ok(end)
}

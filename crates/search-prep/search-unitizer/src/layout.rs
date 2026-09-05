//! Receipt-free layout shared by retained-revision and DIRECT adapters.

use crate::{SourceLineSpan, UnitizationError, UnitizationLimits};

/// A range into one exact materialized text, not a source or revision identity.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct UnitSpan {
    /// Inclusive source-byte start.
    pub source_start: usize,
    /// Exclusive source-byte end.
    pub source_end: usize,
    /// Inclusive zero-based logical line index.
    pub logical_line_start: u64,
    /// Exclusive zero-based logical line index.
    pub logical_line_end: u64,
    /// Whether the start is a full logical-line boundary.
    pub starts_at_line_boundary: bool,
    /// Whether the end is a full logical-line boundary.
    pub ends_at_line_boundary: bool,
}

/// Computes exact contiguous unit ranges without copying source text.
///
/// Empty text requires an empty inventory and produces no units. Non-empty
/// inventories must describe the actual logical lines, not just cover the bytes.
/// The algorithm is O(bytes + units * log(lines)), with bounded output allocation.
/// It performs no I/O, receipt issuance, hashing or authorization.
pub fn unitize_text(
    text: &str,
    lines: &[SourceLineSpan],
    limits: UnitizationLimits,
) -> Result<Vec<UnitSpan>, UnitizationError> {
    let limits = limits.validate()?;
    validate_text(text, lines, limits)?;
    let mut units = Vec::new();
    let mut start = 0;
    while start < text.len() {
        if units.len() >= limits.max_units {
            return Err(UnitizationError::TooManyUnits);
        }
        let end = choose_end(text, lines, start, limits)?;
        if end <= start { return Err(UnitizationError::NoProgress); }
        if end - start > limits.max_unit_bytes { return Err(UnitizationError::UnitTooLarge); }
        let start64 = u64::try_from(start).map_err(|_| UnitizationError::OffsetOverflow)?;
        let end64 = u64::try_from(end).map_err(|_| UnitizationError::OffsetOverflow)?;
        let first_line = lines.partition_point(|line| line.source_end <= start64);
        let last_line = lines.partition_point(|line| line.source_end < end64);
        if first_line >= lines.len() || last_line >= lines.len() {
            return Err(UnitizationError::LineCoverageMismatch);
        }
        units.push(UnitSpan {
            source_start: start,
            source_end: end,
            logical_line_start: lines[first_line].line_index,
            logical_line_end: lines[last_line].line_index.checked_add(1)
                .ok_or(UnitizationError::OffsetOverflow)?,
            starts_at_line_boundary: lines[first_line].source_start == start64,
            ends_at_line_boundary: lines[last_line].source_end == end64,
        });
        start = end;
    }
    // Each iteration begins exactly at the previous end and advances. Together
    // with this final equality this establishes no gaps, overlaps or lost bytes.
    if start != text.len() { return Err(UnitizationError::UnitCoverageMismatch); }
    Ok(units)
}

fn validate_text(
    text: &str,
    lines: &[SourceLineSpan],
    limits: UnitizationLimits,
) -> Result<(), UnitizationError> {
    if text.len() > limits.max_input_bytes { return Err(UnitizationError::InputTooLarge); }
    if lines.len() > limits.max_lines || (lines.is_empty() != text.is_empty()) {
        return Err(UnitizationError::InvalidLineInventory);
    }
    let bytes = text.as_bytes();
    let mut cursor = 0;
    for (index, line) in lines.iter().enumerate() {
        if line.line_index != u64::try_from(index).map_err(|_| UnitizationError::OffsetOverflow)? {
            return Err(UnitizationError::LineIndexMismatch);
        }
        let start = usize::try_from(line.source_start).map_err(|_| UnitizationError::OffsetOverflow)?;
        let end = usize::try_from(line.source_end).map_err(|_| UnitizationError::OffsetOverflow)?;
        let content_end = usize::try_from(line.content_end).map_err(|_| UnitizationError::OffsetOverflow)?;
        if start != cursor || start >= end || content_end < start || content_end > end || end > bytes.len() {
            return Err(UnitizationError::InvalidLineSpan);
        }
        if !text.is_char_boundary(start) || !text.is_char_boundary(content_end) || !text.is_char_boundary(end) {
            return Err(UnitizationError::InvalidUtf8Boundary);
        }
        match bytes.get(content_end..end) {
            Some(b"") | Some(b"\n") | Some(b"\r") | Some(b"\r\n") => {}
            _ => return Err(UnitizationError::InvalidLineEnding),
        }
        cursor = end;
    }
    if cursor != bytes.len() { return Err(UnitizationError::LineCoverageMismatch); }
    for (index, line) in lines.iter().enumerate() {
        // Conversions and ranges were checked in the first pass.
        let start = usize::try_from(line.source_start).map_err(|_| UnitizationError::OffsetOverflow)?;
        let end = usize::try_from(line.source_end).map_err(|_| UnitizationError::OffsetOverflow)?;
        let content_end = usize::try_from(line.content_end).map_err(|_| UnitizationError::OffsetOverflow)?;
        if bytes[start..content_end].iter().any(|byte| matches!(byte, b'\r' | b'\n'))
            || (content_end == end && index + 1 != lines.len())
            || (bytes.get(content_end..end) == Some(b"\r".as_slice()) && bytes.get(end) == Some(&b'\n'))
        {
            return Err(UnitizationError::InvalidLineEnding);
        }
    }
    Ok(())
}

fn choose_end(
    text: &str,
    lines: &[SourceLineSpan],
    start: usize,
    limits: UnitizationLimits,
) -> Result<usize, UnitizationError> {
    let preferred = start.checked_add(limits.preferred_unit_bytes)
        .ok_or(UnitizationError::OffsetOverflow)?.min(text.len());
    let hard = start.checked_add(limits.max_unit_bytes)
        .ok_or(UnitizationError::OffsetOverflow)?.min(text.len());
    if hard == text.len() && text.len() - start <= limits.preferred_unit_bytes {
        return Ok(text.len());
    }
    let preferred64 = u64::try_from(preferred).map_err(|_| UnitizationError::OffsetOverflow)?;
    let start64 = u64::try_from(start).map_err(|_| UnitizationError::OffsetOverflow)?;
    let hard64 = u64::try_from(hard).map_err(|_| UnitizationError::OffsetOverflow)?;
    let after = lines.partition_point(|line| line.source_end <= preferred64);
    if let Some(line) = after.checked_sub(1).and_then(|index| lines.get(index)) {
        if line.source_end > start64 {
            return usize::try_from(line.source_end).map_err(|_| UnitizationError::OffsetOverflow);
        }
    }
    if let Some(line) = lines.get(after) {
        if line.source_end <= hard64 {
            return usize::try_from(line.source_end).map_err(|_| UnitizationError::OffsetOverflow);
        }
    }
    let mut end = preferred;
    while end > start && !text.is_char_boundary(end) { end -= 1; }
    if end <= start {
        end = start.checked_add(1).ok_or(UnitizationError::OffsetOverflow)?;
        while end <= hard && !text.is_char_boundary(end) { end += 1; }
    }
    if end <= start || end > hard { return Err(UnitizationError::NoProgress); }
    Ok(end)
}

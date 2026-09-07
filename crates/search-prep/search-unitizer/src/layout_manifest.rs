//! Versioned, bounded serialization of the exact UTF-8 line/unit layout.
//! Source and residency identities are bound by the revision-store envelope.

use super::{SourceLineSpan, UnitSpan, UnitizationError, UnitizationLimits};

const MAGIC: &[u8; 8] = b"ELSLAY01";
const HEADER: usize = 72;
const LINE_BYTES: usize = 32;
const UNIT_BYTES: usize = 34;
type Layout = (Vec<SourceLineSpan>, Vec<UnitSpan>);

impl UnitizationLimits {
    /// Versioned layout algorithm/codec identity. Changing boundary semantics or
    /// encoding requires a new identity, never reinterpretation of saved layouts.
    pub const LAYOUT_FORMAT: &'static str = "exact-utf8-line-unit-layout/v1";

    /// Builds and encodes a complete deterministic layout without source text.
    /// This pure operation neither persists an object nor issues an admission receipt.
    pub fn encode_layout(
        self,
        text: &str,
        lines: &[SourceLineSpan],
        max_encoded_bytes: usize,
    ) -> Result<Vec<u8>, UnitizationError> {
        let units = super::unitize_text(text, lines, self)?;
        let size = encoded_size(lines.len(), units.len())?;
        if size > max_encoded_bytes { return Err(UnitizationError::InputTooLarge); }
        let mut out = Vec::with_capacity(size);
        out.extend_from_slice(MAGIC);
        for value in dimensions(self).into_iter().chain([text.len(), lines.len(), units.len()]) {
            put(&mut out, u64::try_from(value).map_err(|_| UnitizationError::OffsetOverflow)?);
        }
        for line in lines {
            for value in [line.line_index, line.source_start, line.source_end, line.content_end] {
                put(&mut out, value);
            }
        }
        for unit in units {
            put(&mut out, u64::try_from(unit.source_start).map_err(|_| UnitizationError::OffsetOverflow)?);
            put(&mut out, u64::try_from(unit.source_end).map_err(|_| UnitizationError::OffsetOverflow)?);
            put(&mut out, unit.logical_line_start);
            put(&mut out, unit.logical_line_end);
            out.push(u8::from(unit.starts_at_line_boundary));
            out.push(u8::from(unit.ends_at_line_boundary));
        }
        Ok(out)
    }

    /// Decodes a saved layout against exact verified source bytes and this profile.
    /// Counts and total encoded length are checked before allocation. Verification
    /// uses the existing boundary rules without rebuilding a second unit inventory.
    pub fn decode_layout(
        self,
        text: &str,
        bytes: &[u8],
        max_encoded_bytes: usize,
    ) -> Result<Layout, UnitizationError> {
        self.validate()?;
        if bytes.len() < HEADER || bytes.len() > max_encoded_bytes || &bytes[..8] != MAGIC {
            return Err(UnitizationError::UnitCoverageMismatch);
        }
        let mut cursor = 8;
        for expected in dimensions(self) {
            if number(bytes, &mut cursor)? != expected { return Err(UnitizationError::InvalidLimits); }
        }
        if number(bytes, &mut cursor)? != text.len() { return Err(UnitizationError::UnitCoverageMismatch); }
        let line_count = number(bytes, &mut cursor)?;
        let unit_count = number(bytes, &mut cursor)?;
        if line_count > self.max_lines { return Err(UnitizationError::InvalidLineInventory); }
        if unit_count > self.max_units { return Err(UnitizationError::TooManyUnits); }
        if encoded_size(line_count, unit_count)? != bytes.len() {
            return Err(UnitizationError::UnitCoverageMismatch);
        }
        let mut lines = Vec::with_capacity(line_count);
        for _ in 0..line_count {
            lines.push(SourceLineSpan {
                line_index: read(bytes, &mut cursor)?, source_start: read(bytes, &mut cursor)?,
                source_end: read(bytes, &mut cursor)?, content_end: read(bytes, &mut cursor)?,
            });
        }
        super::validate_text(text, &lines, self)?;
        let mut units = Vec::with_capacity(unit_count);
        let mut expected_start = 0;
        for _ in 0..unit_count {
            let unit = UnitSpan {
                source_start: number(bytes, &mut cursor)?, source_end: number(bytes, &mut cursor)?,
                logical_line_start: read(bytes, &mut cursor)?, logical_line_end: read(bytes, &mut cursor)?,
                starts_at_line_boundary: boolean(bytes, &mut cursor)?,
                ends_at_line_boundary: boolean(bytes, &mut cursor)?,
            };
            if unit.source_start != expected_start || expected_start >= text.len()
                || unit.source_end != super::choose_end(text, &lines, expected_start, self)?
                || unit.source_end <= expected_start || unit.source_end - expected_start > self.max_unit_bytes
                || !text.is_char_boundary(unit.source_start) || !text.is_char_boundary(unit.source_end)
            {
                return Err(UnitizationError::UnitCoverageMismatch);
            }
            let start = u64::try_from(unit.source_start).map_err(|_| UnitizationError::OffsetOverflow)?;
            let end = u64::try_from(unit.source_end).map_err(|_| UnitizationError::OffsetOverflow)?;
            let first = lines.get(lines.partition_point(|line| line.source_end <= start))
                .ok_or(UnitizationError::LineCoverageMismatch)?;
            let last = lines.get(lines.partition_point(|line| line.source_end < end))
                .ok_or(UnitizationError::LineCoverageMismatch)?;
            if unit.logical_line_start != first.line_index
                || Some(unit.logical_line_end) != last.line_index.checked_add(1)
                || unit.starts_at_line_boundary != (first.source_start == start)
                || unit.ends_at_line_boundary != (last.source_end == end)
            {
                return Err(UnitizationError::LineCoverageMismatch);
            }
            expected_start = unit.source_end;
            units.push(unit);
        }
        if expected_start != text.len() || cursor != bytes.len() {
            return Err(UnitizationError::UnitCoverageMismatch);
        }
        Ok((lines, units))
    }
}

fn dimensions(limits: UnitizationLimits) -> [usize; 5] {
    [limits.max_input_bytes, limits.preferred_unit_bytes, limits.max_unit_bytes,
        limits.max_lines, limits.max_units]
}
fn encoded_size(lines: usize, units: usize) -> Result<usize, UnitizationError> {
    lines.checked_mul(LINE_BYTES).and_then(|n| n.checked_add(HEADER))
        .and_then(|n| units.checked_mul(UNIT_BYTES).and_then(|m| n.checked_add(m)))
        .ok_or(UnitizationError::OffsetOverflow)
}
fn put(out: &mut Vec<u8>, value: u64) { out.extend_from_slice(&value.to_be_bytes()); }
fn read(bytes: &[u8], cursor: &mut usize) -> Result<u64, UnitizationError> {
    let end = cursor.checked_add(8).ok_or(UnitizationError::OffsetOverflow)?;
    let value = bytes.get(*cursor..end).ok_or(UnitizationError::UnitCoverageMismatch)?;
    *cursor = end;
    Ok(u64::from_be_bytes(value.try_into().map_err(|_| UnitizationError::UnitCoverageMismatch)?))
}
fn number(bytes: &[u8], cursor: &mut usize) -> Result<usize, UnitizationError> {
    usize::try_from(read(bytes, cursor)?).map_err(|_| UnitizationError::OffsetOverflow)
}
fn boolean(bytes: &[u8], cursor: &mut usize) -> Result<bool, UnitizationError> {
    let value = match bytes.get(*cursor) {
        Some(0) => false, Some(1) => true, _ => return Err(UnitizationError::UnitCoverageMismatch),
    };
    *cursor += 1;
    Ok(value)
}

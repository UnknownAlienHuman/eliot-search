//! Strict exact-layout decoding and source-byte verification.

use crate::{SourceLineSpan, UnitSpan, UnitizationError, UnitizationLimits};

use super::wire::{HEADER, MAGIC, boolean, dimensions, encoded_size, number, read};

impl UnitizationLimits {
    /// Decodes a saved layout against exact verified source bytes and this profile.
    /// Counts and total encoded length are checked before allocation. Verification
    /// uses the existing boundary rules without rebuilding a second unit inventory.
    pub fn decode_layout(
        self,
        text: &str,
        bytes: &[u8],
        max_encoded_bytes: usize,
    ) -> Result<(Vec<SourceLineSpan>, Vec<UnitSpan>), UnitizationError> {
        self.validate()?;
        if bytes.len() < HEADER || bytes.len() > max_encoded_bytes || &bytes[..8] != MAGIC {
            return Err(UnitizationError::UnitCoverageMismatch);
        }
        let mut cursor = 8;
        for expected in dimensions(self) {
            if number(bytes, &mut cursor)? != expected {
                return Err(UnitizationError::InvalidLimits);
            }
        }
        if number(bytes, &mut cursor)? != text.len() {
            return Err(UnitizationError::UnitCoverageMismatch);
        }
        let line_count = number(bytes, &mut cursor)?;
        let unit_count = number(bytes, &mut cursor)?;
        if line_count > self.max_lines {
            return Err(UnitizationError::InvalidLineInventory);
        }
        if unit_count > self.max_units {
            return Err(UnitizationError::TooManyUnits);
        }
        if encoded_size(line_count, unit_count)? != bytes.len() {
            return Err(UnitizationError::UnitCoverageMismatch);
        }
        let mut lines = Vec::with_capacity(line_count);
        for _ in 0..line_count {
            lines.push(SourceLineSpan {
                line_index: read(bytes, &mut cursor)?,
                source_start: read(bytes, &mut cursor)?,
                source_end: read(bytes, &mut cursor)?,
                content_end: read(bytes, &mut cursor)?,
            });
        }
        super::super::validate_text(text, &lines, self)?;
        let mut units = Vec::with_capacity(unit_count);
        let mut expected_start = 0;
        for _ in 0..unit_count {
            let unit = UnitSpan {
                source_start: number(bytes, &mut cursor)?,
                source_end: number(bytes, &mut cursor)?,
                logical_line_start: read(bytes, &mut cursor)?,
                logical_line_end: read(bytes, &mut cursor)?,
                starts_at_line_boundary: boolean(bytes, &mut cursor)?,
                ends_at_line_boundary: boolean(bytes, &mut cursor)?,
            };
            if unit.source_start != expected_start
                || expected_start >= text.len()
                || unit.source_end
                    != super::super::choose_end(text, &lines, expected_start, self)?
                || unit.source_end <= expected_start
                || unit.source_end - expected_start > self.max_unit_bytes
                || !text.is_char_boundary(unit.source_start)
                || !text.is_char_boundary(unit.source_end)
            {
                return Err(UnitizationError::UnitCoverageMismatch);
            }
            let start =
                u64::try_from(unit.source_start).map_err(|_| UnitizationError::OffsetOverflow)?;
            let end =
                u64::try_from(unit.source_end).map_err(|_| UnitizationError::OffsetOverflow)?;
            let first = lines
                .get(lines.partition_point(|line| line.source_end <= start))
                .ok_or(UnitizationError::LineCoverageMismatch)?;
            let last = lines
                .get(lines.partition_point(|line| line.source_end < end))
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

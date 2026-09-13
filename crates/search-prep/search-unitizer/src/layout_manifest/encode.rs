//! Canonical exact-layout encoding.

use crate::{SourceLineSpan, UnitizationError, UnitizationLimits};

use super::wire::{MAGIC, dimensions, encoded_size, put};

impl UnitizationLimits {
    /// Builds and encodes a complete deterministic layout without source text.
    /// This pure operation neither persists an object nor issues an admission receipt.
    pub fn encode_layout(
        self,
        text: &str,
        lines: &[SourceLineSpan],
        max_encoded_bytes: usize,
    ) -> Result<Vec<u8>, UnitizationError> {
        let units = super::super::unitize_text(text, lines, self)?;
        let size = encoded_size(lines.len(), units.len())?;
        if size > max_encoded_bytes {
            return Err(UnitizationError::InputTooLarge);
        }
        let mut out = Vec::with_capacity(size);
        out.extend_from_slice(MAGIC);
        for value in dimensions(self)
            .into_iter()
            .chain([text.len(), lines.len(), units.len()])
        {
            put(
                &mut out,
                u64::try_from(value).map_err(|_| UnitizationError::OffsetOverflow)?,
            );
        }
        for line in lines {
            for value in [
                line.line_index,
                line.source_start,
                line.source_end,
                line.content_end,
            ] {
                put(&mut out, value);
            }
        }
        for unit in units {
            put(
                &mut out,
                u64::try_from(unit.source_start)
                    .map_err(|_| UnitizationError::OffsetOverflow)?,
            );
            put(
                &mut out,
                u64::try_from(unit.source_end)
                    .map_err(|_| UnitizationError::OffsetOverflow)?,
            );
            put(&mut out, unit.logical_line_start);
            put(&mut out, unit.logical_line_end);
            out.push(u8::from(unit.starts_at_line_boundary));
            out.push(u8::from(unit.ends_at_line_boundary));
        }
        Ok(out)
    }
}

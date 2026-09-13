//! Exact UTF-8 byte validation and logical-line scanning.

use super::model::{LineEnding, LineEndingEvidence, LineSpan, MaterializationLimits};
use crate::error::MaterializationError;

pub(super) const fn check_byte_limits(
    length: usize,
    limits: MaterializationLimits,
) -> Result<(), MaterializationError> {
    if length > limits.max_input_bytes {
        return Err(MaterializationError::InputTooLarge);
    }
    if length > limits.max_output_bytes {
        return Err(MaterializationError::OutputTooLarge);
    }
    Ok(())
}

pub(super) fn reject_binary_controls(bytes: &[u8]) -> Result<(), MaterializationError> {
    if bytes.contains(&0) {
        return Err(MaterializationError::BinaryContent);
    }
    let disallowed_controls = bytes
        .iter()
        .filter(|byte| **byte < 0x20 && !matches!(**byte, b'\t' | b'\n' | b'\r' | 0x0c))
        .count();
    let threshold = bytes.len().div_ceil(100).max(4);
    if disallowed_controls >= threshold {
        Err(MaterializationError::BinaryContent)
    } else {
        Ok(())
    }
}

pub(super) fn scan_lines(
    bytes: &[u8],
    limits: MaterializationLimits,
) -> Result<(Vec<LineSpan>, LineEndingEvidence), MaterializationError> {
    let mut lines = Vec::new();
    let mut evidence = LineEndingEvidence::default();
    let mut start = 0_usize;
    let mut index = 0_usize;
    while index < bytes.len() {
        let ending = match bytes[index] {
            b'\n' => Some((LineEnding::Lf, 1_usize)),
            b'\r' if bytes.get(index + 1) == Some(&b'\n') => Some((LineEnding::CrLf, 2_usize)),
            b'\r' => Some((LineEnding::Cr, 1_usize)),
            _ => None,
        };
        let Some((ending, terminator_bytes)) = ending else {
            index += 1;
            continue;
        };
        if lines.len() >= limits.max_lines {
            return Err(MaterializationError::TooManyLines);
        }
        let content_end = index;
        let source_end = index
            .checked_add(terminator_bytes)
            .ok_or(MaterializationError::OffsetOverflow)?;
        lines.push(LineSpan {
            line_index: u64::try_from(lines.len())
                .map_err(|_| MaterializationError::OffsetOverflow)?,
            source_start: u64::try_from(start).map_err(|_| MaterializationError::OffsetOverflow)?,
            source_end: u64::try_from(source_end)
                .map_err(|_| MaterializationError::OffsetOverflow)?,
            content_end: u64::try_from(content_end)
                .map_err(|_| MaterializationError::OffsetOverflow)?,
            ending,
        });
        match ending {
            LineEnding::Lf => evidence.lf += 1,
            LineEnding::CrLf => evidence.crlf += 1,
            LineEnding::Cr => evidence.cr += 1,
            LineEnding::None => {}
        }
        start = source_end;
        index = source_end;
    }
    if start < bytes.len() {
        if lines.len() >= limits.max_lines {
            return Err(MaterializationError::TooManyLines);
        }
        lines.push(LineSpan {
            line_index: u64::try_from(lines.len())
                .map_err(|_| MaterializationError::OffsetOverflow)?,
            source_start: u64::try_from(start).map_err(|_| MaterializationError::OffsetOverflow)?,
            source_end: u64::try_from(bytes.len())
                .map_err(|_| MaterializationError::OffsetOverflow)?,
            content_end: u64::try_from(bytes.len())
                .map_err(|_| MaterializationError::OffsetOverflow)?,
            ending: LineEnding::None,
        });
        evidence.unterminated += 1;
    }
    Ok((lines, evidence))
}

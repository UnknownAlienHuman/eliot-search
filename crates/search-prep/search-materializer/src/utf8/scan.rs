//! Bounded binary classification and exact logical-line scanning.

use std::sync::atomic::{AtomicBool, Ordering};
use std::time::Instant;

use super::model::{LineSpan, MaterializationLimits};
use crate::MaterializationError;

pub(super) fn check(deadline: Instant, cancelled: &AtomicBool) -> Result<(), MaterializationError> {
    if cancelled.load(Ordering::Relaxed) {
        return Err(MaterializationError::Cancelled);
    }
    if Instant::now() >= deadline {
        return Err(MaterializationError::Deadline);
    }
    Ok(())
}

pub(super) fn reject_binary_controls(
    bytes: &[u8],
    deadline: Instant,
    cancelled: &AtomicBool,
) -> Result<(), MaterializationError> {
    let threshold = bytes.len().div_ceil(100).max(4);
    let mut controls = 0;
    for chunk in bytes.chunks(1024) {
        check(deadline, cancelled)?;
        for byte in chunk {
            if *byte == 0 {
                return Err(MaterializationError::BinaryContent);
            }
            if *byte < 0x20 && !matches!(*byte, b'\t' | b'\n' | b'\r' | 0x0c) {
                controls += 1;
                if controls >= threshold {
                    return Err(MaterializationError::BinaryContent);
                }
            }
        }
    }
    Ok(())
}

pub(super) fn scan_lines(
    bytes: &[u8],
    limits: MaterializationLimits,
    deadline: Instant,
    cancelled: &AtomicBool,
) -> Result<Vec<LineSpan>, MaterializationError> {
    let mut lines = Vec::new();
    let mut start = 0;
    let mut index = 0;
    while index < bytes.len() {
        if index.is_multiple_of(1024) {
            check(deadline, cancelled)?;
        }
        let terminator = match bytes[index] {
            b'\r' if bytes.get(index + 1) == Some(&b'\n') => 2,
            b'\n' | b'\r' => 1,
            _ => {
                index += 1;
                continue;
            }
        };
        let end = index
            .checked_add(terminator)
            .ok_or(MaterializationError::OffsetOverflow)?;
        push_line(&mut lines, start, index, end, limits.max_lines)?;
        start = end;
        index = end;
    }
    if start < bytes.len() {
        push_line(
            &mut lines,
            start,
            bytes.len(),
            bytes.len(),
            limits.max_lines,
        )?;
    }
    check(deadline, cancelled)?;
    Ok(lines)
}

fn push_line(
    lines: &mut Vec<LineSpan>,
    start: usize,
    content_end: usize,
    end: usize,
    maximum: usize,
) -> Result<(), MaterializationError> {
    if lines.len() >= maximum {
        return Err(MaterializationError::TooManyLines);
    }
    if lines.len() == lines.capacity() {
        let additional = lines.capacity().max(16).min(maximum - lines.len());
        lines
            .try_reserve_exact(additional)
            .map_err(|_| MaterializationError::AllocationFailed)?;
    }
    lines.push(LineSpan {
        line_index: u64::try_from(lines.len()).map_err(|_| MaterializationError::OffsetOverflow)?,
        source_start: u64::try_from(start).map_err(|_| MaterializationError::OffsetOverflow)?,
        source_end: u64::try_from(end).map_err(|_| MaterializationError::OffsetOverflow)?,
        content_end: u64::try_from(content_end)
            .map_err(|_| MaterializationError::OffsetOverflow)?,
    });
    Ok(())
}

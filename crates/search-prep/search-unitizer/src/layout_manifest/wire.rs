//! Closed layout-v1 wire framing helpers.

use crate::{UnitizationError, UnitizationLimits};

pub(super) const MAGIC: &[u8; 8] = b"ELSLAY01";
pub(super) const HEADER: usize = 72;
const LINE_BYTES: usize = 32;
const UNIT_BYTES: usize = 34;

pub(super) const fn dimensions(limits: UnitizationLimits) -> [usize; 5] {
    [
        limits.max_input_bytes,
        limits.preferred_unit_bytes,
        limits.max_unit_bytes,
        limits.max_lines,
        limits.max_units,
    ]
}

pub(super) fn encoded_size(
    lines: usize,
    units: usize,
) -> Result<usize, UnitizationError> {
    lines
        .checked_mul(LINE_BYTES)
        .and_then(|value| value.checked_add(HEADER))
        .and_then(|value| {
            units
                .checked_mul(UNIT_BYTES)
                .and_then(|tail| value.checked_add(tail))
        })
        .ok_or(UnitizationError::OffsetOverflow)
}

pub(super) fn put(out: &mut Vec<u8>, value: u64) {
    out.extend_from_slice(&value.to_be_bytes());
}

pub(super) fn read(
    bytes: &[u8],
    cursor: &mut usize,
) -> Result<u64, UnitizationError> {
    let end = cursor
        .checked_add(8)
        .ok_or(UnitizationError::OffsetOverflow)?;
    let value = bytes
        .get(*cursor..end)
        .ok_or(UnitizationError::UnitCoverageMismatch)?;
    *cursor = end;
    Ok(u64::from_be_bytes(
        value
            .try_into()
            .map_err(|_| UnitizationError::UnitCoverageMismatch)?,
    ))
}

pub(super) fn number(
    bytes: &[u8],
    cursor: &mut usize,
) -> Result<usize, UnitizationError> {
    usize::try_from(read(bytes, cursor)?).map_err(|_| UnitizationError::OffsetOverflow)
}

pub(super) fn boolean(
    bytes: &[u8],
    cursor: &mut usize,
) -> Result<bool, UnitizationError> {
    let value = match bytes.get(*cursor) {
        Some(0) => false,
        Some(1) => true,
        _ => return Err(UnitizationError::UnitCoverageMismatch),
    };
    *cursor += 1;
    Ok(value)
}

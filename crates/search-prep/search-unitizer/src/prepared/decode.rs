//! Bounded saved-artifact acceptance without reconstructing expected units.

use std::sync::atomic::AtomicBool;
use std::time::Instant;

use search_source_registry::{CurrentSourceRecord, PreparationSettings};

use super::wire::{HEADER_BYTES, MAGIC, Reader, VERSION, encoded_size};
use super::{PreparationLimits, PreparedUtf8, check, verify_digest};
use crate::{PreparationError, UnitSpan};

pub(super) fn decode<'a>(
    artifact: &'a [u8],
    source: &CurrentSourceRecord,
    settings: PreparationSettings,
    limits: PreparationLimits,
    deadline: Instant,
    cancelled: &AtomicBool,
) -> Result<PreparedUtf8<'a>, PreparationError> {
    if artifact.len() > limits.max_encoded_bytes {
        return Err(PreparationError::EncodedTooLarge);
    }
    if artifact.len() < HEADER_BYTES {
        return Err(PreparationError::InvalidArtifact);
    }
    let mut reader = Reader::new(artifact);
    read_binding(&mut reader, source, settings)?;
    let text_length = reader.usize()?;
    let unit_count = reader.usize()?;
    if unit_count > limits.max_units {
        return Err(PreparationError::TooManyUnits);
    }
    limits.check_text_length(text_length)?;
    if u64::try_from(text_length).ok() != Some(source.object_ref.byte_length) {
        return Err(PreparationError::BindingMismatch);
    }
    if encoded_size(text_length, unit_count)? != artifact.len() {
        return Err(PreparationError::InvalidArtifact);
    }
    check(deadline, cancelled)?;
    let text_bytes = reader.take(text_length)?;
    let text = std::str::from_utf8(text_bytes).map_err(|_| PreparationError::InvalidArtifact)?;
    verify_digest(source, text_bytes, deadline, cancelled)?;
    let units = read_units(
        &mut reader,
        text,
        unit_count,
        settings,
        limits,
        deadline,
        cancelled,
    )?;
    if !reader.at_end() {
        return Err(PreparationError::InvalidArtifact);
    }
    check(deadline, cancelled)?;
    Ok(PreparedUtf8 { text, units })
}

fn read_binding(
    reader: &mut Reader<'_>,
    source: &CurrentSourceRecord,
    settings: PreparationSettings,
) -> Result<(), PreparationError> {
    if reader.take(MAGIC.len())? != MAGIC || reader.u16()? != VERSION {
        return Err(PreparationError::InvalidArtifact);
    }
    if reader.take(16)? != source.source_namespace_id.as_bytes()
        || reader.take(16)? != source.source_id.as_bytes()
        || reader.u64()? != source.registration_revision
        || reader.u16()? != source.object_ref.format_version
        || reader.take(1)? != [source.object_ref.kind.tag()]
        || reader.take(32)? != source.object_ref.digest.as_bytes()
        || reader.u64()? != source.object_ref.byte_length
        || reader.u16()? != settings.utf8_version
        || reader.u16()? != settings.line_unitizer_version
        || reader.u32()? != settings.preferred_unit_bytes
        || reader.u32()? != settings.max_unit_bytes
    {
        return Err(PreparationError::BindingMismatch);
    }
    Ok(())
}

fn read_units(
    reader: &mut Reader<'_>,
    text: &str,
    count: usize,
    settings: PreparationSettings,
    limits: PreparationLimits,
    deadline: Instant,
    cancelled: &AtomicBool,
) -> Result<Vec<UnitSpan>, PreparationError> {
    if text.is_empty() != (count == 0) {
        return Err(PreparationError::InvalidArtifact);
    }
    let hard =
        usize::try_from(settings.max_unit_bytes).map_err(|_| PreparationError::OffsetOverflow)?;
    let mut units = Vec::new();
    units
        .try_reserve_exact(count)
        .map_err(|_| PreparationError::AllocationFailed)?;
    let mut lines = Lines {
        bytes: text.as_bytes(),
        offset: 0,
        index: 0,
    };
    let mut current = lines.next(limits.max_lines, deadline, cancelled)?;
    let mut expected_start = 0;
    for _ in 0..count {
        check(deadline, cancelled)?;
        let unit = UnitSpan {
            source_start: reader.usize()?,
            source_end: reader.usize()?,
            logical_line_start: reader.u64()?,
            logical_line_end: reader.u64()?,
            starts_at_line_boundary: reader.boolean()?,
            ends_at_line_boundary: reader.boolean()?,
        };
        if unit.source_start != expected_start
            || unit.source_end <= unit.source_start
            || unit.source_end > text.len()
            || unit.source_end - unit.source_start > hard
            || !text.is_char_boundary(unit.source_start)
            || !text.is_char_boundary(unit.source_end)
        {
            return Err(PreparationError::InvalidArtifact);
        }
        let mut first = current.ok_or(PreparationError::InvalidArtifact)?;
        while first.end <= unit.source_start {
            first = lines
                .next(limits.max_lines, deadline, cancelled)?
                .ok_or(PreparationError::InvalidArtifact)?;
        }
        let mut last = first;
        while last.end < unit.source_end {
            last = lines
                .next(limits.max_lines, deadline, cancelled)?
                .ok_or(PreparationError::InvalidArtifact)?;
        }
        if unit.logical_line_start != first.index
            || Some(unit.logical_line_end) != last.index.checked_add(1)
            || unit.starts_at_line_boundary != (unit.source_start == first.start)
            || unit.ends_at_line_boundary != (unit.source_end == last.end)
        {
            return Err(PreparationError::InvalidArtifact);
        }
        current = Some(last);
        expected_start = unit.source_end;
        units.push(unit);
    }
    if expected_start != text.len() {
        return Err(PreparationError::InvalidArtifact);
    }
    Ok(units)
}

#[derive(Clone, Copy)]
struct Line {
    start: usize,
    end: usize,
    index: u64,
}

struct Lines<'a> {
    bytes: &'a [u8],
    offset: usize,
    index: u64,
}

impl Lines<'_> {
    fn next(
        &mut self,
        maximum: usize,
        deadline: Instant,
        cancelled: &AtomicBool,
    ) -> Result<Option<Line>, PreparationError> {
        check(deadline, cancelled)?;
        if self.offset == self.bytes.len() {
            return Ok(None);
        }
        if usize::try_from(self.index).map_err(|_| PreparationError::OffsetOverflow)? >= maximum {
            return Err(PreparationError::TooManyLines);
        }
        let start = self.offset;
        while self.offset < self.bytes.len() {
            if self.offset.is_multiple_of(1024) {
                check(deadline, cancelled)?;
            }
            let terminator = match self.bytes[self.offset] {
                b'\r' if self.bytes.get(self.offset + 1) == Some(&b'\n') => 2,
                b'\n' | b'\r' => 1,
                _ => {
                    self.offset += 1;
                    continue;
                }
            };
            self.offset += terminator;
            break;
        }
        let line = Line {
            start,
            end: self.offset,
            index: self.index,
        };
        self.index = self
            .index
            .checked_add(1)
            .ok_or(PreparationError::OffsetOverflow)?;
        Ok(Some(line))
    }
}

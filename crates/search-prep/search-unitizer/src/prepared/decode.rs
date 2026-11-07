//! Bounded saved-artifact acceptance without reconstructing expected units.

use search_contracts::Blake3Digest32;
use std::sync::atomic::AtomicBool;
use std::time::Instant;

use search_source_registry::{
    CurrentSourceRecord, MAX_SOURCE_OBJECT_BYTES, ObjectKind, ObjectRef, PreparationSettings,
};

use super::wire::{HEADER_BYTES, MAGIC, Reader, VERSION, encoded_size};
use super::{PreparationLimits, PreparedBinding, PreparedUtf8, check, verify_digest};
use crate::{PreparationError, UnitSpan};

pub(super) fn decode<'a>(
    artifact: &'a [u8],
    source: &CurrentSourceRecord,
    settings: PreparationSettings,
    limits: PreparationLimits,
    deadline: Instant,
    cancelled: &AtomicBool,
) -> Result<PreparedUtf8<'a>, PreparationError> {
    let mut reader = open_artifact(artifact, limits)?;
    let binding = read_binding(&mut reader, Some((source, settings)))?;
    content(artifact, reader, &binding, limits, deadline, cancelled)
}

pub(super) fn inspect<'a>(
    artifact: &'a [u8],
    limits: PreparationLimits,
    deadline: Instant,
    cancelled: &AtomicBool,
) -> Result<(PreparedBinding, PreparedUtf8<'a>), PreparationError> {
    let mut reader = open_artifact(artifact, limits)?;
    let binding = read_binding(&mut reader, None)?;
    if binding.registration_revision == 0 {
        return Err(PreparationError::BindingMismatch);
    }
    if !binding.settings.is_supported() {
        return Err(PreparationError::InvalidSettings);
    }
    let view = content(artifact, reader, &binding, limits, deadline, cancelled)?;
    Ok((binding, view))
}

const fn open_artifact(
    artifact: &[u8],
    limits: PreparationLimits,
) -> Result<Reader<'_>, PreparationError> {
    if artifact.len() > limits.max_encoded_bytes {
        return Err(PreparationError::EncodedTooLarge);
    }
    if artifact.len() < HEADER_BYTES {
        return Err(PreparationError::InvalidArtifact);
    }
    Ok(Reader::new(artifact))
}

fn content<'a>(
    artifact: &'a [u8],
    mut reader: Reader<'a>,
    binding: &PreparedBinding,
    limits: PreparationLimits,
    deadline: Instant,
    cancelled: &AtomicBool,
) -> Result<PreparedUtf8<'a>, PreparationError> {
    let text_length = reader.usize()?;
    let unit_count = reader.usize()?;
    if unit_count > limits.max_units {
        return Err(PreparationError::TooManyUnits);
    }
    limits.check_text_length(text_length)?;
    if u64::try_from(text_length).ok() != Some(binding.input.byte_length) {
        return Err(PreparationError::BindingMismatch);
    }
    if encoded_size(text_length, unit_count)? != artifact.len() {
        return Err(PreparationError::InvalidArtifact);
    }
    check(deadline, cancelled)?;
    let text_bytes = reader.take(text_length)?;
    let text = std::str::from_utf8(text_bytes).map_err(|_| PreparationError::InvalidArtifact)?;
    verify_digest(binding.input, text_bytes, deadline, cancelled)?;
    let units = read_units(
        &mut reader,
        text,
        unit_count,
        binding.settings,
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
    expected: Option<(&CurrentSourceRecord, PreparationSettings)>,
) -> Result<PreparedBinding, PreparationError> {
    if reader.take(MAGIC.len())? != MAGIC || reader.u16()? != VERSION {
        return Err(PreparationError::InvalidArtifact);
    }
    let source_namespace_id: [u8; 16] = reader
        .take(16)?
        .try_into()
        .map_err(|_| PreparationError::InvalidArtifact)?;
    if expected
        .is_some_and(|(source, _)| source_namespace_id != *source.source_namespace_id.as_bytes())
    {
        return Err(PreparationError::BindingMismatch);
    }
    let source_id: [u8; 16] = reader
        .take(16)?
        .try_into()
        .map_err(|_| PreparationError::InvalidArtifact)?;
    if expected.is_some_and(|(source, _)| source_id != *source.source_id.as_bytes()) {
        return Err(PreparationError::BindingMismatch);
    }
    let registration_revision = reader.u64()?;
    if expected.is_some_and(|(source, _)| registration_revision != source.registration_revision) {
        return Err(PreparationError::BindingMismatch);
    }
    let input = read_input(reader, expected.map(|(source, _)| source.object_ref))?;
    let settings = read_settings(reader, expected.map(|(_, settings)| settings))?;
    Ok(PreparedBinding {
        source_namespace_id,
        source_id,
        registration_revision,
        input,
        settings,
    })
}

fn read_input(
    reader: &mut Reader<'_>,
    expected: Option<ObjectRef>,
) -> Result<ObjectRef, PreparationError> {
    let format_version = reader.u16()?;
    if expected.is_some_and(|input| format_version != input.format_version) {
        return Err(PreparationError::BindingMismatch);
    }
    let kind = reader.take(1)?[0];
    if expected.is_some_and(|input| kind != input.kind.tag()) {
        return Err(PreparationError::BindingMismatch);
    }
    let kind = ObjectKind::from_tag(kind).ok_or(PreparationError::BindingMismatch)?;
    let digest: [u8; 32] = reader
        .take(32)?
        .try_into()
        .map_err(|_| PreparationError::InvalidArtifact)?;
    if expected.is_some_and(|input| &digest != input.digest.as_bytes()) {
        return Err(PreparationError::BindingMismatch);
    }
    let byte_length = reader.u64()?;
    if expected.is_some_and(|input| byte_length != input.byte_length) {
        return Err(PreparationError::BindingMismatch);
    }
    let input = ObjectRef {
        format_version,
        kind,
        digest: Blake3Digest32::from_bytes(digest),
        byte_length,
    };
    if input.kind != ObjectKind::Source
        || input.format_version != 1
        || input.byte_length > MAX_SOURCE_OBJECT_BYTES
    {
        return Err(PreparationError::BindingMismatch);
    }
    Ok(input)
}

fn read_settings(
    reader: &mut Reader<'_>,
    expected: Option<PreparationSettings>,
) -> Result<PreparationSettings, PreparationError> {
    let utf8_version = reader.u16()?;
    if expected.is_some_and(|settings| utf8_version != settings.utf8_version) {
        return Err(PreparationError::BindingMismatch);
    }
    let line_unitizer_version = reader.u16()?;
    if expected.is_some_and(|settings| line_unitizer_version != settings.line_unitizer_version) {
        return Err(PreparationError::BindingMismatch);
    }
    let preferred_unit_bytes = reader.u32()?;
    if expected.is_some_and(|settings| preferred_unit_bytes != settings.preferred_unit_bytes) {
        return Err(PreparationError::BindingMismatch);
    }
    let max_unit_bytes = reader.u32()?;
    if expected.is_some_and(|settings| max_unit_bytes != settings.max_unit_bytes) {
        return Err(PreparationError::BindingMismatch);
    }
    Ok(PreparationSettings {
        utf8_version,
        line_unitizer_version,
        preferred_unit_bytes,
        max_unit_bytes,
    })
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

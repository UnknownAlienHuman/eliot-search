//! Bounded UTF-8/UTF-16 decoding with native coordinate evidence.

use crate::profile::{LossBehavior, SourceEncoding, ValidatedMaterializerProfile};
use crate::request::{CancellationToken, MaterializationBudget};
use crate::{LineEnding, MaterializationError};

use super::model::{DecodedLine, DecodedRepresentation, EncodingDecision, StepCounter};

struct LineAccumulator {
    lines: Vec<DecodedLine>,
    start_native: u64,
    start_decoded: u64,
    max_lines: u64,
}

impl LineAccumulator {
    const fn new(max_lines: u64, base_native: u64) -> Self {
        Self {
            lines: Vec::new(),
            start_native: base_native,
            start_decoded: 0,
            max_lines,
        }
    }

    fn push_terminated(
        &mut self,
        content_native: u64,
        end_native: u64,
        content_decoded: u64,
        end_decoded: u64,
        ending: LineEnding,
    ) -> Result<(), MaterializationError> {
        if u64::try_from(self.lines.len()).map_err(|_| MaterializationError::OffsetOverflow)?
            >= self.max_lines
        {
            return Err(MaterializationError::BudgetExhausted);
        }
        self.lines.push(DecodedLine {
            native_start: self.start_native,
            native_end: end_native,
            native_content_end: content_native,
            decoded_start: self.start_decoded,
            decoded_end: end_decoded,
            decoded_content_end: content_decoded,
            ending,
        });
        self.start_native = end_native;
        self.start_decoded = end_decoded;
        Ok(())
    }

    fn finish(&mut self, end_native: u64, end_decoded: u64) -> Result<(), MaterializationError> {
        if self.start_decoded < end_decoded {
            if u64::try_from(self.lines.len()).map_err(|_| MaterializationError::OffsetOverflow)?
                >= self.max_lines
            {
                return Err(MaterializationError::BudgetExhausted);
            }
            self.lines.push(DecodedLine {
                native_start: self.start_native,
                native_end: end_native,
                native_content_end: end_native,
                decoded_start: self.start_decoded,
                decoded_end: end_decoded,
                decoded_content_end: end_decoded,
                ending: LineEnding::None,
            });
        }
        Ok(())
    }
}

fn reject_binary_controls(text: &str) -> Result<(), MaterializationError> {
    let bytes = text.as_bytes();
    if bytes.contains(&0) {
        return Err(MaterializationError::BinaryContent);
    }
    let disallowed = bytes
        .iter()
        .filter(|byte| **byte < 0x20 && !matches!(**byte, b'\t' | b'\n' | b'\r' | 0x0c))
        .count();
    let threshold = bytes.len().div_ceil(100).max(4);
    if disallowed >= threshold {
        return Err(MaterializationError::BinaryContent);
    }
    Ok(())
}

fn checked_add(first: u64, second: u64) -> Result<u64, MaterializationError> {
    first
        .checked_add(second)
        .ok_or(MaterializationError::OffsetOverflow)
}

/// Decodes retained bytes into bounded scalar/text units.
///
/// Replacement is never invisible: `BOM` stripping and UTF-16 transcoding set
/// explicit flags consumed by map construction, malformed or truncated input
/// fails with [`MaterializationError::InvalidSequence`], and a profile that
/// forbids loss fails with [`MaterializationError::Loss`]. Elementary steps
/// accumulate into the shared step counter so one operation-wide bound holds;
/// callers initialize the counter with the effective step bound.
pub fn decode_text_or_code(
    bytes: &[u8],
    decision: &EncodingDecision,
    profile: &ValidatedMaterializerProfile,
    budget: &MaterializationBudget,
    steps: &mut StepCounter,
    cancel: CancellationToken<'_>,
) -> Result<DecodedRepresentation, MaterializationError> {
    if cancel.is_cancelled() {
        return Err(MaterializationError::Cancelled);
    }
    let budget = budget.validate()?;
    if bytes.is_empty() {
        return Err(MaterializationError::EmptyInput);
    }
    let native_len =
        u64::try_from(bytes.len()).map_err(|_| MaterializationError::OffsetOverflow)?;
    let max_input = budget.effective_input(profile.limits().max_input_bytes);
    if native_len > max_input {
        return Err(MaterializationError::BudgetExhausted);
    }
    let max_lines = budget.effective_lines(profile.limits().max_lines);
    let bom_len = u64::try_from(decision.bom_len_bytes())
        .map_err(|_| MaterializationError::OffsetOverflow)?;
    let (text, lines, transcoded) = match decision.encoding() {
        SourceEncoding::Utf8 => {
            let Some(payload) = bytes.get(decision.bom_len_bytes()..) else {
                return Err(MaterializationError::InvalidSequence);
            };
            let text =
                core::str::from_utf8(payload).map_err(|_| MaterializationError::InvalidSequence)?;
            let lines = scan_utf8_lines(text, bom_len, max_lines, &mut *steps, cancel)?;
            (text.to_string(), lines, false)
        }
        SourceEncoding::Utf16Le | SourceEncoding::Utf16Be => {
            let little = decision.encoding() == SourceEncoding::Utf16Le;
            let Some(payload) = bytes.get(decision.bom_len_bytes()..) else {
                return Err(MaterializationError::InvalidSequence);
            };
            decode_utf16_units(payload, bom_len, little, max_lines, &mut *steps, cancel)?
        }
    };
    reject_binary_controls(&text)?;
    let decoded_len_chars =
        u64::try_from(text.chars().count()).map_err(|_| MaterializationError::OffsetOverflow)?;
    let bom_stripped = decision.bom_present();
    if profile.loss_behavior() == LossBehavior::RejectOnAnyLoss && (bom_stripped || transcoded) {
        return Err(MaterializationError::Loss);
    }
    Ok(DecodedRepresentation {
        text,
        lines,
        encoding: decision.encoding(),
        bom_stripped,
        bom_len_bytes: bom_len,
        transcoded,
        native_len,
        decoded_len_chars,
        profile_id: profile.id(),
    })
}

fn scan_utf8_lines(
    text: &str,
    base_native: u64,
    max_lines: u64,
    steps: &mut StepCounter,
    cancel: CancellationToken<'_>,
) -> Result<Vec<DecodedLine>, MaterializationError> {
    let bytes = text.as_bytes();
    let mut lines = LineAccumulator::new(max_lines, base_native);
    let mut char_index = 0_u64;
    let mut index = 0_usize;
    while index < bytes.len() {
        let Some(byte) = bytes.get(index) else {
            return Err(MaterializationError::InvalidSequence);
        };
        let byte = *byte;
        if byte == b'\n' || byte == b'\r' {
            let (ending, term_bytes) =
                if byte == b'\r' && bytes.get(index + 1).is_some_and(|next| *next == b'\n') {
                    (LineEnding::CrLf, 2_u64)
                } else if byte == b'\n' {
                    (LineEnding::Lf, 1_u64)
                } else {
                    (LineEnding::Cr, 1_u64)
                };
            let content_native = checked_add(
                base_native,
                u64::try_from(index).map_err(|_| MaterializationError::OffsetOverflow)?,
            )?;
            let end_native = checked_add(content_native, term_bytes)?;
            let end_decoded = checked_add(char_index, term_bytes)?;
            lines.push_terminated(content_native, end_native, char_index, end_decoded, ending)?;
            if cancel.is_cancelled() {
                return Err(MaterializationError::Cancelled);
            }
            let advance =
                usize::try_from(term_bytes).map_err(|_| MaterializationError::OffsetOverflow)?;
            index += advance;
            char_index = end_decoded;
            steps.consume(term_bytes)?;
            continue;
        }
        if byte & 0xC0 != 0x80 {
            char_index = checked_add(char_index, 1)?;
        }
        index += 1;
        if index.is_multiple_of(4096) {
            steps.consume(4096)?;
            if cancel.is_cancelled() {
                return Err(MaterializationError::Cancelled);
            }
        }
    }
    let end_native = checked_add(
        base_native,
        u64::try_from(bytes.len()).map_err(|_| MaterializationError::OffsetOverflow)?,
    )?;
    lines.finish(end_native, char_index)?;
    steps.consume(1)?;
    Ok(lines.lines)
}

fn decode_utf16_units(
    payload: &[u8],
    base_native: u64,
    little_endian: bool,
    max_lines: u64,
    steps: &mut StepCounter,
    cancel: CancellationToken<'_>,
) -> Result<(String, Vec<DecodedLine>, bool), MaterializationError> {
    if !payload.is_empty() && !payload.len().is_multiple_of(2) {
        return Err(MaterializationError::InvalidSequence);
    }
    let mut text = String::with_capacity(payload.len() / 2 + 1);
    let mut native_of_char: Vec<u64> = Vec::with_capacity(payload.len() / 2 + 1);
    native_of_char.push(base_native);
    let mut pending_high: Option<u16> = None;
    let mut native_pos = base_native;
    let mut word_count = 0_usize;
    let (words, remainder) = payload.as_chunks::<2>();
    if !remainder.is_empty() {
        return Err(MaterializationError::InvalidSequence);
    }
    for pair in words {
        let [first, second] = *pair;
        let unit = if little_endian {
            u16::from_le_bytes([first, second])
        } else {
            u16::from_be_bytes([first, second])
        };
        native_pos = checked_add(native_pos, 2)?;
        word_count += 1;
        if word_count.is_multiple_of(2048) {
            steps.consume(2048)?;
            if cancel.is_cancelled() {
                return Err(MaterializationError::Cancelled);
            }
        }
        if (0xD800..0xDC00).contains(&unit) {
            if pending_high.is_some() {
                return Err(MaterializationError::InvalidSequence);
            }
            pending_high = Some(unit);
            continue;
        }
        if (0xDC00..0xE000).contains(&unit) {
            let Some(high) = pending_high.take() else {
                return Err(MaterializationError::InvalidSequence);
            };
            let scalar = 0x1_0000 + (u32::from(high - 0xD800) << 10) + u32::from(unit - 0xDC00);
            let Some(value) = char::from_u32(scalar) else {
                return Err(MaterializationError::InvalidSequence);
            };
            text.push(value);
            native_of_char.push(native_pos);
            steps.consume(1)?;
            continue;
        }
        if pending_high.is_some() {
            return Err(MaterializationError::InvalidSequence);
        }
        let Some(value) = char::from_u32(u32::from(unit)) else {
            return Err(MaterializationError::InvalidSequence);
        };
        text.push(value);
        native_of_char.push(native_pos);
        steps.consume(1)?;
    }
    if pending_high.is_some() {
        return Err(MaterializationError::InvalidSequence);
    }
    let lines = split_transcoded_lines(&text, &native_of_char, base_native, max_lines, cancel)?;
    steps.consume(1)?;
    Ok((text, lines, true))
}

fn native_at(offsets: &[u64], index: u64) -> Result<u64, MaterializationError> {
    let position = usize::try_from(index).map_err(|_| MaterializationError::OffsetOverflow)?;
    offsets
        .get(position)
        .copied()
        .ok_or(MaterializationError::OffsetOverflow)
}

fn split_transcoded_lines(
    text: &str,
    native_of_char: &[u64],
    base_native: u64,
    max_lines: u64,
    cancel: CancellationToken<'_>,
) -> Result<Vec<DecodedLine>, MaterializationError> {
    let mut lines = LineAccumulator::new(max_lines, base_native);
    let mut chars: Vec<(u64, char)> = Vec::new();
    for (position, value) in text.chars().enumerate() {
        let index = u64::try_from(position).map_err(|_| MaterializationError::OffsetOverflow)?;
        chars.push((index, value));
    }
    let mut cursor = 0_usize;
    while cursor < chars.len() {
        let (index, value) = chars[cursor];
        let terminator = if value == '\n' {
            Some((LineEnding::Lf, 1_u64))
        } else if value == '\r' && chars.get(cursor + 1).is_some_and(|next| next.1 == '\n') {
            Some((LineEnding::CrLf, 2_u64))
        } else if value == '\r' {
            Some((LineEnding::Cr, 1_u64))
        } else {
            None
        };
        let Some((ending, term_chars)) = terminator else {
            cursor += 1;
            continue;
        };
        let end_char = checked_add(index, term_chars)?;
        lines.push_terminated(
            native_at(native_of_char, index)?,
            native_at(native_of_char, end_char)?,
            index,
            end_char,
            ending,
        )?;
        if cancel.is_cancelled() {
            return Err(MaterializationError::Cancelled);
        }
        let advance =
            usize::try_from(term_chars).map_err(|_| MaterializationError::OffsetOverflow)?;
        cursor += advance;
    }
    let total = u64::try_from(chars.len()).map_err(|_| MaterializationError::OffsetOverflow)?;
    lines.finish(native_at(native_of_char, total)?, total)?;
    Ok(lines.lines)
}

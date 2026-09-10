//! Baseline decoding: encoding decision and text/code decoding.
//!
//! Decoding uses only the accepted profile's explicit BOM, declared-encoding
//! and UTF-validity rules. It never guesses from locale, never executes
//! content and never replaces malformed sequences silently: malformed or
//! truncated input is a typed error, and any recorded BOM/transcoding fact
//! travels with the representation for map and assurance construction.

use crate::profile::{
    BomPolicy, LossBehavior, MaterializerProfileId, SourceEncoding, ValidatedMaterializerProfile,
};
use crate::request::{CancellationToken, MaterializationBudget};
use crate::{LineEnding, MaterializationError};

const UTF8_BOM: &[u8] = &[0xEF, 0xBB, 0xBF];
const UTF16LE_BOM: &[u8] = &[0xFF, 0xFE];
const UTF16BE_BOM: &[u8] = &[0xFE, 0xFF];

/// Bounded elementary-step accounting shared by decode, normalize and map
/// stages of one materialization operation.
///
/// One counter travels through every stage so a single operation-wide step
/// bound holds; exhaustion fails with
/// [`MaterializationError::BudgetExhausted`](crate::MaterializationError),
/// never with a truncated product.
pub struct StepCounter {
    used: u64,
    max: u64,
}

impl StepCounter {
    /// Starts an operation step budget.
    #[must_use]
    pub const fn new(max: u64) -> Self {
        Self { used: 0, max }
    }

    /// Consumes steps, failing closed on overflow or budget exhaustion.
    pub fn consume(&mut self, steps: u64) -> Result<(), MaterializationError> {
        self.used = self
            .used
            .checked_add(steps)
            .ok_or(MaterializationError::BudgetExhausted)?;
        if self.used > self.max {
            return Err(MaterializationError::BudgetExhausted);
        }
        Ok(())
    }

    /// Steps consumed so far.
    #[must_use]
    pub const fn used(&self) -> u64 {
        self.used
    }
}

/// Decided encoding with explicit BOM evidence.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct EncodingDecision {
    encoding: SourceEncoding,
    bom_present: bool,
    bom_len_bytes: usize,
}

impl EncodingDecision {
    /// Decided source encoding (always the declared, validated one).
    #[must_use]
    pub const fn encoding(&self) -> SourceEncoding {
        self.encoding
    }

    /// Whether a byte-order mark was observed.
    #[must_use]
    pub const fn bom_present(&self) -> bool {
        self.bom_present
    }

    /// Observed BOM length in native bytes (zero when absent).
    #[must_use]
    pub const fn bom_len_bytes(&self) -> usize {
        self.bom_len_bytes
    }
}

/// Decides the encoding from the declared hint, the profile and BOM evidence.
///
/// A byte prefix that admits another encoding than the declared one is
/// [`MaterializationError::EncodingAmbiguous`]; a declared encoding outside
/// the profile set is [`MaterializationError::EncodingUnsupported`].
pub fn detect_or_validate_encoding(
    bytes: &[u8],
    declared: SourceEncoding,
    profile: &ValidatedMaterializerProfile,
) -> Result<EncodingDecision, MaterializationError> {
    if !profile.encodings().contains(&declared) {
        return Err(MaterializationError::EncodingUnsupported);
    }
    let sniffed = if bytes.starts_with(UTF8_BOM) {
        Some((SourceEncoding::Utf8, UTF8_BOM.len()))
    } else if bytes.starts_with(UTF16LE_BOM) {
        Some((SourceEncoding::Utf16Le, UTF16LE_BOM.len()))
    } else if bytes.starts_with(UTF16BE_BOM) {
        Some((SourceEncoding::Utf16Be, UTF16BE_BOM.len()))
    } else {
        None
    };
    let (encoding, bom_len_bytes) = match (declared, sniffed) {
        (SourceEncoding::Utf8, None) => (SourceEncoding::Utf8, 0),
        (SourceEncoding::Utf8, Some((SourceEncoding::Utf8, len))) => (SourceEncoding::Utf8, len),
        (SourceEncoding::Utf16Le, None) => (SourceEncoding::Utf16Le, 0),
        (SourceEncoding::Utf16Le, Some((SourceEncoding::Utf16Le, len))) => {
            (SourceEncoding::Utf16Le, len)
        }
        (SourceEncoding::Utf16Be, None) => (SourceEncoding::Utf16Be, 0),
        (SourceEncoding::Utf16Be, Some((SourceEncoding::Utf16Be, len))) => {
            (SourceEncoding::Utf16Be, len)
        }
        _ => return Err(MaterializationError::EncodingAmbiguous),
    };
    let bom_present = bom_len_bytes > 0;
    if bom_present && profile.bom_policy() == BomPolicy::RejectWhenPresent {
        return Err(MaterializationError::Unsupported);
    }
    Ok(EncodingDecision {
        encoding,
        bom_present,
        bom_len_bytes,
    })
}

/// One decoded line with native byte and decoded scalar coordinates.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct DecodedLine {
    /// Inclusive native byte start.
    pub native_start: u64,
    /// Exclusive native byte end including the terminator.
    pub native_end: u64,
    /// Exclusive native byte end excluding the terminator.
    pub native_content_end: u64,
    /// Inclusive decoded scalar start.
    pub decoded_start: u64,
    /// Exclusive decoded scalar end including the terminator.
    pub decoded_end: u64,
    /// Exclusive decoded scalar end excluding the terminator.
    pub decoded_content_end: u64,
    /// Exact retained line ending.
    pub ending: LineEnding,
}

/// Bounded canonical scalar/text units with native coordinate evidence.
#[derive(Clone, Eq, PartialEq)]
pub struct DecodedRepresentation {
    text: String,
    lines: Vec<DecodedLine>,
    encoding: SourceEncoding,
    bom_stripped: bool,
    bom_len_bytes: u64,
    transcoded: bool,
    native_len: u64,
    decoded_len_chars: u64,
    profile_id: MaterializerProfileId,
}

impl DecodedRepresentation {
    /// Decoded text (BOM removed, encoding transcoded, newlines untouched).
    #[must_use]
    pub fn text(&self) -> &str {
        &self.text
    }

    /// Decoded line table in native-byte and scalar coordinates.
    #[must_use]
    pub fn lines(&self) -> &[DecodedLine] {
        &self.lines
    }

    /// Decided source encoding.
    #[must_use]
    pub const fn encoding(&self) -> SourceEncoding {
        self.encoding
    }

    /// Whether a BOM was stripped (always recorded in the loss map).
    #[must_use]
    pub const fn bom_stripped(&self) -> bool {
        self.bom_stripped
    }

    /// Stripped BOM length in native bytes.
    #[must_use]
    pub const fn bom_len_bytes(&self) -> u64 {
        self.bom_len_bytes
    }

    /// Whether bytes were transcoded from UTF-16 (always recorded).
    #[must_use]
    pub const fn transcoded(&self) -> bool {
        self.transcoded
    }

    /// Exact native input length in bytes.
    #[must_use]
    pub const fn native_len(&self) -> u64 {
        self.native_len
    }

    /// Decoded length in Unicode scalar values.
    #[must_use]
    pub const fn decoded_len_chars(&self) -> u64 {
        self.decoded_len_chars
    }

    /// Profile identity this decoding was performed under.
    #[must_use]
    pub const fn profile_id(&self) -> MaterializerProfileId {
        self.profile_id
    }
}

impl core::fmt::Debug for DecodedRepresentation {
    fn fmt(&self, formatter: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        formatter
            .debug_struct("DecodedRepresentation")
            .field("text", &format_args!("<{} chars>", self.decoded_len_chars))
            .field("line_count", &self.lines.len())
            .field("encoding", &self.encoding)
            .field("bom_stripped", &self.bom_stripped)
            .field("transcoded", &self.transcoded)
            .field("native_len", &self.native_len)
            .field("profile_id", &self.profile_id)
            .finish_non_exhaustive()
    }
}

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
    let bom_len =
        u64::try_from(decision.bom_len_bytes).map_err(|_| MaterializationError::OffsetOverflow)?;
    let (text, lines, transcoded) = match decision.encoding {
        SourceEncoding::Utf8 => {
            let Some(payload) = bytes.get(decision.bom_len_bytes..) else {
                return Err(MaterializationError::InvalidSequence);
            };
            let text =
                core::str::from_utf8(payload).map_err(|_| MaterializationError::InvalidSequence)?;
            let lines = scan_utf8_lines(text, bom_len, max_lines, &mut *steps, cancel)?;
            (text.to_string(), lines, false)
        }
        SourceEncoding::Utf16Le | SourceEncoding::Utf16Be => {
            let little = decision.encoding == SourceEncoding::Utf16Le;
            let Some(payload) = bytes.get(decision.bom_len_bytes..) else {
                return Err(MaterializationError::InvalidSequence);
            };
            decode_utf16_units(payload, bom_len, little, max_lines, &mut *steps, cancel)?
        }
    };
    reject_binary_controls(&text)?;
    let decoded_len_chars =
        u64::try_from(text.chars().count()).map_err(|_| MaterializationError::OffsetOverflow)?;
    let bom_stripped = decision.bom_present;
    if profile.loss_behavior() == LossBehavior::RejectOnAnyLoss && (bom_stripped || transcoded) {
        return Err(MaterializationError::Loss);
    }
    Ok(DecodedRepresentation {
        text,
        lines,
        encoding: decision.encoding,
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
    // Native byte offset recorded at every decoded scalar boundary.
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

#[cfg(test)]
mod tests {
    use super::*;
    use crate::profile::{baseline_profile_descriptor, validate_materializer_profile};
    use crate::request::DEFAULT_MATERIALIZATION_BUDGET;

    fn profile() -> ValidatedMaterializerProfile {
        validate_materializer_profile(&baseline_profile_descriptor("decode-test", 1))
            .expect("profile")
    }

    fn decide(bytes: &[u8], encoding: SourceEncoding) -> EncodingDecision {
        detect_or_validate_encoding(bytes, encoding, &profile()).expect("decision")
    }

    fn decode(bytes: &[u8], encoding: SourceEncoding) -> DecodedRepresentation {
        let decision = decide(bytes, encoding);
        decode_text_or_code(
            bytes,
            &decision,
            &profile(),
            &MaterializationBudget {
                max_input_bytes: 1024,
                max_output_bytes: 1024,
                max_lines: 64,
                max_map_segments: 128,
                max_loss_records: 128,
                max_steps: 1 << 20,
            },
            &mut StepCounter::new(1 << 20),
            CancellationToken::never(),
        )
        .expect("decode")
    }

    #[test]
    fn utf8_golden_decodes_exactly() {
        let decoded = decode("hello\r\nworld\nγ".as_bytes(), SourceEncoding::Utf8);
        assert_eq!(decoded.text(), "hello\r\nworld\nγ");
        assert!(!decoded.bom_stripped());
        assert!(!decoded.transcoded());
        assert_eq!(decoded.lines().len(), 3);
        assert_eq!(decoded.lines()[0].ending, LineEnding::CrLf);
        assert_eq!(
            (
                decoded.lines()[0].native_start,
                decoded.lines()[0].native_end
            ),
            (0, 7)
        );
        // Non-ASCII scalar coordinates differ from byte offsets by construction.
        // "hello\r\n" is 7 scalars, "world\n" 6 scalars, so "γ" starts at 13.
        assert_eq!(
            (
                decoded.lines()[2].decoded_start,
                decoded.lines()[2].decoded_end
            ),
            (13, 14)
        );
        assert_eq!(
            (
                decoded.lines()[2].native_start,
                decoded.lines()[2].native_end
            ),
            (13, 15)
        );
    }

    #[test]
    fn utf8_bom_is_recorded_not_silent() {
        let mut bytes = UTF8_BOM.to_vec();
        bytes.extend_from_slice(b"abc\n");
        let decoded = decode(&bytes, SourceEncoding::Utf8);
        assert!(decoded.bom_stripped());
        assert_eq!(decoded.bom_len_bytes(), 3);
        assert_eq!(decoded.text(), "abc\n");
        assert_eq!(decoded.lines()[0].native_start, 3);
    }

    #[test]
    fn utf16le_transcodes_with_native_evidence() {
        let text = "Aπ\n";
        let mut bytes = Vec::new();
        for unit in text.encode_utf16() {
            bytes.extend_from_slice(&unit.to_le_bytes());
        }
        let decoded = decode(&bytes, SourceEncoding::Utf16Le);
        assert_eq!(decoded.text(), text);
        assert!(decoded.transcoded());
        assert_eq!(decoded.lines().len(), 1);
        assert_eq!(
            (
                decoded.lines()[0].native_start,
                decoded.lines()[0].native_end
            ),
            (0, 6)
        );
        assert_eq!(
            (
                decoded.lines()[0].decoded_start,
                decoded.lines()[0].decoded_end
            ),
            (0, 3)
        );
    }

    #[test]
    fn utf16be_with_bom_decodes() {
        let mut bytes = UTF16BE_BOM.to_vec();
        for unit in "Hi\r\n".encode_utf16() {
            bytes.extend_from_slice(&unit.to_be_bytes());
        }
        let decoded = decode(&bytes, SourceEncoding::Utf16Be);
        assert_eq!(decoded.text(), "Hi\r\n");
        assert!(decoded.bom_stripped());
        assert_eq!(decoded.lines()[0].ending, LineEnding::CrLf);
    }

    #[test]
    fn mismatched_bom_is_ambiguous() {
        let mut bytes = UTF16LE_BOM.to_vec();
        bytes.extend_from_slice(b"abc");
        assert_eq!(
            detect_or_validate_encoding(&bytes, SourceEncoding::Utf8, &profile()),
            Err(MaterializationError::EncodingAmbiguous)
        );
        assert_eq!(
            detect_or_validate_encoding(&bytes, SourceEncoding::Utf16Be, &profile()),
            Err(MaterializationError::EncodingAmbiguous)
        );
    }

    #[test]
    fn undeclared_encoding_is_unsupported() {
        let mut narrow = baseline_profile_descriptor("narrow-decode", 1);
        narrow.encodings = vec![SourceEncoding::Utf8];
        let narrow = validate_materializer_profile(&narrow).expect("narrow");
        assert_eq!(
            detect_or_validate_encoding(b"abc", SourceEncoding::Utf16Le, &narrow),
            Err(MaterializationError::EncodingUnsupported)
        );
    }

    #[test]
    fn malformed_and_truncated_inputs_are_typed_errors() {
        let decision = decide(&[0xFF, 0x41], SourceEncoding::Utf8);
        assert_eq!(
            decode_text_or_code(
                &[0xFF, 0x41],
                &decision,
                &profile(),
                &DEFAULT_MATERIALIZATION_BUDGET,
                &mut StepCounter::new(1 << 20),
                CancellationToken::never()
            ),
            Err(MaterializationError::InvalidSequence)
        );
        // Odd trailing UTF-16 byte is truncation, not content.
        let units = "AB".encode_utf16().collect::<Vec<u16>>();
        let mut bytes = Vec::new();
        for unit in units {
            bytes.extend_from_slice(&unit.to_le_bytes());
        }
        bytes.push(0x41);
        let decision = decide(&bytes, SourceEncoding::Utf16Le);
        assert_eq!(
            decode_text_or_code(
                &bytes,
                &decision,
                &profile(),
                &DEFAULT_MATERIALIZATION_BUDGET,
                &mut StepCounter::new(1 << 20),
                CancellationToken::never()
            ),
            Err(MaterializationError::InvalidSequence)
        );
        // Lone surrogate halves never decode.
        let lone = [0x3D, 0xD8, 0x41, 0x00];
        let decision = decide(&lone, SourceEncoding::Utf16Le);
        assert_eq!(
            decode_text_or_code(
                &lone,
                &decision,
                &profile(),
                &DEFAULT_MATERIALIZATION_BUDGET,
                &mut StepCounter::new(1 << 20),
                CancellationToken::never()
            ),
            Err(MaterializationError::InvalidSequence)
        );
    }

    #[test]
    fn strict_loss_profile_rejects_transcoding() {
        let mut strict = baseline_profile_descriptor("strict-decode", 1);
        strict.loss_behavior = LossBehavior::RejectOnAnyLoss;
        let strict = validate_materializer_profile(&strict).expect("strict");
        let text = "A\n";
        let mut bytes = Vec::new();
        for unit in text.encode_utf16() {
            bytes.extend_from_slice(&unit.to_le_bytes());
        }
        let decision = detect_or_validate_encoding(&bytes, SourceEncoding::Utf16Le, &strict)
            .expect("decision");
        assert_eq!(
            decode_text_or_code(
                &bytes,
                &decision,
                &strict,
                &DEFAULT_MATERIALIZATION_BUDGET,
                &mut StepCounter::new(1 << 20),
                CancellationToken::never()
            ),
            Err(MaterializationError::Loss)
        );
    }

    #[test]
    fn tiny_step_budget_is_exhausted_not_truncated() {
        let decision = decide(b"aaaa\nbbbb\n", SourceEncoding::Utf8);
        let budget = MaterializationBudget {
            max_input_bytes: 64,
            max_output_bytes: 64,
            max_lines: 64,
            max_map_segments: 64,
            max_loss_records: 64,
            max_steps: 2,
        };
        assert_eq!(
            decode_text_or_code(
                b"aaaa\nbbbb\n",
                &decision,
                &profile(),
                &budget,
                &mut StepCounter::new(2),
                CancellationToken::never()
            ),
            Err(MaterializationError::BudgetExhausted)
        );
    }

    #[test]
    fn debug_never_leaks_decoded_text() {
        let decoded = decode(b"private-bytes\n", SourceEncoding::Utf8);
        assert!(!format!("{decoded:?}").contains("private-bytes"));
    }
}

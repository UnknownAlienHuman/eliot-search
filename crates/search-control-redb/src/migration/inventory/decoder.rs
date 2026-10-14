//! Strict bounded decoder for the H,I,E,T,F legacy inventory profile.
//!
//! The decoder owns no accounting of its own: every accepted row must
//! re-encode byte-for-byte through the existing parent encoder. Header
//! and terminal bytes are compared against a re-encoded expectation, the
//! footer is compared against the encoder finish output and carries the
//! encoder-derived summary, and only items and source events carry
//! parsed fields. Refusal is sticky.

use super::{
    LegacyInventoryBinding, LegacyInventoryEncoder, LegacyInventoryError,
    LegacyInventoryFamily, LegacyInventoryLimits, LegacyInventoryStatus,
    LegacyInventorySummary,
};
use crate::migration::{
    SourceImportRow, SourceLifecycleFlags, MAX_SOURCE_IMPORT_ROW_BYTES,
};
use search_contracts::{Sha256Digest32, SourceId, SourceRevisionId};

/// One decoded inventory row. Item and terminal values are exact wire fields.
///
/// The footer carries the full encoder-derived summary inline, matching the
/// frozen profile ABI; the size difference is accepted by design.
#[expect(
    clippy::large_enum_variant,
    reason = "The frozen decoder ABI carries one bounded fixed-size footer summary inline."
)]
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum LegacyInventoryDecodedRow {
    /// Canonical profile header; identities retain their declared digests.
    Header,
    /// Bounded content-free reference with measured counts.
    Item {
        /// Closed family domain.
        family: LegacyInventoryFamily,
        /// Closed disposition domain.
        status: LegacyInventoryStatus,
        /// Profile-frozen numeric kind.
        kind: u16,
        /// Three content-free digests.
        refs: [Sha256Digest32; 3],
        /// Three measured counts.
        counts: [u64; 3],
    },
    /// One accepted source event.
    SourceEvent(SourceImportRow),
    /// Family terminal derived from the parent encoder state.
    Terminal {
        /// Closed family domain.
        family: LegacyInventoryFamily,
        /// Dominant terminal disposition.
        status: LegacyInventoryStatus,
        /// Fully observed family denominator.
        denominator_complete: bool,
        /// Per-status counts for the ended family.
        counts: [u64; 7],
    },
    /// Stream footer carrying the exact encoder-derived summary.
    Footer(LegacyInventorySummary),
}

/// One bounded stream decoder. Construction performs no row allocation.
pub struct LegacyInventoryDecoder {
    encoder: Option<LegacyInventoryEncoder>,
    binding: Option<LegacyInventoryBinding>,
    footer: Option<LegacyInventorySummary>,
    refused: bool,
}

impl LegacyInventoryDecoder {
    /// Starts fixed-size accounting under validated component limits.
    /// # Errors
    /// Returns Limit for rejected component limits.
    pub fn new(
        expected_binding: LegacyInventoryBinding,
        limits: LegacyInventoryLimits,
    ) -> Result<Self, LegacyInventoryError> {
        let encoder = LegacyInventoryEncoder::new(limits)?;
        Ok(Self {
            encoder: Some(encoder),
            binding: Some(expected_binding),
            footer: None,
            refused: false,
        })
    }

    /// Decodes one bounded wire row. The row length is checked before any
    /// allocation, exactly one trailing newline is mandatory, and every
    /// accepted row re-encodes through the parent encoder for an exact
    /// byte comparison.
    /// # Errors
    /// Returns a closed refusal for any noncanonical, out-of-order,
    /// duplicated, truncated, oversized, missing-footer or trailing row.
    pub fn push(
        &mut self,
        row: &[u8],
    ) -> Result<LegacyInventoryDecodedRow, LegacyInventoryError> {
        if self.refused {
            return Err(LegacyInventoryError::Accounting);
        }
        if self.footer.is_some() {
            return self.refuse(LegacyInventoryError::Accounting);
        }
        if row.is_empty() || row.len() > MAX_SOURCE_IMPORT_ROW_BYTES {
            return self.refuse(LegacyInventoryError::Limit);
        }
        let Some(line) = row.strip_suffix(b"\n") else {
            return self.refuse(LegacyInventoryError::Accounting);
        };
        let Some(tag) = line.first().copied() else {
            return self.refuse(LegacyInventoryError::Accounting);
        };
        // Every canonical row is a tag byte, one tab, then the body.
        if line.get(1) != Some(&b'\t') {
            return self.refuse(LegacyInventoryError::Accounting);
        }
        let body = &line[2..];
        let result = match tag {
            b'H' => self.header_row(line),
            b'I' => self.item_row(body, line),
            b'E' => self.source_event_row(body, line),
            b'T' => self.terminal_row(body, line),
            b'F' => self.footer_row(line),
            _ => Err(LegacyInventoryError::Accounting),
        };
        match result {
            Ok(value) => Ok(value),
            Err(error) => {
                self.refused = true;
                Err(error)
            }
        }
    }

    /// Finishes the stream. The footer must already have been consumed;
    /// a missing footer or rows left after it are a refusal.
    /// # Errors
    /// Returns Accounting when the stream is incomplete or trailing rows
    /// were left after the footer.
    pub fn finish(
        self,
    ) -> Result<LegacyInventorySummary, LegacyInventoryError> {
        if self.refused {
            return Err(LegacyInventoryError::Accounting);
        }
        self.footer.ok_or(LegacyInventoryError::Accounting)
    }

    fn header_row(
        &mut self,
        line: &[u8],
    ) -> Result<LegacyInventoryDecodedRow, LegacyInventoryError> {
        let Some(binding) = self.binding.take() else {
            return Err(LegacyInventoryError::Accounting);
        };
        let encoder = self
            .encoder
            .as_mut()
            .ok_or(LegacyInventoryError::Accounting)?;
        let expected = encoder
            .header(binding)
            .map_err(|_| LegacyInventoryError::Accounting)?;
        // The header carries only declared identities; compare the whole
        // canonical re-encoding instead of parsing any field.
        if !same_line(line, &expected) {
            return Err(LegacyInventoryError::Accounting);
        }
        Ok(LegacyInventoryDecodedRow::Header)
    }

    fn item_row(
        &mut self,
        body: &[u8],
        line: &[u8],
    ) -> Result<LegacyInventoryDecodedRow, LegacyInventoryError> {
        let mut parts = SplitTabs::new(body);
        let family = Self::parse_family(parts.next())?;
        let status = Self::parse_status(parts.next())?;
        let kind = parts
            .next()
            .and_then(|field| parse_decimal_u16(Some(field)))
            .ok_or(LegacyInventoryError::Accounting)?;
        let mut refs = [Sha256Digest32::from_bytes([0; 32]); 3];
        for slot in &mut refs {
            *slot = Self::parse_digest(parts.next())?;
        }
        let mut counts = [0u64; 3];
        for slot in &mut counts {
            *slot = parts
                .next()
                .and_then(|field| parse_decimal_u64(Some(field)))
                .ok_or(LegacyInventoryError::Accounting)?;
        }
        if parts.next().is_some() {
            return Err(LegacyInventoryError::Accounting);
        }
        let encoder = self
            .encoder
            .as_mut()
            .ok_or(LegacyInventoryError::Accounting)?;
        let expected = encoder
            .item(family, status, kind, refs, counts)
            .map_err(|_| LegacyInventoryError::Accounting)?;
        if !same_line(line, &expected) {
            return Err(LegacyInventoryError::Accounting);
        }
        Ok(LegacyInventoryDecodedRow::Item {
            family,
            status,
            kind,
            refs,
            counts,
        })
    }

    fn source_event_row(
        &mut self,
        body: &[u8],
        line: &[u8],
    ) -> Result<LegacyInventoryDecodedRow, LegacyInventoryError> {
        let mut parts = SplitTabs::new(body);
        let family = Self::parse_family(parts.next())?;
        if family != LegacyInventoryFamily::Sources {
            return Err(LegacyInventoryError::Accounting);
        }
        let status = Self::parse_status(parts.next())?;
        if status != LegacyInventoryStatus::Present {
            return Err(LegacyInventoryError::Accounting);
        }
        let Some(hex_row) = parts.next() else {
            return Err(LegacyInventoryError::Accounting);
        };
        if parts.next().is_some() {
            return Err(LegacyInventoryError::Accounting);
        }
        let source = Self::decode_source_row(hex_row)?;
        let encoder = self
            .encoder
            .as_mut()
            .ok_or(LegacyInventoryError::Accounting)?;
        let expected = encoder
            .source_event(&source)
            .map_err(|_| LegacyInventoryError::Accounting)?;
        if !same_line(line, &expected) {
            return Err(LegacyInventoryError::Accounting);
        }
        Ok(LegacyInventoryDecodedRow::SourceEvent(source))
    }

    fn terminal_row(
        &mut self,
        body: &[u8],
        line: &[u8],
    ) -> Result<LegacyInventoryDecodedRow, LegacyInventoryError> {
        let mut parts = SplitTabs::new(body);
        // The terminal carries four structural fields — family tag,
        // status tag, completeness flag and comma-joined counts — but
        // every authoritative value is derived from the parent encoder
        // state, never parsed from the wire.
        for _ in 0..4 {
            if parts.next().is_none() {
                return Err(LegacyInventoryError::Accounting);
            }
        }
        if parts.next().is_some() {
            return Err(LegacyInventoryError::Accounting);
        }
        let encoder = self
            .encoder
            .as_mut()
            .ok_or(LegacyInventoryError::Accounting)?;
        let family = *LegacyInventoryFamily::ALL.get(encoder.family)
            .ok_or(LegacyInventoryError::Accounting)?;
        let counts = encoder.summary.counts[encoder.family];
        let status = super::dominant_status(&counts);
        let incomplete = counts[LegacyInventoryStatus::Gap as usize] > 0
            || counts[LegacyInventoryStatus::Unsupported as usize] > 0
            || counts[LegacyInventoryStatus::Unreadable as usize] > 0;
        // The exact canonical terminal row is re-encoded from parent
        // state; forged, reordered or duplicated terminals fail the
        // byte comparison.
        let expected = encoder
            .end_family(family, status, !incomplete)
            .map_err(|_| LegacyInventoryError::Accounting)?;
        if !same_line(line, &expected) {
            return Err(LegacyInventoryError::Accounting);
        }
        Ok(LegacyInventoryDecodedRow::Terminal {
            family,
            status,
            denominator_complete: !incomplete,
            counts,
        })
    }

    fn footer_row(
        &mut self,
        line: &[u8],
    ) -> Result<LegacyInventoryDecodedRow, LegacyInventoryError> {
        let encoder = self
            .encoder
            .take()
            .ok_or(LegacyInventoryError::Accounting)?;
        let (expected, summary) = encoder
            .finish()
            .map_err(|_| LegacyInventoryError::Accounting)?;
        if !same_line(line, &expected) {
            return Err(LegacyInventoryError::Accounting);
        }
        self.footer = Some(summary);
        Ok(LegacyInventoryDecodedRow::Footer(summary))
    }

    fn parse_family(
        tag: Option<&[u8]>,
    ) -> Result<LegacyInventoryFamily, LegacyInventoryError> {
        LegacyInventoryFamily::ALL
            .into_iter()
            .find(|family| tag == Some(family.tag().as_bytes()))
            .ok_or(LegacyInventoryError::Accounting)
    }

    fn parse_status(
        tag: Option<&[u8]>,
    ) -> Result<LegacyInventoryStatus, LegacyInventoryError> {
        [
            LegacyInventoryStatus::Present,
            LegacyInventoryStatus::AbsentProven,
            LegacyInventoryStatus::Unsupported,
            LegacyInventoryStatus::Corrupt,
            LegacyInventoryStatus::Unreadable,
            LegacyInventoryStatus::Orphan,
            LegacyInventoryStatus::Gap,
        ]
        .into_iter()
        .find(|status| tag == Some(status.tag().as_bytes()))
            .ok_or(LegacyInventoryError::Accounting)
    }

    fn parse_digest(
        tag: Option<&[u8]>,
    ) -> Result<Sha256Digest32, LegacyInventoryError> {
        let Some(text) = tag else {
            return Err(LegacyInventoryError::Accounting);
        };
        if text.len() != 64 {
            return Err(LegacyInventoryError::Accounting);
        }
        let mut bytes = [0u8; 32];
        for (index, byte) in bytes.iter_mut().enumerate() {
            let high = hex_value(text[index * 2])?;
            let low = hex_value(text[index * 2 + 1])?;
            *byte = (high << 4) | low;
        }
        Ok(Sha256Digest32::from_bytes(bytes))
    }

    fn decode_source_row(
        hex_row: &[u8],
    ) -> Result<SourceImportRow, LegacyInventoryError> {
        if hex_row.len() != 371 * 2 {
            return Err(LegacyInventoryError::Metadata);
        }
        let mut bytes = [0u8; 371];
        for (index, byte) in bytes.iter_mut().enumerate() {
            let high = hex_value(hex_row[index * 2])?;
            let low = hex_value(hex_row[index * 2 + 1])?;
            *byte = (high << 4) | low;
        }
        if bytes[32] > 1 {
            return Err(LegacyInventoryError::Metadata);
        }
        let previous_revision = if bytes[32] == 1 {
            Some(SourceRevisionId::from_bytes(
                bytes[33..49].try_into().expect("16 bytes"),
            ))
        } else {
            None
        };
        let occurrence =
            u64::from_be_bytes(bytes[49..57].try_into().expect("8 bytes"));
        let source_event =
            u64::from_be_bytes(bytes[57..65].try_into().expect("8 bytes"));
        let sequence =
            u64::from_be_bytes(bytes[65..73].try_into().expect("8 bytes"));
        let source_bytes =
            u64::from_be_bytes(bytes[73..81].try_into().expect("8 bytes"));
        let flags = bytes[81];
        let native = bytes[82];
        // Bits 3..=7 of the flags byte and native values above 1 are
        // unused by the frozen wire layout.
        if flags & 0b1111_1000 != 0 || native > 1 {
            return Err(LegacyInventoryError::Metadata);
        }
        let lifecycle = SourceLifecycleFlags::new([
            flags & 1 != 0,
            flags & 2 != 0,
            flags & 4 != 0,
            native != 0,
        ]);
        let mut digests = [Sha256Digest32::from_bytes([0; 32]); 9];
        for (index, slot) in digests.iter_mut().enumerate() {
            let start = 83 + index * 32;
            *slot = Sha256Digest32::from_bytes(
                bytes[start..start + 32].try_into().expect("32 bytes"),
            );
        }
        let row = SourceImportRow {
            sequence,
            source: SourceId::from_bytes(
                bytes[0..16].try_into().expect("16 bytes"),
            ),
            revision: SourceRevisionId::from_bytes(
                bytes[16..32].try_into().expect("16 bytes"),
            ),
            previous_revision,
            occurrence,
            source_event,
            source_bytes,
            lifecycle,
            operation: digests[0],
            legacy_source: digests[1],
            legacy_revision: digests[2],
            content: digests[3],
            file_identity: digests[4],
            path: digests[5],
            event: digests[6],
            previous_event: digests[7],
            previous_source_event: digests[8],
        };
        // The existing owning encoder re-derives the canonical wire
        // bytes; a rejected or noncanonical row fails here.
        let reencoded = row
            .encode()
            .map_err(|_| LegacyInventoryError::Metadata)?;
        if reencoded != bytes.to_vec() {
            return Err(LegacyInventoryError::Metadata);
        }
        Ok(row)
    }

    const fn refuse<T>(
        &mut self,
        error: LegacyInventoryError,
    ) -> Result<T, LegacyInventoryError> {
        self.refused = true;
        Err(error)
    }
}
/// Fixed tab-splitting iterator over one row body. It yields None
/// only when the body is exhausted, so trailing-field checks are real.
struct SplitTabs<'a> {
    rest: &'a [u8],
}

impl<'a> SplitTabs<'a> {
    const fn new(rest: &'a [u8]) -> Self {
        Self { rest }
    }

    fn next(&mut self) -> Option<&'a [u8]> {
        if self.rest.is_empty() {
            return None;
        }
        if let Some(index) = self.rest.iter().position(|byte| *byte == b'\t') {
            let (field, tail) = self.rest.split_at(index);
            self.rest = &tail[1..];
            Some(field)
        } else {
            let field = self.rest;
            self.rest = &[][..];
            Some(field)
        }
    }
}

fn parse_decimal_u16(field: Option<&[u8]>) -> Option<u16> {
    let field = field?;
    if field.is_empty() || field.len() > 5 {
        return None;
    }
    let mut value: u16 = 0;
    for byte in field {
        let digit = (*byte).checked_sub(b'0')?;
        if digit > 9 {
            return None;
        }
        value = value.checked_mul(10)?.checked_add(u16::from(digit))?;
    }
    Some(value)
}

fn parse_decimal_u64(field: Option<&[u8]>) -> Option<u64> {
    let field = field?;
    if field.is_empty() || field.len() > 20 {
        return None;
    }
    let mut value: u64 = 0;
    for byte in field {
        let digit = (*byte).checked_sub(b'0')?;
        if digit > 9 {
            return None;
        }
        value = value.checked_mul(10)?.checked_add(u64::from(digit))?;
    }
    Some(value)
}

const fn hex_value(byte: u8) -> Result<u8, LegacyInventoryError> {
    match byte {
        b'0'..=b'9' => Ok(byte - b'0'),
        b'a'..=b'f' => Ok(byte - b'a' + 10),
        _ => Err(LegacyInventoryError::Metadata),
    }
}

fn same_line(line: &[u8], expected_row: &[u8]) -> bool {
    let Some((newline, expected_line)) = expected_row.split_last() else {
        return false;
    };
    *newline == b'\n' && line == expected_line
}

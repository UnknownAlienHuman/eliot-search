//! Strict bounded decoder for the frozen source-content v1 manifest.
//!
//! Rows carry metadata only. Exact source-byte reads and SHA-256/BLAKE3
//! provenance remain the source adapter's responsibility; decoding does not
//! issue a receipt or authorize adoption or cutover.

use core::str::FromStr;

use crate::migration::MAX_SOURCE_IMPORT_ROW_BYTES;

use super::{
    SourceContentManifestEncoder, SourceContentManifestEncodingError as Error,
    SourceContentManifestHeader, SourceContentManifestSummary, SourceContentObjectReadback,
};
use search_contracts::{Blake3Digest32, Sha256Digest32};

const OBJECT_PREFIX: &str = "{\"kind\":\"source_content_readback\",\"ordinal\":";
const END_PREFIX: &str = "{\"kind\":\"source_content_end\",\"objects\":";

/// One canonical source-content manifest row.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum SourceContentManifestDecodedRow {
    /// The exact header bound to the expected manifest inputs.
    Header,
    /// One content-free identity, digest, and length observation.
    Object(SourceContentObjectReadback),
    /// The exact footer and summary recomputed by the encoder.
    End(SourceContentManifestSummary),
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum DecoderPhase {
    Header,
    Objects,
    End,
}

/// Bounded metadata-only decoder for canonical source-content v1 rows.
pub struct SourceContentManifestDecoder {
    encoder: SourceContentManifestEncoder,
    header_row: Vec<u8>,
    phase: DecoderPhase,
    objects: u64,
    summary: Option<SourceContentManifestSummary>,
    refused: Option<Error>,
}

impl SourceContentManifestDecoder {
    /// Starts decoding under the exact header expected by the original target.
    ///
    /// # Errors
    /// Returns the encoder's closed error when the expected header is invalid.
    pub fn new(expected_header: SourceContentManifestHeader) -> Result<Self, Error> {
        let mut encoder = SourceContentManifestEncoder::new(expected_header)?;
        let header_row = encoder.header_row()?;
        Ok(Self {
            encoder,
            header_row,
            phase: DecoderPhase::Header,
            objects: 0,
            summary: None,
            refused: None,
        })
    }

    /// Decodes one bounded row, requiring canonical V1 bytes and order.
    ///
    /// Any malformed, oversized, out-of-order, or noncanonical row refuses
    /// the decoder permanently. Each accepted row is re-encoded through the
    /// frozen encoder and compared byte-for-byte.
    ///
    /// # Errors
    /// Returns a closed encoding error after any invalid input.
    pub fn push(&mut self, row: &[u8]) -> Result<SourceContentManifestDecodedRow, Error> {
        if let Some(error) = self.refused {
            return Err(error);
        }

        match self.decode_row(row) {
            Ok(decoded) => Ok(decoded),
            Err(error) => {
                self.refused = Some(error);
                Err(error)
            }
        }
    }

    /// Completes only after the exact canonical end row has been consumed.
    ///
    /// # Errors
    /// Returns the sticky refusal or `InvalidState` for an incomplete stream.
    pub fn finish(self) -> Result<SourceContentManifestSummary, Error> {
        if let Some(error) = self.refused {
            return Err(error);
        }
        if self.phase != DecoderPhase::End {
            return Err(Error::InvalidState);
        }
        self.summary.ok_or(Error::InvalidState)
    }

    fn decode_row(&mut self, row: &[u8]) -> Result<SourceContentManifestDecodedRow, Error> {
        if row.is_empty() {
            return Err(Error::InvalidState);
        }
        if row.len() > MAX_SOURCE_IMPORT_ROW_BYTES {
            return Err(Error::ObjectLimitExceeded);
        }
        let Some(line) = row.strip_suffix(b"\n") else {
            return Err(Error::InvalidState);
        };
        if line.contains(&b'\n') {
            return Err(Error::InvalidState);
        }
        let text = core::str::from_utf8(line).map_err(|_| Error::InvalidState)?;

        match self.phase {
            DecoderPhase::Header => {
                if row != self.header_row {
                    return Err(Error::InvalidHeader);
                }
                self.phase = DecoderPhase::Objects;
                Ok(SourceContentManifestDecodedRow::Header)
            }
            DecoderPhase::Objects if text.starts_with(OBJECT_PREFIX) => self.object_row(text, row),
            DecoderPhase::Objects if text.starts_with(END_PREFIX) => self.end_row(row),
            DecoderPhase::Objects | DecoderPhase::End => Err(Error::InvalidState),
        }
    }

    fn object_row(
        &mut self,
        text: &str,
        row: &[u8],
    ) -> Result<SourceContentManifestDecodedRow, Error> {
        let (ordinal, object) = parse_object_row(text)?;
        let expected_ordinal = self
            .objects
            .checked_add(1)
            .ok_or(Error::ObjectLimitExceeded)?;
        if ordinal != expected_ordinal {
            return Err(Error::ObjectCountMismatch);
        }

        let expected = self.encoder.object_row(object)?;
        if row != expected {
            return Err(Error::InvalidState);
        }
        self.objects = ordinal;
        Ok(SourceContentManifestDecodedRow::Object(object))
    }

    fn end_row(&mut self, row: &[u8]) -> Result<SourceContentManifestDecodedRow, Error> {
        let (expected, summary) = self.encoder.finish()?;
        if row != expected {
            return Err(Error::InvalidState);
        }
        self.phase = DecoderPhase::End;
        self.summary = Some(summary);
        Ok(SourceContentManifestDecodedRow::End(summary))
    }
}

fn parse_object_row(text: &str) -> Result<(u64, SourceContentObjectReadback), Error> {
    let fields = text
        .strip_prefix(OBJECT_PREFIX)
        .ok_or(Error::InvalidState)?;
    let (ordinal, fields) = fields
        .split_once(",\"legacy_source_id\":\"")
        .ok_or(Error::InvalidState)?;
    let ordinal = parse_decimal_u64(ordinal)?;
    let (legacy_source_id, fields) = fields
        .split_once("\",\"legacy_revision_id\":\"")
        .ok_or(Error::InvalidState)?;
    let (legacy_revision_id, fields) = fields
        .split_once("\",\"content_sha256\":\"")
        .ok_or(Error::InvalidState)?;
    let (content_sha256, fields) = fields
        .split_once("\",\"byte_length\":")
        .ok_or(Error::InvalidState)?;
    let (byte_length, fields) = fields
        .split_once(",\"content_blake3\":\"")
        .ok_or(Error::InvalidState)?;
    let content_blake3 = fields.strip_suffix("\"}").ok_or(Error::InvalidState)?;

    let object = SourceContentObjectReadback {
        legacy_source_id: Sha256Digest32::from_str(legacy_source_id)
            .map_err(|_| Error::InvalidState)?,
        legacy_revision_id: Sha256Digest32::from_str(legacy_revision_id)
            .map_err(|_| Error::InvalidState)?,
        content_sha256: Sha256Digest32::from_str(content_sha256)
            .map_err(|_| Error::InvalidState)?,
        byte_length: parse_decimal_u64(byte_length)?,
        content_blake3: Blake3Digest32::from_str(content_blake3)
            .map_err(|_| Error::InvalidState)?,
    };
    Ok((ordinal, object))
}

fn parse_decimal_u64(value: &str) -> Result<u64, Error> {
    if value.is_empty() || value.len() > 20 || !value.bytes().all(|byte| byte.is_ascii_digit()) {
        return Err(Error::InvalidState);
    }
    value.parse().map_err(|_| Error::InvalidState)
}

//! Strict additive wire schema for the bounded progressive `retrieve_evidence@1`
//! and `orient_scope@1` frames (#213).
//!
//! This module serializes the six closed frame kinds of one progressive
//! exchange. It reuses the shared bounded schema kernel, so unknown, missing,
//! duplicate, reordered and unversioned spellings are refused rather than
//! defaulted, and it re-decodes every value to reject alternate spellings.
//!
//! The frames are transport data only. A decoded frame carries no authority:
//! scope resolution, access, ranking, exact readback and coverage accounting
//! remain with their owning packages, and no live query or native capability is
//! claimed by encoding or decoding one.
//!
//! The record and enum schemas for the shared evidence/card vocabulary are
//! declared once in [`super::evidence_schema`]; this module adds only the frame
//! records and the versioned frame envelope.

use search_contracts::evidence_frames::{
    EvidenceAcceptedFrame, EvidenceCardBatchFrame, EvidenceCoverageSnapshotFrame,
    EvidenceFrameHeader, EvidenceFrameKind, EvidenceFrameV2, EvidenceTerminalFrame,
    EvidenceTerminalKind, MAX_EVIDENCE_FRAME_BYTES, OrientationBatchFrame,
    SuggestedCheckBatchFrame,
};

use super::wire::{Decoder, Encoder, Result, Schema, record, tagged};
use crate::config::DEFAULT_PROTOCOL_LIMITS;
use crate::error::ProtocolError;
use crate::frame::FrameCodec;

/// Schema version of the additive progressive frame envelope.
pub const EVIDENCE_FRAME_SCHEMA_VERSION: u16 = 2;

impl Schema for EvidenceFrameKind {
    fn put(&self, output: &mut Encoder) -> Result<()> {
        output.text(self.as_str())
    }
    fn get(input: &mut Decoder<'_>) -> Result<Self> {
        Self::parse(&input.text(64)?).map_err(|_| ProtocolError::InvalidBody)
    }
}

impl Schema for EvidenceTerminalKind {
    fn put(&self, output: &mut Encoder) -> Result<()> {
        output.text(self.as_str())
    }
    fn get(input: &mut Decoder<'_>) -> Result<Self> {
        Self::parse(&input.text(64)?).map_err(|_| ProtocolError::InvalidBody)
    }
}

record!(EvidenceFrameHeader {
    request_id,
    kind,
    event_sequence,
    profile_id
});
record!(EvidenceAcceptedFrame { header, progressive, declared_frame_budget }
    => |value: &EvidenceAcceptedFrame| value.validate().map_err(|_| ProtocolError::InvalidBody));
record!(EvidenceCoverageSnapshotFrame { header, coverage, complete_negative_possible }
=> |value: &EvidenceCoverageSnapshotFrame| {
    value.validate().map_err(|_| ProtocolError::InvalidBody)
});
record!(EvidenceCardBatchFrame { header, cards, groups, last_batch, batch_offset }
=> |value: &EvidenceCardBatchFrame| {
    value.validate().map_err(|_| ProtocolError::InvalidBody)
});
record!(OrientationBatchFrame {
    header, cards, groups, recommended_reading, last_batch, batch_offset,
} => |value: &OrientationBatchFrame| value.validate().map_err(|_| ProtocolError::InvalidBody));
record!(SuggestedCheckBatchFrame { header, checks, last_batch, batch_offset }
=> |value: &SuggestedCheckBatchFrame| {
    value.validate().map_err(|_| ProtocolError::InvalidBody)
});
record!(EvidenceTerminalFrame { header, terminal, coverage, reason_codes, budget_exhausted }
    => |value: &EvidenceTerminalFrame| value.validate().map_err(|_| ProtocolError::InvalidBody));

tagged!(EvidenceFrameV2 {
    Accepted => "accepted",
    CoverageSnapshot => "coverage_snapshot",
    CardBatch => "card_batch",
    OrientationBatch => "orientation_batch",
    SuggestedCheckBatch => "suggested_check_batch",
    Terminal => "terminal",
});

fn open_frame(bytes: &[u8]) -> Result<Decoder<'_>> {
    if bytes.is_empty() {
        return Err(ProtocolError::InvalidBody);
    }
    if bytes.len() > MAX_EVIDENCE_FRAME_BYTES {
        return Err(ProtocolError::FrameTooLarge);
    }
    FrameCodec::validate_payload(bytes, DEFAULT_PROTOCOL_LIMITS)?;
    let mut input = Decoder::new(bytes);
    input.open(b'{')?;
    Ok(input)
}

/// Reads the `v` and `frame` envelope fields that precede a frame body.
fn read_envelope(input: &mut Decoder<'_>, first: &mut bool) -> Result<EvidenceFrameKind> {
    input.field(first, "v")?;
    let version = <u16 as Schema>::get(input)?;
    if version != EVIDENCE_FRAME_SCHEMA_VERSION {
        return Err(ProtocolError::NoCompatibleVersion);
    }
    input.field(first, "frame")?;
    let tag = input.text(64)?;
    EvidenceFrameKind::parse(&tag).map_err(|_| ProtocolError::InvalidBody)
}

/// Encodes one bounded progressive frame with its versioned envelope.
///
/// The body is written under a single `body` field so the decoded value is an
/// ordinary record with two envelope fields plus one tagged body, and re-encoding
/// the decoded value reproduces the exact bytes.
pub fn encode_evidence_frame(frame: &EvidenceFrameV2) -> Result<Vec<u8>> {
    let mut output = Encoder::new(MAX_EVIDENCE_FRAME_BYTES);
    output.open(b'{')?;
    let mut first = true;
    output.field(&mut first, "v")?;
    EVIDENCE_FRAME_SCHEMA_VERSION.put(&mut output)?;
    output.field(&mut first, "frame")?;
    output.text(frame.kind().as_str())?;
    output.field(&mut first, "body")?;
    frame.validate().map_err(|_| ProtocolError::InvalidBody)?;
    frame.put(&mut output)?;
    output.close(b'}')?;
    let bytes = output.finish()?;
    FrameCodec::validate_payload(&bytes, DEFAULT_PROTOCOL_LIMITS)?;
    Ok(bytes)
}

/// Decodes one bounded progressive frame with its versioned envelope.
///
/// The `frame` envelope tag, the body tag and the frame record must all agree.
/// An unexpected, unversioned or wrong-version envelope is refused, a family
/// mismatch is refused, and the decoded value is re-encoded before it is returned
/// so an alternate spelling cannot survive a round trip.
pub fn decode_evidence_frame(bytes: &[u8]) -> Result<EvidenceFrameV2> {
    let mut input = open_frame(bytes)?;
    let mut first = true;
    let declared = read_envelope(&mut input, &mut first)?;
    input.field(&mut first, "body")?;
    let frame = EvidenceFrameV2::get(&mut input)?;
    if frame.kind() != declared {
        return Err(ProtocolError::InvalidBody);
    }
    input.close(b'}')?;
    input.finish()?;
    frame.validate().map_err(|_| ProtocolError::InvalidBody)?;
    if encode_evidence_frame(&frame)?.as_slice() != bytes {
        return Err(ProtocolError::InvalidBody);
    }
    Ok(frame)
}

/// Canonical frame-kind order used for dispatch and comparison.
///
/// This is not emission order; emission order is carried by each frame
/// `event_sequence` and validated by the owning exchange rule.
pub fn evidence_frame_order() -> Vec<EvidenceFrameKind> {
    EvidenceFrameKind::ALL.to_vec()
}

/// Reads only the versioned envelope tag of one encoded frame.
pub fn peek_evidence_frame(bytes: &[u8]) -> Result<EvidenceFrameKind> {
    let mut input = open_frame(bytes)?;
    let mut first = true;
    read_envelope(&mut input, &mut first)
}

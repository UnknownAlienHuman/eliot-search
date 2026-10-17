//! Strict additive wire schema for the bounded progressive provider response
//! and terminal lifecycle of `retrieve_evidence@1` and `orient_scope@1` (#213).
//!
//! One admitted V2 evidence request emits exactly one response envelope per
//! provider event. The response body is selected by the shared request-local
//! `event_sequence` and is closed over the six existing
//! [`super::evidence_frames_schema`] progressive frames plus the existing V2
//! recipe result. No new frame is invented here, and the eleven-recipe v1
//! `request`/`progress`/`result`/`error`/`cancelled` bodies keep their
//! exact bytes.
//!
//! Every response frame binds its header to the admitted request and connection
//! identity. Encoding validates the body only; live grant, scope, access,
//! coverage accounting and source readback stay with their owning packages, so a
//! decoded response is still untrusted data.

use search_contracts::evidence_frames::{
    EvidenceFrameV2, EvidenceTerminalKind, MAX_EVIDENCE_FRAME_BYTES,
};
use search_contracts::{
    BindingId, BoundedBytes, InstallationIncarnationId, MAX_FRAME_BYTES, MessageKind,
    ProtocolRange, ProtocolVersion, RequestId,
};

use super::evidence_schema::EvidenceRecipeResultV2;
use super::wire::{Decoder, Encoder, Result, Schema};
use crate::config::ProtocolLimits;
use crate::error::ProtocolError;
use crate::frame::FrameCodec;
use crate::terminal::TerminalKind;

/// Body version of the additive progressive response envelope.
pub const EVIDENCE_RESPONSE_SCHEMA_VERSION: u16 = 2;

/// Closed response-body union of one progressive evidence exchange.
///
/// `Progress` carries an existing V1 progress body unchanged, so a progressive
/// V2 exchange can report phase and bounded counts between evidence frames.
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum EvidenceResponseBodyV2 {
    /// One bounded progressive frame of this exchange.
    Frame(EvidenceFrameV2),
    /// One bounded `retrieve_evidence@1`/`orient_scope@1` result.
    Result(EvidenceRecipeResultV2),
    /// One existing V1 progress body; never carries a frame or source content.
    Progress(search_contracts::ProgressBody),
    /// Existing content-free failure body; ends this request as failed.
    Error(search_contracts::ErrorBody),
    /// Existing target cancellation acknowledgement; ends this request as cancelled.
    Cancelled(search_contracts::CancelledBody),
}

impl EvidenceResponseBodyV2 {
    /// Exact shared message kind this body travels under.
    #[must_use]
    pub const fn message_kind(&self) -> MessageKind {
        match self {
            Self::Frame(_) | Self::Result(_) => MessageKind::Result,
            Self::Progress(_) => MessageKind::Progress,
            Self::Error(_) => MessageKind::Error,
            Self::Cancelled(_) => MessageKind::Cancelled,
        }
    }

    /// Terminal classification carried by this body, if it is terminal.
    ///
    /// The existing terminal frame, failure and cancelled bodies end a request.
    /// A recipe result is nonterminal and retains its own coverage/fence.
    #[must_use]
    pub const fn terminal(&self) -> Option<TerminalKind> {
        match self {
            Self::Frame(EvidenceFrameV2::Terminal(frame)) => Some(match frame.terminal {
                EvidenceTerminalKind::Success => TerminalKind::Success,
                EvidenceTerminalKind::Partial => TerminalKind::Partial,
                EvidenceTerminalKind::Failure => TerminalKind::Failed,
            }),
            Self::Frame(_) | Self::Result(_) | Self::Progress(_) => None,
            Self::Error(_) => Some(TerminalKind::Failed),
            Self::Cancelled(_) => Some(TerminalKind::Cancelled),
        }
    }
}

/// One authenticated provider response carrying one closed V2 evidence body.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct EvidenceResponseEnvelopeV2 {
    /// Negotiated provider protocol major.
    pub protocol_major: u16,
    /// Negotiated provider protocol minor.
    pub protocol_minor: u16,
    /// Authenticated local installation incarnation.
    pub installation_incarnation_id: InstallationIncarnationId,
    /// Authenticated provider binding.
    pub binding_id: BindingId,
    /// Shared monotonic connection sequence.
    pub connection_sequence: u64,
    /// Request identity owned by the shared session.
    pub request_id: RequestId,
    /// Shared request-local event sequence.
    pub event_sequence: u64,
    /// Closed response body of this exchange.
    pub body: EvidenceResponseBodyV2,
}

impl EvidenceResponseEnvelopeV2 {
    /// Validates response shape, correlation and exchange binding.
    ///
    /// Every frame binds to this response's request, profile and event sequence;
    /// a result must match the admitted recipe identity. No session, grant or
    /// scope decision is implied.
    pub fn validate(&self) -> Result<()> {
        self.body.validate()?;
        if self.event_sequence == 0 {
            return Err(ProtocolError::InvalidBody);
        }
        let header = match &self.body {
            EvidenceResponseBodyV2::Frame(frame) => Some(frame.header()),
            _ => None,
        };
        if let Some(header) = header
            && (header.request_id != self.request_id
                || header.event_sequence != self.event_sequence)
        {
            return Err(ProtocolError::InvalidBody);
        }
        let result_id = match &self.body {
            EvidenceResponseBodyV2::Result(result) => match result {
                EvidenceRecipeResultV2::RetrieveEvidence(value) => {
                    if value
                        .result_fence
                        .planned_snapshot
                        .installation_incarnation_id
                        != self.installation_incarnation_id
                    {
                        return Err(ProtocolError::InvalidBody);
                    }
                    value.request_id
                }
                EvidenceRecipeResultV2::OrientScope(value) => {
                    if value
                        .result_fence
                        .planned_snapshot
                        .installation_incarnation_id
                        != self.installation_incarnation_id
                    {
                        return Err(ProtocolError::InvalidBody);
                    }
                    value.request_id
                }
            },
            EvidenceResponseBodyV2::Progress(progress) => {
                if progress.event_sequence != self.event_sequence {
                    return Err(ProtocolError::InvalidBody);
                }
                self.request_id
            }
            EvidenceResponseBodyV2::Cancelled(cancelled) => {
                if !cancelled.terminal {
                    return Err(ProtocolError::InvalidBody);
                }
                cancelled.target_request_id
            }
            _ => self.request_id,
        };
        if result_id != self.request_id {
            return Err(ProtocolError::InvalidBody);
        }
        Ok(())
    }
}

impl EvidenceResponseBodyV2 {
    fn validate(&self) -> Result<()> {
        match self {
            Self::Frame(frame) => {
                frame.validate().map_err(|_| ProtocolError::InvalidBody)?;
                if frame.event_sequence() == 0 {
                    return Err(ProtocolError::InvalidBody);
                }
            }
            Self::Result(result) => result.validate()?,
            Self::Progress(progress) => {
                // Reuse the shared progress shape so a V2 exchange cannot widen
                // the existing content-free progress vocabulary.
                let _ = progress;
            }
            Self::Error(_) | Self::Cancelled(_) => {}
        }
        Ok(())
    }
}

struct VersionedEvidenceResponseBodyV2 {
    event_sequence: u64,
    body: EvidenceResponseBodyV2,
}

impl Schema for VersionedEvidenceResponseBodyV2 {
    fn put(&self, output: &mut Encoder) -> Result<()> {
        self.body.validate()?;
        let inner_sequence = match &self.body {
            EvidenceResponseBodyV2::Frame(frame) => frame.event_sequence(),
            EvidenceResponseBodyV2::Progress(progress) => progress.event_sequence,
            _ => self.event_sequence,
        };
        if self.event_sequence == 0 || inner_sequence != self.event_sequence {
            return Err(ProtocolError::InvalidBody);
        }
        output.tag("response_v2")?;
        output.open(b'{')?;
        let mut first = true;
        output.field(&mut first, "v")?;
        EVIDENCE_RESPONSE_SCHEMA_VERSION.put(output)?;
        output.field(&mut first, "event_sequence")?;
        self.event_sequence.put(output)?;
        output.field(&mut first, "body")?;
        match &self.body {
            EvidenceResponseBodyV2::Frame(frame) => {
                output.tag("frame")?;
                super::evidence_frames_schema::put_evidence_frame(frame, output)?;
            }
            EvidenceResponseBodyV2::Result(result) => {
                output.tag("result")?;
                result.put(output)?;
            }
            EvidenceResponseBodyV2::Progress(progress) => {
                output.tag("progress")?;
                progress.put(output)?;
            }
            EvidenceResponseBodyV2::Error(error) => {
                output.tag("error")?;
                error.put(output)?;
            }
            EvidenceResponseBodyV2::Cancelled(cancelled) => {
                output.tag("cancelled")?;
                cancelled.put(output)?;
            }
        }
        output.close(b'}')?;
        output.close(b'}')?;
        output.close(b'}')
    }

    fn get(input: &mut Decoder<'_>) -> Result<Self> {
        if input.tag()? != "response_v2" {
            return Err(ProtocolError::InvalidBody);
        }
        input.open(b'{')?;
        let mut first = true;
        input.field(&mut first, "v")?;
        if u16::get(input)? != EVIDENCE_RESPONSE_SCHEMA_VERSION {
            return Err(ProtocolError::NoCompatibleVersion);
        }
        input.field(&mut first, "event_sequence")?;
        let event_sequence = u64::get(input)?;
        if event_sequence == 0 {
            return Err(ProtocolError::InvalidBody);
        }
        input.field(&mut first, "body")?;
        let body = match input.tag()?.as_str() {
            "frame" => {
                let frame = super::evidence_frames_schema::get_evidence_frame(input)?;
                EvidenceResponseBodyV2::Frame(frame)
            }
            "result" => EvidenceResponseBodyV2::Result(Schema::get(input)?),
            "progress" => EvidenceResponseBodyV2::Progress(Schema::get(input)?),
            "error" => EvidenceResponseBodyV2::Error(Schema::get(input)?),
            "cancelled" => EvidenceResponseBodyV2::Cancelled(Schema::get(input)?),
            _ => return Err(ProtocolError::InvalidBody),
        };
        input.close(b'}')?;
        input.close(b'}')?;
        input.close(b'}')?;
        let body = Self {
            event_sequence,
            body,
        };
        body.body.validate()?;
        Ok(body)
    }
}

fn put_header_envelope(output: &mut Encoder, envelope: &EvidenceResponseEnvelopeV2) -> Result<()> {
    output.open(b'{')?;
    let mut first = true;
    output.field(&mut first, "protocol_major")?;
    envelope.protocol_major.put(output)?;
    output.field(&mut first, "protocol_minor")?;
    envelope.protocol_minor.put(output)?;
    output.field(&mut first, "installation_incarnation_id")?;
    envelope.installation_incarnation_id.put(output)?;
    output.field(&mut first, "binding_id")?;
    envelope.binding_id.put(output)?;
    output.field(&mut first, "connection_sequence")?;
    envelope.connection_sequence.put(output)?;
    output.field(&mut first, "request_id")?;
    envelope.request_id.put(output)?;
    output.field(&mut first, "message_kind")?;
    envelope.body.message_kind().put(output)?;
    output.field(&mut first, "relative_deadline_ms")?;
    Option::<u64>::None.put(output)?;
    output.field(&mut first, "body")?;
    VersionedEvidenceResponseBodyV2 {
        event_sequence: envelope.event_sequence,
        body: envelope.body.clone(),
    }
    .put(output)?;
    output.close(b'}')
}

fn open_response_envelope(bytes: &[u8]) -> Result<Decoder<'_>> {
    if bytes.is_empty() || bytes.len() > MAX_EVIDENCE_FRAME_BYTES {
        return Err(if bytes.is_empty() {
            ProtocolError::InvalidBody
        } else {
            ProtocolError::FrameTooLarge
        });
    }
    FrameCodec::validate_payload(bytes, crate::config::DEFAULT_PROTOCOL_LIMITS)?;
    let input = Decoder::new(bytes);
    Ok(input)
}

/// Encodes one exact additive progressive response body schema.
///
/// The body is written as `{"response_v2":{...}}`; re-encoding the decoded
/// value reproduces the exact bytes, so an alternate spelling cannot survive.
pub fn encode_evidence_response_body(
    body: &EvidenceResponseBodyV2,
    event_sequence: u64,
) -> Result<Vec<u8>> {
    body.validate()?;
    let mut output = Encoder::new(MAX_EVIDENCE_FRAME_BYTES);
    VersionedEvidenceResponseBodyV2 {
        event_sequence,
        body: body.clone(),
    }
    .put(&mut output)?;
    let bytes = output.finish()?;
    FrameCodec::validate_payload(&bytes, crate::config::DEFAULT_PROTOCOL_LIMITS)?;
    Ok(bytes)
}

/// Decodes one exact additive progressive response body schema.
pub fn decode_evidence_response_body(bytes: &[u8]) -> Result<(u64, EvidenceResponseBodyV2)> {
    let mut input = open_response_envelope(bytes)?;
    let body = VersionedEvidenceResponseBodyV2::get(&mut input)?;
    input.finish()?;
    body.body.validate()?;
    if encode_evidence_response_body(&body.body, body.event_sequence)?.as_slice() != bytes {
        return Err(ProtocolError::InvalidBody);
    }
    Ok((body.event_sequence, body.body))
}

/// Encodes the additive authenticated V2 evidence response envelope.
///
/// The outer transport header and framing match the current provider protocol;
/// only the closed body tag is new, so the eleven-recipe v1 wire bytes and the
/// separate response proof transcript are unchanged.
pub fn encode_evidence_response_v2(
    envelope: &EvidenceResponseEnvelopeV2,
    limits: ProtocolLimits,
    supported: ProtocolRange,
) -> Result<BoundedBytes<MAX_FRAME_BYTES>> {
    let limits = limits.validate()?;
    let version = ProtocolVersion {
        major: envelope.protocol_major,
        minor: envelope.protocol_minor,
    };
    if supported.minimum > supported.maximum {
        return Err(ProtocolError::InvalidLimits);
    }
    if version != (ProtocolVersion { major: 1, minor: 0 }) || !supported.contains(version) {
        return Err(ProtocolError::NoCompatibleVersion);
    }
    envelope.validate()?;
    let maximum = limits
        .max_body_bytes
        .min(MAX_EVIDENCE_FRAME_BYTES - crate::config::FRAME_PREFIX_BYTES)
        .min(
            limits
                .max_frame_bytes
                .checked_sub(crate::config::FRAME_PREFIX_BYTES)
                .ok_or(ProtocolError::InvalidLimits)?,
        );
    let mut output = Encoder::new(maximum);
    put_header_envelope(&mut output, envelope)?;
    let payload = search_contracts::protocol::JsonFramePayload::new(output.finish()?)
        .map_err(|_| ProtocolError::FrameTooLarge)?;
    FrameCodec::encode(&payload, limits)
}

/// Decodes the exact additive V2 evidence response envelope.
///
/// Direction is validated against the shared client codec, so a request or
/// cancel frame cannot be admitted as a response. Correlation with the live
/// request and every grant/scope decision remain caller-owned.
pub fn decode_evidence_response_v2(
    bytes: &[u8],
    limits: ProtocolLimits,
    supported: ProtocolRange,
) -> Result<EvidenceResponseEnvelopeV2> {
    let limits = limits.validate()?;
    if supported.minimum > supported.maximum {
        return Err(ProtocolError::InvalidLimits);
    }
    let payload = FrameCodec::decode(bytes, limits)?;
    let mut input = Decoder::new(payload.as_slice());
    input.open(b'{')?;
    let mut first = true;
    macro_rules! field {
        ($name:ident) => {{
            input.field(&mut first, stringify!($name))?;
            Schema::get(&mut input)?
        }};
    }
    let protocol_major = field!(protocol_major);
    let protocol_minor = field!(protocol_minor);
    let version = ProtocolVersion {
        major: protocol_major,
        minor: protocol_minor,
    };
    if version != (ProtocolVersion { major: 1, minor: 0 }) || !supported.contains(version) {
        return Err(ProtocolError::NoCompatibleVersion);
    }
    let installation_incarnation_id = field!(installation_incarnation_id);
    let binding_id = field!(binding_id);
    let connection_sequence = field!(connection_sequence);
    let request_id = field!(request_id);
    let message_kind: MessageKind = field!(message_kind);
    if !matches!(
        message_kind,
        MessageKind::Progress | MessageKind::Result | MessageKind::Error | MessageKind::Cancelled
    ) {
        return Err(ProtocolError::InvalidEnvelope);
    }
    let relative_deadline_ms: Option<u64> = field!(relative_deadline_ms);
    if relative_deadline_ms.is_some() {
        return Err(ProtocolError::InvalidBody);
    }
    input.field(&mut first, "body")?;
    let VersionedEvidenceResponseBodyV2 {
        event_sequence,
        body,
    } = Schema::get(&mut input)?;
    if body.message_kind() != message_kind {
        return Err(ProtocolError::InvalidBody);
    }
    input.close(b'}')?;
    input.finish()?;
    let envelope = EvidenceResponseEnvelopeV2 {
        protocol_major,
        protocol_minor,
        installation_incarnation_id,
        binding_id,
        connection_sequence,
        request_id,
        event_sequence,
        body,
    };
    envelope.validate()?;
    if encode_evidence_response_v2(&envelope, limits, supported)?.as_slice() != bytes {
        return Err(ProtocolError::InvalidEnvelope);
    }
    Ok(envelope)
}

impl super::response::ServerEnvelopeCodec {
    /// Encodes an additive response with the existing bounded schema and framing.
    pub fn encode_evidence_response_v2(
        envelope: &EvidenceResponseEnvelopeV2,
        limits: ProtocolLimits,
        supported: ProtocolRange,
    ) -> Result<BoundedBytes<MAX_FRAME_BYTES>> {
        encode_evidence_response_v2(envelope, limits, supported)
    }

    /// Decodes data only; session custody and authorization remain mandatory.
    pub fn decode_evidence_response_v2(
        bytes: &[u8],
        limits: ProtocolLimits,
        supported: ProtocolRange,
    ) -> Result<EvidenceResponseEnvelopeV2> {
        if bytes.len() > MAX_EVIDENCE_FRAME_BYTES {
            return Err(ProtocolError::FrameTooLarge);
        }
        decode_evidence_response_v2(bytes, limits, supported)
    }
}

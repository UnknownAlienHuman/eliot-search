//! Typed client-to-provider envelopes from the existing P00 schema.
//!
//! Client and server codecs share one header and bounded schema implementation.
//! Direction is validated before body allocation; provider messages cannot be
//! admitted as client commands. Existing shell and standalone-grant envelopes
//! keep their independent codecs/transcripts.
//!
//! Wire fields follow the P00 record order. Unions have one named key; recipe
//! body keys are exact versioned RecipeIdV1 values. IDs are hyphenated lowercase
//! UUIDs, digests lowercase hex, raw/token bytes unpadded base64url, and optional
//! values explicit null. The schema decoder rejects noncanonical spelling,
//! missing/duplicate/unknown fields, order changes, unsupported tags and trailing
//! input. This is transport serialization, not an identity/fingerprint encoding.
//!
//! Decoding supplies claims and request data, never a grant decision. Pairing,
//! current binding/incarnation, sequences, replay, deadline, revocation, scope
//! intersection and source authority must still be checked by their live owners.

mod grant;
mod primitives;
mod recipe;
mod response;
mod source;
mod wire;

use search_contracts::{
    BoundedBytes, CancelBody, HelloBody, JsonFramePayload, MessageKind, ProtocolRange,
    ProtocolVersion, ProviderBodyV1, ProviderEnvelope, RequestBody, MAX_FRAME_BYTES,
};

use crate::config::{FRAME_PREFIX_BYTES, ProtocolLimits};
use crate::error::ProtocolError;
use super::FrameCodec;
use wire::{Decoder, Encoder, Result, Schema, record};

pub use response::ServerEnvelopeCodec;

#[derive(Clone, Copy)]
enum Direction { Client, Server }

/// Strict typed codec for P00 client-to-provider hello, request and cancel.
///
/// No socket/storage I/O, request admission or source authorization occurs here.
/// The baseline schema is exactly protocol 1.0. Both methods also require the
/// negotiated range; a wider range cannot authorize an unimplemented extension.
/// Output is bounded before each buffer extension, including expanded escaping
/// and base64url. Input framing is checked before typed allocation; nested
/// containers use the existing P00 JSON/anchor/collection limits.
pub struct ClientEnvelopeCodec;

impl ClientEnvelopeCodec {
    /// Encodes one client message with the canonical four-byte length prefix.
    ///
    /// # Errors
    ///
    /// Rejects invalid limits/ranges, version or direction, inconsistent tags
    /// and identities, invalid nested schema values or an oversized frame.
    pub fn encode(
        envelope: &ProviderEnvelope,
        limits: ProtocolLimits,
        supported: ProtocolRange,
    ) -> Result<BoundedBytes<MAX_FRAME_BYTES>> {
        Self::encode_direction(envelope, limits, supported, Direction::Client)
    }

    fn encode_direction(
        envelope: &ProviderEnvelope,
        limits: ProtocolLimits,
        supported: ProtocolRange,
        direction: Direction,
    ) -> Result<BoundedBytes<MAX_FRAME_BYTES>> {
        let limits = limits.validate()?;
        validate_range(supported)?;
        validate_version(envelope.protocol_version(), supported)?;
        validate_shape(envelope, direction)?;
        let maximum = limits.max_body_bytes.min(
            limits.max_frame_bytes.checked_sub(FRAME_PREFIX_BYTES)
                .ok_or(ProtocolError::InvalidLimits)?,
        );
        let mut output = Encoder::new(maximum);
        output.open(b'{')?;
        let mut first = true;
        macro_rules! field {
            ($name:ident) => {
                output.field(&mut first, stringify!($name))?;
                envelope.$name.put(&mut output)?;
            };
        }
        field!(protocol_major);
        field!(protocol_minor);
        field!(installation_incarnation_id);
        field!(binding_id);
        field!(connection_sequence);
        field!(request_id);
        field!(message_kind);
        field!(relative_deadline_ms);
        output.field(&mut first, "body")?;
        put_body(&envelope.body, &mut output)?;
        output.close(b'}')?;
        let payload = JsonFramePayload::new(output.finish()?)
            .map_err(|_| ProtocolError::FrameTooLarge)?;
        FrameCodec::encode(&payload, limits)
    }

    /// Decodes only a complete, correctly framed client message.
    ///
    /// # Errors
    ///
    /// Rejects invalid syntax, noncanonical schema spelling, unknown/duplicate
    /// fields, wrong direction, version, tag or correlated request identities.
    /// The result is untrusted data pending live session/grant validation.
    pub fn decode(
        bytes: &[u8],
        limits: ProtocolLimits,
        supported: ProtocolRange,
    ) -> Result<ProviderEnvelope> {
        Self::decode_direction(bytes, limits, supported, Direction::Client)
    }

    fn decode_direction(
        bytes: &[u8],
        limits: ProtocolLimits,
        supported: ProtocolRange,
        direction: Direction,
    ) -> Result<ProviderEnvelope> {
        validate_range(supported)?;
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
        validate_version(ProtocolVersion { major: protocol_major, minor: protocol_minor }, supported)?;
        let installation_incarnation_id = field!(installation_incarnation_id);
        let binding_id = field!(binding_id);
        let connection_sequence = field!(connection_sequence);
        let request_id = field!(request_id);
        let message_kind = field!(message_kind);
        validate_direction(message_kind, direction)?;
        let relative_deadline_ms = field!(relative_deadline_ms);
        input.field(&mut first, "body")?;
        let body = get_body(message_kind, &mut input)?;
        input.close(b'}')?;
        input.finish()?;
        let envelope = ProviderEnvelope {
            protocol_major, protocol_minor, installation_incarnation_id, binding_id,
            connection_sequence, request_id, message_kind, relative_deadline_ms, body,
        };
        validate_shape(&envelope, direction)?;
        Ok(envelope)
    }
}

fn validate_range(supported: ProtocolRange) -> Result<()> {
    if supported.minimum > supported.maximum { return Err(ProtocolError::InvalidLimits); }
    Ok(())
}

fn validate_version(version: ProtocolVersion, supported: ProtocolRange) -> Result<()> {
    if version != (ProtocolVersion { major: 1, minor: 0 }) || !supported.contains(version) {
        return Err(ProtocolError::NoCompatibleVersion);
    }
    Ok(())
}

fn validate_direction(kind: MessageKind, direction: Direction) -> Result<()> {
    match (direction, kind) {
        (_, MessageKind::Hello)
        | (Direction::Client, MessageKind::Request | MessageKind::Cancel)
        | (Direction::Server, MessageKind::Progress | MessageKind::Result
            | MessageKind::Error | MessageKind::Cancelled) => Ok(()),
        _ => Err(ProtocolError::InvalidEnvelope),
    }
}

fn validate_shape(envelope: &ProviderEnvelope, direction: Direction) -> Result<()> {
    validate_direction(envelope.message_kind, direction)?;
    envelope.validate().map_err(|_| ProtocolError::InvalidEnvelope)?;
    if let ProviderBodyV1::Request(body) = &envelope.body {
        if body.recipe_request.request_id != envelope.request_id
            || body.grant.binding_id != envelope.binding_id
            || body.grant.installation_incarnation_id != envelope.installation_incarnation_id
        {
            return Err(ProtocolError::InvalidBody);
        }
    }
    if let ProviderBodyV1::Result(body) = &envelope.body {
        response::validate_result_identity(&body.result, envelope)?;
    }
    Ok(())
}

fn put_body(body: &ProviderBodyV1, output: &mut Encoder) -> Result<()> {
    output.tag(body.message_kind().as_str())?;
    match body {
        ProviderBodyV1::Hello(value) => value.put(output)?,
        ProviderBodyV1::Request(value) => value.put(output)?,
        ProviderBodyV1::Cancel(value) => value.put(output)?,
        ProviderBodyV1::Progress(value) => value.put(output)?,
        ProviderBodyV1::Result(value) => value.put(output)?,
        ProviderBodyV1::Error(value) => value.put(output)?,
        ProviderBodyV1::Cancelled(value) => value.put(output)?,
    }
    output.close(b'}')
}

fn get_body(kind: MessageKind, input: &mut Decoder<'_>) -> Result<ProviderBodyV1> {
    // The duplicate discriminant must match BEFORE decoding a potentially large
    // request body; it is never inferred from whatever fields happen to fit.
    if input.tag()? != kind.as_str() { return Err(ProtocolError::InvalidBody); }
    let body = match kind {
        MessageKind::Hello => ProviderBodyV1::Hello(Schema::get(input)?),
        MessageKind::Request => ProviderBodyV1::Request(Schema::get(input)?),
        MessageKind::Cancel => ProviderBodyV1::Cancel(Schema::get(input)?),
        MessageKind::Progress => ProviderBodyV1::Progress(Schema::get(input)?),
        MessageKind::Result => ProviderBodyV1::Result(Schema::get(input)?),
        MessageKind::Error => ProviderBodyV1::Error(Schema::get(input)?),
        MessageKind::Cancelled => ProviderBodyV1::Cancelled(Schema::get(input)?),
    };
    input.close(b'}')?;
    Ok(body)
}

record!(ProtocolVersion { major, minor });
record!(ProtocolRange { minimum, maximum } => |value: &ProtocolRange| validate_range(*value));
record!(HelloBody {
    peer_role, pairing_proof_ref, supported_protocol_range, requested_capability_digest,
});
record!(RequestBody { grant, recipe_request });
record!(CancelBody { target_request_id });

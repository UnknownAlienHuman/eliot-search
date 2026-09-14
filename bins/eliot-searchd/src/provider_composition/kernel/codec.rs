//! Strict bounded provider line framing and envelope codec.

use search_contracts::{ProtocolRange, ProtocolVersion, RequestId};
use search_provider_protocol::negotiation::negotiate_hello;
use search_provider_protocol::request::{
    AuthenticatedEnvelope, AuthenticatedResponse, decode_envelope, encode_response,
};
use search_provider_protocol::{DEFAULT_PROTOCOL_LIMITS, ProtocolError};

use super::spec::{
    ENVELOPE_LINE_PREFIX, MAX_ENVELOPE_HEX, MAX_OP_ARG_HEX, OP_LINE_PREFIX,
    PROVIDER_ENVELOPE_REQUIRED, PROVIDER_PROTOCOL_RANGE, PROVIDER_UNKNOWN_COMMAND,
    ProviderOperation, protocol_reason,
};

/// Parsed `op` line: operation plus its validated argument.
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum OpArgument {
    /// No argument (`status`).
    None,
    /// Cancellation target identity.
    CancelTarget(RequestId),
    /// Opaque validated hex argument for gated recipes.
    Blob(Vec<u8>),
}

/// Parses one `op\t...` line with strict arity and bounded hex validation.
///
/// Envelope-only operations (`health`/`version`/`shutdown`) are rejected
/// here with [`PROVIDER_ENVELOPE_REQUIRED`]: they must travel as sealed
/// envelopes, never as bare operation names.
pub fn parse_op_line(line: &str) -> Result<(ProviderOperation, OpArgument), &'static str> {
    let rest = line
        .strip_prefix(OP_LINE_PREFIX)
        .ok_or(PROVIDER_UNKNOWN_COMMAND)?;
    let (name, args) = rest.find('\t').map_or((rest, None), |index| {
        (&rest[..index], Some(&rest[index + 1..]))
    });
    let operation = ProviderOperation::parse(name)?;
    if operation.requires_envelope() {
        return Err(PROVIDER_ENVELOPE_REQUIRED);
    }
    match (operation, args) {
        (ProviderOperation::Status, None) => Ok((operation, OpArgument::None)),
        (ProviderOperation::Cancel, Some(arg)) => {
            let raw = decode_hex_exact(arg, 16).ok_or(PROVIDER_UNKNOWN_COMMAND)?;
            let mut id = [0_u8; 16];
            id.copy_from_slice(&raw);
            Ok((
                operation,
                OpArgument::CancelTarget(RequestId::from_bytes(id)),
            ))
        }
        (
            ProviderOperation::Ingest | ProviderOperation::Query | ProviderOperation::Expand,
            Some(arg),
        ) => Ok((operation, OpArgument::Blob(decode_op_hex(arg)?))),
        _ => Err(PROVIDER_UNKNOWN_COMMAND),
    }
}

/// Parses `op\thello[\t<min_major>.<min_minor>-<maj>.<min>]`.
///
/// The range form is strict digits with no whitespace; a bare `op\thello`
/// assumes the exact daemon range.
pub fn parse_hello_line(line: &str) -> Result<Option<ProtocolRange>, &'static str> {
    let rest = line
        .strip_prefix(OP_LINE_PREFIX)
        .ok_or(PROVIDER_UNKNOWN_COMMAND)?;
    let (name, args) = rest.find('\t').map_or((rest, None), |index| {
        (&rest[..index], Some(&rest[index + 1..]))
    });
    if name != "hello" {
        return Err(PROVIDER_UNKNOWN_COMMAND);
    }
    args.map(parse_client_range).transpose()
}

/// Parses a strict `MAJ.MIN-MAJ.MIN` client range with `min <= max`.
pub fn parse_client_range(text: &str) -> Result<ProtocolRange, &'static str> {
    if text.is_empty()
        || text.len() > 23
        || !text
            .bytes()
            .all(|byte| byte.is_ascii_digit() || matches!(byte, b'.' | b'-'))
    {
        return Err(PROVIDER_UNKNOWN_COMMAND);
    }
    let (min_text, max_text) = text.split_once('-').ok_or(PROVIDER_UNKNOWN_COMMAND)?;
    let parse_version = |part: &str| {
        let (major, minor) = part.split_once('.').ok_or(PROVIDER_UNKNOWN_COMMAND)?;
        if major.is_empty() || minor.is_empty() || major.len() > 5 || minor.len() > 5 {
            return Err(PROVIDER_UNKNOWN_COMMAND);
        }
        if (major.len() > 1 && major.starts_with('0'))
            || (minor.len() > 1 && minor.starts_with('0'))
        {
            return Err(PROVIDER_UNKNOWN_COMMAND);
        }
        let major = major.parse::<u16>().map_err(|_| PROVIDER_UNKNOWN_COMMAND)?;
        let minor = minor.parse::<u16>().map_err(|_| PROVIDER_UNKNOWN_COMMAND)?;
        Ok(ProtocolVersion { major, minor })
    };
    let minimum = parse_version(min_text)?;
    let maximum = parse_version(max_text)?;
    ProtocolRange::new(minimum, maximum).map_err(|_| PROVIDER_UNKNOWN_COMMAND)
}

/// Negotiates the connection version between the daemon range and the client.
pub fn negotiate_connection_version(
    client: ProtocolRange,
) -> Result<ProtocolVersion, ProtocolError> {
    negotiate_hello(PROVIDER_PROTOCOL_RANGE, client)
}

/// Parses one `envelope\t<seq>\t<hex>` line.
///
/// The client sequence is a strict decimal `u64`; the frame hex is lowercase,
/// even-length and bounded by [`MAX_ENVELOPE_HEX`].
pub fn parse_envelope_line(line: &str) -> Result<(u64, Vec<u8>), &'static str> {
    let rest = line
        .strip_prefix(ENVELOPE_LINE_PREFIX)
        .ok_or(PROVIDER_UNKNOWN_COMMAND)?;
    let (seq_text, hex) = rest.split_once('\t').ok_or(PROVIDER_UNKNOWN_COMMAND)?;
    if seq_text.is_empty()
        || seq_text.len() > 20
        || !seq_text.bytes().all(|byte| byte.is_ascii_digit())
        || (seq_text.len() > 1 && seq_text.starts_with('0'))
    {
        return Err(protocol_reason(ProtocolError::InvalidEnvelope));
    }
    let sequence = seq_text
        .parse::<u64>()
        .map_err(|_| protocol_reason(ProtocolError::InvalidEnvelope))?;
    if hex.len() > MAX_ENVELOPE_HEX {
        return Err(protocol_reason(ProtocolError::FrameTooLarge));
    }
    let frame = decode_hex(hex).ok_or(protocol_reason(ProtocolError::InvalidEnvelope))?;
    Ok((sequence, frame))
}

/// Decodes one framed envelope after prefix validation against the daemon range.
pub fn decode_envelope_frame(frame: &[u8]) -> Result<AuthenticatedEnvelope, ProtocolError> {
    decode_envelope(frame, DEFAULT_PROTOCOL_LIMITS, PROVIDER_PROTOCOL_RANGE)
}

/// Encodes one envelope response to transmittable frame bytes.
pub fn encode_response_frame(response: &AuthenticatedResponse) -> Result<Vec<u8>, ProtocolError> {
    encode_response(response, DEFAULT_PROTOCOL_LIMITS).map(|bounded| bounded.as_slice().to_vec())
}

fn decode_op_hex(arg: &str) -> Result<Vec<u8>, &'static str> {
    if arg.len() > MAX_OP_ARG_HEX {
        return Err(protocol_reason(ProtocolError::FrameTooLarge));
    }
    decode_hex(arg).ok_or(PROVIDER_UNKNOWN_COMMAND)
}

fn decode_hex_exact(arg: &str, expected_bytes: usize) -> Option<Vec<u8>> {
    if arg.len() != expected_bytes * 2 {
        return None;
    }
    decode_hex(arg)
}

fn decode_hex(text: &str) -> Option<Vec<u8>> {
    if !text.len().is_multiple_of(2) || text.is_empty() {
        return None;
    }
    let mut output = Vec::with_capacity(text.len() / 2);
    let bytes = text.as_bytes();
    let mut index = 0;
    while index < bytes.len() {
        let high = hex_value(bytes[index])?;
        let low = hex_value(bytes[index + 1])?;
        output.push((high << 4) | low);
        index += 2;
    }
    Some(output)
}

const fn hex_value(byte: u8) -> Option<u8> {
    match byte {
        b'0'..=b'9' => Some(byte - b'0'),
        b'a'..=b'f' => Some(byte - b'a' + 10),
        _ => None,
    }
}

/// Lowercase hex encoding for non-secret framing bytes.
#[must_use]
pub fn hex_encode(bytes: &[u8]) -> String {
    const HEX: &[u8; 16] = b"0123456789abcdef";
    let mut output = String::with_capacity(bytes.len() * 2);
    for byte in bytes {
        output.push(char::from(HEX[usize::from(byte >> 4)]));
        output.push(char::from(HEX[usize::from(byte & 0x0F)]));
    }
    output
}

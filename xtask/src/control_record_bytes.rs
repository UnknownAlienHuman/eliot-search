//! Exact byte verification for signed TOML control-record digest boundaries.
//!
//! This primitive checks canonical record bytes and the embedded payload digest.
//! It does not validate a record schema, actor, signature, trust store or authority.

use std::fmt;

use sha2::{Digest, Sha256};
use toml::Value;

#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq)]
pub struct SignedPayloadDigest([u8; 32]);

impl SignedPayloadDigest {
    #[must_use]
    pub fn to_hex(self) -> String {
        digest_hex(&self.0)
    }
}

#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq)]
pub struct ExactRecordFileDigest([u8; 32]);

impl ExactRecordFileDigest {
    #[must_use]
    pub fn to_hex(self) -> String {
        digest_hex(&self.0)
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct VerifiedControlRecordBytes {
    pub signed_payload_sha256: SignedPayloadDigest,
    pub exact_record_file_sha256: ExactRecordFileDigest,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ControlRecordBytesError {
    ZeroByteLimit,
    ByteLimitExceeded,
    ByteOrderMarkForbidden,
    CarriageReturnForbidden,
    InvalidUtf8,
    FinalLineFeedRequired,
    TrailingWhitespaceForbidden,
    CommentsForbidden,
    InvalidToml,
    SignatureHeaderInvalid,
    SignatureNotTerminal,
    RecordDigestMissing,
    PayloadDigestMismatch,
}

impl fmt::Display for ControlRecordBytesError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        let message = match self {
            Self::ZeroByteLimit => "caller byte limit must be positive",
            Self::ByteLimitExceeded => "record exceeds caller byte limit",
            Self::ByteOrderMarkForbidden => "UTF-8 byte-order mark is forbidden",
            Self::CarriageReturnForbidden => "carriage-return bytes are forbidden",
            Self::InvalidUtf8 => "record is not valid UTF-8",
            Self::FinalLineFeedRequired => "record must end with a line feed",
            Self::TrailingWhitespaceForbidden => "trailing whitespace is forbidden",
            Self::CommentsForbidden => "TOML comments are forbidden",
            Self::InvalidToml => "record is not valid TOML",
            Self::SignatureHeaderInvalid => "record must contain one canonical signature table",
            Self::SignatureNotTerminal => "signature table is not the final table boundary",
            Self::RecordDigestMissing => "signature record digest is missing",
            Self::PayloadDigestMismatch => "signature record digest does not match payload bytes",
        };
        formatter.write_str(message)
    }
}

impl std::error::Error for ControlRecordBytesError {}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
struct SignatureBoundary {
    header_offset: usize,
    header_count: usize,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum StringMode {
    Basic,
    Literal,
    MultilineBasic,
    MultilineLiteral,
}

/// Verifies exact canonical bytes and both non-interchangeable record digests.
///
/// The positive byte limit is supplied by the caller and checked before UTF-8
/// decoding or TOML parsing. The returned digests are computed from `raw`
/// without normalization or reserialization.
pub fn verify_control_record_bytes(
    raw: &[u8],
    max_bytes: usize,
) -> Result<VerifiedControlRecordBytes, ControlRecordBytesError> {
    if max_bytes == 0 {
        return Err(ControlRecordBytesError::ZeroByteLimit);
    }
    if raw.len() > max_bytes {
        return Err(ControlRecordBytesError::ByteLimitExceeded);
    }
    if raw.starts_with(&[0xEF, 0xBB, 0xBF]) {
        return Err(ControlRecordBytesError::ByteOrderMarkForbidden);
    }
    if raw.contains(&b'\r') {
        return Err(ControlRecordBytesError::CarriageReturnForbidden);
    }
    let text = std::str::from_utf8(raw).map_err(|_| ControlRecordBytesError::InvalidUtf8)?;
    if !raw.ends_with(b"\n") {
        return Err(ControlRecordBytesError::FinalLineFeedRequired);
    }
    if has_trailing_whitespace(text) {
        return Err(ControlRecordBytesError::TrailingWhitespaceForbidden);
    }

    let parsed = toml::from_str::<Value>(text).map_err(|_| ControlRecordBytesError::InvalidToml)?;
    let boundary = scan_record_structure(raw)?;
    if boundary.header_count != 1
        || boundary.header_offset == 0
        || raw[boundary.header_offset - 1] != b'\n'
    {
        return Err(ControlRecordBytesError::SignatureHeaderInvalid);
    }

    let signature = parsed
        .get("signature")
        .and_then(Value::as_table)
        .ok_or(ControlRecordBytesError::SignatureHeaderInvalid)?;
    let embedded_digest = signature
        .get("record_sha256")
        .and_then(Value::as_str)
        .ok_or(ControlRecordBytesError::RecordDigestMissing)?;

    let signed_payload_sha256 = SignedPayloadDigest(sha256(&raw[..boundary.header_offset]));
    if embedded_digest != signed_payload_sha256.to_hex() {
        return Err(ControlRecordBytesError::PayloadDigestMismatch);
    }

    Ok(VerifiedControlRecordBytes {
        signed_payload_sha256,
        exact_record_file_sha256: ExactRecordFileDigest(sha256(raw)),
    })
}

fn has_trailing_whitespace(text: &str) -> bool {
    text.split_terminator('\n')
        .any(|line| line.chars().next_back().is_some_and(char::is_whitespace))
}

fn scan_record_structure(raw: &[u8]) -> Result<SignatureBoundary, ControlRecordBytesError> {
    let mut index = 0;
    let mut line_start = 0;
    let mut line_prefix = true;
    let mut header_line = false;
    let mut value_pending = false;
    let mut array_depth = 0_usize;
    let mut inline_table_depth = 0_usize;
    let mut string_mode = None;
    let mut signature_offset = None;
    let mut signature_count = 0_usize;

    while let Some(&byte) = raw.get(index) {
        if byte == b'\n' {
            line_start = index + 1;
            line_prefix = true;
            header_line = false;
            index += 1;
            continue;
        }

        if let Some(mode) = string_mode {
            match mode {
                StringMode::Basic | StringMode::Literal => {
                    let delimiter = if mode == StringMode::Basic {
                        b'"'
                    } else {
                        b'\''
                    };
                    if mode == StringMode::Basic && byte == b'\\' {
                        index += 1;
                        line_prefix = false;
                        if index < raw.len() {
                            index += 1;
                            line_prefix = false;
                        }
                    } else if byte == delimiter {
                        string_mode = None;
                        index += 1;
                        line_prefix = false;
                    } else {
                        index += 1;
                        line_prefix = false;
                    }
                }
                StringMode::MultilineBasic | StringMode::MultilineLiteral => {
                    let delimiter = if mode == StringMode::MultilineBasic {
                        b'"'
                    } else {
                        b'\''
                    };
                    if mode == StringMode::MultilineBasic && byte == b'\\' {
                        index += 1;
                        line_prefix = false;
                        if index < raw.len() {
                            let escaped = raw[index];
                            if escaped == b'\n' {
                                line_start = index + 1;
                                line_prefix = true;
                            } else {
                                line_prefix = false;
                            }
                            index += 1;
                        }
                    } else if byte == delimiter {
                        let run_end = quote_run_end(raw, index, delimiter);
                        let run_length = run_end - index;
                        if run_length >= 3 {
                            string_mode = None;
                        }
                        index = run_end;
                        line_prefix = false;
                    } else {
                        index += 1;
                        line_prefix = false;
                    }
                }
            }
            continue;
        }

        if line_prefix {
            if byte == b' ' || byte == b'\t' {
                index += 1;
                continue;
            }
            if array_depth == 0 && inline_table_depth == 0 && !value_pending && byte == b'[' {
                if signature_offset.is_some() {
                    return Err(ControlRecordBytesError::SignatureNotTerminal);
                }
                let line_end = raw[index..]
                    .iter()
                    .position(|candidate| *candidate == b'\n')
                    .map_or(raw.len(), |length| index + length);
                if index == line_start && &raw[index..line_end] == b"[signature]" {
                    signature_count += 1;
                    signature_offset = Some(index);
                }
                header_line = true;
                index += 1;
                continue;
            }
        }

        if byte == b'#' {
            return Err(ControlRecordBytesError::CommentsForbidden);
        }

        if byte == b'"' {
            value_pending = false;
            if raw[index..].starts_with(b"\"\"\"") {
                string_mode = Some(StringMode::MultilineBasic);
                index += 3;
            } else {
                string_mode = Some(StringMode::Basic);
                index += 1;
            }
            line_prefix = false;
            continue;
        }
        if byte == b'\'' {
            value_pending = false;
            if raw[index..].starts_with(b"'''") {
                string_mode = Some(StringMode::MultilineLiteral);
                index += 3;
            } else {
                string_mode = Some(StringMode::Literal);
                index += 1;
            }
            line_prefix = false;
            continue;
        }

        let was_value_pending = value_pending;
        if value_pending && byte != b' ' && byte != b'\t' {
            value_pending = false;
        }

        if !header_line {
            match byte {
                b'=' if array_depth == 0 && inline_table_depth == 0 => {
                    value_pending = true;
                }
                b'[' if was_value_pending || array_depth > 0 || inline_table_depth > 0 => {
                    array_depth += 1;
                }
                b']' if array_depth > 0 => {
                    array_depth -= 1;
                }
                b'{' if was_value_pending || array_depth > 0 || inline_table_depth > 0 => {
                    inline_table_depth += 1;
                }
                b'}' if inline_table_depth > 0 => {
                    inline_table_depth -= 1;
                }
                _ => {}
            }
        }

        index += 1;
        line_prefix = false;
    }

    let header_offset = signature_offset.ok_or(ControlRecordBytesError::SignatureHeaderInvalid)?;
    Ok(SignatureBoundary {
        header_offset,
        header_count: signature_count,
    })
}

fn quote_run_end(raw: &[u8], start: usize, quote: u8) -> usize {
    let mut end = start;
    while raw.get(end) == Some(&quote) {
        end += 1;
    }
    end
}

fn sha256(raw: &[u8]) -> [u8; 32] {
    Sha256::digest(raw).into()
}

fn digest_hex(digest: &[u8; 32]) -> String {
    const HEX: &[u8; 16] = b"0123456789abcdef";
    let mut output = String::with_capacity(64);
    for byte in digest {
        output.push(char::from(HEX[usize::from(byte >> 4)]));
        output.push(char::from(HEX[usize::from(byte & 0x0F)]));
    }
    output
}

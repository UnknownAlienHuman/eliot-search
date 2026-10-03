//! Canonical JSON and domain-separated ticket-planner digests.

use serde_json::Value;
use sha2::{Digest, Sha256};

use super::spec::{DOMAIN_SEPARATOR, PLANNER_FILE_BYTE_CEILING};

/// SHA-256 hex of raw bytes.
#[must_use]
pub fn exact_sha256_hex(data: &[u8]) -> String {
    let mut hasher = Sha256::new();
    hasher.update(data);
    format!("{:x}", hasher.finalize())
}

fn append_json_escaped(output: &mut Vec<u8>, text: &str) {
    const HEX: &[u8; 16] = b"0123456789abcdef";
    output.push(b'"');
    for character in text.chars() {
        match character {
            '"' => output.extend_from_slice(b"\\\""),
            '\\' => output.extend_from_slice(b"\\\\"),
            '\u{08}' => output.extend_from_slice(b"\\b"),
            '\u{09}' => output.extend_from_slice(b"\\t"),
            '\u{0A}' => output.extend_from_slice(b"\\n"),
            '\u{0C}' => output.extend_from_slice(b"\\f"),
            '\u{0D}' => output.extend_from_slice(b"\\r"),
            value if (value as u32) < 0x20 => {
                output.extend_from_slice(b"\\u00");
                let byte = value as u8;
                output.push(HEX[usize::from(byte >> 4)]);
                output.push(HEX[usize::from(byte & 0x0f)]);
            }
            value => {
                let mut buffer = [0_u8; 4];
                output.extend_from_slice(value.encode_utf8(&mut buffer).as_bytes());
            }
        }
    }
    output.push(b'"');
}

fn append_canonical(output: &mut Vec<u8>, value: &Value) {
    match value {
        Value::Null => output.extend_from_slice(b"null"),
        Value::Bool(true) => output.extend_from_slice(b"true"),
        Value::Bool(false) => output.extend_from_slice(b"false"),
        Value::Number(number) => {
            if let Some(signed) = number.as_i64() {
                output.extend_from_slice(signed.to_string().as_bytes());
            } else if let Some(unsigned) = number.as_u64() {
                output.extend_from_slice(unsigned.to_string().as_bytes());
            } else if let Some(float) = number.as_f64() {
                // Floats are outside the pinned planner domain.
                output.extend_from_slice(float.to_string().as_bytes());
            }
        }
        Value::String(text) => append_json_escaped(output, text),
        Value::Array(items) => {
            output.push(b'[');
            for (index, item) in items.iter().enumerate() {
                if index > 0 {
                    output.push(b',');
                }
                append_canonical(output, item);
            }
            output.push(b']');
        }
        Value::Object(map) => {
            output.push(b'{');
            for (index, (key, item)) in map.iter().enumerate() {
                if index > 0 {
                    output.push(b',');
                }
                append_json_escaped(output, key);
                output.push(b':');
                append_canonical(output, item);
            }
            output.push(b'}');
        }
    }
}

/// Canonical JSON bytes plus a trailing newline.
///
/// This matches `json.dumps(..., ensure_ascii=False, sort_keys=True,
/// separators=(",", ":"))` for the planner's float-free payload domain.
#[must_use]
pub fn canonical_json_bytes(value: &Value) -> Vec<u8> {
    let mut output = Vec::new();
    append_canonical(&mut output, value);
    output.push(b'\n');
    output
}

/// SHA-256 over the planner domain separator and canonical plan bytes.
#[must_use]
pub fn plan_digest(payload_without_digest: &Value) -> String {
    let mut hasher = Sha256::new();
    hasher.update(DOMAIN_SEPARATOR);
    hasher.update(canonical_json_bytes(payload_without_digest));
    format!("{:x}", hasher.finalize())
}

/// Compatibility wrapper returning the signed-payload digest for a canonical
/// signed TOML record. Uses the existing planner per-file read ceiling.
#[must_use]
pub fn signed_payload_digest(raw: &[u8]) -> Option<String> {
    let max_bytes = usize::try_from(PLANNER_FILE_BYTE_CEILING).ok()?;
    let verified = crate::control_record_bytes::verify_control_record_bytes(raw, max_bytes).ok()?;
    Some(verified.signed_payload_sha256.to_hex())
}

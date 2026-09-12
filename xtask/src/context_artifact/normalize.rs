//! Source normalization and canonical JSON-value fencing.

use serde_json::Value;

use super::error::ContextArtifactError;

/// Strict UTF-8 decode, NUL fence and CRLF/CR to LF normalization.
///
/// # Errors
///
/// Returns `CONTEXT_SOURCE_NOT_UTF8` for invalid UTF-8 or
/// `CONTEXT_SOURCE_CONTAINS_NUL` when the decoded source contains NUL.
pub fn normalize_utf8_lf(raw: &[u8]) -> Result<Vec<u8>, ContextArtifactError> {
    let text = std::str::from_utf8(raw).map_err(|_| {
        ContextArtifactError::new(
            "CONTEXT_SOURCE_NOT_UTF8",
            "source is not strict UTF-8",
        )
    })?;
    if text.contains('\0') {
        return Err(ContextArtifactError::new(
            "CONTEXT_SOURCE_CONTAINS_NUL",
            "source contains NUL",
        ));
    }
    Ok(text.replace("\r\n", "\n").replace('\r', "\n").into_bytes())
}

fn require_json_inner(value: &Value) -> bool {
    match value {
        Value::Null => false,
        Value::Bool(_) | Value::String(_) => true,
        Value::Number(number) => number.is_i64() || number.is_u64(),
        Value::Array(items) => items.iter().all(require_json_inner),
        Value::Object(map) => map.values().all(require_json_inner),
    }
}

/// Rejects null and floating-point values recursively.
///
/// # Errors
///
/// Returns `REGISTRY_FRAGMENT_NONCANONICAL` for forbidden values.
pub fn require_json_value(value: &Value) -> Result<(), ContextArtifactError> {
    if require_json_inner(value) {
        Ok(())
    } else {
        Err(ContextArtifactError::new(
            "REGISTRY_FRAGMENT_NONCANONICAL",
            "value contains a forbidden null or floating-point value",
        ))
    }
}

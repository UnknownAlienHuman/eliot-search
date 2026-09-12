//! Error-producing scalar requirements.

use serde_json::Value;

use super::format::{
    RFC3339_DIGIT_POSITIONS, actor_identity_valid, opaque_id_valid,
    rfc3339_valid, sha256_hex_valid,
};
use crate::context_materialization::MaterializationPlanError;

/// Requires one lowercase SHA-256 hex value.
///
/// # Errors
///
/// Returns `MATERIALIZATION_INPUT_INVALID` for invalid input.
pub fn require_sha(
    value: &Value,
    label: &str,
) -> Result<String, MaterializationPlanError> {
    match value {
        Value::String(text) if sha256_hex_valid(text) => Ok(text.clone()),
        _ => Err(MaterializationPlanError::new(
            "MATERIALIZATION_INPUT_INVALID",
            format!("{label} is not SHA-256"),
        )),
    }
}

/// Requires one bounded opaque identifier.
///
/// # Errors
///
/// Returns `MATERIALIZATION_INPUT_INVALID` for invalid input.
pub fn require_opaque(
    value: &Value,
    label: &str,
) -> Result<String, MaterializationPlanError> {
    match value {
        Value::String(text) if opaque_id_valid(text) => Ok(text.clone()),
        _ => Err(MaterializationPlanError::new(
            "MATERIALIZATION_INPUT_INVALID",
            format!("{label} is not OpaqueId"),
        )),
    }
}

/// Requires one canonical actor identity.
///
/// # Errors
///
/// Returns `MATERIALIZATION_INPUT_INVALID` for invalid input.
pub fn require_actor(
    value: &Value,
    label: &str,
) -> Result<String, MaterializationPlanError> {
    match value {
        Value::String(text) if actor_identity_valid(text) => Ok(text.clone()),
        _ => Err(MaterializationPlanError::new(
            "MATERIALIZATION_INPUT_INVALID",
            format!("{label} is not ActorIdentity"),
        )),
    }
}

/// Requires whole-second UTC RFC3339 with a valid calendar date.
///
/// # Errors
///
/// Returns `MATERIALIZATION_INPUT_INVALID` for invalid input.
pub fn require_rfc3339(
    value: &Value,
    label: &str,
) -> Result<String, MaterializationPlanError> {
    let Some(text) = value.as_str() else {
        return Err(MaterializationPlanError::new(
            "MATERIALIZATION_INPUT_INVALID",
            format!("{label} is not whole-second UTC RFC3339"),
        ));
    };
    let bytes = text.as_bytes();
    let shape = bytes.len() == 20
        && bytes[4] == b'-'
        && bytes[7] == b'-'
        && bytes[10] == b'T'
        && bytes[13] == b':'
        && bytes[16] == b':'
        && bytes[19] == b'Z'
        && RFC3339_DIGIT_POSITIONS
            .iter()
            .all(|position| bytes[*position].is_ascii_digit());
    if !shape {
        return Err(MaterializationPlanError::new(
            "MATERIALIZATION_INPUT_INVALID",
            format!("{label} is not whole-second UTC RFC3339"),
        ));
    }
    if !rfc3339_valid(text) {
        return Err(MaterializationPlanError::new(
            "MATERIALIZATION_INPUT_INVALID",
            format!("{label} is not a valid calendar timestamp"),
        ));
    }
    Ok(text.to_owned())
}

/// Requires a JSON integer in `0..=u64::MAX`.
///
/// # Errors
///
/// Returns `MATERIALIZATION_INPUT_INVALID` for invalid input.
pub fn require_u64(
    value: &Value,
    label: &str,
) -> Result<u64, MaterializationPlanError> {
    let invalid = || {
        MaterializationPlanError::new(
            "MATERIALIZATION_INPUT_INVALID",
            format!("{label} is not u64"),
        )
    };
    match value {
        Value::Number(number) => match (number.as_u64(), number.as_i64()) {
            (Some(unsigned), _) => Ok(unsigned),
            (None, Some(signed)) => u64::try_from(signed).map_err(|_| invalid()),
            (None, None) => Err(invalid()),
        },
        _ => Err(invalid()),
    }
}

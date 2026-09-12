//! Validation of immutable artifact and optional-signature references.

use serde_json::Value;

use super::model::{ArtifactRef, OptionalSignature, SignatureValue};
use super::super::error::MaterializationPlanError;
use super::super::scalars::{
    require_actor, require_opaque, require_sha, require_u64,
};

/// Validates exact artifact-reference fields, grammars and byte identity.
///
/// # Errors
///
/// Returns `MATERIALIZATION_ARTIFACT_REF_INVALID` for an invalid field set,
/// `MATERIALIZATION_INPUT_INVALID` for scalar failures, or
/// `MATERIALIZATION_ARTIFACT_READBACK_MISMATCH` when the reference does not
/// identify `bundle_bytes`.
pub fn validate_artifact_ref(
    value: &Value,
    bundle_bytes: &[u8],
) -> Result<ArtifactRef, MaterializationPlanError> {
    let Value::Object(map) = value else {
        return Err(MaterializationPlanError::new(
            "MATERIALIZATION_ARTIFACT_REF_INVALID",
            "artifact_ref field set is invalid",
        ));
    };
    if map.len() != 4
        || !map.contains_key("store_profile_ref")
        || !map.contains_key("artifact_id")
        || !map.contains_key("bytes")
        || !map.contains_key("sha256")
    {
        return Err(MaterializationPlanError::new(
            "MATERIALIZATION_ARTIFACT_REF_INVALID",
            "artifact_ref field set is invalid",
        ));
    }
    let result = ArtifactRef {
        store_profile_ref: require_opaque(
            &map["store_profile_ref"],
            "artifact_ref.store_profile_ref",
        )?,
        artifact_id: require_opaque(
            &map["artifact_id"],
            "artifact_ref.artifact_id",
        )?,
        bytes: require_u64(&map["bytes"], "artifact_ref.bytes")?,
        sha256: require_sha(&map["sha256"], "artifact_ref.sha256")?,
    };
    if result.bytes != u64::try_from(bundle_bytes.len()).unwrap_or(u64::MAX)
        || result.sha256
            != crate::ticket_planner::exact_sha256_hex(bundle_bytes)
    {
        return Err(MaterializationPlanError::new(
            "MATERIALIZATION_ARTIFACT_READBACK_MISMATCH",
            "artifact_ref does not identify bundle bytes",
        ));
    }
    Ok(result)
}

/// Validates one `OptionalV1` signature reference and actor binding.
///
/// # Errors
///
/// Returns `MATERIALIZATION_SIGNATURE_REF_INVALID` for invalid shape/grammar,
/// or `MATERIALIZATION_SIGNATURE_ACTOR_MISMATCH` when the embedded actor
/// differs from `expected_actor`.
pub fn validate_optional_signature(
    value: &Value,
    expected_actor: &str,
    label: &str,
) -> Result<OptionalSignature, MaterializationPlanError> {
    let invalid = |message: String| {
        MaterializationPlanError::new(
            "MATERIALIZATION_SIGNATURE_REF_INVALID",
            message,
        )
    };
    let Value::Object(map) = value else {
        return Err(invalid(format!("{label} OptionalV1 is invalid")));
    };
    if map.len() != 2
        || !map.contains_key("state")
        || !map.contains_key("value")
    {
        return Err(invalid(format!("{label} OptionalV1 is invalid")));
    }
    let Some(Value::String(state)) = map.get("state") else {
        return Err(invalid(format!("{label} state/value is invalid")));
    };
    if state == "ABSENT" {
        if map["value"] != Value::String(String::new()) {
            return Err(invalid(format!(
                "{label} ABSENT requires empty string"
            )));
        }
        return Ok(OptionalSignature {
            state: state.clone(),
            value: None,
        });
    }
    let Value::Object(wrapped) = &map["value"] else {
        return Err(invalid(format!("{label} state/value is invalid")));
    };
    if state != "PRESENT" {
        return Err(invalid(format!("{label} state/value is invalid")));
    }
    if wrapped.len() != 4
        || !wrapped.contains_key("approval_profile_ref")
        || !wrapped.contains_key("approval_artifact_ref")
        || !wrapped.contains_key("signed_payload_sha256")
        || !wrapped.contains_key("actor_identity")
    {
        return Err(invalid(format!("{label} field set is invalid")));
    }
    let actor = require_actor(
        &wrapped["actor_identity"],
        &format!("{label}.actor_identity"),
    )?;
    if actor != expected_actor {
        return Err(MaterializationPlanError::new(
            "MATERIALIZATION_SIGNATURE_ACTOR_MISMATCH",
            format!("{label} actor differs from selected actor"),
        ));
    }
    let Value::Object(approval) = &wrapped["approval_artifact_ref"] else {
        return Err(invalid(format!(
            "{label}.approval_artifact_ref is invalid"
        )));
    };
    if approval.len() != 4
        || !approval.contains_key("store_profile_ref")
        || !approval.contains_key("artifact_id")
        || !approval.contains_key("bytes")
        || !approval.contains_key("sha256")
    {
        return Err(invalid(format!(
            "{label}.approval_artifact_ref is invalid"
        )));
    }
    Ok(OptionalSignature {
        state: state.clone(),
        value: Some(SignatureValue {
            approval_profile_ref: require_opaque(
                &wrapped["approval_profile_ref"],
                &format!("{label}.approval_profile_ref"),
            )?,
            approval_artifact_ref: ArtifactRef {
                store_profile_ref: require_opaque(
                    &approval["store_profile_ref"],
                    &format!(
                        "{label}.approval_artifact_ref.store_profile_ref"
                    ),
                )?,
                artifact_id: require_opaque(
                    &approval["artifact_id"],
                    &format!("{label}.approval_artifact_ref.artifact_id"),
                )?,
                bytes: require_u64(
                    &approval["bytes"],
                    &format!("{label}.approval_artifact_ref.bytes"),
                )?,
                sha256: require_sha(
                    &approval["sha256"],
                    &format!("{label}.approval_artifact_ref.sha256"),
                )?,
            },
            signed_payload_sha256: require_sha(
                &wrapped["signed_payload_sha256"],
                &format!("{label}.signed_payload_sha256"),
            )?,
            actor_identity: actor,
        }),
    })
}

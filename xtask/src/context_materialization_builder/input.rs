//! Canonical candidate, bundle and external-selection loading.

use std::fs;
use std::path::Path;

use serde_json::Value;

use crate::context_artifact::{
    ARTIFACT_FORMAT, AUTHORITY_FIELDS, STATUS as CANDIDATE_STATUS,
    assert_candidate_digest, candidate_id, parse_bundle,
};
use crate::context_materialization::{
    ArtifactRef, MaterializationPlanError, require_actor, require_opaque,
    require_rfc3339, require_sha, require_u64, validate_artifact_ref,
    validate_optional_signature,
};
use crate::ticket_planner::{
    canonical_json_bytes, exact_sha256_hex, safe_path, under,
};

use super::model::{ArtifactReadback, CandidateInput, Selection};

const CANDIDATE_ROOT: &str = "artifacts/context-artifact-candidates";
const SELECTION_ROOT: &str = "artifacts/context-materialization-inputs";
const MAX_INPUT_BYTES: u64 = 20 * 1024 * 1024;

/// Loads and validates one candidate plus its exact bundle.
pub(super) fn load_candidate(
    root: &Path,
    candidate_path: &str,
    bundle_override: Option<&str>,
) -> Result<CandidateInput, MaterializationPlanError> {
    let candidate_path = checked_relative(candidate_path, CANDIDATE_ROOT)?;
    let candidate_bytes = read_regular(root, &candidate_path, MAX_INPUT_BYTES)?;
    let candidate = parse_canonical_json(&candidate_bytes, "candidate")?;
    validate_candidate_shape(&candidate)?;

    let declared_bundle = candidate
        .pointer("/artifact_candidate/relative_path")
        .and_then(Value::as_str)
        .ok_or_else(|| invalid("candidate artifact path is missing"))?;
    let bundle_path = checked_relative(
        bundle_override.unwrap_or(declared_bundle),
        CANDIDATE_ROOT,
    )?;
    if bundle_override.is_some_and(|value| value.replace('\\', "/") != declared_bundle) {
        return Err(MaterializationPlanError::new(
            "MATERIALIZATION_BUNDLE_MISMATCH",
            "bundle override differs from candidate artifact path",
        ));
    }
    let bundle = read_regular(root, &bundle_path, MAX_INPUT_BYTES)?;
    validate_candidate_bundle(&candidate, &bundle)?;
    let (preamble, blocks) = parse_bundle(&bundle).map_err(|error| {
        MaterializationPlanError::new(error.reason(), error.message())
    })?;
    validate_bundle_projection(&candidate, &preamble, &blocks)?;

    Ok(CandidateInput {
        candidate_path,
        bundle_path,
        candidate,
        bundle,
        preamble,
        blocks,
    })
}

/// Loads one optional canonical selection document.
pub(super) fn load_selection(
    root: &Path,
    relative: Option<&str>,
    bundle: &[u8],
) -> Result<Option<Selection>, MaterializationPlanError> {
    let Some(relative) = relative else {
        return Ok(None);
    };
    let relative = checked_relative(relative, SELECTION_ROOT)?;
    let raw = read_regular(root, &relative, 1024 * 1024)?;
    let value = parse_canonical_json(&raw, "selection")?;
    let Value::Object(map) = value else {
        return Err(invalid("selection must be an object"));
    };
    let expected = [
        "context_id",
        "created_at",
        "materializer_identity",
        "reviewer_identity",
        "artifact_ref",
        "artifact_readback",
        "materializer_signature_ref",
        "reviewer_signature_ref",
    ];
    if map.len() != expected.len() || expected.iter().any(|key| !map.contains_key(*key)) {
        return Err(invalid("selection field set is invalid"));
    }

    let context_id = require_opaque(&map["context_id"], "context_id")?;
    let created_at = require_rfc3339(&map["created_at"], "created_at")?;
    let materializer_identity = require_actor(
        &map["materializer_identity"],
        "materializer_identity",
    )?;
    let reviewer_identity = require_actor(
        &map["reviewer_identity"],
        "reviewer_identity",
    )?;
    if materializer_identity == reviewer_identity {
        return Err(MaterializationPlanError::new(
            "MATERIALIZATION_ACTOR_CONFLICT",
            "materializer and reviewer identities must differ",
        ));
    }
    let artifact_ref = validate_artifact_ref(&map["artifact_ref"], bundle)?;
    let readback = validate_readback(&map["artifact_readback"], &artifact_ref)?;
    let materializer_signature = validate_optional_signature(
        &map["materializer_signature_ref"],
        &materializer_identity,
        "materializer_signature_ref",
    )?;
    let reviewer_signature = validate_optional_signature(
        &map["reviewer_signature_ref"],
        &reviewer_identity,
        "reviewer_signature_ref",
    )?;

    Ok(Some(Selection {
        context_id,
        created_at,
        materializer_identity,
        reviewer_identity,
        artifact_ref,
        readback,
        materializer_signature,
        reviewer_signature,
    }))
}

fn validate_candidate_shape(candidate: &Value) -> Result<(), MaterializationPlanError> {
    let Value::Object(map) = candidate else {
        return Err(MaterializationPlanError::new(
            "MATERIALIZATION_CANDIDATE_INVALID",
            "candidate must be an object",
        ));
    };
    let basic = map.get("schema_version").and_then(Value::as_i64) == Some(1)
        && map.get("record_kind").and_then(Value::as_str)
            == Some("context_artifact_candidate_v1")
        && map.get("status").and_then(Value::as_str) == Some(CANDIDATE_STATUS)
        && map.get("reason_codes").and_then(Value::as_array).is_some_and(Vec::is_empty)
        && map
            .get("control_record_mutations")
            .and_then(Value::as_array)
            .is_some_and(Vec::is_empty)
        && assert_candidate_digest(candidate);
    if !basic {
        return Err(MaterializationPlanError::new(
            "MATERIALIZATION_CANDIDATE_INVALID",
            "candidate identity, status or digest failed",
        ));
    }
    let authority = map.get("authority").and_then(Value::as_object);
    if authority.is_none_or(|values| {
        values.len() != AUTHORITY_FIELDS.len()
            || AUTHORITY_FIELDS.iter().any(|field| {
                values.get(*field).and_then(Value::as_bool) != Some(false)
            })
    }) {
        return Err(MaterializationPlanError::new(
            "MATERIALIZATION_CANDIDATE_INVALID",
            "candidate authority ceiling failed",
        ));
    }
    if candidate
        .pointer("/verification/bundle_roundtrip_verified")
        .and_then(Value::as_bool)
        != Some(true)
        || candidate
            .pointer("/verification/authoritative_artifact_store_readback_verified")
            .and_then(Value::as_bool)
            != Some(false)
        || candidate
            .pointer("/manifest_projection/schema_instance")
            .and_then(Value::as_bool)
            != Some(false)
    {
        return Err(MaterializationPlanError::new(
            "MATERIALIZATION_CANDIDATE_INVALID",
            "candidate verification or manifest projection failed",
        ));
    }
    Ok(())
}

fn validate_candidate_bundle(
    candidate: &Value,
    bundle: &[u8],
) -> Result<(), MaterializationPlanError> {
    let artifact = candidate
        .get("artifact_candidate")
        .and_then(Value::as_object)
        .ok_or_else(|| invalid("artifact_candidate is missing"))?;
    let expected_digest = exact_sha256_hex(bundle);
    let expected_id = candidate_id(bundle);
    if artifact.get("format").and_then(Value::as_str) != Some(ARTIFACT_FORMAT)
        || artifact.get("bytes").and_then(Value::as_u64)
            != u64::try_from(bundle.len()).ok()
        || artifact.get("sha256").and_then(Value::as_str)
            != Some(expected_digest.as_str())
        || artifact
            .get("local_file_is_immutable_artifact_ref")
            .and_then(Value::as_bool)
            != Some(false)
        || candidate.get("candidate_id").and_then(Value::as_str)
            != Some(expected_id.as_str())
    {
        return Err(MaterializationPlanError::new(
            "MATERIALIZATION_BUNDLE_MISMATCH",
            "candidate bundle identity mismatch",
        ));
    }
    Ok(())
}

fn validate_bundle_projection(
    candidate: &Value,
    preamble: &Value,
    blocks: &[crate::context_artifact::BundleBlock],
) -> Result<(), MaterializationPlanError> {
    let candidate_base = candidate
        .pointer("/repository/base_commit")
        .and_then(Value::as_str);
    let candidate_package = candidate
        .pointer("/package/name")
        .and_then(Value::as_str);
    let source_count = blocks.iter().filter(|block| block.kind == "source").count();
    let fragment_count = blocks
        .iter()
        .filter(|block| block.kind == "registry_fragment")
        .count();
    let handoff_count = blocks
        .iter()
        .filter(|block| block.kind == "accepted_handoff")
        .count();
    let coherent = preamble.get("base_commit").and_then(Value::as_str) == candidate_base
        && preamble.get("package").and_then(Value::as_str) == candidate_package
        && preamble.get("source_count").and_then(Value::as_u64)
            == u64::try_from(source_count).ok()
        && preamble
            .get("registry_fragment_count")
            .and_then(Value::as_u64)
            == u64::try_from(fragment_count).ok()
        && preamble
            .get("accepted_handoff_count")
            .and_then(Value::as_u64)
            == u64::try_from(handoff_count).ok()
        && candidate.get("sources").and_then(Value::as_array).map(Vec::len)
            == Some(source_count)
        && candidate
            .get("registry_fragments")
            .and_then(Value::as_array)
            .map(Vec::len)
            == Some(fragment_count)
        && candidate
            .get("accepted_handoffs")
            .and_then(Value::as_array)
            .map(Vec::len)
            == Some(handoff_count);
    if coherent {
        Ok(())
    } else {
        Err(MaterializationPlanError::new(
            "MATERIALIZATION_BUNDLE_MISMATCH",
            "bundle preamble/counts differ from candidate",
        ))
    }
}

fn validate_readback(
    value: &Value,
    artifact: &ArtifactRef,
) -> Result<ArtifactReadback, MaterializationPlanError> {
    let Value::Object(map) = value else {
        return Err(invalid("artifact_readback must be an object"));
    };
    let expected = ["verified", "verifier_identity", "verified_at", "sha256", "bytes"];
    if map.len() != expected.len() || expected.iter().any(|key| !map.contains_key(*key)) {
        return Err(invalid("artifact_readback field set is invalid"));
    }
    let verifier_identity = require_actor(
        &map["verifier_identity"],
        "artifact_readback.verifier_identity",
    )?;
    let verified_at = require_rfc3339(
        &map["verified_at"],
        "artifact_readback.verified_at",
    )?;
    let sha256 = require_sha(&map["sha256"], "artifact_readback.sha256")?;
    let bytes = require_u64(&map["bytes"], "artifact_readback.bytes")?;
    if map["verified"].as_bool() != Some(true)
        || sha256 != artifact.sha256
        || bytes != artifact.bytes
    {
        return Err(MaterializationPlanError::new(
            "MATERIALIZATION_ARTIFACT_READBACK_MISMATCH",
            "artifact readback does not match immutable artifact ref",
        ));
    }
    Ok(ArtifactReadback {
        verifier_identity,
        verified_at,
        sha256,
        bytes,
    })
}

fn parse_canonical_json(
    raw: &[u8],
    label: &str,
) -> Result<Value, MaterializationPlanError> {
    let text = std::str::from_utf8(raw).map_err(|_| invalid(&format!("{label} is not UTF-8")))?;
    let value: Value = serde_json::from_str(text)
        .map_err(|error| invalid(&format!("{label} JSON is invalid: {error}")))?;
    if canonical_json_bytes(&value) != raw {
        return Err(MaterializationPlanError::new(
            "MATERIALIZATION_INPUT_NONCANONICAL",
            format!("{label} is not canonical compact JSON with terminal LF"),
        ));
    }
    Ok(value)
}

fn checked_relative(
    value: &str,
    prefix: &str,
) -> Result<String, MaterializationPlanError> {
    let normalized = value.replace('\\', "/");
    if safe_path(&normalized) && under(&normalized, prefix) {
        Ok(normalized)
    } else {
        Err(invalid(&format!("input path is outside {prefix}")))
    }
}

fn read_regular(
    root: &Path,
    relative: &str,
    max_bytes: u64,
) -> Result<Vec<u8>, MaterializationPlanError> {
    let root = fs::canonicalize(root).map_err(|error| {
        invalid(&format!("unable to canonicalize repository root: {error}"))
    })?;
    let path = root.join(relative.replace('/', std::path::MAIN_SEPARATOR_STR));
    let metadata = fs::symlink_metadata(&path)
        .map_err(|error| invalid(&format!("unable to inspect {relative}: {error}")))?;
    if metadata.file_type().is_symlink() || !metadata.is_file() || metadata.len() > max_bytes {
        return Err(invalid(&format!("input is not a bounded regular file: {relative}")));
    }
    fs::read(&path).map_err(|error| invalid(&format!("unable to read {relative}: {error}")))
}

fn invalid(message: &str) -> MaterializationPlanError {
    MaterializationPlanError::new("MATERIALIZATION_INPUT_INVALID", message)
}

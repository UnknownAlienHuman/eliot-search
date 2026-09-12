//! Deterministic context-artifact candidate assembly.

use std::path::Path;

use serde_json::{Map as JsonMap, Value as JsonValue, json};
use toml::Value as TomlValue;

use crate::context_artifact::{
    ARTIFACT_FORMAT, RECORD_KIND, SCHEMA_VERSION, STATUS,
    UNRESOLVED_MANIFEST_FIELDS, assert_candidate_digest, authority_map,
    candidate_id, candidate_metadata_digest, parse_bundle, render_bundle,
};
use crate::context_artifact_io::validate_output_root;
use crate::ticket_planner::{canonical_json_bytes, exact_sha256_hex};

use super::extract::extract;
use super::model::{CandidateBuild, ContextArtifactBuildError};
use super::preflight;

const MAX_CANDIDATE_METADATA_BYTES: usize = 1024 * 1024;
const MAX_PREFLIGHT_CHECKS: usize = 512;

/// Builds one deterministic non-authoritative candidate from an immutable Git
/// commit. This function performs no output write.
///
/// # Errors
///
/// Returns one closed builder failure for repository, draft, preflight,
/// extraction, bundle or output-root validation errors.
pub fn build_candidate(
    root: &Path,
    package: &str,
    base_commit: &str,
    accepted_handoffs: &[String],
    output_root: &str,
) -> Result<CandidateBuild, ContextArtifactBuildError> {
    let output = validate_output_root(root, output_root).map_err(map_output)?;
    let preflight = preflight::run(root, package, base_commit, accepted_handoffs)?;
    if preflight.checks.len() > MAX_PREFLIGHT_CHECKS {
        return Err(ContextArtifactBuildError::with_checks(
            "CONTEXT_BUDGET_EXCEEDED",
            "preflight check count exceeds the closed ceiling",
            preflight.checks,
        ));
    }
    let extracted = extract(&preflight, package)?;
    let stage = toml_text(&preflight.pair.ticket, "stage")?;
    let phase = toml_text(&preflight.pair.ticket, "phase")?;
    let wave = toml_integer(&preflight.pair.ticket, "wave")?;

    let preamble = json!({
        "artifact_format": ARTIFACT_FORMAT,
        "repository": "UnknownAlienHuman/eliot-search",
        "base_commit": preflight.tree.tagged_commit(),
        "package": package,
        "package_path": preflight.package_path,
        "stage": stage,
        "phase": phase,
        "wave": wave,
        "context_draft_path": preflight.pair.context_path,
        "context_draft_git_blob_id": preflight.pair.context_blob,
        "context_draft_exact_sha256": preflight.pair.context_sha256,
        "source_count": extracted.sources.len(),
        "registry_fragment_count": extracted.fragments.len(),
        "accepted_handoff_count": extracted.handoffs.len(),
        "required_unavailable_checks": preflight.pair.unavailable_checks,
    });
    let bundle_bytes = render_bundle(&preamble, &extracted.blocks).map_err(map_primitive)?;
    let (roundtrip_preamble, roundtrip_blocks) =
        parse_bundle(&bundle_bytes).map_err(map_primitive)?;
    if roundtrip_preamble != preamble || roundtrip_blocks != extracted.blocks {
        return Err(ContextArtifactBuildError::new(
            "BUNDLE_FORMAT_INVALID",
            "bundle round-trip changed semantic structure",
        ));
    }

    let identifier = candidate_id(&bundle_bytes);
    let bundle_relative = format!(
        "{}/{package}/{identifier}.context",
        output.relative()
    );
    let candidate_relative = format!(
        "{}/{package}/{identifier}.json",
        output.relative()
    );
    let artifact_sha256 = exact_sha256_hex(&bundle_bytes);
    let preflight_checks: Vec<JsonValue> = preflight
        .checks
        .iter()
        .map(super::model::CandidateCheck::as_json)
        .collect();

    let manifest_projection = json!({
        "target_record_kind": "context_manifest_v1",
        "schema_instance": false,
        "status": "PROJECTION_REQUIRES_EXTERNAL_STORE_DUAL_SIGNATURE_AND_COMMIT",
        "known": {
            "identity.package": package,
            "identity.stage": stage,
            "identity.wave": wave,
            "identity.base_commit": preflight.tree.tagged_commit(),
            "draft.path": preflight.pair.context_path,
            "draft.git_blob_id": preflight.pair.context_blob,
            "draft.exact_file_sha256": preflight.pair.context_sha256,
            "artifact.sha256": artifact_sha256,
            "artifact.bytes": bundle_bytes.len(),
            "artifact.format": ARTIFACT_FORMAT,
            "sources": extracted.sources,
            "registry_fragments": extracted.fragments,
            "accepted_handoff_inputs": extracted.handoffs,
            "verification.source_count": preamble["source_count"],
            "verification.registry_fragment_count": preamble["registry_fragment_count"],
            "verification.accepted_handoff_count": preamble["accepted_handoff_count"],
            "verification.forbidden_path_scan_passed": true,
        },
        "unresolved_fields": UNRESOLVED_MANIFEST_FIELDS,
    });

    let mut candidate = json!({
        "schema_version": SCHEMA_VERSION,
        "record_kind": RECORD_KIND,
        "status": STATUS,
        "candidate_id": identifier,
        "repository": {
            "name": "UnknownAlienHuman/eliot-search",
            "base_commit": preflight.tree.tagged_commit(),
            "working_tree_used_as_input": false,
        },
        "package": {
            "name": package,
            "path": preflight.package_path,
            "stage": stage,
            "phase": phase,
            "wave": wave,
        },
        "draft": {
            "path": preflight.pair.context_path,
            "git_blob_id": preflight.pair.context_blob,
            "exact_file_sha256": preflight.pair.context_sha256,
            "source_ceiling_class": preflight.pair.source_ceiling_class,
        },
        "artifact_candidate": {
            "relative_path": bundle_relative,
            "sha256": artifact_sha256,
            "bytes": bundle_bytes.len(),
            "format": ARTIFACT_FORMAT,
            "local_file_is_immutable_artifact_ref": false,
        },
        "candidate_metadata_path": candidate_relative,
        "sources": manifest_projection["known"]["sources"],
        "registry_fragments": manifest_projection["known"]["registry_fragments"],
        "accepted_handoffs": manifest_projection["known"]["accepted_handoff_inputs"],
        "required_unavailable_checks": preamble["required_unavailable_checks"],
        "preflight_checks": preflight_checks,
        "reason_codes": [],
        "verification": {
            "source_count": preamble["source_count"],
            "registry_fragment_count": preamble["registry_fragment_count"],
            "accepted_handoff_count": preamble["accepted_handoff_count"],
            "forbidden_path_scan_passed": true,
            "bundle_roundtrip_verified": true,
            "local_output_readback_required": true,
            "authoritative_artifact_store_readback_verified": false,
        },
        "manifest_projection": manifest_projection,
        "ordinary_artifact_writes": [bundle_relative, candidate_relative],
        "control_record_mutations": [],
        "authority": authority_map(),
    });
    let digest = candidate_metadata_digest(&candidate);
    let JsonValue::Object(map) = &mut candidate else {
        unreachable!("candidate literal is an object")
    };
    map.insert("candidate_sha256".to_owned(), JsonValue::String(digest));
    if !assert_candidate_digest(&candidate) {
        return Err(ContextArtifactBuildError::new(
            "CONTEXT_ARTIFACT_BUILDER_CONTRACT_MISMATCH",
            "candidate metadata digest is not reproducible",
        ));
    }
    let candidate_bytes = canonical_json_bytes(&candidate);
    if candidate_bytes.len() > MAX_CANDIDATE_METADATA_BYTES {
        return Err(ContextArtifactBuildError::new(
            "CONTEXT_BUDGET_EXCEEDED",
            "candidate metadata exceeds the closed byte ceiling",
        ));
    }
    Ok(CandidateBuild::new(
        candidate,
        candidate_bytes,
        bundle_bytes,
        candidate_relative,
        bundle_relative,
    ))
}

fn toml_text<'a>(
    value: &'a TomlValue,
    key: &str,
) -> Result<&'a str, ContextArtifactBuildError> {
    value.get(key).and_then(TomlValue::as_str).ok_or_else(|| {
        ContextArtifactBuildError::new(
            "DRAFT_PAIR_MISMATCH",
            format!("ticket field is not text: {key}"),
        )
    })
}

fn toml_integer(
    value: &TomlValue,
    key: &str,
) -> Result<i64, ContextArtifactBuildError> {
    value.get(key).and_then(TomlValue::as_integer).ok_or_else(|| {
        ContextArtifactBuildError::new(
            "DRAFT_PAIR_MISMATCH",
            format!("ticket field is not integer: {key}"),
        )
    })
}

fn map_output(
    error: crate::context_artifact_io::CandidateOutputError,
) -> ContextArtifactBuildError {
    ContextArtifactBuildError::new(error.reason(), error.message())
}

fn map_primitive(
    error: crate::context_artifact::ContextArtifactError,
) -> ContextArtifactBuildError {
    ContextArtifactBuildError::new(error.reason(), error.message())
}

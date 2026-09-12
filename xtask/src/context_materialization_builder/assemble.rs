//! Non-authoritative context-materialization plan assembly.

use std::path::Path;

use serde_json::{Value, json};

use crate::context_materialization::{
    DECISION_COMMIT, DECISION_MISSING, DECISION_PARTIAL_SIGNATURE,
    DECISION_SIGNATURES, MaterializationPlanError, PLAN_ROOT,
    REASON_MISSING_SELECTION, REASON_PARTIAL_SIGNATURE, RECORD_KIND,
    SCHEMA_VERSION, STATUS, advisory_output_target, authority_map, plan_digest,
};
use crate::ticket_planner::canonical_json_bytes;

use super::input::{load_candidate, load_selection};
use super::manifest::project;
use super::model::MaterializationBuild;

const MAX_PLAN_BYTES: usize = 1024 * 1024;
const MAX_PAYLOAD_BYTES: usize = 4 * 1024 * 1024;
const MAX_MANIFEST_BYTES: usize = 4 * 1024 * 1024;

/// Builds one context-materialization advisory plan without writing output.
///
/// # Errors
///
/// Candidate/bundle, selection, render, signature and output-root failures are
/// returned as closed [`MaterializationPlanError`] values.
pub fn build_plan(
    root: &Path,
    candidate_path: &str,
    bundle_path: Option<&str>,
    selection_path: Option<&str>,
    output_root: &str,
) -> Result<MaterializationBuild, MaterializationPlanError> {
    let output_root = advisory_output_target(output_root)?;
    let input = load_candidate(root, candidate_path, bundle_path)?;
    let selection = load_selection(root, selection_path, &input.bundle)?;
    let package = text(&input.candidate, "/package/name")?;
    let candidate_id = text(&input.candidate, "/candidate_id")?;
    let candidate_sha = text(&input.candidate, "/candidate_sha256")?;
    let bundle_sha = text(&input.candidate, "/artifact_candidate/sha256")?;
    let output_directory = format!("{output_root}/{package}/{candidate_id}");
    let plan_path = format!("{output_directory}/plan.json");
    let payload_path = format!("{output_directory}/context-manifest.payload.toml");
    let manifest_path = format!("{output_directory}/context-manifest.prospective.toml");

    let (decision, reason_codes, selection_json, handoffs, operation, prospective, payload, manifest) =
        if let Some(selection) = selection {
            let projection = project(&input, &selection)?;
            if projection.payload.len() > MAX_PAYLOAD_BYTES {
                return Err(MaterializationPlanError::new(
                    "MATERIALIZATION_PAYLOAD_TOO_LARGE",
                    "prospective signed payload exceeds the closed byte ceiling",
                ));
            }
            if projection
                .manifest
                .as_ref()
                .is_some_and(|bytes| bytes.len() > MAX_MANIFEST_BYTES)
            {
                return Err(MaterializationPlanError::new(
                    "MATERIALIZATION_MANIFEST_TOO_LARGE",
                    "prospective complete manifest exceeds the closed byte ceiling",
                ));
            }
            let materializer_present = selection.materializer_signature.state == "PRESENT";
            let reviewer_present = selection.reviewer_signature.state == "PRESENT";
            let (decision, reasons) = match (materializer_present, reviewer_present) {
                (false, false) => (DECISION_SIGNATURES, Vec::<String>::new()),
                (true, true) => (DECISION_COMMIT, Vec::<String>::new()),
                _ => (
                    DECISION_PARTIAL_SIGNATURE,
                    vec![REASON_PARTIAL_SIGNATURE.to_owned()],
                ),
            };
            let selection_json = selection_view(selection_path, &selection);
            let operation = json!({
                "operation_kind": "materialize_context_v1",
                "operation_id": projection.operation_id,
                "input": projection.operation_input,
                "signature_refs_are_inputs": false,
            });
            let prospective = json!({
                "status": if decision == DECISION_COMMIT {
                    "COMPLETE_PROPOSAL_NOT_COMMITTED"
                } else {
                    "SIGNED_PAYLOAD_NOT_DUAL_SIGNED"
                },
                "payload_relative_path": payload_path,
                "signed_payload_sha256": projection.signed_payload_sha256,
                "payload_bytes": projection.payload.len(),
                "complete_relative_path": projection.manifest.as_ref().map(|_| manifest_path.clone()),
                "exact_record_file_sha256": projection.exact_record_file_sha256,
                "target_control_record_path": projection.target_control_record_path,
                "committed": false,
            });
            (
                decision,
                reasons,
                selection_json,
                projection.accepted_handoffs,
                operation,
                prospective,
                Some(projection.payload),
                projection.manifest,
            )
        } else {
            (
                DECISION_MISSING,
                vec![REASON_MISSING_SELECTION.to_owned()],
                json!({"state": "ABSENT", "path": Value::Null}),
                Vec::new(),
                json!({
                    "operation_kind": "materialize_context_v1",
                    "operation_id": Value::Null,
                    "input": Value::Null,
                    "signature_refs_are_inputs": false,
                }),
                json!({
                    "status": "UNAVAILABLE_SELECTION_REQUIRED",
                    "payload_relative_path": Value::Null,
                    "signed_payload_sha256": Value::Null,
                    "payload_bytes": 0,
                    "complete_relative_path": Value::Null,
                    "exact_record_file_sha256": Value::Null,
                    "target_control_record_path": Value::Null,
                    "committed": false,
                }),
                None,
                None,
            )
        };

    let mut ordinary_writes = vec![Value::String(plan_path.clone())];
    if payload.is_some() {
        ordinary_writes.push(Value::String(payload_path));
    }
    if manifest.is_some() {
        ordinary_writes.push(Value::String(manifest_path));
    }
    let mut plan = json!({
        "schema_version": SCHEMA_VERSION,
        "record_kind": RECORD_KIND,
        "status": STATUS,
        "repository": {
            "name": "UnknownAlienHuman/eliot-search",
            "root": ".",
            "working_tree_used_as_input": false,
        },
        "candidate": {
            "path": input.candidate_path,
            "candidate_id": candidate_id,
            "candidate_sha256": candidate_sha,
            "bundle_path": input.bundle_path,
            "bundle_sha256": bundle_sha,
            "bundle_bytes": input.bundle.len(),
        },
        "selection": selection_json,
        "accepted_handoff_projections": handoffs,
        "operation": operation,
        "prospective_manifest": prospective,
        "decision": decision,
        "reason_codes": reason_codes,
        "ordinary_artifact_writes": ordinary_writes,
        "control_record_mutations": [],
        "authority": authority_map(),
    });
    let digest = plan_digest(&plan);
    plan.as_object_mut()
        .expect("plan literal is an object")
        .insert("plan_sha256".to_owned(), Value::String(digest));
    let plan_bytes = canonical_json_bytes(&plan);
    if plan_bytes.len() > MAX_PLAN_BYTES {
        return Err(MaterializationPlanError::new(
            "MATERIALIZATION_PLAN_TOO_LARGE",
            "materialization plan exceeds the closed byte ceiling",
        ));
    }
    Ok(MaterializationBuild::new(
        plan,
        plan_bytes,
        payload,
        manifest,
        output_directory,
    ))
}

fn selection_view(
    path: Option<&str>,
    selection: &super::model::Selection,
) -> Value {
    json!({
        "state": "PRESENT",
        "path": path,
        "context_id": selection.context_id,
        "created_at": selection.created_at,
        "materializer_identity": selection.materializer_identity,
        "reviewer_identity": selection.reviewer_identity,
        "artifact_ref": {
            "store_profile_ref": selection.artifact_ref.store_profile_ref,
            "artifact_id": selection.artifact_ref.artifact_id,
            "bytes": selection.artifact_ref.bytes,
            "sha256": selection.artifact_ref.sha256,
        },
        "artifact_readback": {
            "verified": true,
            "verifier_identity": selection.readback.verifier_identity,
            "verified_at": selection.readback.verified_at,
            "sha256": selection.readback.sha256,
            "bytes": selection.readback.bytes,
        },
        "materializer_signature_state": selection.materializer_signature.state,
        "reviewer_signature_state": selection.reviewer_signature.state,
    })
}

fn text<'a>(value: &'a Value, pointer: &str) -> Result<&'a str, MaterializationPlanError> {
    value
        .pointer(pointer)
        .and_then(Value::as_str)
        .ok_or_else(|| {
            MaterializationPlanError::new(
                "MATERIALIZATION_CANDIDATE_INVALID",
                format!("candidate field is invalid: {pointer}"),
            )
        })
}

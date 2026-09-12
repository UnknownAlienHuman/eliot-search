use std::path::Path;

use serde_json::{Value, json};

use crate::context_artifact::{
    ARTIFACT_FORMAT, AUTHORITY_FIELDS, STATUS, UNRESOLVED_MANIFEST_FIELDS,
    assert_candidate_digest, authority_map, candidate_metadata_digest,
};

use super::Validation;

pub(super) fn validate_repository(
    root: &Path,
    validation: &mut Validation,
) {
    validate_text_contracts(root, validation);
    validate_workflow(root, validation);
    validate_artifact_root(root, validation);
    validate_pure_candidate_ceiling(validation);
    validate_implementation_sentinels(root, validation);
}

fn validate_text_contracts(root: &Path, validation: &mut Validation) {
    let requirements: [(&str, &[&str]); 2] = [
        (
            "docs/handoff/CONTEXT_ARTIFACT_CANDIDATE_V1.md",
            &[
                "working tree",
                "ELIOT_SWARM_CONTEXT_1",
                "identity.context_id",
                "artifact.ref",
                "CANDIDATE_OUTPUT_CONFLICT",
                "does not permit implementation",
            ],
        ),
        (
            "docs/handoff/CONTEXT_ARTIFACT_CANDIDATE_DIGEST_V1.md",
            &["candidate_id", "candidate_sha256", "fixed-point"],
        ),
    ];
    for (relative, tokens) in requirements {
        let text = std::fs::read_to_string(root.join(relative)).unwrap_or_default();
        for token in tokens {
            validation.require(
                text.contains(token),
                &format!("text:{relative}:{token}"),
                &format!("contains {token}"),
            );
        }
    }
}

fn validate_workflow(root: &Path, validation: &mut Validation) {
    let workflow = std::fs::read_to_string(
        root.join(".github/workflows/context-artifact-candidate.yml"),
    )
    .unwrap_or_default();
    let manual = workflow.contains("\n  workflow_dispatch:")
        || workflow.starts_with("on:\n  workflow_dispatch:");
    let automatic = [
        "\n  push:",
        "\n  pull_request:",
        "\n  schedule:",
        "\n  workflow_run:",
        "\n  repository_dispatch:",
        "\n  workflow_call:",
    ]
    .iter()
    .any(|trigger| workflow.contains(trigger));
    validation.require(
        manual
            && !automatic
            && workflow.contains("\n  contents: read")
            && workflow.contains("persist-credentials: false"),
        "workflow-policy",
        "builder workflow is manual/read-only/credential-free",
    );
}

fn validate_artifact_root(root: &Path, validation: &mut Validation) {
    let artifact_root = root.join("artifacts/context-artifact-candidates");
    validation.require(
        artifact_root.join("README.md").is_file(),
        "artifact-readme",
        "artifact root README exists",
    );
    validation.require(
        artifact_root.join(".gitignore").is_file(),
        "artifact-ignore",
        "generated candidate outputs are ignored",
    );
}

fn validate_pure_candidate_ceiling(validation: &mut Validation) {
    let authority = authority_map();
    let authority_closed = authority.as_object().is_some_and(|map| {
        map.len() == AUTHORITY_FIELDS.len()
            && AUTHORITY_FIELDS.iter().all(|field| {
                map.get(*field).and_then(Value::as_bool) == Some(false)
            })
    });
    validation.require(
        authority_closed,
        "actual-authority",
        "candidate authority ceiling is exact and all false",
    );

    let mut candidate = json!({
        "schema_version": 1,
        "record_kind": "context_artifact_candidate_v1",
        "status": STATUS,
        "candidate_id": "00".repeat(32),
        "reason_codes": [],
        "control_record_mutations": [],
        "authority": authority,
        "manifest_projection": {
            "schema_instance": false,
            "unresolved_fields": UNRESOLVED_MANIFEST_FIELDS,
        },
        "artifact_candidate": {
            "format": ARTIFACT_FORMAT,
            "local_file_is_immutable_artifact_ref": false,
        },
    });
    let digest = candidate_metadata_digest(&candidate);
    candidate
        .as_object_mut()
        .expect("candidate projection is an object")
        .insert("candidate_sha256".to_owned(), Value::String(digest));
    validation.require(
        assert_candidate_digest(&candidate),
        "actual-digest",
        "candidate metadata digest is non-circular and exact",
    );
    validation.require(
        candidate
            .get("control_record_mutations")
            .and_then(Value::as_array)
            .is_some_and(Vec::is_empty),
        "actual-mutations",
        "candidate contains no control mutation",
    );
    validation.require(
        candidate
            .pointer("/manifest_projection/schema_instance")
            .and_then(Value::as_bool)
            == Some(false),
        "actual-projection",
        "manifest projection is explicitly non-instance",
    );
}

fn validate_implementation_sentinels(root: &Path, validation: &mut Validation) {
    let assemble = std::fs::read_to_string(
        root.join("xtask/src/context_artifact_builder/assemble.rs"),
    )
    .unwrap_or_default();
    let write = std::fs::read_to_string(
        root.join("xtask/src/context_artifact_builder/write.rs"),
    )
    .unwrap_or_default();
    for (id, source, token, detail) in [
        (
            "implementation-status",
            assemble.as_str(),
            "\"status\": STATUS",
            "builder emits the closed candidate status",
        ),
        (
            "implementation-reasons",
            assemble.as_str(),
            "\"reason_codes\": []",
            "builder emits no success reason codes",
        ),
        (
            "implementation-mutations",
            assemble.as_str(),
            "\"control_record_mutations\": []",
            "builder emits no control-record mutations",
        ),
        (
            "implementation-authority",
            assemble.as_str(),
            "\"authority\": authority_map()",
            "builder uses the all-false authority map",
        ),
        (
            "implementation-digest",
            assemble.as_str(),
            "let digest = candidate_metadata_digest(&candidate);",
            "builder embeds the non-self-referential metadata digest",
        ),
        (
            "implementation-roundtrip",
            assemble.as_str(),
            "parse_bundle(&bundle_bytes)",
            "builder performs bundle round-trip validation",
        ),
        (
            "implementation-readback",
            write.as_str(),
            "candidate local readback failed",
            "builder verifies exact local output readback",
        ),
    ] {
        validation.require(source.contains(token), id, detail);
    }
}

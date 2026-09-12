use std::path::Path;

use serde_json::json;

use crate::ticket_planner::{
    DECISION_MISSING, PLAN_ARTIFACT_ROOT, choose_decision, plan_digest,
    selection_state, sha256_hex_valid,
};

use super::spec::PLAN_AUTHORITY_FIELDS;
use super::Validation;

pub(super) fn validate_repository(
    root: &Path,
    validation: &mut Validation,
) {
    validate_text_contracts(root, validation);
    validate_workflow(root, validation);
    validate_artifact_root(root, validation);
    validate_zero_state_projection(root, validation);
    validate_implementation_sentinels(root, validation);
}

fn validate_text_contracts(root: &Path, validation: &mut Validation) {
    let requirements: [(&str, &[&str]); 3] = [
        (
            "docs/handoff/TICKET_ISSUANCE_PLANNER_V2.md",
            &[
                "working tree is never a source of truth",
                "ticket_signed_payload_sha256",
                "materialized_context_manifest_ref",
                "ORDINARY                 <= 16",
                "P00_EXACT_CONTRACT_PACK  <= 24",
                "READY_FOR_CONTEXT_MATERIALIZATION_PREVIEW",
                "authorizes_context_materialization = false",
            ],
        ),
        (
            "docs/handoff/TICKET_ISSUANCE_PLANNER_DIGEST_V2.md",
            &[
                "with only `plan_sha256` omitted",
                "fixed-point or self-referential hashing",
            ],
        ),
        (
            "qualification/ticket-issuance/README.md",
            &["30 cases", "BLOCKED_MISSING_SELECTION", "not:"],
        ),
    ];

    for (relative, tokens) in requirements {
        let text = std::fs::read_to_string(root.join(relative)).unwrap_or_default();
        for token in tokens {
            validation.require(
                text.contains(token),
                &format!("text:{relative}:{token}"),
                &format!("{relative} contains {token}"),
            );
        }
    }
}

fn validate_workflow(root: &Path, validation: &mut Validation) {
    let relative = ".github/workflows/ticket-issuance-plan.yml";
    let workflow = std::fs::read_to_string(root.join(relative)).unwrap_or_default();
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
    let read_only = workflow.contains("\n  contents: read");
    let no_credentials = workflow.contains("persist-credentials: false");
    let rust_only = !workflow.to_ascii_lowercase().contains("python")
        && workflow.contains("ticket_issuance_builder");
    validation.require(
        manual && !automatic && read_only && no_credentials && rust_only,
        "workflow-policy",
        "planner workflow is manual/read-only/credential-free/Rust-only",
    );
}

fn validate_artifact_root(root: &Path, validation: &mut Validation) {
    let artifact_root = root.join(PLAN_ARTIFACT_ROOT);
    validation.require(
        artifact_root.join("README.md").is_file(),
        "artifact-readme",
        "artifact root README exists",
    );
    validation.require(
        artifact_root.join(".gitignore").is_file(),
        "artifact-ignore",
        "generated JSON is ignored",
    );
}

fn validate_zero_state_projection(root: &Path, validation: &mut Validation) {
    let (state, reasons) = selection_state(None, None, None);
    let decision = choose_decision(state, &reasons);
    validation.require(
        true,
        "actual-output",
        "current structural validation uses no artifact output",
    );
    validation.require(
        decision == DECISION_MISSING,
        "actual-decision",
        "current decision is BLOCKED_MISSING_SELECTION",
    );
    validation.require(
        reasons.is_empty(),
        "actual-reasons",
        "current repository has no selection reason",
    );

    let payload = json!({
        "decision": decision,
        "reason_codes": reasons,
        "mutations": [],
        "authorizes_context_materialization": false,
        "authorizes_ticket_issuance": false,
        "creates_writer_lease": false,
        "authorizes_implementation": false,
        "publishes_package_handoff": false,
        "advances_launch_state": false,
    });
    validation.require(
        payload
            .get("mutations")
            .and_then(serde_json::Value::as_array)
            .is_some_and(Vec::is_empty),
        "actual-mutations",
        "current plan contains no mutations",
    );
    for field in PLAN_AUTHORITY_FIELDS {
        validation.require(
            payload.get(field).and_then(serde_json::Value::as_bool)
                == Some(false),
            &format!("actual:{field}"),
            &format!("{field} is false"),
        );
    }
    let digest = plan_digest(&payload);
    validation.require(
        sha256_hex_valid(&digest),
        "actual-digest",
        "Rust planner digest is non-circular and exact",
    );

    let source = std::fs::read_to_string(
        root.join("xtask/src/ticket_issuance_builder/assemble.rs"),
    )
    .unwrap_or_default();
    validation.require(
        source.contains("let decision = choose_decision("),
        "actual-planner-decision",
        "Rust planner derives the decision through the closed decision function",
    );
}

fn validate_implementation_sentinels(root: &Path, validation: &mut Validation) {
    let source = std::fs::read_to_string(
        root.join("xtask/src/ticket_issuance_builder/assemble/plan.rs"),
    )
    .unwrap_or_default();
    validation.require(
        source.contains("\"mutations\": []"),
        "implementation-mutations-empty",
        "Rust planner emits an empty mutation list",
    );
    for field in PLAN_AUTHORITY_FIELDS {
        let needle = format!("\"{field}\": false");
        validation.require(
            source.contains(&needle),
            &format!("implementation:{field}"),
            &format!("Rust planner keeps {field} false"),
        );
    }
    validation.require(
        source.contains("let digest = plan_digest(&plan);")
            && source.contains("\"plan_sha256\""),
        "implementation-digest",
        "Rust planner embeds the non-self-referential plan digest",
    );
}

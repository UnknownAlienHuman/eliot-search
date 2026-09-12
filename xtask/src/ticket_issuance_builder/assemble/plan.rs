//! Canonical advisory-plan projection.

use serde_json::{Value as JsonValue, json};
use toml::Value;

use crate::ticket_planner::{
    DECISION_INVALID, INVALID_REASONS, RECORD_KIND, REPOSITORY_NAME,
    SCHEMA_VERSION, STATUS, plan_digest,
};

use super::super::model::{
    Checks, DraftPair, RegistrySnapshot, TicketIssuanceBuildOptions,
};
use super::super::util::{count_string, integer, text, toml_to_json};

#[allow(clippy::too_many_arguments)]
pub(super) fn assemble_plan(
    options: &TicketIssuanceBuildOptions,
    tree: &crate::git_tree::GitTree,
    registries: &RegistrySnapshot,
    pair: Option<&DraftPair>,
    sources: Vec<JsonValue>,
    accepted_handoffs: Vec<JsonValue>,
    classification: &'static str,
    decision: &'static str,
    checks: &Checks,
) -> JsonValue {
    let package_path = registries
        .package_row
        .as_ref()
        .and_then(|row| text(row, "path"))
        .unwrap_or_default();
    let package_wave = registries
        .package_row
        .as_ref()
        .and_then(|row| integer(row, "wave"))
        .unwrap_or(-1);
    let scope = registries
        .function_row
        .as_ref()
        .and_then(|row| text(row, "write_scope"))
        .unwrap_or_default();
    let stage_id = pair
        .and_then(|pair| text(&pair.ticket, "stage"))
        .unwrap_or("UNKNOWN");
    let phase = pair
        .and_then(|pair| text(&pair.ticket, "phase"))
        .unwrap_or("UNKNOWN");
    let registry_wave = registries
        .stage_row
        .as_ref()
        .and_then(|row| integer(row, "wave"))
        .unwrap_or(-1);
    let conditional_requirements = registries
        .launch
        .get("conditional_activation")
        .and_then(Value::as_table)
        .and_then(|table| table.get(&options.package))
        .map_or_else(|| json!({}), toml_to_json);

    let mut plan = json!({
        "schema_version": SCHEMA_VERSION,
        "record_kind": RECORD_KIND,
        "status": STATUS,
        "repository": {
            "name": REPOSITORY_NAME,
            "view_commit": tree.tagged_commit(),
            "object_format": tree.object_format(),
            "working_tree_used_as_input": false,
        },
        "package": {
            "name": options.package.as_str(),
            "path": package_path,
            "wave": package_wave,
            "write_scope": scope,
            "source_ceiling_class": pair.map_or(
                "",
                |pair| pair.source_ceiling_class.as_str(),
            ),
        },
        "stage": {
            "id": stage_id,
            "phase": phase,
            "active_stage": text(&registries.launch, "active_stage")
                .unwrap_or("UNKNOWN"),
            "active_wave": integer(&registries.launch, "active_wave")
                .unwrap_or(-1),
            "registry_wave": registry_wave,
        },
        "launch": {
            "classification": classification,
            "classification_recognized": matches!(
                classification,
                "AUTHORIZED" | "CONDITIONAL"
            ),
            "conditional_requirements": conditional_requirements,
        },
        "selection": {
            "state": selection_state(options),
            "base_commit": options.base_commit.as_deref().unwrap_or(""),
            "writer": options.writer.as_deref().unwrap_or(""),
            "reviewer": options.reviewer.as_deref().unwrap_or(""),
        },
        "drafts": {
            "ticket_path": pair.map_or(
                "",
                |pair| pair.ticket_path.as_str(),
            ),
            "ticket_git_blob_id": pair.map_or(
                "",
                |pair| pair.ticket_blob.as_str(),
            ),
            "ticket_exact_sha256": pair.map_or(
                "",
                |pair| pair.ticket_sha256.as_str(),
            ),
            "context_path": pair.map_or(
                "",
                |pair| pair.context_path.as_str(),
            ),
            "context_git_blob_id": pair.map_or(
                "",
                |pair| pair.context_blob.as_str(),
            ),
            "context_exact_sha256": pair.map_or(
                "",
                |pair| pair.context_sha256.as_str(),
            ),
            "sources": sources,
            "registry_selectors": pair.map_or_else(
                Vec::new,
                |pair| pair.selectors.clone(),
            ),
            "accepted_handoff_slots": pair.map_or_else(
                Vec::new,
                |pair| pair.handoff_slots.clone(),
            ),
            "unavailable_checks": pair.map_or_else(
                Vec::new,
                |pair| pair.unavailable_checks.clone(),
            ),
        },
        "prerequisites": {
            "accepted_handoffs": accepted_handoffs,
        },
        "checks": checks.checks_json(),
        "decision": decision,
        "reason_codes": checks.reasons(),
        "mutations": [],
        "authorizes_context_materialization": false,
        "authorizes_ticket_issuance": false,
        "creates_writer_lease": false,
        "authorizes_implementation": false,
        "publishes_package_handoff": false,
        "advances_launch_state": false,
    });
    let digest = plan_digest(&plan);
    plan.as_object_mut()
        .expect("ticket plan is an object")
        .insert("plan_sha256".to_owned(), JsonValue::String(digest));
    debug_assert_eq!(
        decision == DECISION_INVALID,
        checks
            .reasons()
            .iter()
            .any(|reason| INVALID_REASONS.contains(&reason.as_str()))
    );
    plan
}

pub(super) fn launch_class(
    launch: &Value,
    package: &str,
) -> &'static str {
    if count_string(launch.get("authorized_packages"), package) == 1 {
        "AUTHORIZED"
    } else if count_string(launch.get("conditional_packages"), package) == 1 {
        "CONDITIONAL"
    } else {
        "UNKNOWN"
    }
}

fn selection_state(options: &TicketIssuanceBuildOptions) -> &'static str {
    let count = [
        options.base_commit.as_ref(),
        options.writer.as_ref(),
        options.reviewer.as_ref(),
    ]
    .iter()
    .filter(|value| value.is_some())
    .count();
    match count {
        0 => "NONE",
        3 => "COMPLETE",
        _ => "PARTIAL",
    }
}

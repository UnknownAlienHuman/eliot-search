//! Read-only structural validation for the context-materialization planner.
//!
//! This checks registry/schema/digest/corpus closure and the all-false
//! authority ceiling. It does not execute or replace the planner itself.

use std::collections::BTreeSet;
use std::path::Path;

use serde_json::json;
use toml::Value;

const REQUIRED: [&str; 13] = [
    "swarm/context-materialization-planner-v1.toml",
    "swarm/context-materialization-plan-schema-v1.toml",
    "swarm/context-materialization-plan-digest-v1.toml",
    "swarm/context-manifest-instance-v1.toml",
    "swarm/context-manifest-renderer-v1.toml",
    "swarm/accepted-evidence-digest-v1.toml",
    "tools/plan-context-materialization.py",
    "tools/context_materialization_planner_v1/core.py",
    "tools/context_materialization_planner_v1/manifest.py",
    "tools/context_materialization_planner_v1/plan.py",
    "qualification/context-materialization/cases-v1.toml",
    "qualification/context-materialization/test_context_materialization_plan_v1.py",
    "docs/handoff/CONTEXT_MATERIALIZATION_PLAN_V1.md",
];

/// Structural validation result compatible with the retired Python command.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ContextMaterializationValidationReport {
    pub required_files: usize,
    pub cases: usize,
    pub decisions: usize,
    pub errors: Vec<String>,
}

impl ContextMaterializationValidationReport {
    /// True when every structural rule passed.
    #[must_use]
    pub const fn passed(&self) -> bool {
        self.errors.is_empty()
    }
}

/// Validate the planner's checked-in closure without executing it.
#[must_use]
pub fn validate_context_materialization_plan(
    root: &Path,
) -> ContextMaterializationValidationReport {
    let mut errors = Vec::new();
    for relative in REQUIRED {
        if !root.join(relative).is_file() {
            errors.push(format!("missing: {relative}"));
        }
    }

    let registry = read_toml(root, REQUIRED[0], &mut errors);
    let schema = read_toml(root, REQUIRED[1], &mut errors);
    let digest = read_toml(root, REQUIRED[2], &mut errors);
    let instance = read_toml(root, REQUIRED[3], &mut errors);
    let renderer = read_toml(root, REQUIRED[4], &mut errors);
    let cases = read_toml(root, REQUIRED[10], &mut errors);

    if string(&registry, "component")
        != Some("context_materialization_planner_v1")
    {
        errors.push("planner component mismatch".to_owned());
    }
    if string(&registry, "operation_kind") != Some("materialize_context_v1") {
        errors.push("operation kind mismatch".to_owned());
    }
    if !all_false_table(registry.get("authority")) {
        errors.push("planner authority ceiling must be all false".to_owned());
    }
    if boolean(&registry, "signature_refs_are_operation_id_inputs")
        != Some(false)
    {
        errors.push("signature refs must be excluded from operation ID".to_owned());
    }
    if boolean(
        &registry,
        "signature_refs_bind_post_operation_signed_payload",
    ) != Some(true)
    {
        errors.push("post-operation signature binding missing".to_owned());
    }

    if string(&schema, "record_kind")
        != Some("context_materialization_plan_v1")
    {
        errors.push("plan schema kind mismatch".to_owned());
    }
    let schema_invariants = schema.get("invariants").and_then(Value::as_table);
    if schema_invariants.is_none_or(|invariants| {
        invariants
            .get("control_record_mutations_must_be_empty")
            .and_then(Value::as_bool)
            != Some(true)
            || invariants
                .get("all_authority_fields_must_be_false")
                .and_then(Value::as_bool)
                != Some(true)
    }) {
        errors.push("plan schema authority invariants missing".to_owned());
    }
    if boolean(&digest, "self_referential_digest_allowed") != Some(false) {
        errors.push("plan digest must remain non-self-referential".to_owned());
    }
    if string(&instance, "instance_status") != Some("MATERIALIZED") {
        errors.push("context manifest instance status mismatch".to_owned());
    }
    if boolean(&renderer, "signature_table_excluded_from_signed_payload")
        != Some(true)
    {
        errors.push("renderer signature boundary mismatch".to_owned());
    }
    let renderer_invariants = renderer.get("invariants").and_then(Value::as_table);
    if renderer_invariants.is_none_or(|invariants| {
        invariants
            .get("self_referential_complete_file_digest_allowed")
            .and_then(Value::as_bool)
            != Some(false)
    }) {
        errors.push("renderer allows complete-file self hash".to_owned());
    }

    let case_rows = cases.get("case").and_then(Value::as_array);
    let case_count = case_rows.map_or(0, Vec::len);
    if integer(&cases, "case_count") != Some(12) || case_count != 12 {
        errors.push("materialization corpus must contain exactly twelve cases".to_owned());
    }
    let case_ids: Option<Vec<&str>> = case_rows.map(|rows| {
        rows.iter()
            .filter_map(|row| {
                row.as_table()
                    .and_then(|table| table.get("id"))
                    .and_then(Value::as_str)
            })
            .collect()
    });
    if case_ids.as_ref().is_some_and(|ids| {
        ids.len() != ids.iter().copied().collect::<BTreeSet<_>>().len()
    }) {
        errors.push("materialization case IDs are not unique".to_owned());
    }

    validate_workflow(root, &mut errors);
    validate_implementation_sentinels(root, &mut errors);

    ContextMaterializationValidationReport {
        required_files: REQUIRED.len(),
        cases: case_count,
        decisions: schema
            .get("closed_decisions")
            .and_then(Value::as_array)
            .map_or(0, Vec::len),
        errors,
    }
}

/// Exit code compatible with the retired Python validator.
#[must_use]
pub const fn exit_code(
    report: &ContextMaterializationValidationReport,
) -> i32 {
    if report.passed() { 0 } else { 1 }
}

/// Render the stable JSON report.
#[must_use]
pub fn render_report_json(
    report: &ContextMaterializationValidationReport,
) -> String {
    serde_json::to_string_pretty(&json!({
        "status": if report.passed() { "PASS" } else { "FAIL" },
        "required_files": report.required_files,
        "cases": report.cases,
        "decisions": report.decisions,
        "errors": report.errors,
    }))
    .expect("serializing a bounded validation report cannot fail")
}

fn read_toml(root: &Path, relative: &str, errors: &mut Vec<String>) -> Value {
    let path = root.join(relative);
    let text = match std::fs::read_to_string(&path) {
        Ok(text) => text,
        Err(error) => {
            errors.push(format!("{relative}: {error}"));
            return empty_table();
        }
    };
    match toml::from_str(&text) {
        Ok(value) => value,
        Err(error) => {
            errors.push(format!("{relative}: {error}"));
            empty_table()
        }
    }
}

fn empty_table() -> Value {
    Value::Table(toml::map::Map::new())
}

fn string<'a>(value: &'a Value, key: &str) -> Option<&'a str> {
    value.get(key).and_then(Value::as_str)
}

fn boolean(value: &Value, key: &str) -> Option<bool> {
    value.get(key).and_then(Value::as_bool)
}

fn integer(value: &Value, key: &str) -> Option<i64> {
    value.get(key).and_then(Value::as_integer)
}

fn all_false_table(value: Option<&Value>) -> bool {
    value.and_then(Value::as_table).is_some_and(|table| {
        !table.is_empty()
            && table
                .values()
                .all(|entry| entry.as_bool() == Some(false))
    })
}

fn validate_workflow(root: &Path, errors: &mut Vec<String>) {
    let path = root.join(".github/workflows/context-materialization-plan.yml");
    let text = match std::fs::read_to_string(path) {
        Ok(text) => text,
        Err(_) => {
            errors.push("missing manual workflow".to_owned());
            return;
        }
    };
    for token in [
        "workflow_dispatch:",
        "contents: read",
        "persist-credentials: false",
    ] {
        if !text.contains(token) {
            errors.push(format!("workflow missing {token}"));
        }
    }
    for trigger in [
        "\n  push:",
        "\n  pull_request:",
        "\n  schedule:",
        "\n  workflow_run:",
    ] {
        if text.contains(trigger) {
            errors.push("workflow has automatic trigger".to_owned());
            break;
        }
    }
}

fn validate_implementation_sentinels(root: &Path, errors: &mut Vec<String>) {
    let plan = std::fs::read_to_string(
        root.join("tools/context_materialization_planner_v1/plan.py"),
    )
    .unwrap_or_default();
    let core = std::fs::read_to_string(
        root.join("tools/context_materialization_planner_v1/core.py"),
    )
    .unwrap_or_default();
    if !plan.contains("\"control_record_mutations\": []") {
        errors.push("plan implementation lacks empty control mutation field".to_owned());
    }
    if !core.contains("AUTHORITY_FIELDS") {
        errors.push("authority field registry missing".to_owned());
    }
}

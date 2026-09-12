//! Read-only structural validation for the Rust context-materialization planner.
//!
//! This checks registry/schema/digest/corpus closure and the all-false
//! authority ceiling. It does not build or publish a plan.

use std::collections::BTreeSet;
use std::path::Path;

use serde_json::json;
use toml::Value;

const REQUIRED: [&str; 19] = [
    "swarm/context-materialization-planner-v1.toml",
    "swarm/context-materialization-plan-schema-v1.toml",
    "swarm/context-materialization-plan-digest-v1.toml",
    "swarm/context-manifest-instance-v1.toml",
    "swarm/context-manifest-renderer-v1.toml",
    "swarm/accepted-evidence-digest-v1.toml",
    "xtask/src/context_materialization_builder.rs",
    "xtask/src/context_materialization_builder/assemble.rs",
    "xtask/src/context_materialization_builder/input.rs",
    "xtask/src/context_materialization_builder/manifest.rs",
    "xtask/src/context_materialization_builder/model.rs",
    "xtask/src/context_materialization_builder/write.rs",
    "xtask/src/command/context_materialization_build.rs",
    "tools/plan-context-materialization.ps1",
    "qualification/context-materialization/cases-v1.toml",
    "xtask/tests/context_materialization_builder.rs",
    "xtask/tests/context_materialization_runtime_boundary.rs",
    "docs/handoff/CONTEXT_MATERIALIZATION_PLAN_V1.md",
    ".github/workflows/context-materialization-plan.yml",
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
    let cases = read_toml(root, REQUIRED[14], &mut errors);

    validate_registry(root, &registry, &mut errors);
    validate_contracts(&schema, &digest, &instance, &renderer, &mut errors);
    let case_count = validate_cases(&cases, &mut errors);
    validate_workflow(root, &mut errors);
    validate_implementation_sentinels(root, &mut errors);
    validate_retired_python(root, &mut errors);

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

fn validate_registry(root: &Path, registry: &Value, errors: &mut Vec<String>) {
    if string(registry, "component")
        != Some("context_materialization_planner_v1")
        || string(registry, "status")
            != Some("EXECUTABLE_ADVISORY_NON_AUTHORITATIVE")
        || string(registry, "implementation")
            != Some("xtask/src/context_materialization_builder.rs")
        || string(registry, "powershell_wrapper")
            != Some("tools/plan-context-materialization.ps1")
        || string(registry, "qualification_tests")
            != Some("xtask/tests/context_materialization_builder.rs")
    {
        errors.push("planner implementation registry mismatch".to_owned());
    }
    let modules = string_array(registry.get("implementation_modules"));
    if modules.as_ref().is_none_or(|modules| {
        modules.len() != 5
            || modules.iter().any(|path| !root.join(*path).is_file())
            || modules.iter().copied().collect::<BTreeSet<_>>().len()
                != modules.len()
    }) {
        errors.push("planner implementation module closure mismatch".to_owned());
    }
    if string(registry, "operation_kind") != Some("materialize_context_v1") {
        errors.push("operation kind mismatch".to_owned());
    }
    if !all_false_table(registry.get("authority")) {
        errors.push("planner authority ceiling must be all false".to_owned());
    }
    if boolean(registry, "signature_refs_are_operation_id_inputs")
        != Some(false)
        || boolean(
            registry,
            "signature_refs_bind_post_operation_signed_payload",
        ) != Some(true)
    {
        errors.push("signature/operation binding mismatch".to_owned());
    }
}

fn validate_contracts(
    schema: &Value,
    digest: &Value,
    instance: &Value,
    renderer: &Value,
    errors: &mut Vec<String>,
) {
    if string(schema, "record_kind")
        != Some("context_materialization_plan_v1")
    {
        errors.push("plan schema kind mismatch".to_owned());
    }
    let invariants = schema.get("invariants").and_then(Value::as_table);
    if invariants.is_none_or(|invariants| {
        invariants
            .get("control_record_mutations_must_be_empty")
            .and_then(Value::as_bool)
            != Some(true)
            || invariants
                .get("all_authority_fields_must_be_false")
                .and_then(Value::as_bool)
                != Some(true)
            || invariants
                .get("signature_refs_excluded_from_operation_id")
                .and_then(Value::as_bool)
                != Some(true)
    }) {
        errors.push("plan schema authority/operation invariants missing".to_owned());
    }
    if boolean(digest, "self_referential_digest_allowed") != Some(false) {
        errors.push("plan digest must remain non-self-referential".to_owned());
    }
    if string(instance, "instance_status") != Some("MATERIALIZED") {
        errors.push("context manifest instance status mismatch".to_owned());
    }
    if boolean(renderer, "signature_table_excluded_from_signed_payload")
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
}

fn validate_cases(cases: &Value, errors: &mut Vec<String>) -> usize {
    let rows = cases.get("case").and_then(Value::as_array);
    let count = rows.map_or(0, Vec::len);
    if integer(cases, "case_count") != Some(12) || count != 12 {
        errors.push("materialization corpus must contain exactly twelve cases".to_owned());
    }
    let ids: Vec<&str> = rows
        .into_iter()
        .flatten()
        .filter_map(|row| {
            row.as_table()
                .and_then(|table| table.get("id"))
                .and_then(Value::as_str)
        })
        .collect();
    if ids.len() != count || ids.iter().copied().collect::<BTreeSet<_>>().len() != count {
        errors.push("materialization case IDs are missing or non-unique".to_owned());
    }
    count
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
    let text = match std::fs::read_to_string(root.join(relative)) {
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

fn string_array(value: Option<&Value>) -> Option<Vec<&str>> {
    value?
        .as_array()?
        .iter()
        .map(Value::as_str)
        .collect()
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
    let text = match std::fs::read_to_string(
        root.join(".github/workflows/context-materialization-plan.yml"),
    ) {
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
        "cargo test --locked -p xtask --test context_materialization_builder",
    ] {
        if !text.contains(token) {
            errors.push(format!("workflow missing {token}"));
        }
    }
    if [
        "\n  push:",
        "\n  pull_request:",
        "\n  schedule:",
        "\n  workflow_run:",
    ]
    .iter()
    .any(|trigger| text.contains(trigger))
    {
        errors.push("workflow has automatic trigger".to_owned());
    }
    if text.to_ascii_lowercase().contains("python") {
        errors.push("workflow restored Python runtime".to_owned());
    }
}

fn validate_implementation_sentinels(root: &Path, errors: &mut Vec<String>) {
    let assemble = std::fs::read_to_string(
        root.join("xtask/src/context_materialization_builder/assemble.rs"),
    )
    .unwrap_or_default();
    let manifest = std::fs::read_to_string(
        root.join("xtask/src/context_materialization_builder/manifest.rs"),
    )
    .unwrap_or_default();
    let write = std::fs::read_to_string(
        root.join("xtask/src/context_materialization_builder/write.rs"),
    )
    .unwrap_or_default();
    for (source, token, failure) in [
        (
            assemble.as_str(),
            "\"control_record_mutations\": []",
            "plan implementation lacks empty control mutation field",
        ),
        (
            assemble.as_str(),
            "\"authority\": authority_map()",
            "plan implementation lacks all-false authority projection",
        ),
        (
            assemble.as_str(),
            "\"signature_refs_are_inputs\": false",
            "operation ID signature exclusion sentinel missing",
        ),
        (
            manifest.as_str(),
            "accepted_evidence_digest_toml",
            "accepted handoff evidence projection missing",
        ),
        (
            write.as_str(),
            "write_exact_idempotent",
            "idempotent ordinary output writer missing",
        ),
    ] {
        if !source.contains(token) {
            errors.push(failure.to_owned());
        }
    }
}

fn validate_retired_python(root: &Path, errors: &mut Vec<String>) {
    for relative in [
        "tools/plan-context-materialization.py",
        "tools/context_materialization_planner_v1/__init__.py",
        "tools/context_materialization_planner_v1/core.py",
        "tools/context_materialization_planner_v1/manifest.py",
        "tools/context_materialization_planner_v1/plan.py",
        "qualification/context-materialization/test_context_materialization_plan_v1.py",
        "tools/context_artifact_builder_v1/__init__.py",
        "tools/context_artifact_builder_v1/core.py",
        "tools/context_artifact_builder_v1/bundle.py",
    ] {
        if root.join(relative).exists() {
            errors.push(format!("retired Python planner returned: {relative}"));
        }
    }
}

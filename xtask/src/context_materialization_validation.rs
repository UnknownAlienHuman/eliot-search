//! Read-only structural validation for the Rust context-materialization planner.
//!
//! This checks registry/schema/digest/corpus closure and the all-false
//! authority ceiling. It does not build or publish a plan.

mod repository;
mod rules;
mod spec;

use std::path::Path;

use serde_json::json;
use toml::Value;

use repository::{
    read_toml, validate_implementation_sentinels, validate_retired_python,
    validate_workflow,
};
use rules::{validate_cases, validate_contracts, validate_registry};
use spec::{
    CASES, DIGEST, INSTANCE, REGISTRY, RENDERER, REQUIRED, SCHEMA,
};

/// Structural validation result compatible with the retired Python command.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ContextMaterializationValidationReport {
    /// Number of required checked-in files.
    pub required_files: usize,
    /// Number of qualification cases.
    pub cases: usize,
    /// Number of closed planner decisions.
    pub decisions: usize,
    /// Stable structural errors.
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

    let registry = read_toml(root, REGISTRY, &mut errors);
    let schema = read_toml(root, SCHEMA, &mut errors);
    let digest = read_toml(root, DIGEST, &mut errors);
    let instance = read_toml(root, INSTANCE, &mut errors);
    let renderer = read_toml(root, RENDERER, &mut errors);
    let cases = read_toml(root, CASES, &mut errors);

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

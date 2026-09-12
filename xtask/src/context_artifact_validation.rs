//! Read-only structural validation for context-artifact candidate builder v1.
//!
//! The Python builder and twenty-case corpus remain separate until the full
//! immutable-tree extraction/write pipeline is ported. This validator owns
//! registry/schema/digest/workflow closure and the zero-authority boundary; it
//! does not build or write a candidate.

mod registry;
mod repository;
mod schema;
mod spec;

use std::path::Path;

use serde_json::json;
use toml::Value;

use registry::validate_registry;
use repository::validate_repository;
use schema::validate_schema;
use spec::{CASES_PATH, DIGEST_PATH, REGISTRY_PATH, SCHEMA_PATH};

/// One stable context-artifact structural check.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ContextArtifactValidationCheck {
    /// Stable check identity.
    pub id: String,
    /// `PASS` or `FAIL`.
    pub status: &'static str,
    /// Human-readable closed detail.
    pub detail: String,
}

/// Read-only structural validator report.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ContextArtifactValidationReport {
    /// Deterministic structural checks.
    pub checks: Vec<ContextArtifactValidationCheck>,
    /// Stable failed-check details.
    pub errors: Vec<String>,
}

impl ContextArtifactValidationReport {
    /// True only when every structural rule passed.
    #[must_use]
    pub const fn passed(&self) -> bool {
        self.errors.is_empty()
    }
}

pub(super) struct Validation {
    checks: Vec<ContextArtifactValidationCheck>,
    errors: Vec<String>,
}

impl Validation {
    fn new() -> Self {
        Self {
            checks: Vec::new(),
            errors: Vec::new(),
        }
    }

    pub(super) fn require(
        &mut self,
        condition: bool,
        check_id: &str,
        detail: &str,
    ) {
        let status = if condition { "PASS" } else { "FAIL" };
        self.checks.push(ContextArtifactValidationCheck {
            id: check_id.to_owned(),
            status,
            detail: detail.to_owned(),
        });
        if !condition {
            self.errors.push(format!("{check_id}: {detail}"));
        }
    }

    fn finish(self) -> ContextArtifactValidationReport {
        ContextArtifactValidationReport {
            checks: self.checks,
            errors: self.errors,
        }
    }
}

/// Validate the checked-in builder closure without executing Python.
#[must_use]
pub fn validate_context_artifact_candidate(
    root: &Path,
) -> ContextArtifactValidationReport {
    let mut validation = Validation::new();
    let registry = load_toml(root, REGISTRY_PATH, &mut validation);
    let schema = load_toml(root, SCHEMA_PATH, &mut validation);
    let digest = load_toml(root, DIGEST_PATH, &mut validation);
    let cases = load_toml(root, CASES_PATH, &mut validation);

    validate_registry(root, &registry, &mut validation);
    validate_schema(&schema, &digest, &cases, &mut validation);
    validate_repository(root, &mut validation);
    validation.finish()
}

/// Exit code compatible with the retired Python structural validator.
#[must_use]
pub const fn exit_code(report: &ContextArtifactValidationReport) -> i32 {
    if report.passed() { 0 } else { 1 }
}

/// Render compact stable JSON.
#[must_use]
pub fn render_report_json(report: &ContextArtifactValidationReport) -> String {
    let checks: Vec<_> = report
        .checks
        .iter()
        .map(|check| {
            json!({
                "id": check.id.as_str(),
                "status": check.status,
                "detail": check.detail.as_str(),
            })
        })
        .collect();
    serde_json::to_string(&json!({
        "schema_version": 1,
        "validator": "context_artifact_candidate_v1",
        "status": if report.passed() { "PASS" } else { "FAIL" },
        "checks": checks,
        "errors": &report.errors,
        "current_candidate_id": "UNAVAILABLE",
        "authoritative_context_materialized": false,
        "context_manifest_created": false,
        "ticket_issued": false,
        "writer_lease_created": false,
        "implementation_authorized": false,
        "package_acceptance_claimed": false,
        "g0_acceptance_claimed": false,
        "w0_acceptance_claimed": false,
        "w1_authority_claimed": false,
    }))
    .expect("serializing a bounded context-artifact report cannot fail")
}

/// Render the human CLI result.
#[must_use]
pub fn render_report_text(report: &ContextArtifactValidationReport) -> String {
    if report.passed() {
        return format!(
            "PASS: {} checks; candidate creates no authority",
            report.checks.len()
        );
    }
    let mut output = format!("FAIL: {} error(s)", report.errors.len());
    for error in &report.errors {
        output.push_str("\n- ");
        output.push_str(error);
    }
    output
}

fn load_toml(root: &Path, relative: &str, validation: &mut Validation) -> Value {
    let text = match std::fs::read_to_string(root.join(relative)) {
        Ok(text) => text,
        Err(error) => {
            validation.require(
                false,
                &format!("file:{relative}"),
                &format!("unable to load TOML: {error}"),
            );
            return empty_table();
        }
    };
    match toml::from_str::<Value>(&text) {
        Ok(value) if value.is_table() => {
            validation.require(
                true,
                &format!("file:{relative}"),
                "TOML root is a table",
            );
            value
        }
        Ok(_) => {
            validation.require(
                false,
                &format!("file:{relative}"),
                "TOML root is a table",
            );
            empty_table()
        }
        Err(error) => {
            validation.require(
                false,
                &format!("file:{relative}"),
                &format!("unable to load TOML: {error}"),
            );
            empty_table()
        }
    }
}

fn empty_table() -> Value {
    Value::Table(toml::map::Map::new())
}

pub(super) fn string<'a>(value: &'a Value, key: &str) -> Option<&'a str> {
    value.get(key).and_then(Value::as_str)
}

pub(super) fn boolean(value: &Value, key: &str) -> Option<bool> {
    value.get(key).and_then(Value::as_bool)
}

pub(super) fn integer(value: &Value, key: &str) -> Option<i64> {
    value.get(key).and_then(Value::as_integer)
}

pub(super) fn string_array(value: Option<&Value>) -> Option<Vec<&str>> {
    value?
        .as_array()?
        .iter()
        .map(Value::as_str)
        .collect()
}

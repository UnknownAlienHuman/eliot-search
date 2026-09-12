//! Read-only structural validation for the schema-v2 ticket-issuance planner.
//!
//! The advisory planner and its 30-case operation corpus remain separate.
//! This Rust validator owns registry/schema/digest/workflow closure and the
//! current zero-selection authority ceiling. It never issues a control record,
//! materializes context, creates a lease or advances launch state.

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

/// One stable structural check.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct TicketIssuanceValidationCheck {
    /// Stable check identity.
    pub id: String,
    /// `PASS` or `FAIL`.
    pub status: &'static str,
    /// Human-readable closed detail.
    pub detail: String,
}

/// Read-only validator report compatible with the retired Python entrypoint.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct TicketIssuanceValidationReport {
    /// Every performed structural check in deterministic order.
    pub checks: Vec<TicketIssuanceValidationCheck>,
    /// Stable failed-check details.
    pub errors: Vec<String>,
}

impl TicketIssuanceValidationReport {
    /// True only when every structural check passed.
    #[must_use]
    pub const fn passed(&self) -> bool {
        self.errors.is_empty()
    }
}

pub(super) struct Validation {
    checks: Vec<TicketIssuanceValidationCheck>,
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
        self.checks.push(TicketIssuanceValidationCheck {
            id: check_id.to_owned(),
            status,
            detail: detail.to_owned(),
        });
        if !condition {
            self.errors.push(format!("{check_id}: {detail}"));
        }
    }

    fn finish(self) -> TicketIssuanceValidationReport {
        TicketIssuanceValidationReport {
            checks: self.checks,
            errors: self.errors,
        }
    }
}

/// Validate the checked-in planner closure without executing Python.
#[must_use]
pub fn validate_ticket_issuance_plan(
    root: &Path,
) -> TicketIssuanceValidationReport {
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

/// Exit code compatible with the retired Python validator.
#[must_use]
pub const fn exit_code(report: &TicketIssuanceValidationReport) -> i32 {
    if report.passed() { 0 } else { 1 }
}

/// Render the compact stable JSON report.
#[must_use]
pub fn render_report_json(report: &TicketIssuanceValidationReport) -> String {
    let checks: Vec<_> = report
        .checks
        .iter()
        .map(|check| {
            json!({
                "id": check.id,
                "status": check.status,
                "detail": check.detail,
            })
        })
        .collect();
    serde_json::to_string(&json!({
        "schema_version": 2,
        "validator": "ticket_issuance_planner_v2",
        "status": if report.passed() { "PASS" } else { "FAIL" },
        "non_authoritative": true,
        "package_acceptance_claimed": false,
        "g0_acceptance_claimed": false,
        "w0_acceptance_claimed": false,
        "w1_authority_claimed": false,
        "checks": checks,
        "errors": report.errors,
    }))
    .expect("serializing a bounded ticket validation report cannot fail")
}

/// Render the human CLI result.
#[must_use]
pub fn render_report_text(report: &TicketIssuanceValidationReport) -> String {
    if report.passed() {
        return format!(
            "PASS: {} planner-v2 structural checks",
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
    let path = root.join(relative);
    let text = match std::fs::read_to_string(&path) {
        Ok(text) => text,
        Err(error) => {
            validation.require(
                false,
                &format!("file:{relative}"),
                &format!("unreadable TOML: {error}"),
            );
            return empty_table();
        }
    };
    match toml::from_str::<Value>(&text) {
        Ok(value) => {
            validation.require(
                value.is_table(),
                &format!("file:{relative}"),
                "TOML root is a table",
            );
            if value.is_table() {
                value
            } else {
                empty_table()
            }
        }
        Err(error) => {
            validation.require(
                false,
                &format!("file:{relative}"),
                &format!("unreadable TOML: {error}"),
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

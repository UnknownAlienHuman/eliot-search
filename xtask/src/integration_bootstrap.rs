//! Fail-closed Rust validator for the P00 integration bootstrap.
//!
//! This module proves checked-in repository structure only. It never issues
//! control records or claims package, gate, wave, runtime or product
//! acceptance.

mod checks;

#[cfg(test)]
mod tests;

use std::path::Path;

use serde_json::{Value as JsonValue, json};
use toml::Value;

pub(crate) const EXPECTED_PROFILES: [&str; 5] = [
    "P00_FOUNDATION",
    "DIRECT_BASELINE",
    "LEXICAL_BASELINE",
    "CODE_CURRENT",
    "OPTIONAL_DEPTH",
];

pub(crate) const EXPECTED_LAYOUT_DIRECTORIES: [(&str, &str); 7] = [
    ("control", "control"),
    ("objects", "objects"),
    ("qdrant", "qdrant"),
    ("runtime", "runtime"),
    ("temporary", "tmp"),
    ("backups", "backups"),
    ("quarantine", "quarantine"),
];

/// One stable structural bootstrap finding.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct Finding {
    pub code: String,
    pub path: String,
    pub detail: String,
}

impl Finding {
    pub(crate) fn new(
        code: impl Into<String>,
        path: &Path,
        detail: impl Into<String>,
    ) -> Self {
        Self {
            code: code.into(),
            path: normalize_path(path),
            detail: detail.into(),
        }
    }
}

/// Complete result of the integration-bootstrap structural validator.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct IntegrationBootstrapReport {
    pub findings: Vec<Finding>,
}

impl IntegrationBootstrapReport {
    /// True only when no structural finding exists.
    #[must_use]
    pub const fn passed(&self) -> bool {
        self.findings.is_empty()
    }
}

/// Validate one repository root without mutating it.
#[must_use]
pub fn validate_integration_bootstrap(
    root: &Path,
    allow_missing_lock: bool,
) -> IntegrationBootstrapReport {
    let mut findings = Vec::new();
    checks::validate_toolchain(root, &mut findings);
    checks::validate_cargo_config(root, &mut findings);
    checks::validate_build_profiles(root, &mut findings);
    checks::validate_data_layout(root, &mut findings);
    checks::validate_workspace(root, &mut findings);
    checks::validate_lock(root, allow_missing_lock, &mut findings);
    checks::validate_workflow(root, &mut findings);
    IntegrationBootstrapReport { findings }
}

/// Exit code compatible with the retired Python entrypoint.
#[must_use]
pub const fn exit_code(report: &IntegrationBootstrapReport) -> i32 {
    if report.passed() { 0 } else { 1 }
}

/// Render the compact machine-readable v1 report.
#[must_use]
pub fn render_report_json(report: &IntegrationBootstrapReport) -> String {
    let findings: Vec<JsonValue> = report
        .findings
        .iter()
        .map(|finding| {
            json!({
                "code": finding.code,
                "path": finding.path,
                "detail": finding.detail,
            })
        })
        .collect();
    let payload = json!({
        "schema_version": 1,
        "validator": "integration_bootstrap_v1",
        "status": if report.passed() { "PASS" } else { "FAIL" },
        "authority": {
            "issues_control_records": false,
            "accepts_package": false,
            "accepts_gate": false,
            "accepts_wave": false,
            "advances_launch_state": false,
            "claims_runtime_evidence": false,
            "claims_product_acceptance": false,
        },
        "findings": findings,
    });
    serde_json::to_string(&payload)
        .expect("serializing a bounded bootstrap report cannot fail")
}

/// Render the human-readable report used by the compatibility wrapper.
#[must_use]
pub fn render_report_text(report: &IntegrationBootstrapReport) -> String {
    let mut output = format!(
        "{}: {} finding(s)",
        if report.passed() { "PASS" } else { "FAIL" },
        report.findings.len()
    );
    for finding in &report.findings {
        output.push('\n');
        output.push_str(&finding.code);
        output.push_str(": ");
        output.push_str(&finding.path);
        output.push_str(": ");
        output.push_str(&finding.detail);
    }
    output
}

pub(crate) fn load_toml(path: &Path) -> Result<Value, String> {
    let text = std::fs::read_to_string(path)
        .map_err(|error| format!("cannot load TOML: {error}"))?;
    let value: Value = toml::from_str(&text)
        .map_err(|error| format!("cannot load TOML: {error}"))?;
    if value.as_table().is_none() {
        return Err("TOML root must be a table".to_owned());
    }
    Ok(value)
}

pub(crate) fn string_array(value: Option<&Value>) -> Option<Vec<String>> {
    value?
        .as_array()?
        .iter()
        .map(|entry| entry.as_str().map(str::to_owned))
        .collect()
}

fn normalize_path(path: &Path) -> String {
    path.to_string_lossy().replace('\\', "/")
}

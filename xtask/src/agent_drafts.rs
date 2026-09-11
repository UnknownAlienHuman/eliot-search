//! Rust validators for non-claimable wave agent drafts.
//!
//! Python entrypoints are retired wave by wave. The public command/report
//! shape stays stable while common TOML/file-system handling lives here and
//! wave-specific invariants remain in bounded modules.

#[allow(unused_imports)]
mod w1;
mod w2;
mod w3;
mod w4;

use std::collections::{BTreeMap, BTreeSet};
use std::path::Path;

use serde_json::json;
use toml::Value;

pub use w3::{
    W3AgentDraftReport, exit_code as w3_exit_code,
    render_report_json as render_w3_report_json, validate_w3_agent_drafts,
};
pub use w4::{
    W4AgentDraftReport, exit_code as w4_exit_code,
    render_report_json as render_w4_report_json, validate_w4_agent_drafts,
};

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct AgentDraftReport {
    pub complete: bool,
    pub packages: usize,
    pub ticket_drafts: usize,
    pub context_drafts: usize,
    pub qualification_cases: usize,
    pub launch_stage: Option<String>,
    pub launch_wave: Option<i64>,
    pub errors: Vec<String>,
}

impl AgentDraftReport {
    #[must_use]
    pub const fn passed(&self) -> bool {
        self.complete && self.errors.is_empty()
    }

    pub(crate) fn early(error: String) -> Self {
        Self {
            complete: false,
            packages: 0,
            ticket_drafts: 0,
            context_drafts: 0,
            qualification_cases: 0,
            launch_stage: None,
            launch_wave: None,
            errors: vec![error],
        }
    }
}

#[must_use]
pub fn validate_w1_agent_drafts(root: &Path) -> AgentDraftReport {
    w1::validate_w1_agent_drafts(root)
}

#[must_use]
pub fn validate_w2_agent_drafts(root: &Path) -> AgentDraftReport {
    w2::validate_w2_agent_drafts(root)
}

#[must_use]
pub const fn exit_code(report: &AgentDraftReport) -> i32 {
    if report.passed() { 0 } else { 1 }
}

#[must_use]
pub fn render_report_json(report: &AgentDraftReport) -> String {
    let value = if report.complete {
        json!({
            "status": if report.passed() { "PASS" } else { "FAIL" },
            "packages": report.packages,
            "ticket_drafts": report.ticket_drafts,
            "context_drafts": report.context_drafts,
            "qualification_cases": report.qualification_cases,
            "launch_stage": report.launch_stage,
            "launch_wave": report.launch_wave,
            "errors": report.errors,
        })
    } else {
        json!({
            "status": "FAIL",
            "errors": report.errors,
        })
    };
    serde_json::to_string_pretty(&value)
        .expect("serializing a bounded agent-draft report cannot fail")
}

pub(crate) fn load_doc(root: &Path, relative: &str) -> Result<Value, String> {
    let bytes = std::fs::read(root.join(relative))
        .map_err(|error| format!("{relative}: {error}"))?;
    let text = String::from_utf8(bytes)
        .map_err(|error| format!("{relative}: {error}"))?;
    toml::from_str(&text).map_err(|error| format!("{relative}: {error}"))
}

pub(crate) fn indexed_rows(
    document: &Value,
    key: &str,
    identity: &str,
) -> Result<BTreeMap<String, Value>, String> {
    let rows = document
        .get(key)
        .and_then(Value::as_array)
        .ok_or_else(|| format!("{key} is not an array of tables"))?;
    let mut result = BTreeMap::new();
    for row in rows {
        let name = row
            .as_table()
            .and_then(|table| table.get(identity))
            .and_then(Value::as_str)
            .ok_or_else(|| format!("invalid {key} row"))?;
        if result.insert(name.to_owned(), row.clone()).is_some() {
            return Err(format!("duplicate {key} identity: {name}"));
        }
    }
    Ok(result)
}

pub(crate) fn string<'a>(value: &'a Value, key: &str) -> Option<&'a str> {
    value.get(key).and_then(Value::as_str)
}

pub(crate) fn integer(value: &Value, key: &str) -> Option<i64> {
    value.get(key).and_then(Value::as_integer)
}

pub(crate) fn boolean(value: &Value, key: &str) -> Option<bool> {
    value.get(key).and_then(Value::as_bool)
}

pub(crate) fn string_list(value: &Value, key: &str) -> Option<Vec<String>> {
    value
        .get(key)?
        .as_array()?
        .iter()
        .map(|item| item.as_str().map(str::to_owned))
        .collect()
}

pub(crate) fn child<'a>(
    value: &'a Value,
    table: &str,
    key: &str,
) -> Option<&'a Value> {
    value.get(table)?.as_table()?.get(key)
}

pub(crate) fn child_string<'a>(
    value: &'a Value,
    table: &str,
    key: &str,
) -> Option<&'a str> {
    child(value, table, key).and_then(Value::as_str)
}

pub(crate) fn child_bool(
    value: &Value,
    table: &str,
    key: &str,
) -> Option<bool> {
    child(value, table, key).and_then(Value::as_bool)
}

pub(crate) fn child_string_list(
    value: &Value,
    table: &str,
    key: &str,
) -> Option<Vec<String>> {
    child(value, table, key)?
        .as_array()?
        .iter()
        .map(|item| item.as_str().map(str::to_owned))
        .collect()
}

pub(crate) fn expected_strings(values: &[&str]) -> Vec<String> {
    values.iter().map(|value| (*value).to_owned()).collect()
}

pub(crate) fn symmetric_difference(
    actual: &BTreeSet<String>,
    expected: &BTreeSet<String>,
) -> Vec<String> {
    actual
        .symmetric_difference(expected)
        .cloned()
        .collect()
}

pub(crate) fn require_regular_file(
    root: &Path,
    owner: &str,
    value: Option<&str>,
    errors: &mut Vec<String>,
) {
    if value.is_none_or(|relative| !root.join(relative).is_file()) {
        errors.push(format!(
            "{owner}: missing referenced file {}",
            value.unwrap_or("<non-string>")
        ));
    }
}

pub(crate) fn read_text(root: &Path, relative: &str) -> Result<String, String> {
    std::fs::read_to_string(root.join(relative))
        .map_err(|error| format!("{relative}: {error}"))
}

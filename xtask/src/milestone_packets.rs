//! Rust validators for bounded, non-claimable milestone packets.
//!
//! These commands validate checked-in planning topology only. They never issue
//! tickets, leases, handoffs, gates or launch authority.

mod w1;

use std::collections::BTreeMap;
use std::path::Path;

use serde_json::json;
use toml::Value;

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct MilestonePacketReport {
    pub complete: bool,
    pub packages: usize,
    pub milestones: usize,
    pub cases: usize,
    pub launch_stage: Option<String>,
    pub launch_wave: Option<i64>,
    pub errors: Vec<String>,
}

impl MilestonePacketReport {
    #[must_use]
    pub const fn passed(&self) -> bool {
        self.complete && self.errors.is_empty()
    }

    pub(crate) fn early(error: String) -> Self {
        Self {
            complete: false,
            packages: 0,
            milestones: 0,
            cases: 0,
            launch_stage: None,
            launch_wave: None,
            errors: vec![error],
        }
    }
}

#[must_use]
pub fn validate_w1_milestone_packets(root: &Path) -> MilestonePacketReport {
    w1::validate_w1_milestone_packets(root)
}

#[must_use]
pub const fn exit_code(report: &MilestonePacketReport) -> i32 {
    if report.passed() { 0 } else { 1 }
}

#[must_use]
pub fn render_report_json(report: &MilestonePacketReport) -> String {
    let value = if report.complete {
        json!({
            "status": if report.passed() { "PASS" } else { "FAIL" },
            "packages": report.packages,
            "milestones": report.milestones,
            "cases": report.cases,
            "launch_stage": report.launch_stage,
            "launch_wave": report.launch_wave,
            "errors": report.errors,
        })
    } else {
        json!({"status": "FAIL", "errors": report.errors})
    };
    serde_json::to_string_pretty(&value)
        .expect("serializing a bounded milestone-packet report cannot fail")
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
        .ok_or_else(|| format!("{key} must be array"))?;
    let mut result = BTreeMap::new();
    for row in rows {
        let name = row
            .as_table()
            .and_then(|table| table.get(identity))
            .and_then(Value::as_str)
            .ok_or_else(|| format!("bad {key} row"))?;
        if result.insert(name.to_owned(), row.clone()).is_some() {
            return Err(format!("duplicate {name}"));
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

pub(crate) fn expected_strings(values: &[&str]) -> Vec<String> {
    values.iter().map(|value| (*value).to_owned()).collect()
}

pub(crate) fn read_text(root: &Path, relative: &str) -> Result<String, String> {
    std::fs::read_to_string(root.join(relative))
        .map_err(|error| format!("{relative}: {error}"))
}

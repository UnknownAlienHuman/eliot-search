//! Rust owner for the architecture coverage contract-closure validator.
//!
//! This validator covers the operation/task/qualification contract layer that
//! was previously implemented by `validate-architecture-coverage-contracts.py`.
//! The broader architecture graph validator remains a separate migration slice.

mod operations;
mod qualification;
mod tasks;

use std::collections::BTreeMap;
use std::path::Path;

use serde_json::json;
use toml::Value;

/// Structural architecture coverage contract report.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ArchitectureCoverageContractsReport {
    /// Whether all mandatory repository inputs were loaded and parsed.
    pub complete: bool,
    /// Number of registered packages.
    pub packages: usize,
    /// Number of unique package assignment tasks.
    pub assignment_tasks: usize,
    /// Number of delivery-slice tasks.
    pub delivery_tasks: usize,
    /// Number of non-foundation function source packets.
    pub package_function_sources: usize,
    /// Number of package-qualified operations derived from function sources.
    pub derived_package_qualified_operations: usize,
    /// Number of qualification inventory rows.
    pub qualification_cases: usize,
    /// Current launch stage.
    pub launch_stage: Option<String>,
    /// Current launch wave.
    pub launch_wave: Option<i64>,
    /// Stable validation errors.
    pub errors: Vec<String>,
}

impl ArchitectureCoverageContractsReport {
    /// Returns true only for a complete report without errors.
    #[must_use]
    pub const fn passed(&self) -> bool {
        self.complete && self.errors.is_empty()
    }

    fn early(error: String) -> Self {
        Self {
            complete: false,
            packages: 0,
            assignment_tasks: 0,
            delivery_tasks: 0,
            package_function_sources: 0,
            derived_package_qualified_operations: 0,
            qualification_cases: 0,
            launch_stage: None,
            launch_wave: None,
            errors: vec![error],
        }
    }
}

/// Validates architecture operation/task/qualification closure.
#[must_use]
pub fn validate_architecture_coverage_contracts(
    root: &Path,
) -> ArchitectureCoverageContractsReport {
    match validate(root) {
        Ok(report) => report,
        Err(error) => ArchitectureCoverageContractsReport::early(error),
    }
}

/// Stable process exit code for a report.
#[must_use]
pub const fn exit_code(report: &ArchitectureCoverageContractsReport) -> i32 {
    if report.passed() { 0 } else { 1 }
}

/// Renders the report as deterministic pretty JSON.
#[must_use]
pub fn render_report_json(
    report: &ArchitectureCoverageContractsReport,
) -> String {
    let value = if report.complete {
        json!({
            "status": if report.passed() { "PASS" } else { "FAIL" },
            "packages": report.packages,
            "assignment_tasks": report.assignment_tasks,
            "delivery_tasks": report.delivery_tasks,
            "package_function_sources": report.package_function_sources,
            "derived_package_qualified_operations": report.derived_package_qualified_operations,
            "qualification_cases": report.qualification_cases,
            "launch_stage": report.launch_stage,
            "launch_wave": report.launch_wave,
            "errors": report.errors,
        })
    } else {
        json!({"status": "FAIL", "errors": report.errors})
    };
    serde_json::to_string_pretty(&value)
        .expect("serializing a bounded architecture coverage report cannot fail")
}

fn validate(root: &Path) -> Result<ArchitectureCoverageContractsReport, String> {
    let manifest = load(root, "swarm/coverage/manifest.toml")?;
    let packages = indexed_rows(
        &load(root, "swarm/crates.toml")?,
        "package",
        "name",
    )?;
    let functions_doc = load(root, "swarm/function-packets.toml")?;
    let foundations = indexed_rows(&functions_doc, "foundation", "package")?;
    let functions = indexed_rows(&functions_doc, "package", "name")?;
    let modules_doc = load(root, "swarm/module-packets.toml")?;
    let operations_doc = load(root, "swarm/coverage/operations.toml")?;
    let tasks_doc = load(root, "swarm/coverage/tasks.toml")?;
    let delivery = indexed_rows(
        &load(root, "swarm/coverage/delivery-slices.toml")?,
        "slice",
        "id",
    )?;
    let cases_doc = load(
        root,
        "qualification/architecture-coverage/cases-v1.toml",
    )?;
    let launch = load(root, "swarm/launch-state.toml")?;

    let mut errors = Vec::new();
    require(
        &mut errors,
        !packages.is_empty() && packages.len() == 45,
        "package set must contain 45 packages",
    );

    let operation_count = operations::validate(
        root,
        &packages,
        &foundations,
        &functions,
        &operations_doc,
        &mut errors,
    );
    let (assignment_count, delivery_count) = tasks::validate(
        root,
        &packages,
        &tasks_doc,
        &delivery,
        &mut errors,
    );
    let qualification_count = qualification::validate(
        &manifest,
        &modules_doc,
        &cases_doc,
        &launch,
        assignment_count,
        delivery_count,
        &mut errors,
    );

    Ok(ArchitectureCoverageContractsReport {
        complete: true,
        packages: packages.len(),
        assignment_tasks: assignment_count,
        delivery_tasks: delivery_count,
        package_function_sources: functions.len(),
        derived_package_qualified_operations: operation_count,
        qualification_cases: qualification_count,
        launch_stage: string(&launch, "active_stage").map(str::to_owned),
        launch_wave: integer(&launch, "active_wave"),
        errors,
    })
}

pub(super) fn load(root: &Path, relative: &str) -> Result<Value, String> {
    let bytes = std::fs::read(root.join(relative))
        .map_err(|error| format!("{relative}: {error}"))?;
    let text = String::from_utf8(bytes)
        .map_err(|error| format!("{relative}: {error}"))?;
    toml::from_str(&text).map_err(|error| format!("{relative}: {error}"))
}

pub(super) fn indexed_rows(
    document: &Value,
    key: &str,
    identity: &str,
) -> Result<BTreeMap<String, Value>, String> {
    let rows = document
        .get(key)
        .and_then(Value::as_array)
        .ok_or_else(|| format!("{key} must be an array of tables"))?;
    let mut result = BTreeMap::new();
    for row in rows {
        let name = row
            .as_table()
            .and_then(|table| table.get(identity))
            .and_then(Value::as_str)
            .ok_or_else(|| format!("invalid {key} row"))?;
        if result.insert(name.to_owned(), row.clone()).is_some() {
            return Err(format!("duplicate {key} row {name}"));
        }
    }
    Ok(result)
}

pub(super) fn require(
    errors: &mut Vec<String>,
    condition: bool,
    message: impl Into<String>,
) {
    if !condition {
        errors.push(message.into());
    }
}

pub(super) fn string<'a>(value: &'a Value, key: &str) -> Option<&'a str> {
    value.get(key).and_then(Value::as_str)
}

pub(super) fn integer(value: &Value, key: &str) -> Option<i64> {
    value.get(key).and_then(Value::as_integer)
}

pub(super) fn boolean(value: &Value, key: &str) -> Option<bool> {
    value.get(key).and_then(Value::as_bool)
}

pub(super) fn string_list(value: &Value, key: &str) -> Vec<String> {
    value
        .get(key)
        .and_then(Value::as_array)
        .map(|items| {
            items
                .iter()
                .filter_map(|item| item.as_str().map(str::to_owned))
                .collect()
        })
        .unwrap_or_default()
}

pub(super) fn child<'a>(
    value: &'a Value,
    table: &str,
    key: &str,
) -> Option<&'a Value> {
    value.get(table)?.as_table()?.get(key)
}

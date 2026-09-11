use std::collections::BTreeSet;
use std::path::Path;

use super::model::{Docs, Indexes, ProgramReport};

fn load_doc(root: &Path, relative: &str) -> Result<toml::Value, String> {
    match std::fs::read(root.join(relative)) {
        Err(err) if err.kind() == std::io::ErrorKind::NotFound => {
            Err(format!("FileNotFoundError: {relative}: {err}"))
        }
        Err(err) => Err(format!("OSError: {relative}: {err}")),
        Ok(bytes) => match String::from_utf8(bytes) {
            Err(err) => Err(format!("UnicodeDecodeError: {relative}: {err}")),
            Ok(text) => text
                .parse::<toml::Value>()
                .map_err(|err| format!("TOMLDecodeError: {relative}: {err}")),
        },
    }
}

/// Array-of-tables keyed by `identity`, duplicate-rejecting, in document order.
fn rows<'a>(
    document: &'a toml::Value,
    key: &str,
    identity: &str,
) -> Result<Vec<(String, &'a toml::Value)>, String> {
    let Some(items) = document.get(key).and_then(toml::Value::as_array) else {
        return Err(format!("ValueError: {key} must be an array of tables"));
    };
    let mut result = Vec::with_capacity(items.len());
    let mut seen = BTreeSet::new();
    for row in items {
        let Some(id) = row.get(identity).and_then(toml::Value::as_str) else {
            return Err(format!("ValueError: invalid {key} row"));
        };
        if !seen.insert(id.to_owned()) {
            return Err(format!("ValueError: duplicate {key} identity: {id}"));
        }
        result.push((id.to_owned(), row));
    }
    Ok(result)
}

pub(super) fn child<'a>(
    value: Option<&'a toml::Value>,
    key: &str,
    errors: &mut Vec<String>,
) -> Option<&'a toml::Value> {
    match value.and_then(|table| table.get(key)) {
        None => None,
        Some(inner @ toml::Value::Table(_)) => Some(inner),
        Some(_) => {
            errors.push(format!("{key} is not a table"));
            None
        }
    }
}

pub(super) fn get<'a>(table: Option<&'a toml::Value>, key: &str) -> Option<&'a toml::Value> {
    table.and_then(|value| value.get(key))
}

pub(super) fn is_str(table: Option<&toml::Value>, key: &str, expected: &str) -> bool {
    get(table, key).and_then(toml::Value::as_str) == Some(expected)
}

pub(super) fn is_int(table: Option<&toml::Value>, key: &str, expected: i64) -> bool {
    get(table, key).and_then(toml::Value::as_integer) == Some(expected)
}

pub(super) fn is_bool(table: Option<&toml::Value>, key: &str, expected: bool) -> bool {
    get(table, key).and_then(toml::Value::as_bool) == Some(expected)
}

pub(super) fn is_str_list(
    table: Option<&toml::Value>,
    key: &str,
    expected: &[&str],
) -> bool {
    let Some(items) = get(table, key).and_then(toml::Value::as_array) else {
        return false;
    };
    items.len() == expected.len()
        && items
            .iter()
            .zip(expected.iter())
            .all(|(item, want)| item.as_str() == Some(*want))
}

pub(super) const fn is_zero(value: Option<&toml::Value>) -> bool {
    match value {
        Some(toml::Value::Integer(0) | toml::Value::Boolean(false)) => true,
        Some(toml::Value::Float(f)) => {
            f.to_bits() == 0.0_f64.to_bits() || f.to_bits() == (-0.0_f64).to_bits()
        }
        _ => false,
    }
}

pub(super) const fn is_one(value: Option<&toml::Value>) -> bool {
    match value {
        Some(toml::Value::Integer(1) | toml::Value::Boolean(true)) => true,
        Some(toml::Value::Float(f)) => f.to_bits() == 1.0_f64.to_bits(),
        _ => false,
    }
}

pub(super) fn is_non_blank_str(value: Option<&toml::Value>) -> bool {
    value
        .and_then(toml::Value::as_str)
        .is_some_and(|text| !text.trim().is_empty())
}

pub(super) fn scalar_text(value: &toml::Value) -> String {
    match value {
        toml::Value::String(text) => text.clone(),
        toml::Value::Integer(number) => number.to_string(),
        toml::Value::Float(number) => number.to_string(),
        toml::Value::Boolean(true) => "True".to_owned(),
        toml::Value::Boolean(false) => "False".to_owned(),
        _ => format!("{value:?}"),
    }
}

pub(super) fn python_str_list(items: &BTreeSet<String>) -> String {
    let mut out = String::from("[");
    for (index, item) in items.iter().enumerate() {
        if index > 0 {
            out.push_str(", ");
        }
        out.push('\'');
        out.push_str(item);
        out.push('\'');
    }
    out.push(']');
    out
}

pub(super) fn require(errors: &mut Vec<String>, condition: bool, message: String) {
    if !condition {
        errors.push(message);
    }
}

pub(super) fn load_all(root: &Path) -> Result<Docs, String> {
    Ok(Docs {
        program: load_doc(root, "swarm/implementation-program.toml")?,
        launch: load_doc(root, "swarm/launch-state.toml")?,
        stages_doc: load_doc(root, "swarm/stages.toml")?,
        gates_doc: load_doc(root, "swarm/gates.toml")?,
        packages_doc: load_doc(root, "swarm/crates.toml")?,
        metrics: load_doc(root, "qualification/product-pulse/metrics.toml")?,
        coverage: load_doc(root, "swarm/coverage/manifest.toml")?,
        cases: load_doc(root, "qualification/implementation-program/cases-v1.toml")?,
    })
}

pub(super) fn index_all(docs: &Docs) -> Result<Indexes<'_>, String> {
    Ok(Indexes {
        program_stages: rows(&docs.program, "stage", "id")?,
        stages: rows(&docs.stages_doc, "stage", "id")?,
        gates: rows(&docs.gates_doc, "gate", "id")?,
        packages: rows(&docs.packages_doc, "package", "name")?,
        targets: rows(&docs.program, "target", "id")?,
        integration_steps: rows(&docs.program, "integration_step", "id")?,
        next_steps: rows(&docs.program, "next_step", "id")?,
    })
}

pub(super) fn incomplete(message: String) -> ProgramReport {
    ProgramReport {
        complete: false,
        passed: false,
        stages: 0,
        packages: 0,
        targets: 0,
        integration_steps: 0,
        next_steps: 0,
        baseline_requirements: 0,
        current_stage: None,
        current_wave: None,
        cargo_lock_present: false,
        errors: vec![message],
    }
}

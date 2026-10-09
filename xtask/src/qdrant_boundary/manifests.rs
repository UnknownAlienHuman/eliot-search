//! Narrow syntax policy only: unused pins/inheritance are not Cargo graph facts.
use toml::Value;

use super::{BRIDGE_MANIFEST, ROOT_MANIFEST, VENDOR_CRATE};

pub(super) fn validate_workspace_pin_syntax(
    document: &Value,
    errors: &mut Vec<String>,
) -> Option<String> {
    reject_vendor_overrides(document, ROOT_MANIFEST, errors);
    let Some(dependencies) =
        value_at(document, &["workspace", "dependencies"]).and_then(Value::as_table)
    else {
        errors.push(format!("{ROOT_MANIFEST}: workspace dependencies missing"));
        return None;
    };
    for (name, entry) in dependencies {
        if name != VENDOR_CRATE
            && entry.get("package").and_then(Value::as_str) == Some(VENDOR_CRATE)
        {
            errors.push(format!(
                "{ROOT_MANIFEST}: renamed workspace vendor pin {name} is forbidden"
            ));
        }
    }
    let Some(table) = dependencies.get(VENDOR_CRATE).and_then(Value::as_table) else {
        errors.push(format!(
            "{ROOT_MANIFEST}: workspace.dependencies.{VENDOR_CRATE} table is missing"
        ));
        return None;
    };
    for key in table.keys() {
        if !matches!(key.as_str(), "version" | "default-features") {
            errors.push(format!(
                "{ROOT_MANIFEST}: workspace {VENDOR_CRATE} forbidden pin field {key}"
            ));
        }
    }
    if table.get("default-features").and_then(Value::as_bool) != Some(false) {
        errors.push(format!(
            "{ROOT_MANIFEST}: workspace {VENDOR_CRATE} must keep default-features = false"
        ));
    }
    let version = table
        .get("version")
        .and_then(Value::as_str)
        .and_then(|version| version.strip_prefix('='));
    match version {
        Some(version) if numeric_release_pin(version) => Some(version.to_owned()),
        _ => {
            errors.push(format!("{ROOT_MANIFEST}: workspace {VENDOR_CRATE} version must be an exact =major.minor.patch pin"));
            None
        }
    }
}

// The accepted qualified client uses a numeric release triplet. This checks
// only the literal pin policy; Cargo still supplies resolved version facts.
fn numeric_release_pin(version: &str) -> bool {
    let mut parts = version.split('.');
    (0..3).all(|_| {
        parts.next().is_some_and(|part| {
            !part.is_empty()
                && part.bytes().all(|byte| byte.is_ascii_digit())
                && (part.len() == 1 || !part.starts_with('0'))
                && part.parse::<u64>().is_ok()
        })
    }) && parts.next().is_none()
}

pub(super) fn validate_bridge_inheritance_syntax(document: &Value, errors: &mut Vec<String>) {
    reject_vendor_overrides(document, BRIDGE_MANIFEST, errors);
    let Some(table) = value_at(document, &["dependencies", VENDOR_CRATE]).and_then(Value::as_table)
    else {
        errors.push(format!(
            "{BRIDGE_MANIFEST}: {VENDOR_CRATE} must inherit the workspace dependency"
        ));
        return;
    };
    if table.get("workspace").and_then(Value::as_bool) != Some(true) {
        errors.push(format!(
            "{BRIDGE_MANIFEST}: {VENDOR_CRATE}.workspace must be true"
        ));
    }
    for key in table.keys() {
        if key != "workspace" {
            errors.push(format!("{BRIDGE_MANIFEST}: {VENDOR_CRATE} must not override workspace inheritance with {key}"));
        }
    }
}

fn vendor_entry(name: &str, entry: &Value) -> bool {
    // This is a conservative unused-override syntax ban, not package-ID
    // resolution. Cargo accepts colon/@ versions and explicit URL fragments.
    name.split('#').any(|segment| {
        let segment = segment.split('?').next().unwrap_or(segment);
        let component = segment.rsplit('/').next().unwrap_or(segment);
        let component = component.split([':', '@']).next().unwrap_or(component);
        component.strip_suffix(".git").unwrap_or(component) == VENDOR_CRATE
    }) || entry.get("package").and_then(Value::as_str) == Some(VENDOR_CRATE)
}

fn reject_vendor_overrides(document: &Value, label: &str, errors: &mut Vec<String>) {
    if let Some(patches) = document.get("patch").and_then(Value::as_table) {
        for (source, entries) in patches {
            if let Some(entries) = entries.as_table() {
                for (name, entry) in entries {
                    if vendor_entry(name, entry) {
                        errors.push(format!("{label}: vendor dependency declared at patch.{source}.{name}; qualified source cannot be substituted"));
                    }
                }
            }
        }
    }
    if let Some(entries) = document.get("replace").and_then(Value::as_table) {
        for (name, entry) in entries {
            if vendor_entry(name, entry) {
                errors.push(format!("{label}: vendor dependency declared at replace.{name}; qualified source cannot be substituted"));
            }
        }
    }
}

pub(super) fn value_at<'a>(document: &'a Value, path: &[&str]) -> Option<&'a Value> {
    let mut current = document;
    for key in path {
        current = current.as_table()?.get(*key)?;
    }
    Some(current)
}

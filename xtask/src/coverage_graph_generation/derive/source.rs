//! Shared bounded source loading for coverage derivation.

use std::collections::{BTreeMap, BTreeSet};
use std::path::Path;
use std::process::Command;

use toml::Value;

const MAX_SOURCE_BYTES: u64 = 16 * 1024 * 1024;

pub(super) fn git_files(root: &Path) -> Result<Vec<String>, String> {
    let output = Command::new("git")
        .arg("-C")
        .arg(root)
        .args(["ls-files", "-z"])
        .output()
        .map_err(|error| format!("git ls-files: {error}"))?;
    if !output.status.success() {
        return Err(format!(
            "git ls-files failed: {}",
            String::from_utf8_lossy(&output.stderr).trim()
        ));
    }
    let mut files = Vec::new();
    for record in output.stdout.split(|byte| *byte == 0).filter(|row| !row.is_empty()) {
        files.push(
            std::str::from_utf8(record)
                .map_err(|_| "git ls-files returned a non-UTF-8 path".to_owned())?
                .to_owned(),
        );
    }
    files.sort();
    Ok(files)
}

pub(super) fn read_bounded_utf8(root: &Path, relative: &str) -> Result<String, String> {
    let path = root.join(relative);
    let metadata = std::fs::metadata(&path)
        .map_err(|error| format!("{relative}: {error}"))?;
    if !metadata.is_file() || metadata.len() > MAX_SOURCE_BYTES {
        return Err(format!("{relative}: source is not a bounded regular file"));
    }
    std::fs::read_to_string(path)
        .map_err(|error| format!("{relative}: strict UTF-8 read failed: {error}"))
}

pub(super) fn load_toml(root: &Path, relative: &str) -> Result<Value, String> {
    let text = read_bounded_utf8(root, relative)?;
    toml::from_str(&text).map_err(|error| format!("{relative}: {error}"))
}

pub(super) fn indexed(
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
            .get(identity)
            .and_then(Value::as_str)
            .ok_or_else(|| format!("{key}: row missing {identity}"))?;
        if result.insert(name.to_owned(), row.clone()).is_some() {
            return Err(format!("{key}: duplicate identity {name}"));
        }
    }
    Ok(result)
}

pub(super) fn indexed_or_empty(
    document: &Value,
    key: &str,
    identity: &str,
    errors: &mut Vec<String>,
) -> BTreeMap<String, Value> {
    match indexed(document, key, identity) {
        Ok(rows) => rows,
        Err(error) => {
            errors.push(error);
            BTreeMap::new()
        }
    }
}

pub(super) fn string_array(value: &Value, key: &str) -> Vec<String> {
    value
        .get(key)
        .and_then(Value::as_array)
        .map(|items| {
            items
                .iter()
                .filter_map(Value::as_str)
                .map(str::to_owned)
                .collect()
        })
        .unwrap_or_default()
}

pub(super) fn compare_identity_sets(
    label: &str,
    expected: BTreeSet<String>,
    actual: BTreeSet<String>,
    errors: &mut Vec<String>,
) {
    if expected == actual {
        return;
    }
    let difference: Vec<String> = expected
        .symmetric_difference(&actual)
        .take(24)
        .cloned()
        .collect();
    errors.push(format!(
        "{label} registry drift (first differences): {difference:?}"
    ));
}

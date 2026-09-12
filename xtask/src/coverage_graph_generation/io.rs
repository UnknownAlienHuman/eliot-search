//! Bounded filesystem reconciliation for derived coverage files.

use std::fs;
use std::path::Path;

pub(super) fn stale(root: &Path, expected: &[(&str, &str)]) -> Vec<String> {
    let mut result = Vec::new();
    for &(relative, content) in expected {
        match fs::read_to_string(root.join(relative)) {
            Ok(actual) if actual == content => {}
            _ => result.push(relative.to_owned()),
        }
    }
    result.sort();
    result
}

pub(super) fn write_all(
    root: &Path,
    expected: &[(&str, &str)],
) -> Result<(), String> {
    for &(relative, content) in expected {
        let target = root.join(relative);
        if let Some(parent) = target.parent() {
            fs::create_dir_all(parent)
                .map_err(|error| format!("{relative}: {error}"))?;
        }
        fs::write(&target, content.as_bytes())
            .map_err(|error| format!("{relative}: {error}"))?;
        let readback = fs::read(&target)
            .map_err(|error| format!("{relative}: readback failed: {error}"))?;
        if readback != content.as_bytes() {
            return Err(format!("{relative}: exact readback mismatch"));
        }
    }
    Ok(())
}

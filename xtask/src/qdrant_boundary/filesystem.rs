use std::fs::{self, File};
use std::io::Read;
use std::path::{Path, PathBuf};

use toml::Value;

const DEFAULT_MAX_DEPTH: usize = 64;
const DEFAULT_MAX_ENTRIES: usize = 200_000;
const DEFAULT_MAX_FILE_BYTES: u64 = 16 * 1024 * 1024;
const DEFAULT_MAX_TOTAL_BYTES: u64 = 256 * 1024 * 1024;

#[derive(Clone, Copy, Debug)]
struct ScanLimits {
    depth: usize,
    entries: usize,
    file_bytes: u64,
    total_bytes: u64,
}

impl Default for ScanLimits {
    fn default() -> Self {
        Self {
            depth: DEFAULT_MAX_DEPTH,
            entries: DEFAULT_MAX_ENTRIES,
            file_bytes: DEFAULT_MAX_FILE_BYTES,
            total_bytes: DEFAULT_MAX_TOTAL_BYTES,
        }
    }
}

#[derive(Debug, Default)]
pub(super) struct ScanBudget {
    limits: ScanLimits,
    entries_seen: usize,
    bytes_read: u64,
    stopped: bool,
}

impl ScanBudget {
    #[cfg(test)]
    const fn with_limits(limits: ScanLimits) -> Self {
        Self {
            limits,
            entries_seen: 0,
            bytes_read: 0,
            stopped: false,
        }
    }

    fn stop(&mut self, errors: &mut Vec<String>, message: String) {
        if !self.stopped {
            errors.push(message);
            self.stopped = true;
        }
    }

    fn observe_entry(&mut self, errors: &mut Vec<String>) -> bool {
        self.entries_seen = self.entries_seen.saturating_add(1);
        if self.entries_seen > self.limits.entries {
            self.stop(
                errors,
                format!(
                    "repository scan exceeded the {}-entry limit",
                    self.limits.entries
                ),
            );
            false
        } else {
            true
        }
    }

    const fn remaining_bytes(&self) -> u64 {
        self.limits.total_bytes.saturating_sub(self.bytes_read)
    }
}

pub(super) fn collect_files(
    root: &Path,
    directory: &Path,
    depth: usize,
    budget: &mut ScanBudget,
    files: &mut Vec<PathBuf>,
    errors: &mut Vec<String>,
) {
    if budget.stopped {
        return;
    }
    if depth > budget.limits.depth {
        budget.stop(
            errors,
            format!(
                "{}: repository scan exceeded the {}-directory-depth limit",
                relative_path(root, directory),
                budget.limits.depth
            ),
        );
        return;
    }
    if depth == 0 && !validate_root(root, errors) {
        return;
    }

    let entries = match fs::read_dir(directory) {
        Ok(entries) => entries,
        Err(error) => {
            errors.push(format!(
                "{}: unable to read directory: {error}",
                relative_path(root, directory)
            ));
            return;
        }
    };

    let Some(mut entries) = collect_bounded_entries(
        entries,
        &relative_path(root, directory),
        budget,
        errors,
    ) else {
        return;
    };
    entries.sort_by_key(std::fs::DirEntry::file_name);

    for entry in entries {
        if budget.stopped {
            return;
        }
        let name = entry.file_name();
        if ignored_name(&name) {
            continue;
        }

        let path = entry.path();
        let file_type = match entry.file_type() {
            Ok(file_type) => file_type,
            Err(error) => {
                errors.push(format!(
                    "{}: unable to inspect file type: {error}",
                    relative_path(root, &path)
                ));
                continue;
            }
        };
        if file_type.is_symlink() {
            errors.push(format!(
                "{}: symbolic links are not allowed in the Qdrant boundary scan",
                relative_path(root, &path)
            ));
        } else if file_type.is_dir() {
            collect_files(root, &path, depth + 1, budget, files, errors);
        } else if file_type.is_file() {
            files.push(path);
        } else {
            errors.push(format!(
                "{}: unsupported filesystem entry type",
                relative_path(root, &path)
            ));
        }
    }
}

// Charge entries before buffering or sorting them. A limit checked only in the
// later traversal loop does not bound the directory allocation itself. Buffered
// parent entries and recursive child entries share this one non-refundable budget.
fn collect_bounded_entries<T>(
    entries: impl IntoIterator<Item = std::io::Result<T>>,
    label: &str,
    budget: &mut ScanBudget,
    errors: &mut Vec<String>,
) -> Option<Vec<T>> {
    if budget.stopped {
        return None;
    }
    let mut collected = Vec::new();
    for entry in entries {
        // At most one extra entry is observed to detect exhaustion; it is never
        // retained and the iterator is not drained after the limit is reached.
        if !budget.observe_entry(errors) {
            return None;
        }
        match entry {
            Ok(entry) => collected.push(entry),
            Err(error) => {
                errors.push(format!(
                    "{label}: unable to enumerate directory: {error}"
                ));
                return None;
            }
        }
    }
    Some(collected)
}

fn validate_root(root: &Path, errors: &mut Vec<String>) -> bool {
    match fs::symlink_metadata(root) {
        Ok(metadata) if metadata.file_type().is_symlink() => {
            errors.push(".: repository scan root is a symbolic link".to_owned());
            false
        }
        Ok(metadata) if metadata.is_dir() => true,
        Ok(_) => {
            errors.push(".: repository scan root is not a directory".to_owned());
            false
        }
        Err(error) => {
            errors.push(format!(".: unable to inspect repository root: {error}"));
            false
        }
    }
}

fn ignored_name(name: &std::ffi::OsStr) -> bool {
    matches!(
        name.to_str(),
        Some(
            ".git"
                | "target"
                | ".venv"
                | "__pycache__"
                | "node_modules"
        )
    )
}

pub(super) fn relative_path(root: &Path, path: &Path) -> String {
    let relative = path.strip_prefix(root).unwrap_or(path);
    let rendered = relative.to_string_lossy().replace('\\', "/");
    if rendered.is_empty() {
        ".".to_owned()
    } else {
        rendered
    }
}

pub(super) fn read_text(
    path: &Path,
    label: &str,
    budget: &mut ScanBudget,
    errors: &mut Vec<String>,
) -> Option<String> {
    if budget.stopped {
        return None;
    }
    match fs::symlink_metadata(path) {
        Ok(metadata) if metadata.file_type().is_symlink() => {
            errors.push(format!(
                "{label}: symbolic links are not allowed in the Qdrant boundary scan"
            ));
            return None;
        }
        Ok(metadata) if !metadata.is_file() => {
            errors.push(format!("{label}: expected a regular file"));
            return None;
        }
        Ok(_) => {}
        Err(error) => {
            errors.push(format!("{label}: unable to inspect file: {error}"));
            return None;
        }
    }

    let file = match File::open(path) {
        Ok(file) => file,
        Err(error) => {
            errors.push(format!("{label}: unable to open file: {error}"));
            return None;
        }
    };
    let declared_bytes = match file.metadata() {
        Ok(metadata) if metadata.is_file() => metadata.len(),
        Ok(_) => {
            errors.push(format!("{label}: opened object is not a regular file"));
            return None;
        }
        Err(error) => {
            errors.push(format!("{label}: unable to inspect opened file: {error}"));
            return None;
        }
    };
    if declared_bytes > budget.limits.file_bytes {
        errors.push(format!(
            "{label}: file length {declared_bytes} exceeds the {}-byte per-file limit",
            budget.limits.file_bytes
        ));
        return None;
    }
    let remaining = budget.remaining_bytes();
    if declared_bytes > remaining {
        budget.stop(
            errors,
            format!(
                "{label}: repository text scan exceeded the {}-byte aggregate limit",
                budget.limits.total_bytes
            ),
        );
        return None;
    }

    read_bounded_utf8(file, label, budget, errors)
}

fn read_bounded_utf8(
    reader: impl Read,
    label: &str,
    budget: &mut ScanBudget,
    errors: &mut Vec<String>,
) -> Option<String> {
    if budget.stopped {
        return None;
    }
    let read_limit = budget.limits.file_bytes.min(budget.remaining_bytes());
    let mut bytes = Vec::new();
    let result = reader
        .take(read_limit.saturating_add(1))
        .read_to_end(&mut bytes);

    // Charge physical bytes before any error or decoding exit. read_to_string
    // may discard invalid UTF-8, and read_to_end can fail after a partial read;
    // neither case refunds the work already done. The single overflow probe
    // byte is charged too, and aggregate exhaustion latches the entire scan.
    let actual_bytes = u64::try_from(bytes.len()).unwrap_or(u64::MAX);
    budget.bytes_read = budget.bytes_read.saturating_add(actual_bytes);
    if budget.bytes_read > budget.limits.total_bytes {
        budget.stop(
            errors,
            format!(
                "{label}: repository text scan exceeded the {}-byte aggregate limit",
                budget.limits.total_bytes
            ),
        );
        return None;
    }
    if actual_bytes > read_limit {
        errors.push(format!(
            "{label}: file grew beyond the bounded read allowance"
        ));
        return None;
    }
    if let Err(error) = result {
        errors.push(format!("{label}: unable to read UTF-8 text: {error}"));
        return None;
    }
    match String::from_utf8(bytes) {
        Ok(text) => Some(text),
        Err(error) => {
            errors.push(format!("{label}: unable to read UTF-8 text: {error}"));
            None
        }
    }
}

pub(super) fn read_toml(
    path: &Path,
    label: &str,
    budget: &mut ScanBudget,
    errors: &mut Vec<String>,
) -> Option<Value> {
    let text = read_text(path, label, budget, errors)?;
    match text.parse::<Value>() {
        Ok(document) => Some(document),
        Err(error) => {
            errors.push(format!("{label}: invalid TOML: {error}"));
            None
        }
    }
}

#[cfg(test)]
mod tests;

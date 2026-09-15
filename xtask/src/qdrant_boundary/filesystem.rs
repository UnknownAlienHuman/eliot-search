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

    let mut entries = match entries.collect::<Result<Vec<_>, _>>() {
        Ok(entries) => entries,
        Err(error) => {
            errors.push(format!(
                "{}: unable to enumerate directory: {error}",
                relative_path(root, directory)
            ));
            return;
        }
    };
    entries.sort_by_key(std::fs::DirEntry::file_name);

    for entry in entries {
        if budget.stopped || !budget.observe_entry(errors) {
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

    let read_limit = budget.limits.file_bytes.min(remaining);
    let mut text = String::new();
    let mut reader = file.take(read_limit.saturating_add(1));
    if let Err(error) = reader.read_to_string(&mut text) {
        errors.push(format!("{label}: unable to read UTF-8 text: {error}"));
        return None;
    }
    let actual_bytes = u64::try_from(text.len()).unwrap_or(u64::MAX);
    if actual_bytes > read_limit {
        errors.push(format!(
            "{label}: file grew beyond the bounded read allowance"
        ));
        return None;
    }
    budget.bytes_read = budget.bytes_read.saturating_add(actual_bytes);
    Some(text)
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
mod tests {
    use std::io::ErrorKind;
    use std::sync::atomic::{AtomicU64, Ordering};

    use super::*;

    struct Fixture {
        root: PathBuf,
    }

    impl Fixture {
        fn new() -> Self {
            static NEXT: AtomicU64 = AtomicU64::new(0);
            for _ in 0..128 {
                let root = std::env::temp_dir().join(format!(
                    "eliot-qdrant-boundary-fs-{}-{}",
                    std::process::id(),
                    NEXT.fetch_add(1, Ordering::Relaxed),
                ));
                match fs::create_dir(&root) {
                    Ok(()) => return Self { root },
                    Err(error) if error.kind() == ErrorKind::AlreadyExists => {}
                    Err(error) => panic!("cannot create fixture: {error}"),
                }
            }
            panic!("fixture directory collision budget exhausted");
        }

        fn write(&self, relative: &str, text: &str) -> PathBuf {
            let path = self.root.join(relative);
            fs::create_dir_all(path.parent().expect("fixture parent"))
                .expect("create fixture parent");
            fs::write(&path, text).expect("write fixture");
            path
        }
    }

    impl Drop for Fixture {
        fn drop(&mut self) {
            let _ = fs::remove_dir_all(&self.root);
        }
    }

    #[test]
    fn entry_budget_stops_enumeration() {
        let fixture = Fixture::new();
        fixture.write("a.rs", "");
        fixture.write("b.rs", "");
        let mut budget = ScanBudget::with_limits(ScanLimits {
            entries: 1,
            ..ScanLimits::default()
        });
        let mut files = Vec::new();
        let mut errors = Vec::new();
        collect_files(
            &fixture.root,
            &fixture.root,
            0,
            &mut budget,
            &mut files,
            &mut errors,
        );
        assert!(errors.iter().any(|error| error.contains("entry limit")));
    }

    #[test]
    fn depth_budget_stops_recursion() {
        let fixture = Fixture::new();
        fixture.write("a/b/c.rs", "");
        let mut budget = ScanBudget::with_limits(ScanLimits {
            depth: 1,
            ..ScanLimits::default()
        });
        let mut files = Vec::new();
        let mut errors = Vec::new();
        collect_files(
            &fixture.root,
            &fixture.root,
            0,
            &mut budget,
            &mut files,
            &mut errors,
        );
        assert!(errors.iter().any(|error| error.contains("depth limit")));
    }

    #[test]
    fn per_file_and_aggregate_byte_limits_fail_closed() {
        let fixture = Fixture::new();
        let path = fixture.write("large.rs", "1234");

        let mut per_file_budget = ScanBudget::with_limits(ScanLimits {
            file_bytes: 3,
            ..ScanLimits::default()
        });
        let mut per_file_errors = Vec::new();
        assert!(
            read_text(
                &path,
                "large.rs",
                &mut per_file_budget,
                &mut per_file_errors,
            )
            .is_none()
        );
        assert!(
            per_file_errors
                .iter()
                .any(|error| error.contains("per-file limit"))
        );

        let mut aggregate_budget = ScanBudget::with_limits(ScanLimits {
            total_bytes: 3,
            ..ScanLimits::default()
        });
        let mut aggregate_errors = Vec::new();
        assert!(
            read_text(
                &path,
                "large.rs",
                &mut aggregate_budget,
                &mut aggregate_errors,
            )
            .is_none()
        );
        assert!(
            aggregate_errors
                .iter()
                .any(|error| error.contains("aggregate limit"))
        );
    }

    #[cfg(unix)]
    #[test]
    fn symbolic_link_is_reported_instead_of_skipped() {
        use std::os::unix::fs::symlink;

        let fixture = Fixture::new();
        let target = fixture.write("target.rs", "");
        symlink(target, fixture.root.join("escape.rs")).expect("create symlink");

        let mut budget = ScanBudget::default();
        let mut files = Vec::new();
        let mut errors = Vec::new();
        collect_files(
            &fixture.root,
            &fixture.root,
            0,
            &mut budget,
            &mut files,
            &mut errors,
        );
        assert!(
            errors
                .iter()
                .any(|error| error.contains("symbolic links are not allowed"))
        );
    }
}

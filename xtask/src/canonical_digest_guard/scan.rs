use std::fs::{self, File};
use std::io::Read;
use std::path::{Path, PathBuf};

pub(super) const MAX_FILE_BYTES: u64 = 2 * 1024 * 1024;
const MAX_TOTAL_BYTES: u64 = 64 * 1024 * 1024;
const MAX_ENTRIES: usize = 40_000;
const MAX_DEPTH: usize = 48;

#[derive(Default)]
pub(super) struct Budget {
    entries: usize,
    bytes: u64,
}

pub(super) fn files(root: &Path, budget: &mut Budget) -> Result<Vec<PathBuf>, String> {
    let metadata = fs::symlink_metadata(root).map_err(|e| e.to_string())?;
    if !metadata.is_dir() || reparse(&metadata) {
        return Err("scan root must be an ordinary directory".into());
    }
    let mut output = Vec::new();
    walk(root, 0, budget, &mut output)?;
    output.sort();
    Ok(output)
}

fn walk(
    path: &Path,
    depth: usize,
    budget: &mut Budget,
    output: &mut Vec<PathBuf>,
) -> Result<(), String> {
    if depth > MAX_DEPTH {
        return Err("directory depth limit exceeded".into());
    }
    for entry in fs::read_dir(path).map_err(|e| e.to_string())? {
        if budget.entries == MAX_ENTRIES {
            return Err("entry limit exceeded before buffering".into());
        }
        budget.entries += 1;
        let entry = entry.map_err(|e| e.to_string())?;
        // Build output and VCS object stores are outside the source denominator.
        // Archives, fixtures, tools, tests and optional workers are not excluded.
        if entry.file_name() == ".git" || (depth == 0 && entry.file_name() == "target") {
            continue;
        }
        let path = entry.path();
        let metadata = fs::symlink_metadata(&path).map_err(|e| e.to_string())?;
        if reparse(&metadata) {
            return Err(format!(
                "{}: links/reparse points are not inspected",
                path.display()
            ));
        }
        if metadata.is_dir() {
            walk(&path, depth + 1, budget, output)?;
        } else if metadata.is_file() {
            if path.extension().is_some_and(|ext| ext == "rs")
                || path.file_name().is_some_and(|name| name == "Cargo.toml")
            {
                output.push(path);
            }
        } else {
            return Err(format!("{}: unsupported entry", path.display()));
        }
    }
    Ok(())
}

fn reparse(metadata: &fs::Metadata) -> bool {
    if metadata.file_type().is_symlink() {
        return true;
    }
    #[cfg(windows)]
    {
        use std::os::windows::fs::MetadataExt;
        metadata.file_attributes() & 0x400 != 0
    }
    #[cfg(not(windows))]
    {
        false
    }
}

pub(super) fn read(path: &Path, budget: &mut Budget) -> Result<String, String> {
    read_bounded(path, budget, MAX_FILE_BYTES)
}

pub(super) fn read_bounded(
    path: &Path,
    budget: &mut Budget,
    file_limit: u64,
) -> Result<String, String> {
    let metadata = fs::symlink_metadata(path).map_err(|e| e.to_string())?;
    if reparse(&metadata) || !metadata.is_file() {
        return Err(format!("{}: expected ordinary file", path.display()));
    }
    let file = File::open(path).map_err(|e| e.to_string())?;
    let length = file.metadata().map_err(|e| e.to_string())?.len();
    let allowance = file_limit.min(MAX_TOTAL_BYTES.saturating_sub(budget.bytes));
    if length > allowance {
        return Err(format!("{}: read budget exceeded", path.display()));
    }
    let mut bytes = Vec::new();
    let result = file.take(allowance + 1).read_to_end(&mut bytes);
    let actual = u64::try_from(bytes.len()).map_err(|e| e.to_string())?;
    budget.bytes = budget.bytes.saturating_add(actual);
    if actual > allowance {
        return Err(format!("{}: file grew beyond read budget", path.display()));
    }
    result.map_err(|e| e.to_string())?;
    String::from_utf8(bytes).map_err(|e| e.to_string())
}

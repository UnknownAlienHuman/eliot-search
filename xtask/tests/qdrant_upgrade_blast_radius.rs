//! Prevents one qualified Qdrant release from being compiled into unrelated
//! production packages.
//!
//! Exact client/server identities belong to the adapter and qualification
//! records. Supervisor/daemon/query/publication/control production code must
//! consume validated artifact evidence rather than branch on a release string.

use std::fs;
use std::path::{Path, PathBuf};

const BRIDGE_PREFIX: &str =
    "crates/search-index-qdrant/search-qdrant-bridge/";
const QUALIFIED_SERVER_VERSION: &str = "1.19.0";
const QUALIFIED_SERVER_BUILD: &str = "74f3e85b";

#[test]
fn qualified_release_literals_do_not_escape_adapter_production_code() {
    let root = Path::new(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .expect("xtask must be a workspace member");
    let mut files = Vec::new();
    collect_rust_files(&root.join("crates"), &mut files);
    collect_rust_files(&root.join("bins"), &mut files);

    let mut violations = Vec::new();
    for path in files {
        let relative = path
            .strip_prefix(root)
            .expect("workspace source must remain under root")
            .to_string_lossy()
            .replace('\\', "/");
        if relative.starts_with(BRIDGE_PREFIX)
            || relative.contains("/tests/")
            || relative.ends_with("/build.rs")
        {
            continue;
        }
        let source = fs::read_to_string(&path)
            .unwrap_or_else(|error| panic!("{}: {error}", path.display()));
        for (line_number, line) in production_lines(&source) {
            if line.contains(&format!("\"{QUALIFIED_SERVER_VERSION}\""))
                || line.contains(&format!("\"{QUALIFIED_SERVER_BUILD}\""))
            {
                violations.push(format!("{relative}:{line_number}"));
            }
        }
    }

    assert!(
        violations.is_empty(),
        "qualified Qdrant release escaped adapter/qualification ownership: {violations:?}"
    );
}

fn production_lines(source: &str) -> Vec<(usize, &str)> {
    let mut output = Vec::new();
    let mut pending_test_item = false;
    let mut test_depth: Option<isize> = None;

    for (index, line) in source.lines().enumerate() {
        let trimmed = line.trim();
        if let Some(depth) = test_depth.as_mut() {
            *depth += brace_delta(trimmed);
            if *depth <= 0 {
                test_depth = None;
            }
            continue;
        }
        if trimmed.starts_with("#[cfg(test)]") {
            pending_test_item = true;
            continue;
        }
        if pending_test_item {
            if trimmed.is_empty() || trimmed.starts_with("#") {
                continue;
            }
            let depth = brace_delta(trimmed);
            if depth > 0 {
                test_depth = Some(depth);
            }
            pending_test_item = false;
            continue;
        }
        if trimmed.starts_with("//") {
            continue;
        }
        output.push((index + 1, line));
    }
    output
}

fn brace_delta(line: &str) -> isize {
    let opens = line.bytes().filter(|byte| *byte == b'{').count();
    let closes = line.bytes().filter(|byte| *byte == b'}').count();
    isize::try_from(opens).unwrap_or(isize::MAX)
        - isize::try_from(closes).unwrap_or(isize::MAX)
}

fn collect_rust_files(directory: &Path, files: &mut Vec<PathBuf>) {
    for entry in fs::read_dir(directory)
        .unwrap_or_else(|error| panic!("{}: {error}", directory.display()))
    {
        let entry = entry.expect("source directory entry must be readable");
        let path = entry.path();
        let file_type = entry
            .file_type()
            .expect("source directory entry type must be readable");
        if file_type.is_dir() {
            collect_rust_files(&path, files);
        } else if file_type.is_file()
            && path.extension().and_then(|value| value.to_str()) == Some("rs")
        {
            files.push(path);
        }
    }
}

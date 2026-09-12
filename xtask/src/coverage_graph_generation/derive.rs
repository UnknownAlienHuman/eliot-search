//! Source-to-reviewed-registry drift detection for coverage graph v2.
//!
//! This module derives identities and source locations only. It never chooses a
//! package-local module. Module ownership remains an explicit reviewed input.

use std::collections::{BTreeMap, BTreeSet};
use std::path::Path;
use std::process::Command;

use toml::Value;

const RESERVED: [&str; 6] = ["if", "for", "while", "match", "loop", "return"];
const GENERATED_MARKDOWN: [&str; 2] = [
    "docs/handoff/COVERAGE_GRAPH_V2.md",
    "docs/handoff/PACKAGE_MAP_INDEX_V2.md",
];
const SUPPLEMENT_NAMES: [&str; 6] = [
    "W7_HARDENING.md",
    "P18_SCALE.md",
    "W8_INTEGRATION.md",
    "W10_INTEGRATION.md",
    "W8_CLIENT.md",
    "W10_OPTIONAL_EVALUATION.md",
];
const MAX_SOURCE_BYTES: u64 = 16 * 1024 * 1024;

pub(super) fn validate(
    root: &Path,
    manifest: &Value,
    package_document: &Value,
    operation_document: &Value,
    documentation_document: &Value,
    dependency_document: &Value,
    module_document: &Value,
) -> Vec<String> {
    let mut errors = Vec::new();
    let files = match git_files(root) {
        Ok(files) => files,
        Err(error) => return vec![error],
    };
    validate_operations(
        root,
        manifest,
        package_document,
        operation_document,
        &files,
        &mut errors,
    );
    validate_documentation(
        root,
        documentation_document,
        &files,
        &mut errors,
    );
    validate_dependencies(package_document, dependency_document, &mut errors);
    validate_modules(root, manifest, module_document, &mut errors);
    errors
}

fn validate_operations(
    root: &Path,
    manifest: &Value,
    package_document: &Value,
    operation_document: &Value,
    files: &[String],
    errors: &mut Vec<String>,
) {
    let function_path = manifest
        .get("function_registry")
        .and_then(Value::as_str)
        .unwrap_or("swarm/function-packets.toml");
    let function_document = match load_toml(root, function_path) {
        Ok(document) => document,
        Err(error) => {
            errors.push(error);
            return;
        }
    };
    let packages = match indexed(package_document, "package", "name") {
        Ok(rows) => rows,
        Err(error) => {
            errors.push(error);
            return;
        }
    };
    let foundations = indexed_or_empty(&function_document, "foundation", "package", errors);
    let functions = indexed_or_empty(&function_document, "package", "name", errors);

    let mut expected: BTreeMap<String, BTreeSet<String>> = BTreeMap::new();
    for (package, row) in &packages {
        let mut sources = Vec::new();
        if let Some(foundation) = foundations.get(package) {
            if let Some(source) = foundation.get("primary_contract").and_then(Value::as_str) {
                sources.push(source.to_owned());
            }
            if package == "search-contracts" {
                sources.extend(
                    files
                        .iter()
                        .filter(|path| {
                            path.starts_with("docs/contracts/p00/")
                                && path.ends_with(".md")
                                && Path::new(path.as_str())
                                    .file_name()
                                    .and_then(|name| name.to_str())
                                    != Some("README.md")
                        })
                        .cloned(),
                );
            }
        } else if let Some(function) = functions.get(package) {
            if let Some(source) = function.get("functions").and_then(Value::as_str) {
                sources.push(source.to_owned());
            }
        }
        let Some(package_path) = row.get("path").and_then(Value::as_str) else {
            errors.push(format!("{package}: package path missing"));
            continue;
        };
        let prefix = format!("{package_path}/");
        sources.extend(
            files
                .iter()
                .filter(|path| {
                    path.starts_with(&prefix)
                        && path.ends_with(".md")
                        && Path::new(path.as_str())
                            .file_name()
                            .and_then(|name| name.to_str())
                            .is_some_and(is_supplement_name)
                })
                .cloned(),
        );
        sources.sort();
        sources.dedup();

        for source in sources {
            let text = match read_bounded_utf8(root, &source) {
                Ok(text) => text,
                Err(error) => {
                    errors.push(error);
                    continue;
                }
            };
            for operation in operation_names(&text) {
                expected
                    .entry(format!("{package}::{operation}"))
                    .or_default()
                    .insert(source.clone());
            }
        }
    }

    let actual = indexed_or_empty(operation_document, "operation", "id", errors);
    compare_identity_sets(
        "source-derived operation",
        expected.keys().cloned().collect(),
        actual.keys().cloned().collect(),
        errors,
    );
    for (identity, expected_sources) in expected {
        let Some(row) = actual.get(&identity) else {
            continue;
        };
        let actual_sources: BTreeSet<String> =
            string_array(row, "sources").into_iter().collect();
        if actual_sources != expected_sources {
            errors.push(format!(
                "{identity}: reviewed operation source set differs from source derivation"
            ));
        }
    }
}

fn validate_documentation(
    root: &Path,
    documentation_document: &Value,
    files: &[String],
    errors: &mut Vec<String>,
) {
    let selected: Vec<&String> = files
        .iter()
        .filter(|path| {
            path.ends_with(".md")
                && !path.starts_with("artifacts/")
                && !path.starts_with("docs/generated/")
                && !GENERATED_MARKDOWN.contains(&path.as_str())
        })
        .collect();
    let mut expected: BTreeMap<String, (String, i64, i64, String)> = BTreeMap::new();
    for path in selected {
        let text = match read_bounded_utf8(root, path) {
            Ok(text) => text,
            Err(error) => {
                errors.push(error);
                continue;
            }
        };
        let mut headings = crate::coverage_graph::heading_rows(&text);
        if headings.is_empty() {
            headings.push(crate::coverage_graph::Heading {
                line: 1,
                level: 0,
                raw: "document root".to_owned(),
                title: "document root".to_owned(),
            });
        }
        let mut duplicate_counts: BTreeMap<String, usize> = BTreeMap::new();
        for heading in headings {
            let base = crate::coverage_graph::slug(&heading.title);
            let occurrence = duplicate_counts.entry(base.clone()).or_insert(0);
            *occurrence = occurrence.saturating_add(1);
            let suffix = if *occurrence > 1 {
                format!("@{occurrence}")
            } else {
                String::new()
            };
            let id = format!("{path}#{base}{suffix}");
            expected.insert(
                id,
                (
                    path.to_string(),
                    i64::try_from(heading.line).unwrap_or(i64::MAX),
                    i64::from(heading.level),
                    heading.title,
                ),
            );
        }
    }

    let actual = indexed_or_empty(documentation_document, "node", "id", errors);
    compare_identity_sets(
        "documentation heading",
        expected.keys().cloned().collect(),
        actual.keys().cloned().collect(),
        errors,
    );
    for (identity, (path, line, level, heading)) in expected {
        let Some(row) = actual.get(&identity) else {
            continue;
        };
        if row.get("path").and_then(Value::as_str) != Some(path.as_str())
            || row.get("line").and_then(Value::as_integer) != Some(line)
            || row.get("level").and_then(Value::as_integer) != Some(level)
            || row.get("heading").and_then(Value::as_str) != Some(heading.as_str())
        {
            errors.push(format!(
                "{identity}: reviewed documentation location differs from tracked Markdown"
            ));
        }
    }
}

fn validate_dependencies(
    package_document: &Value,
    dependency_document: &Value,
    errors: &mut Vec<String>,
) {
    let packages = indexed_or_empty(package_document, "package", "name", errors);
    let mut expected = BTreeSet::new();
    for (consumer, row) in packages {
        for producer in string_array(&row, "deps") {
            expected.insert(format!("{consumer}->{producer}"));
        }
    }
    let actual = indexed_or_empty(dependency_document, "edge", "id", errors);
    compare_identity_sets(
        "package dependency",
        expected,
        actual.keys().cloned().collect(),
        errors,
    );
}

fn validate_modules(
    root: &Path,
    manifest: &Value,
    module_document: &Value,
    errors: &mut Vec<String>,
) {
    let registry_path = manifest
        .get("module_registry")
        .and_then(Value::as_str)
        .unwrap_or("swarm/module-packets.toml");
    let registry = match load_toml(root, registry_path) {
        Ok(document) => document,
        Err(error) => {
            errors.push(error);
            return;
        }
    };
    let mut expected = BTreeSet::new();
    let Some(packets) = registry.get("packet").and_then(Value::as_array) else {
        errors.push(format!("{registry_path}: packet must be an array"));
        return;
    };
    for packet in packets {
        let Some(path) = packet.get("path").and_then(Value::as_str) else {
            errors.push(format!("{registry_path}: packet path missing"));
            continue;
        };
        let document = match load_toml(root, path) {
            Ok(document) => document,
            Err(error) => {
                errors.push(error);
                continue;
            }
        };
        let rows = indexed_or_empty(&document, "package", "name", errors);
        for (package, row) in rows {
            for module in string_array(&row, "modules") {
                expected.insert(format!("{package}:{module}"));
            }
        }
    }
    let actual = indexed_or_empty(module_document, "module", "id", errors);
    compare_identity_sets(
        "logical module",
        expected,
        actual.keys().cloned().collect(),
        errors,
    );
}

fn compare_identity_sets(
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

fn git_files(root: &Path) -> Result<Vec<String>, String> {
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

fn read_bounded_utf8(root: &Path, relative: &str) -> Result<String, String> {
    let path = root.join(relative);
    let metadata = std::fs::metadata(&path)
        .map_err(|error| format!("{relative}: {error}"))?;
    if !metadata.is_file() || metadata.len() > MAX_SOURCE_BYTES {
        return Err(format!("{relative}: source is not a bounded regular file"));
    }
    std::fs::read_to_string(path)
        .map_err(|error| format!("{relative}: strict UTF-8 read failed: {error}"))
}

fn load_toml(root: &Path, relative: &str) -> Result<Value, String> {
    let text = read_bounded_utf8(root, relative)?;
    toml::from_str(&text).map_err(|error| format!("{relative}: {error}"))
}

fn indexed(
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

fn indexed_or_empty(
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

fn string_array(value: &Value, key: &str) -> Vec<String> {
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

fn is_supplement_name(name: &str) -> bool {
    if SUPPLEMENT_NAMES.contains(&name) {
        return true;
    }
    let Some(stem) = name.strip_suffix(".md") else {
        return false;
    };
    let bytes = stem.as_bytes();
    if !matches!(bytes.first().copied(), Some(b'W') | Some(b'P')) {
        return false;
    }
    let Some(underscore) = bytes.iter().position(|byte| *byte == b'_') else {
        return false;
    };
    underscore > 1 && bytes[1..underscore].iter().all(u8::is_ascii_digit)
}

fn operation_names(text: &str) -> BTreeSet<String> {
    let mut names = BTreeSet::new();
    for line in text.lines() {
        if let Some(name) = heading_operation(line) {
            names.insert(name.to_owned());
        }
    }
    for segment in inline_code_segments(text) {
        if let Some((name, rest)) = identifier_prefix(segment)
            && rest.starts_with('(')
        {
            names.insert(name.to_owned());
        }
    }
    for block in fenced_blocks(text) {
        for line in block.lines() {
            if let Some(name) = callable_line(line) {
                names.insert(name.to_owned());
            }
        }
    }
    for reserved in RESERVED {
        names.remove(reserved);
    }
    names
}

fn heading_operation(line: &str) -> Option<&str> {
    let bytes = line.as_bytes();
    let hashes = bytes.iter().take_while(|byte| **byte == b'#').count();
    if !(2..=3).contains(&hashes) {
        return None;
    }
    let mut rest = &line[hashes..];
    if rest.is_empty() || !rest.as_bytes()[0].is_ascii_whitespace() {
        return None;
    }
    rest = rest.trim_start_matches(char::is_whitespace);
    rest = rest.strip_prefix('`')?;
    identifier_prefix(rest).map(|(name, _)| name)
}

fn inline_code_segments(text: &str) -> Vec<&str> {
    let mut segments = Vec::new();
    let mut rest = text;
    loop {
        let Some(start) = rest.find('`') else {
            break;
        };
        let after = &rest[start + 1..];
        let Some(end) = after.find('`') else {
            break;
        };
        segments.push(&after[..end]);
        rest = &after[end + 1..];
    }
    segments
}

fn fenced_blocks(text: &str) -> Vec<&str> {
    let mut blocks = Vec::new();
    let mut rest = text;
    loop {
        let Some(start) = rest.find("```") else {
            break;
        };
        let after_marker = &rest[start + 3..];
        let Some(line_end) = after_marker.find('\n') else {
            break;
        };
        let content = &after_marker[line_end + 1..];
        let Some(end) = content.find("```") else {
            break;
        };
        blocks.push(&content[..end]);
        rest = &content[end + 3..];
    }
    blocks
}

fn callable_line(line: &str) -> Option<&str> {
    let mut rest = line.trim_start_matches(char::is_whitespace);
    if let Some(after) = strip_keyword(rest, "pub") {
        rest = after;
    }
    if let Some(after) = strip_keyword(rest, "async") {
        rest = after;
    }
    if let Some(after) = strip_keyword(rest, "fn") {
        rest = after;
    }
    let (name, tail) = identifier_prefix(rest)?;
    tail.trim_start_matches(char::is_whitespace)
        .starts_with('(')
        .then_some(name)
}

fn strip_keyword<'a>(text: &'a str, keyword: &str) -> Option<&'a str> {
    let tail = text.strip_prefix(keyword)?;
    if tail.is_empty() || !tail.as_bytes()[0].is_ascii_whitespace() {
        return None;
    }
    Some(tail.trim_start_matches(char::is_whitespace))
}

fn identifier_prefix(text: &str) -> Option<(&str, &str)> {
    let bytes = text.as_bytes();
    if !bytes.first().is_some_and(u8::is_ascii_lowercase) {
        return None;
    }
    let end = bytes
        .iter()
        .position(|byte| {
            !(byte.is_ascii_lowercase()
                || byte.is_ascii_digit()
                || *byte == b'_')
        })
        .unwrap_or(bytes.len());
    Some((&text[..end], &text[end..]))
}

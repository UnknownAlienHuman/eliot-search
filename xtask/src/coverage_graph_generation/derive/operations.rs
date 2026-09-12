//! Derivation of operation identities and exact source-file sets.

use std::collections::{BTreeMap, BTreeSet};
use std::path::Path;

use toml::Value;

use super::source::{
    compare_identity_sets, indexed, indexed_or_empty, load_toml,
    read_bounded_utf8, string_array,
};

const RESERVED: [&str; 6] = ["if", "for", "while", "match", "loop", "return"];
const SUPPLEMENT_NAMES: [&str; 6] = [
    "W7_HARDENING.md",
    "P18_SCALE.md",
    "W8_INTEGRATION.md",
    "W10_INTEGRATION.md",
    "W8_CLIENT.md",
    "W10_OPTIONAL_EVALUATION.md",
];

pub(super) fn validate(
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
                        .filter(|path| is_contract_source(path))
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
        sources.extend(
            files
                .iter()
                .filter(|path| is_package_supplement(path, package_path))
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

fn is_contract_source(path: &str) -> bool {
    let path = Path::new(path);
    path.extension().and_then(|value| value.to_str()) == Some("md")
        && path.parent().and_then(Path::to_str) == Some("docs/contracts/p00")
        && path.file_name().and_then(|value| value.to_str()) != Some("README.md")
}

fn is_package_supplement(path: &str, package_path: &str) -> bool {
    let path = Path::new(path);
    path.parent().and_then(Path::to_str) == Some(package_path)
        && path
            .file_name()
            .and_then(|value| value.to_str())
            .is_some_and(is_supplement_name)
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
    let line = line.trim_end();
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
    let (name, tail) = identifier_prefix(rest)?;
    (tail == "`").then_some(name)
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
    if let Some(after) = strip_visibility(rest) {
        rest = after;
    }
    if let Some(after) = strip_keyword(rest, "async") {
        rest = after;
    }
    rest = strip_keyword(rest, "fn")?;
    let (name, tail) = identifier_prefix(rest)?;
    tail.trim_start_matches(char::is_whitespace)
        .starts_with('(')
        .then_some(name)
}

fn strip_visibility(text: &str) -> Option<&str> {
    if let Some(after) = strip_keyword(text, "pub") {
        return Some(after);
    }
    let scoped = text.strip_prefix("pub(")?;
    let closing = scoped.find(')')?;
    let tail = &scoped[closing + 1..];
    if tail.is_empty() || !tail.as_bytes()[0].is_ascii_whitespace() {
        return None;
    }
    Some(tail.trim_start_matches(char::is_whitespace))
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn exact_operation_grammar_matches_legacy_extractor() {
        let names = operation_names(
            concat!(
                "## `heading_call`\n",
                "## `not_a_heading` extra\n",
                "Use `inline_call(value)` here.\n",
                "```rust\n",
                "pub(crate) async fn scoped_call(value: u64) {}\n",
                "pub(super) fn other_call() {}\n",
                "if(value)\n",
                "```\n",
            ),
        );
        assert!(names.contains("heading_call"));
        assert!(names.contains("inline_call"));
        assert!(names.contains("scoped_call"));
        assert!(names.contains("other_call"));
        assert!(!names.contains("not_a_heading"));
        assert!(!names.contains("if"));
    }

    #[test]
    fn supplements_must_be_direct_package_children() {
        assert!(is_package_supplement(
            "crates/example/W7_HARDENING.md",
            "crates/example"
        ));
        assert!(!is_package_supplement(
            "crates/example/nested/W7_HARDENING.md",
            "crates/example"
        ));
        assert!(is_contract_source(
            "docs/contracts/p00/CANONICAL_TYPES.md"
        ));
        assert!(!is_contract_source(
            "docs/contracts/p00/nested/CANONICAL_TYPES.md"
        ));
    }
}

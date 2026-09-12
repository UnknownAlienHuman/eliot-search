//! Derivation of tracked Markdown heading identities and source locations.

use std::collections::BTreeMap;
use std::path::Path;

use toml::Value;

use super::source::{
    compare_identity_sets, indexed_or_empty, read_bounded_utf8,
};

const GENERATED_MARKDOWN: [&str; 2] = [
    "docs/handoff/COVERAGE_GRAPH_V2.md",
    "docs/handoff/PACKAGE_MAP_INDEX_V2.md",
];

pub(super) fn validate(
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

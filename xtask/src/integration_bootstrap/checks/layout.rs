use std::path::Path;

use toml::Value;

use super::super::{EXPECTED_LAYOUT_DIRECTORIES, Finding, load_toml};

pub(super) fn validate_data_layout(
    root: &Path,
    findings: &mut Vec<Finding>,
) {
    let path = root.join("config/data-layout-v1.toml");
    let document = match load_toml(&path) {
        Ok(document) => document,
        Err(detail) => {
            findings.push(Finding::new("DATA_LAYOUT_INVALID", &path, detail));
            return;
        }
    };

    for field in [
        "root_must_be_dedicated",
        "root_must_not_be_repository_checkout",
        "root_must_not_be_source_identity",
        "owner_only_acl_required",
        "inherited_broad_acl_forbidden",
        "symlink_or_reparse_escape_forbidden",
        "plaintext_secret_storage_forbidden",
    ] {
        if document.get(field).and_then(Value::as_bool) != Some(true) {
            findings.push(Finding::new(
                "DATA_LAYOUT_FAIL_CLOSED_FIELD",
                &path,
                format!("{field} must be true"),
            ));
        }
    }

    let exact_directories = document
        .get("directories")
        .and_then(Value::as_table)
        .is_some_and(|directories| {
            directories.len() == EXPECTED_LAYOUT_DIRECTORIES.len()
                && EXPECTED_LAYOUT_DIRECTORIES.iter().all(|(name, value)| {
                    directories.get(*name).and_then(Value::as_str)
                        == Some(*value)
                })
        });
    if !exact_directories {
        findings.push(Finding::new(
            "DATA_LAYOUT_DIRECTORY_SET_NOT_EXACT",
            &path,
            "directory registry differs from the frozen v1 layout",
        ));
    }

    if nested_string(&document, "control", "redb_role")
        != Some("CONTROL_JOURNAL_ONLY")
        || nested_bool(&document, "control", "searchable_corpus_forbidden")
            != Some(true)
    {
        findings.push(Finding::new(
            "REDB_ROLE_INVALID",
            &path,
            "redb must remain control-journal-only",
        ));
    }
    if nested_bool(&document, "qdrant", "sole_search_index") != Some(true) {
        findings.push(Finding::new(
            "QDRANT_ROLE_INVALID",
            &path,
            "Qdrant must remain the sole search/index database",
        ));
    }
    if nested_bool(
        &document,
        "runtime",
        "unsaved_bytes_must_remain_memory_only",
    ) != Some(true)
    {
        findings.push(Finding::new(
            "UNSAVED_BYTES_LAYOUT_INVALID",
            &path,
            "unsaved bytes must remain memory-only",
        ));
    }
    if nested_bool(
        &document,
        "migration",
        "unknown_outcome_requires_quarantine",
    ) != Some(true)
    {
        findings.push(Finding::new(
            "MIGRATION_UNKNOWN_OUTCOME_UNSAFE",
            &path,
            "unknown migration outcomes must quarantine",
        ));
    }
}

fn nested_string<'a>(
    document: &'a Value,
    table: &str,
    key: &str,
) -> Option<&'a str> {
    document
        .get(table)
        .and_then(Value::as_table)
        .and_then(|section| section.get(key))
        .and_then(Value::as_str)
}

fn nested_bool(document: &Value, table: &str, key: &str) -> Option<bool> {
    document
        .get(table)
        .and_then(Value::as_table)
        .and_then(|section| section.get(key))
        .and_then(Value::as_bool)
}

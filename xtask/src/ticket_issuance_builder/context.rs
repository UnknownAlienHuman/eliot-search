//! Committed context-source and registry-selector validation.

use serde_json::{Value as JsonValue, json};

use crate::git_tree::GitTree;
use crate::ticket_planner::{
    CONTEXT_TOTAL_BYTE_CEILING, SelectorDocs, SelectorStatus,
    context_source_forbidden, exact_sha256_hex, resolve_selector,
};

use super::model::{Checks, DraftPair};
use super::util::toml_to_json;

pub(super) fn validate_context(
    tree: &GitTree,
    pair: &DraftPair,
    package: &str,
    checks: &mut Checks,
) -> Vec<JsonValue> {
    let mut result = Vec::new();
    let mut total_bytes = 0_u64;
    for (index, path) in pair.sources.iter().enumerate() {
        let check_id = format!("source-{index:02}");
        if context_source_forbidden(path) {
            checks.fail(
                check_id,
                "CONTEXT_SOURCE_FORBIDDEN",
                format!("forbidden context source: {path}"),
            );
            continue;
        }
        let (raw, entry) = match tree.read_bytes(path) {
            Ok(value) => value,
            Err(error) => {
                let reason = match error.reason() {
                    "CONTEXT_SOURCE_MISSING"
                    | "CONTEXT_SOURCE_NOT_REGULAR"
                    | "CONTEXT_BUDGET_EXCEEDED" => error.reason(),
                    _ => "CONTEXT_SOURCE_MISSING",
                };
                checks.fail(check_id, reason, error.message());
                continue;
            }
        };
        if std::str::from_utf8(&raw).is_err() {
            checks.fail(
                check_id,
                "CONTEXT_SOURCE_NOT_UTF8",
                format!("context source is not UTF-8: {path}"),
            );
            continue;
        }
        total_bytes = total_bytes
            .checked_add(u64::try_from(raw.len()).unwrap_or(u64::MAX))
            .unwrap_or(u64::MAX);
        checks.pass(
            check_id,
            format!("exact regular UTF-8 Git blob: {path}"),
        );
        result.push(json!({
            "order": index,
            "path": path,
            "git_blob_id": tree.blob_identity(&entry),
            "exact_sha256": exact_sha256_hex(&raw),
            "exact_bytes": raw.len(),
        }));
    }
    if total_bytes <= CONTEXT_TOTAL_BYTE_CEILING {
        checks.pass(
            "context-total-bytes",
            format!("declared context source bytes are bounded: {total_bytes}"),
        );
    } else {
        checks.fail(
            "context-total-bytes",
            "CONTEXT_BUDGET_EXCEEDED",
            "declared context exceeds 16 MiB planner ceiling",
        );
    }

    let crates = selector_document(tree, "swarm/crates.toml");
    let functions = selector_document(tree, "swarm/function-packets.toml");
    let stages = selector_document(tree, "swarm/stages.toml");
    let launch = selector_document(tree, "swarm/launch-state.toml");
    let docs = SelectorDocs {
        crates: crates.as_ref(),
        functions: functions.as_ref(),
        stages: stages.as_ref(),
        launch: launch.as_ref(),
    };
    for (index, selector) in pair.selectors.iter().enumerate() {
        let (status, detail) = resolve_selector(&docs, selector, package);
        let check_id = format!("selector-{index:02}");
        match status {
            SelectorStatus::Ok => checks.pass(
                check_id,
                format!("selector resolved exactly once: {selector}"),
            ),
            SelectorStatus::Unsupported => checks.fail(
                check_id,
                "CONTEXT_SELECTOR_INVALID",
                format!("{detail}: {selector}"),
            ),
            SelectorStatus::NotUnique => checks.fail(
                check_id,
                "CONTEXT_SELECTOR_NOT_UNIQUE",
                format!("{detail}: {selector}"),
            ),
        }
    }
    result
}

fn selector_document(tree: &GitTree, path: &str) -> Option<JsonValue> {
    tree.load_toml(path)
        .ok()
        .map(|(document, _)| toml_to_json(&document))
}

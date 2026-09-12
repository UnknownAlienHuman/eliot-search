//! Repository/file closure for context-materialization validation.

use std::path::Path;

use toml::Value;

use super::spec::{RETIRED_PYTHON, WORKFLOW};

pub(super) fn read_toml(
    root: &Path,
    relative: &str,
    errors: &mut Vec<String>,
) -> Value {
    let text = match std::fs::read_to_string(root.join(relative)) {
        Ok(text) => text,
        Err(error) => {
            errors.push(format!("{relative}: {error}"));
            return empty_table();
        }
    };
    match toml::from_str(&text) {
        Ok(value) => value,
        Err(error) => {
            errors.push(format!("{relative}: {error}"));
            empty_table()
        }
    }
}

pub(super) fn validate_workflow(root: &Path, errors: &mut Vec<String>) {
    let text = match std::fs::read_to_string(root.join(WORKFLOW)) {
        Ok(text) => text,
        Err(_) => {
            errors.push("missing manual workflow".to_owned());
            return;
        }
    };
    for token in [
        "workflow_dispatch:",
        "contents: read",
        "persist-credentials: false",
        "cargo test --locked -p xtask --test context_materialization_builder",
    ] {
        if !text.contains(token) {
            errors.push(format!("workflow missing {token}"));
        }
    }
    if [
        "\n  push:",
        "\n  pull_request:",
        "\n  schedule:",
        "\n  workflow_run:",
    ]
    .iter()
    .any(|trigger| text.contains(trigger))
    {
        errors.push("workflow has automatic trigger".to_owned());
    }
    if text.to_ascii_lowercase().contains("python") {
        errors.push("workflow restored Python runtime".to_owned());
    }
}

pub(super) fn validate_implementation_sentinels(
    root: &Path,
    errors: &mut Vec<String>,
) {
    let assemble = std::fs::read_to_string(
        root.join("xtask/src/context_materialization_builder/assemble.rs"),
    )
    .unwrap_or_default();
    let manifest = std::fs::read_to_string(
        root.join("xtask/src/context_materialization_builder/manifest.rs"),
    )
    .unwrap_or_default();
    let write = std::fs::read_to_string(
        root.join("xtask/src/context_materialization_builder/write.rs"),
    )
    .unwrap_or_default();
    for (source, token, failure) in [
        (
            assemble.as_str(),
            "\"control_record_mutations\": []",
            "plan implementation lacks empty control mutation field",
        ),
        (
            assemble.as_str(),
            "\"authority\": authority_map()",
            "plan implementation lacks all-false authority projection",
        ),
        (
            assemble.as_str(),
            "\"signature_refs_are_inputs\": false",
            "operation ID signature exclusion sentinel missing",
        ),
        (
            manifest.as_str(),
            "accepted_evidence_digest_toml",
            "accepted handoff evidence projection missing",
        ),
        (
            write.as_str(),
            "write_exact_idempotent",
            "idempotent ordinary output writer missing",
        ),
    ] {
        if !source.contains(token) {
            errors.push(failure.to_owned());
        }
    }
}

pub(super) fn validate_retired_python(
    root: &Path,
    errors: &mut Vec<String>,
) {
    for relative in RETIRED_PYTHON {
        if root.join(relative).exists() {
            errors.push(format!("retired Python planner returned: {relative}"));
        }
    }
}

fn empty_table() -> Value {
    Value::Table(toml::map::Map::new())
}

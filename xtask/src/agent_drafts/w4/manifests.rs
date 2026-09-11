use std::fs;
use std::path::{Path, PathBuf};

use toml::Value;

use super::super::{integer, read_text, string};

pub(super) fn validate_manifests(
    packet_doc: &Value,
    ticket_manifest: &Value,
    context_manifest: &Value,
    cases: &Value,
    root: &Path,
    errors: &mut Vec<String>,
) {
    if integer(ticket_manifest, "draft_count") != Some(9)
        || integer(ticket_manifest, "issued_ticket_count") != Some(0)
        || integer(ticket_manifest, "active_lease_count") != Some(0)
    {
        errors.push("W4 ticket manifest counts invalid".to_owned());
    }
    if integer(context_manifest, "draft_count") != Some(9)
        || integer(context_manifest, "materialized_context_count")
            != Some(0)
    {
        errors.push("W4 context manifest counts invalid".to_owned());
    }

    let current_state = packet_doc.get("current_state");
    if current_state
        .and_then(|state| integer(state, "accepted_W4_package_handoffs"))
        != Some(0)
        || current_state
            .and_then(|state| string(state, "W4_G2_receipt"))
            != Some("ABSENT")
    {
        errors.push("W4 packet current state is non-zero".to_owned());
    }

    for protected in [
        "swarm/tickets",
        "swarm/leases",
        "swarm/submissions",
        "swarm/reviews",
        "swarm/handoffs",
        "swarm/supersessions",
    ] {
        if !machine_files(root, protected).is_empty() {
            errors.push(format!(
                "issued control records exist under {protected}"
            ));
        }
    }

    validate_cases(cases, errors);
    validate_workflow(root, errors);
}

fn validate_cases(cases: &Value, errors: &mut Vec<String>) {
    let rows = cases.get("case").and_then(Value::as_array);
    if integer(cases, "case_count") != Some(24)
        || rows.is_none_or(|rows| rows.len() != 24)
        || rows.is_some_and(|rows| {
            rows.iter().any(|row| {
                row.as_table().is_none_or(|table| {
                    table.get("mandatory").and_then(Value::as_bool)
                        != Some(true)
                        || table.get("result").and_then(Value::as_str)
                            != Some("UNAVAILABLE")
                })
            })
        })
    {
        errors.push("qualification case inventory mismatch".to_owned());
    }
}

fn validate_workflow(root: &Path, errors: &mut Vec<String>) {
    let Ok(workflow) =
        read_text(root, ".github/workflows/w4-agent-drafts.yml")
    else {
        errors.push("missing manual workflow".to_owned());
        return;
    };
    for token in [
        "workflow_dispatch:",
        "contents: read",
        "persist-credentials: false",
    ] {
        if !workflow.contains(token) {
            errors.push(format!("workflow missing {token}"));
        }
    }
    for forbidden in [
        "\n  push:",
        "\n  pull_request:",
        "\n  schedule:",
        "\n  workflow_run:",
        "\n  repository_dispatch:",
    ] {
        if workflow.contains(forbidden) {
            errors.push(format!(
                "automatic workflow trigger: {}",
                forbidden.trim()
            ));
        }
    }
}

fn machine_files(root: &Path, relative: &str) -> Vec<String> {
    let directory = root.join(relative);
    if !directory.exists() {
        return Vec::new();
    }
    let mut files = Vec::new();
    collect_files(&directory, &directory, &mut files);
    files.sort();
    files
}

fn collect_files(
    root: &Path,
    directory: &Path,
    files: &mut Vec<String>,
) {
    let Ok(entries) = fs::read_dir(directory) else {
        return;
    };
    for entry in entries.flatten() {
        let path = entry.path();
        let Ok(file_type) = entry.file_type() else {
            continue;
        };
        if file_type.is_dir() {
            collect_files(root, &path, files);
        } else if file_type.is_file() && !ignored(&path) {
            let relative = path.strip_prefix(root).unwrap_or(&path);
            files.push(
                relative.to_string_lossy().replace('\\', "/"),
            );
        }
    }
}

fn ignored(path: &PathBuf) -> bool {
    path.file_name()
        .and_then(|name| name.to_str())
        .is_some_and(|name| {
            matches!(name, "README.md" | ".gitkeep" | ".gitignore")
        })
}

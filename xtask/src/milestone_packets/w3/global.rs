use std::path::Path;

use toml::Value;

use super::super::{
    boolean, expected_strings, integer, read_text, string, string_list,
};

#[allow(clippy::too_many_arguments)]
pub(super) fn validate_global(
    root: &Path,
    document: &Value,
    launch: &Value,
    artifact: &Value,
    collection: &Value,
    probes: &Value,
    cases: &Value,
    errors: &mut Vec<String>,
) {
    validate_registry(document, launch, errors);
    validate_qdrant_state(artifact, collection, probes, errors);
    validate_transition(document, errors);
    validate_current_state(document, errors);
    validate_cases(cases, errors);
    validate_workflow(root, errors);
}

fn validate_registry(
    document: &Value,
    launch: &Value,
    errors: &mut Vec<String>,
) {
    if integer(document, "package_count") != Some(9)
        || integer(document, "milestone_count") != Some(36)
    {
        errors.push("package/milestone count mismatch".to_owned());
    }
    if string(document, "status")
        != Some("BLOCKED_ON_G1_W2_G1_AND_QDRANT_QUALIFICATION")
    {
        errors.push("registry is not qualification-blocked".to_owned());
    }
    if string_list(document, "requires_accepted_gates")
        != Some(expected_strings(&["G1"]))
        || string_list(document, "requires_accepted_receipts")
            != Some(expected_strings(&["W2_G1"]))
    {
        errors.push("stage prerequisite mismatch".to_owned());
    }
    if boolean(document, "one_writer_one_package") != Some(true)
        || boolean(document, "sequential_milestones_per_package")
            != Some(true)
    {
        errors.push("ownership/order invariant disabled".to_owned());
    }
    if boolean(document, "parallel_milestones_within_package")
        != Some(false)
        || boolean(
            document,
            "implementation_authorized_by_this_registry",
        ) != Some(false)
        || boolean(document, "indexed_mode_enabled") != Some(false)
    {
        errors.push("authority/indexed-mode ceiling failed".to_owned());
    }
    if string(launch, "active_stage") != Some("P00")
        || integer(launch, "active_wave") != Some(0)
        || string_list(launch, "authorized_packages")
            != Some(expected_strings(&["search-contracts"]))
    {
        errors.push("launch authority moved".to_owned());
    }
}

fn validate_qdrant_state(
    artifact: &Value,
    collection: &Value,
    probes: &Value,
    errors: &mut Vec<String>,
) {
    if string(artifact, "status") != Some("UNQUALIFIED")
        || nested_non_empty_string(artifact, "server", "version")
        || nested_non_empty_string(artifact, "client", "version")
    {
        errors.push("Qdrant artifact/client selected or qualified".to_owned());
    }
    if nested_bool(artifact, "server", "automatic_download") != Some(false)
        || nested_bool(artifact, "server", "automatic_upgrade")
            != Some(false)
    {
        errors.push("automatic Qdrant acquisition enabled".to_owned());
    }

    let sparse_value = collection.get("sparse_vector");
    let sparse_vectors = sparse_value
        .and_then(Value::as_array)
        .map(Vec::as_slice)
        .unwrap_or_default();
    if string(collection, "status") != Some("DESIGNED_NOT_EXECUTED")
        || sparse_value.is_some_and(|value| !value.is_array())
        || sparse_vectors.iter().any(|row| {
            row.as_table().is_none_or(|table| {
                table.get("profile_status").and_then(Value::as_str)
                    != Some("UNQUALIFIED")
            })
        })
    {
        errors.push("collection/profile qualification state changed".to_owned());
    }

    let probe_value = probes.get("probe");
    let probe_rows = probe_value
        .and_then(Value::as_array)
        .map(Vec::as_slice)
        .unwrap_or_default();
    if string(probes, "status") != Some("NOT_EXECUTED")
        || probe_value.is_some_and(|value| !value.is_array())
        || probe_rows.iter().any(|row| {
            row.as_table().is_none_or(|table| {
                table.get("mandatory").and_then(Value::as_bool)
                    != Some(true)
                    || table.get("result").and_then(Value::as_str)
                        != Some("UNAVAILABLE")
            })
        })
    {
        errors.push("mandatory probe state changed".to_owned());
    }
}

fn validate_transition(document: &Value, errors: &mut Vec<String>) {
    let transition = document.get("transition");
    for key in [
        "require_raw_command_outcomes",
        "require_no_blocking_contract_challenge",
        "require_package_only_diff",
        "require_dependency_handoff_digests",
        "require_qualification_receipts_where_applicable",
        "require_line_budget",
    ] {
        if transition
            .and_then(Value::as_table)
            .and_then(|table| table.get(key))
            .and_then(Value::as_bool)
            != Some(true)
        {
            errors.push(format!("transition invariant disabled: {key}"));
        }
    }
    if [
        "advance_launch_state",
        "publish_package_handoff",
        "enable_indexed_mode",
    ]
    .iter()
    .any(|key| {
        transition
            .and_then(Value::as_table)
            .and_then(|table| table.get(*key))
            .and_then(Value::as_bool)
            != Some(false)
    })
    {
        errors.push("transition creates authority".to_owned());
    }
}

fn validate_current_state(document: &Value, errors: &mut Vec<String>) {
    let valid = match document.get("current_state") {
        None => true,
        Some(value) => value
            .as_table()
            .is_some_and(|state| state.values().all(allowed_zero_state_value)),
    };
    if !valid {
        errors.push("current state contains success/authority".to_owned());
    }
}

fn allowed_zero_state_value(value: &Value) -> bool {
    value.as_bool() == Some(false)
        || value.as_integer() == Some(0)
        || value.as_str() == Some("ABSENT")
}

fn validate_cases(cases: &Value, errors: &mut Vec<String>) {
    let case_rows = cases.get("case").and_then(Value::as_array);
    if integer(cases, "case_count") != Some(20)
        || case_rows.is_none_or(|rows| rows.len() != 20)
        || case_rows.is_some_and(|rows| {
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
        errors.push("case inventory mismatch".to_owned());
    }
}

fn validate_workflow(root: &Path, errors: &mut Vec<String>) {
    let Ok(workflow) =
        read_text(root, ".github/workflows/w3-milestone-packets.yml")
    else {
        errors.push("workflow missing".to_owned());
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
    for token in [
        "\n  push:",
        "\n  pull_request:",
        "\n  schedule:",
        "\n  workflow_run:",
        "\n  repository_dispatch:",
    ] {
        if workflow.contains(token) {
            errors.push(format!("automatic trigger {}", token.trim()));
        }
    }
}

fn nested_non_empty_string(value: &Value, table: &str, key: &str) -> bool {
    value
        .get(table)
        .and_then(Value::as_table)
        .and_then(|section| section.get(key))
        .and_then(Value::as_str)
        .is_some_and(|text| !text.is_empty())
}

fn nested_bool(value: &Value, table: &str, key: &str) -> Option<bool> {
    value
        .get(table)
        .and_then(Value::as_table)
        .and_then(|section| section.get(key))
        .and_then(Value::as_bool)
}

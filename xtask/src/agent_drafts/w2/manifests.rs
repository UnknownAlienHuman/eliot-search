use std::path::Path;

use toml::Value;

use super::super::{expected_strings, integer, read_text, string_list};

pub(super) fn validate_manifests(
    packet_doc: &Value,
    ticket_manifest: &Value,
    context_manifest: &Value,
    cases: &Value,
    root: &Path,
    errors: &mut Vec<String>,
) {
    validate_counts_and_prerequisites(ticket_manifest, context_manifest, errors);
    validate_cases(cases, errors);
    validate_workflow(root, errors);
    validate_zero_state(packet_doc, errors);
}

fn validate_counts_and_prerequisites(
    ticket_manifest: &Value,
    context_manifest: &Value,
    errors: &mut Vec<String>,
) {
    if integer(ticket_manifest, "draft_count") != Some(8)
        || integer(ticket_manifest, "issued_ticket_count") != Some(0)
    {
        errors.push("W2 ticket manifest counts invalid".to_owned());
    }
    if integer(context_manifest, "draft_count") != Some(8)
        || integer(context_manifest, "materialized_context_count") != Some(0)
    {
        errors.push("W2 context manifest counts invalid".to_owned());
    }
    if string_list(ticket_manifest, "requires_accepted_gates")
        != Some(expected_strings(&["G0"]))
        || string_list(ticket_manifest, "requires_accepted_receipts")
            != Some(expected_strings(&["W1"]))
    {
        errors.push("W2 ticket manifest prerequisites mismatch".to_owned());
    }
    if string_list(context_manifest, "requires_accepted_gates")
        != Some(expected_strings(&["G0"]))
        || string_list(context_manifest, "requires_accepted_receipts")
            != Some(expected_strings(&["W1"]))
    {
        errors.push("W2 context manifest prerequisites mismatch".to_owned());
    }
}

fn validate_cases(cases: &Value, errors: &mut Vec<String>) {
    let rows = cases.get("case").and_then(Value::as_array);
    if integer(cases, "case_count") != Some(20)
        || rows.is_none_or(|rows| rows.len() != 20)
    {
        errors.push("W2 qualification case inventory mismatch".to_owned());
        return;
    }
    if rows.is_some_and(|rows| {
        rows.iter().any(|case| {
            case.as_table().is_none_or(|table| {
                table.get("mandatory").and_then(Value::as_bool) != Some(true)
                    || table.get("result").and_then(Value::as_str) != Some("UNAVAILABLE")
            })
        })
    }) {
        errors.push("W2 qualification cases are not mandatory UNAVAILABLE".to_owned());
    }
}

fn validate_workflow(root: &Path, errors: &mut Vec<String>) {
    let Ok(workflow) = read_text(root, ".github/workflows/w2-agent-drafts.yml") else {
        errors.push("missing W2 manual workflow".to_owned());
        return;
    };
    for token in ["workflow_dispatch:", "contents: read", "persist-credentials: false"] {
        if !workflow.contains(token) {
            errors.push(format!("workflow missing {token}"));
        }
    }
    for forbidden in [
        "\n  push:",
        "\n  pull_request:",
        "\n  schedule:",
        "\n  workflow_run:",
    ] {
        if workflow.contains(forbidden) {
            errors.push(format!("automatic workflow trigger: {}", forbidden.trim()));
        }
    }
}

fn validate_zero_state(packet_doc: &Value, errors: &mut Vec<String>) {
    let valid = packet_doc
        .get("current_state")
        .and_then(Value::as_table)
        .is_some_and(|state| {
            state.len() == 7
                && state.get("accepted_G0").and_then(Value::as_bool) == Some(false)
                && state.get("accepted_W1").and_then(Value::as_bool) == Some(false)
                && state
                    .get("materialized_W2_contexts")
                    .and_then(Value::as_integer)
                    == Some(0)
                && state
                    .get("issued_W2_tickets")
                    .and_then(Value::as_integer)
                    == Some(0)
                && state
                    .get("active_W2_leases")
                    .and_then(Value::as_integer)
                    == Some(0)
                && state
                    .get("accepted_W2_package_handoffs")
                    .and_then(Value::as_integer)
                    == Some(0)
                && state.get("W2_G1_receipt").and_then(Value::as_str) == Some("ABSENT")
        });
    if !valid {
        errors.push("W2 current-state zero disposition mismatch".to_owned());
    }
}

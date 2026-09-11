use std::collections::BTreeSet;
use std::path::Path;

use super::super::model::{WORKFLOW, WORKFLOW_XTASK_TOKEN};
use super::super::parse::{
    child, get, is_bool, is_int, is_one, is_str, is_zero, require,
};

pub(super) fn check_slo(program: &toml::Value, metrics: &toml::Value, errors: &mut Vec<String>) {
    let target_slo = child(Some(program), "architecture_targets", errors);
    let metric_slo = child(Some(metrics), "candidate_slo", errors);
    let percentiles = child(Some(metrics), "percentiles", errors);
    for key in [
        "warm_exact_keyword_navigation_p95_ms",
        "warm_single_scope_lexical_p95_ms",
        "warm_cross_repository_comparison_p95_ms",
        "first_useful_progressive_card_ms",
    ] {
        require(
            errors,
            get(target_slo, key) == get(metric_slo, key),
            format!("architecture target mismatch: {key}"),
        );
    }
    require(
        errors,
        get(target_slo, "minimum_percentile_samples").and_then(toml::Value::as_integer) == Some(30)
            && get(percentiles, "minimum_measured_samples").and_then(toml::Value::as_integer)
                == Some(30),
        "percentile sample floor mismatch".to_owned(),
    );
    require(
        errors,
        is_str(target_slo, "status", "TARGET_NOT_MEASURED"),
        "performance targets are overclaimed".to_owned(),
    );
}

pub(super) fn check_blockers(program: &toml::Value, errors: &mut Vec<String>) {
    let blockers = child(Some(program), "release_hard_blockers", errors);
    for key in [
        "false_complete_negative_claim_count",
        "stale_leakage_count",
        "access_leakage_count",
        "secret_or_content_leakage_count",
        "protocol_resource_leak_count",
    ] {
        require(
            errors,
            is_zero(get(blockers, key)),
            format!("release hard blocker is not zero: {key}"),
        );
    }
    require(
        errors,
        is_one(get(blockers, "required_fault_cell_recovery_rate")),
        "fault recovery acceptance is not 100%".to_owned(),
    );
    for key in [
        "all_mandatory_package_handoffs_required",
        "all_active_leases_closed",
        "all_unreviewed_submissions_closed",
        "windows_install_upgrade_recovery_rollback_uninstall_required",
        "independent_g5_review_required",
    ] {
        require(
            errors,
            is_bool(blockers, key, true),
            format!("release prerequisite disabled: {key}"),
        );
    }
}

pub(super) fn check_sequence(
    program: &toml::Value,
    key: &str,
    expected: &[&str],
    count: usize,
    order_message: &str,
    ordinal_message: &str,
    errors: &mut Vec<String>,
) {
    let Some(items) = program.get(key).and_then(toml::Value::as_array) else {
        // CPython raises an uncaught `TypeError` here; record FAIL instead.
        errors.push(order_message.to_owned());
        errors.push(ordinal_message.to_owned());
        return;
    };
    let mut ordered: Vec<&toml::Value> = items.iter().collect();
    ordered.sort_by_key(|item| {
        item.get("order")
            .and_then(toml::Value::as_integer)
            .unwrap_or(-1)
    });
    let ids: Vec<&str> = ordered
        .iter()
        .filter_map(|item| item.get("id").and_then(toml::Value::as_str))
        .collect();
    require(errors, ids == expected, order_message.to_owned());
    let orders: Option<Vec<i64>> = items
        .iter()
        .map(|item| item.get("order").and_then(toml::Value::as_integer))
        .collect();
    let mut sorted = orders.clone().unwrap_or_default();
    sorted.sort_unstable();
    let want: Vec<i64> = (1..=i64::try_from(count).unwrap_or(0)).collect();
    require(
        errors,
        orders.is_some() && sorted == want,
        ordinal_message.to_owned(),
    );
}

pub(super) fn check_coverage_link(coverage: &toml::Value, errors: &mut Vec<String>) {
    require(
        errors,
        is_str(
            Some(coverage),
            "implementation_program",
            "swarm/implementation-program.toml",
        ),
        "coverage manifest does not link the implementation program".to_owned(),
    );
}

pub(super) fn check_cross_cutting(program: &toml::Value, errors: &mut Vec<String>) {
    let cross = child(Some(program), "cross_cutting", errors);
    for section in [
        "error_model",
        "resources",
        "security",
        "persistence",
        "observability",
        "packaging",
        "testing",
    ] {
        let row = child(cross, section, errors);
        require(
            errors,
            get(row, "required")
                .and_then(toml::Value::as_array)
                .is_some_and(|items| !items.is_empty()),
            format!("cross-cutting requirement set missing: {section}"),
        );
    }
    let testing = child(cross, "testing", errors);
    require(
        errors,
        is_bool(testing, "compile_or_unit_tests_alone_pass_gate", false),
        "compile/unit tests can incorrectly pass a gate".to_owned(),
    );
}

pub(super) fn check_cases(cases: &toml::Value, errors: &mut Vec<String>) {
    require(
        errors,
        is_str(Some(cases), "status", "STRUCTURAL_NOT_EXECUTED"),
        "qualification case status mismatch".to_owned(),
    );
    require(
        errors,
        is_int(Some(cases), "case_count", 24),
        "qualification case count mismatch".to_owned(),
    );
    let Some(case_rows) = cases.get("case").and_then(toml::Value::as_array) else {
        errors.push("qualification case inventory mismatch".to_owned());
        return;
    };
    require(
        errors,
        case_rows.len() == 24,
        "qualification case inventory mismatch".to_owned(),
    );
    let ids: Vec<Option<&str>> = case_rows
        .iter()
        .map(|row| row.get("id").and_then(toml::Value::as_str))
        .collect();
    require(
        errors,
        ids.iter().collect::<BTreeSet<_>>().len() == 24 && ids.len() == 24,
        "qualification case IDs are not unique".to_owned(),
    );
    require(
        errors,
        case_rows.iter().all(|row| {
            row.get("mandatory").and_then(toml::Value::as_bool) == Some(true)
                && row.get("result").and_then(toml::Value::as_str) == Some("UNAVAILABLE")
        }),
        "qualification cases contain premature evidence".to_owned(),
    );
}

pub(super) fn check_workflow(root: &Path, errors: &mut Vec<String>) {
    let workflow_path = root.join(WORKFLOW);
    require(
        errors,
        workflow_path.is_file(),
        "implementation program workflow missing".to_owned(),
    );
    let Ok(workflow) = std::fs::read_to_string(&workflow_path) else {
        errors.push(format!("{WORKFLOW} is not readable"));
        return;
    };
    for token in [
        "workflow_dispatch:",
        "contents: read",
        "persist-credentials: false",
        WORKFLOW_XTASK_TOKEN,
    ] {
        require(
            errors,
            workflow.contains(token),
            format!("implementation workflow missing token: {token}"),
        );
    }
    for (trigger, name) in [
        ("\n  push:", "push:"),
        ("\n  pull_request:", "pull_request:"),
        ("\n  pull_request_target:", "pull_request_target:"),
        ("\n  merge_group:", "merge_group:"),
        ("\n  schedule:", "schedule:"),
        ("\n  workflow_run:", "workflow_run:"),
        ("\n  repository_dispatch:", "repository_dispatch:"),
        ("\n  workflow_call:", "workflow_call:"),
    ] {
        require(
            errors,
            !workflow.contains(trigger),
            format!("automatic workflow trigger present: {name}"),
        );
    }
}

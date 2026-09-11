use std::path::Path;

use super::super::model::{CurrentView, EXPECTED_PATHS};
use super::super::parse::{child, get, is_bool, is_int, is_str, is_str_list, require};

pub(super) fn check_paths(program: &toml::Value, root: &Path, errors: &mut Vec<String>) {
    for (key, expected) in EXPECTED_PATHS {
        require(
            errors,
            get(Some(program), key).and_then(toml::Value::as_str) == Some(expected),
            format!("program path mismatch: {key}"),
        );
        require(
            errors,
            root.join(expected).is_file(),
            format!("program path missing: {expected}"),
        );
    }
}

pub(super) fn check_identity(program: &toml::Value, errors: &mut Vec<String>) {
    require(
        errors,
        is_int(Some(program), "schema_version", 1),
        "implementation program schema version mismatch".to_owned(),
    );
    require(
        errors,
        is_str(Some(program), "status", "PLANNED_NOT_AUTHORIZED"),
        "implementation program status is not non-authoritative".to_owned(),
    );
    require(
        errors,
        get(Some(program), "source_main_commit")
            .and_then(toml::Value::as_str)
            .is_some_and(|commit| {
                commit.len() == 40
                    && commit
                        .bytes()
                        .all(|byte| matches!(byte, b'0'..=b'9' | b'a'..=b'f'))
            }),
        "source main commit is not an exact SHA".to_owned(),
    );
    for key in [
        "implementation_authorized_by_this_program",
        "launch_state_changed",
        "package_acceptance_claimed",
        "gate_or_wave_acceptance_claimed",
        "runtime_evidence_available",
        "product_acceptance_claimed",
    ] {
        require(
            errors,
            is_bool(Some(program), key, false),
            format!("authority/non-claim flag changed: {key}"),
        );
    }
}

pub(super) fn check_discipline(
    program: &toml::Value,
    packages_doc: &toml::Value,
    errors: &mut Vec<String>,
) {
    let discipline = child(Some(program), "discipline", errors);
    for key in [
        "one_writer_one_package",
        "one_worktree_one_task",
        "package_write_scope_only",
        "accepted_public_handoffs_only",
    ] {
        require(
            errors,
            is_bool(discipline, key, true),
            format!("discipline invariant disabled: {key}"),
        );
    }
    for key in [
        "dependency_implementation_reads_allowed",
        "package_writer_may_edit_shared_registries",
        "package_writer_may_self_review",
        "package_writer_may_advance_launch_state",
    ] {
        require(
            errors,
            is_bool(discipline, key, false),
            format!("discipline prohibition disabled: {key}"),
        );
    }
    require(
        errors,
        is_str(
            discipline,
            "ordinary_architecture_master_access",
            "exception-only",
        ),
        "architecture access policy mismatch".to_owned(),
    );
    require(
        errors,
        is_int(discipline, "maximum_static_context_files", 16),
        "static context ceiling mismatch".to_owned(),
    );
    require(
        errors,
        is_int(discipline, "normal_handwritten_src_target", 7500),
        "normal source target mismatch".to_owned(),
    );
    require(
        errors,
        is_int(discipline, "mandatory_split_review_lines", 8500),
        "split-review line threshold mismatch".to_owned(),
    );
    require(
        errors,
        is_int(discipline, "hard_handwritten_line_limit", 10_000)
            && is_int(
                Some(packages_doc),
                "hard_handwritten_rust_line_limit",
                10_000,
            ),
        "hard line limit mismatch".to_owned(),
    );
}

pub(super) fn check_current_state(
    program: &toml::Value,
    launch: &toml::Value,
    coverage: &toml::Value,
    root: &Path,
    errors: &mut Vec<String>,
) -> CurrentView {
    let current = child(Some(program), "current_state", errors);
    let draft = child(Some(launch), "draft_control", errors);
    let coverage_state = child(Some(coverage), "current_state", errors);
    let launch_view = Some(launch);
    require(
        errors,
        get(current, "active_stage").and_then(toml::Value::as_str) == Some("P00")
            && get(launch_view, "active_stage").and_then(toml::Value::as_str) == Some("P00"),
        "current active stage mismatch".to_owned(),
    );
    require(
        errors,
        get(current, "active_wave").and_then(toml::Value::as_integer) == Some(0)
            && get(launch_view, "active_wave").and_then(toml::Value::as_integer) == Some(0),
        "current active wave mismatch".to_owned(),
    );
    require(
        errors,
        is_str_list(current, "authorized_packages", &["search-contracts"])
            && is_str_list(launch_view, "authorized_packages", &["search-contracts"]),
        "authorized package mismatch".to_owned(),
    );
    require(
        errors,
        is_str_list(
            current,
            "conditional_packages",
            &["search-domain", "search-ports"],
        ) && is_str_list(
            launch_view,
            "conditional_packages",
            &["search-domain", "search-ports"],
        ),
        "conditional package mismatch".to_owned(),
    );
    for (key, coverage_key) in [
        ("implemented_packages", "implemented_packages"),
        ("materialized_writer_contexts", "materialized_contexts"),
        ("issued_implementation_tickets", "issued_tickets"),
        ("active_writer_leases", "active_leases"),
        ("accepted_package_handoffs", "accepted_package_handoffs"),
        ("accepted_gate_receipts", "accepted_gates"),
        ("accepted_wave_receipts", "accepted_wave_receipts"),
    ] {
        let left = get(current, key).and_then(toml::Value::as_integer);
        let right = if coverage_key == "implemented_packages"
            || coverage_key == "accepted_gates"
            || coverage_key == "accepted_wave_receipts"
        {
            get(coverage_state, coverage_key).and_then(toml::Value::as_integer)
        } else {
            get(draft, coverage_key).and_then(toml::Value::as_integer)
        };
        let message = match key {
            "implemented_packages" => "implemented package count mismatch",
            "materialized_writer_contexts" => "materialized context count mismatch",
            "issued_implementation_tickets" => "issued ticket count mismatch",
            "active_writer_leases" => "active lease count mismatch",
            "accepted_package_handoffs" => "accepted handoff count mismatch",
            "accepted_gate_receipts" => "accepted gate count mismatch",
            _ => "accepted wave count mismatch",
        };
        require(
            errors,
            left == Some(0) && right == Some(0),
            message.to_owned(),
        );
    }
    let lock_present = root.join("Cargo.lock").is_file();
    require(
        errors,
        get(current, "cargo_lock_present").and_then(toml::Value::as_bool) == Some(lock_present),
        "Cargo.lock presence is reported incorrectly".to_owned(),
    );
    for key in [
        "windows_toolchain_selected",
        "qdrant_profile_selected",
        "rust_parser_profile_selected",
        "product_pulse_accepted",
        "optional_depth_selected",
    ] {
        require(
            errors,
            is_bool(current, key, false),
            format!("current unselected state changed: {key}"),
        );
    }
    CurrentView {
        stage: get(current, "active_stage")
            .and_then(toml::Value::as_str)
            .map(str::to_owned),
        wave: get(current, "active_wave").and_then(toml::Value::as_integer),
        lock_present,
    }
}

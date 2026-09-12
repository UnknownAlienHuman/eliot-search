//! Static package-assignment, delivery and qualification task closure.

use std::collections::{BTreeMap, BTreeSet};
use std::path::Path;

use toml::Value;

use super::{child, require, string, string_list};

pub(super) fn validate(
    root: &Path,
    packages: &BTreeMap<String, Value>,
    tasks: &Value,
    delivery: &BTreeMap<String, Value>,
    errors: &mut Vec<String>,
) -> (usize, usize) {
    require(
        errors,
        string(tasks, "status")
            == Some("STATIC_TASK_OWNERSHIP_CLOSED_NOT_IMPLEMENTED"),
        "task registry status changed",
    );
    require(
        errors,
        string(tasks, "package_registry") == Some("swarm/crates.toml"),
        "task package registry mismatch",
    );
    require(
        errors,
        string(tasks, "delivery_registry")
            == Some("swarm/coverage/delivery-slices.toml"),
        "task delivery registry mismatch",
    );
    require(
        errors,
        tasks
            .get("package_assignment_task_count")
            .and_then(Value::as_integer)
            == Some(45),
        "task assignment count mismatch",
    );
    require(
        errors,
        tasks
            .get("delivery_slice_task_count")
            .and_then(Value::as_integer)
            == Some(19),
        "task delivery count mismatch",
    );
    require(
        errors,
        tasks
            .get("implementation_authorized_by_this_registry")
            .and_then(Value::as_bool)
            == Some(false),
        "task registry authorizes implementation",
    );

    let assignments = validate_assignments(root, packages, tasks, errors);
    let delivery_count = validate_delivery(delivery, tasks, errors);
    validate_qualification_roots(root, tasks, errors);
    validate_invariants(tasks, errors);
    (assignments, delivery_count)
}

fn validate_assignments(
    root: &Path,
    packages: &BTreeMap<String, Value>,
    tasks: &Value,
    errors: &mut Vec<String>,
) -> usize {
    require(
        errors,
        child(tasks, "package_assignment_tasks", "source")
            .and_then(Value::as_str)
            == Some("swarm/crates.toml::package.assignment"),
        "package task source mismatch",
    );
    for key in [
        "one_assignment_per_package",
        "assignment_file_required",
        "assignment_must_name_owned_state_or_behavior",
        "assignment_must_name_forbidden_or_non_owned_behavior",
        "assignment_must_name_exact_package_write_scope",
    ] {
        require(
            errors,
            child(tasks, "package_assignment_tasks", key)
                .and_then(Value::as_bool)
                == Some(true),
            format!("package task invariant disabled: {key}"),
        );
    }
    require(
        errors,
        child(
            tasks,
            "package_assignment_tasks",
            "assignment_must_not_override_dependency_or_function_registry",
        )
        .and_then(Value::as_bool)
            == Some(true),
        "assignment override guard disabled",
    );

    let mut assignment_paths = BTreeSet::new();
    for (package, row) in packages {
        let path = row.get("assignment").and_then(Value::as_str);
        require(
            errors,
            path.is_some_and(|relative| root.join(relative).is_file()),
            format!("{package}: assignment file missing"),
        );
        let Some(relative) = path else {
            continue;
        };
        if !root.join(relative).is_file() {
            continue;
        }
        require(
            errors,
            assignment_paths.insert(relative.to_owned()),
            format!("duplicate assignment path {relative}"),
        );
        let Ok(text) = std::fs::read_to_string(root.join(relative)) else {
            continue;
        };
        require(
            errors,
            text.contains(package.as_str()),
            format!("{package}: assignment does not identify package"),
        );
        require(
            errors,
            ["Own", "Ownership", "Mission"]
                .iter()
                .any(|token| text.contains(token)),
            format!("{package}: assignment lacks owned behavior"),
        );
        require(
            errors,
            ["Forbidden", "Do not", "Never"]
                .iter()
                .any(|token| text.contains(token)),
            format!("{package}: assignment lacks non-owned behavior"),
        );
    }
    assignment_paths.len()
}

fn validate_delivery(
    delivery: &BTreeMap<String, Value>,
    tasks: &Value,
    errors: &mut Vec<String>,
) -> usize {
    require(
        errors,
        child(tasks, "delivery_tasks", "source").and_then(Value::as_str)
            == Some(
                "docs/architecture/ELIOT_SEARCH_8.4_IMPLEMENTATION_MASTER.md#H17",
            ),
        "delivery task source mismatch",
    );
    for key in [
        "one_registry_entry_per_delivery_slice",
        "primary_package_set_required",
        "module_set_required",
        "required_outputs_required",
        "exit_evidence_required",
    ] {
        require(
            errors,
            child(tasks, "delivery_tasks", key).and_then(Value::as_bool)
                == Some(true),
            format!("delivery task invariant disabled: {key}"),
        );
    }

    let expected: BTreeSet<String> =
        (0..19).map(|index| format!("P{index:02}")).collect();
    let actual: BTreeSet<String> = delivery.keys().cloned().collect();
    require(
        errors,
        actual == expected,
        "delivery task set must be P00-P18",
    );
    for (slice_id, row) in delivery {
        for (key, message) in [
            ("primary_packages", "primary package set missing"),
            ("modules", "module set missing"),
            ("required_outputs", "required outputs missing"),
            ("exit_evidence", "exit evidence missing"),
        ] {
            require(
                errors,
                row.get(key)
                    .and_then(Value::as_array)
                    .is_some_and(|items| !items.is_empty()),
                format!("{slice_id}: {message}"),
            );
        }
    }
    delivery.len()
}

fn validate_qualification_roots(
    root: &Path,
    tasks: &Value,
    errors: &mut Vec<String>,
) {
    let roots = tasks
        .get("qualification_tasks")
        .map(|value| string_list(value, "source_roots"))
        .unwrap_or_default();
    require(
        errors,
        roots == vec!["qualification".to_owned(), "tests".to_owned()],
        "qualification task roots mismatch",
    );
    for relative in &roots {
        require(
            errors,
            root.join(relative).is_dir(),
            format!("qualification task root missing: {relative}"),
        );
    }
    for key in [
        "qualification_is_evidence_requirement_not_success",
        "unavailable_or_unexecuted_state_must_not_authorize",
        "package_writer_may_not_accept_own_evidence",
    ] {
        require(
            errors,
            child(tasks, "qualification_tasks", key)
                .and_then(Value::as_bool)
                == Some(true),
            format!("qualification task invariant disabled: {key}"),
        );
    }
}

fn validate_invariants(tasks: &Value, errors: &mut Vec<String>) {
    for key in [
        "package_without_assignment_blocks_merge",
        "orphan_assignment_file_blocks_merge",
        "delivery_slice_without_owner_blocks_merge",
        "delivery_slice_without_exit_evidence_blocks_merge",
        "package_absent_from_all_delivery_slices_blocks_merge",
    ] {
        require(
            errors,
            child(tasks, "invariants", key).and_then(Value::as_bool)
                == Some(true),
            format!("task merge guard disabled: {key}"),
        );
    }
    require(
        errors,
        child(
            tasks,
            "invariants",
            "assignment_or_delivery_presence_authorizes_implementation",
        )
        .and_then(Value::as_bool)
            == Some(false),
        "task presence authorizes implementation",
    );
}

//! Qualification inventory, manifest closure and launch-state fences.

use std::collections::BTreeSet;

use toml::Value;

use super::{integer, require, string, string_list};

#[allow(clippy::too_many_arguments)]
pub(super) fn validate(
    manifest: &Value,
    modules: &Value,
    cases: &Value,
    launch: &Value,
    assignment_count: usize,
    delivery_count: usize,
    errors: &mut Vec<String>,
) -> usize {
    let case_rows = cases.get("case").and_then(Value::as_array);
    let case_count = case_rows.map_or(0, Vec::len);

    require(
        errors,
        integer(cases, "schema_version") == Some(1),
        "coverage case schema mismatch",
    );
    require(
        errors,
        string(cases, "suite") == Some("architecture_coverage_closure_v1"),
        "coverage case suite mismatch",
    );
    require(
        errors,
        string(cases, "status") == Some("STRUCTURAL_NOT_EXECUTED"),
        "coverage case status changed",
    );
    require(
        errors,
        integer(cases, "case_count") == Some(40),
        "coverage case_count must be 40",
    );
    require(
        errors,
        case_count == 40,
        "coverage case inventory must contain 40 rows",
    );

    if let Some(rows) = case_rows {
        let mut ids = Vec::new();
        let mut unique = BTreeSet::new();
        for row in rows {
            if let Some(table) = row.as_table() {
                let id = table.get("id").and_then(Value::as_str);
                ids.push(id.map(str::to_owned));
                if let Some(id) = id {
                    unique.insert(id.to_owned());
                }
                require(
                    errors,
                    table.get("mandatory").and_then(Value::as_bool)
                        == Some(true),
                    "coverage case must be mandatory",
                );
                require(
                    errors,
                    table.get("result").and_then(Value::as_str)
                        == Some("UNAVAILABLE"),
                    "coverage case must remain UNAVAILABLE",
                );
            } else {
                ids.push(None);
                require(errors, false, "coverage case must be mandatory");
                require(errors, false, "coverage case must remain UNAVAILABLE");
            }
        }
        require(
            errors,
            ids.len() == 40 && unique.len() == 40 && ids.iter().all(Option::is_some),
            "coverage case IDs must be 40 unique values",
        );
    }

    require(
        errors,
        string(manifest, "operation_registry")
            == Some("swarm/coverage/operations.toml"),
        "coverage manifest operation registry link mismatch",
    );
    require(
        errors,
        string(manifest, "task_registry") == Some("swarm/coverage/tasks.toml"),
        "coverage manifest task registry link mismatch",
    );
    require(
        errors,
        integer(manifest, "package_assignment_task_count")
            == i64::try_from(assignment_count).ok()
            && assignment_count == 45,
        "coverage manifest assignment count mismatch",
    );
    require(
        errors,
        integer(manifest, "delivery_slice_count")
            == i64::try_from(delivery_count).ok()
            && delivery_count == 19,
        "coverage manifest delivery task count mismatch",
    );
    require(
        errors,
        integer(modules, "package_count") == Some(45)
            && integer(modules, "module_count") == Some(479),
        "module registry summary mismatch",
    );
    require(
        errors,
        string(launch, "active_stage") == Some("P00")
            && integer(launch, "active_wave") == Some(0),
        "launch authority moved from P00/W0",
    );
    require(
        errors,
        string_list(launch, "authorized_packages")
            == ["search-contracts".to_owned()],
        "authorized package set changed",
    );
    case_count
}

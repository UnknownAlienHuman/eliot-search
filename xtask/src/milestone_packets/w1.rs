mod spec;

use std::collections::BTreeSet;
use std::path::Path;

use toml::Value;

use super::{
    MilestonePacketReport, boolean, expected_strings, indexed_rows, integer,
    load_doc, read_text, string, string_list,
};
use spec::PACKAGES;

#[must_use]
pub(super) fn validate_w1_milestone_packets(
    root: &Path,
) -> MilestonePacketReport {
    match validate(root) {
        Ok(report) => report,
        Err(error) => MilestonePacketReport::early(error),
    }
}

fn validate(root: &Path) -> Result<MilestonePacketReport, String> {
    let document = load_doc(root, "swarm/w1-milestone-packets.toml")?;
    let agent_rows = indexed_rows(
        &load_doc(root, "swarm/w1-agent-packets.toml")?,
        "package",
        "name",
    )?;
    let launch = load_doc(root, "swarm/launch-state.toml")?;
    let cases = load_doc(root, "qualification/w1-milestones/cases-v1.toml")?;
    let rows = indexed_rows(&document, "package", "name")?;
    let mut errors = Vec::new();

    let expected_names: BTreeSet<String> =
        PACKAGES.iter().map(|spec| spec.name.to_owned()).collect();
    let actual_names: BTreeSet<String> = rows.keys().cloned().collect();
    if actual_names != expected_names {
        errors.push("package set mismatch".to_owned());
    }

    if integer(&document, "package_count") != Some(7)
        || integer(&document, "milestone_count") != Some(28)
    {
        errors.push("count mismatch".to_owned());
    }
    if string(&document, "status") != Some("BLOCKED_ON_G0_AND_W0") {
        errors.push("registry not blocked".to_owned());
    }
    if string_list(&document, "requires_accepted_gates")
        != Some(expected_strings(&["G0"]))
        || string_list(&document, "requires_accepted_receipts")
            != Some(expected_strings(&["W0"]))
    {
        errors.push("prerequisite mismatch".to_owned());
    }
    if boolean(&document, "one_writer_one_package") != Some(true)
        || boolean(&document, "sequential_milestones_per_package")
            != Some(true)
    {
        errors.push("ownership/order disabled".to_owned());
    }
    if boolean(&document, "parallel_milestones_within_package")
        != Some(false)
        || boolean(
            &document,
            "implementation_authorized_by_this_registry",
        ) != Some(false)
    {
        errors.push("authority ceiling failed".to_owned());
    }

    if string(&launch, "active_stage") != Some("P00")
        || integer(&launch, "active_wave") != Some(0)
        || string_list(&launch, "authorized_packages")
            != Some(expected_strings(&["search-contracts"]))
    {
        errors.push("launch moved".to_owned());
    }

    for spec in &PACKAGES {
        validate_package(
            root,
            spec,
            rows.get(spec.name),
            agent_rows.get(spec.name),
            &mut errors,
        );
    }

    let case_rows = cases.get("case").and_then(Value::as_array);
    if integer(&cases, "case_count") != Some(16)
        || case_rows.is_none_or(|items| items.len() != 16)
    {
        errors.push("case inventory".to_owned());
    } else if case_rows.is_some_and(|items| {
        items.iter().any(|row| {
            row.as_table().is_none_or(|table| {
                table.get("mandatory").and_then(Value::as_bool) != Some(true)
                    || table.get("result").and_then(Value::as_str)
                        != Some("UNAVAILABLE")
            })
        })
    }) {
        errors.push("case state".to_owned());
    }

    validate_workflow(root, &mut errors);

    Ok(MilestonePacketReport {
        complete: true,
        packages: rows.len(),
        milestones: PACKAGES
            .iter()
            .map(|spec| spec.milestones.len())
            .sum(),
        cases: case_rows.map_or(0, Vec::len),
        launch_stage: string(&launch, "active_stage").map(str::to_owned),
        launch_wave: integer(&launch, "active_wave"),
        errors,
    })
}

fn validate_package(
    root: &Path,
    spec: &spec::PackageSpec,
    row: Option<&Value>,
    agent: Option<&Value>,
    errors: &mut Vec<String>,
) {
    let empty = Value::Table(toml::map::Map::new());
    let row = row.unwrap_or(&empty);
    let empty_agent = Value::Table(toml::map::Map::new());
    let agent = agent.unwrap_or(&empty_agent);
    let write_scope = format!("{}/**", spec.path);

    if string(row, "path") != Some(spec.path)
        || string(row, "write_scope") != Some(write_scope.as_str())
    {
        errors.push(format!("{}: scope", spec.name));
    }
    if string_list(row, "required_handoff_packages")
        != Some(expected_strings(spec.deps))
        || string_list(agent, "required_handoff_packages")
            != Some(expected_strings(spec.deps))
    {
        errors.push(format!("{}: deps", spec.name));
    }
    if string_list(row, "milestone_ids")
        != Some(expected_strings(spec.milestones))
        || boolean(row, "one_active_milestone") != Some(true)
        || boolean(row, "claimable") != Some(false)
    {
        errors.push(format!("{}: milestones", spec.name));
    }

    let Some(packet) = string(row, "packet") else {
        errors.push(format!("{}: packet missing", spec.name));
        return;
    };
    let Ok(text) = read_text(root, packet) else {
        errors.push(format!("{}: packet missing", spec.name));
        return;
    };
    for &milestone in spec.milestones {
        if !text.contains(&format!("## {milestone} —")) {
            errors.push(format!("{}: missing {milestone}", spec.name));
        }
    }
    if text.contains("docs/architecture/") || text.contains("/src/") {
        errors.push(format!("{}: forbidden read", spec.name));
    }
}

fn validate_workflow(root: &Path, errors: &mut Vec<String>) {
    let Ok(workflow) =
        read_text(root, ".github/workflows/w1-milestone-packets.yml")
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
    for forbidden in [
        "\n  push:",
        "\n  pull_request:",
        "\n  schedule:",
        "\n  workflow_run:",
    ] {
        if workflow.contains(forbidden) {
            errors.push(format!("automatic trigger {}", forbidden.trim()));
        }
    }
}

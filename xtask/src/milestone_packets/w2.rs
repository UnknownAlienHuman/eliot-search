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
pub(super) fn validate_w2_milestone_packets(
    root: &Path,
) -> MilestonePacketReport {
    match validate(root) {
        Ok(report) => report,
        Err(error) => MilestonePacketReport::early(error),
    }
}

fn validate(root: &Path) -> Result<MilestonePacketReport, String> {
    let document = load_doc(root, "swarm/w2-milestone-packets.toml")?;
    let packet_rows = indexed_rows(&document, "package", "name")?;
    let agent_rows = indexed_rows(
        &load_doc(root, "swarm/w2-agent-packets.toml")?,
        "package",
        "name",
    )?;
    let launch = load_doc(root, "swarm/launch-state.toml")?;
    let cases = load_doc(root, "qualification/w2-milestones/cases-v1.toml")?;
    let daemon_context =
        load_doc(root, "swarm/context-drafts/w2/eliot-searchd.toml")?;
    let mut errors = Vec::new();

    let expected_names: BTreeSet<String> =
        PACKAGES.iter().map(|spec| spec.name.to_owned()).collect();
    let actual_names: BTreeSet<String> = packet_rows.keys().cloned().collect();
    if actual_names != expected_names {
        let difference: Vec<String> = actual_names
            .symmetric_difference(&expected_names)
            .cloned()
            .collect();
        errors.push(format!("package set mismatch: {difference:?}"));
    }

    if string(&document, "status") != Some("BLOCKED_ON_G0_AND_W1") {
        errors.push("W2 milestone registry is not blocked".to_owned());
    }
    if string_list(&document, "requires_accepted_gates")
        != Some(expected_strings(&["G0"]))
    {
        errors.push("W2 milestone gate prerequisite mismatch".to_owned());
    }
    if string_list(&document, "requires_accepted_receipts")
        != Some(expected_strings(&["W1"]))
    {
        errors.push("W2 milestone receipt prerequisite mismatch".to_owned());
    }
    if integer(&document, "package_count") != Some(8)
        || integer(&document, "milestone_count") != Some(32)
    {
        errors.push("W2 package/milestone counts mismatch".to_owned());
    }
    if boolean(&document, "one_writer_one_package") != Some(true) {
        errors.push("one-writer-one-package invariant disabled".to_owned());
    }
    if boolean(&document, "sequential_milestones_per_package") != Some(true) {
        errors.push("sequential milestone invariant disabled".to_owned());
    }
    if boolean(&document, "parallel_milestones_within_package")
        != Some(false)
    {
        errors.push("parallel milestones inside package must remain false".to_owned());
    }
    if boolean(
        &document,
        "implementation_authorized_by_this_registry",
    ) != Some(false)
    {
        errors.push("W2 milestone registry authorizes implementation".to_owned());
    }

    if string(&launch, "active_stage") != Some("P00")
        || integer(&launch, "active_wave") != Some(0)
    {
        errors.push("launch authority moved from P00/W0".to_owned());
    }
    if string_list(&launch, "authorized_packages")
        != Some(expected_strings(&["search-contracts"]))
    {
        errors.push("current authorized package set changed".to_owned());
    }

    for spec in &PACKAGES {
        validate_package(
            root,
            spec,
            packet_rows.get(spec.name),
            agent_rows.get(spec.name),
            &mut errors,
        );
    }

    validate_daemon_reentry(root, &daemon_context, &mut errors);
    if !zero_state_matches(&document) {
        errors.push("W2 milestone zero-state mismatch".to_owned());
    }

    let case_rows = cases.get("case").and_then(Value::as_array);
    if integer(&cases, "case_count") != Some(18)
        || case_rows.is_none_or(|items| items.len() != 18)
    {
        errors.push("qualification case inventory mismatch".to_owned());
    } else if case_rows.is_some_and(|items| {
        items.iter().any(|case| {
            case.as_table().is_none_or(|table| {
                table.get("mandatory").and_then(Value::as_bool) != Some(true)
                    || table.get("result").and_then(Value::as_str)
                        != Some("UNAVAILABLE")
            })
        })
    }) {
        errors.push(
            "qualification cases are not mandatory UNAVAILABLE".to_owned(),
        );
    }

    validate_workflow(root, &mut errors);

    Ok(MilestonePacketReport {
        complete: true,
        packages: packet_rows.len(),
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
        errors.push(format!("{}: path/write scope mismatch", spec.name));
    }
    if string_list(row, "required_handoff_packages")
        != Some(expected_strings(spec.deps))
    {
        errors.push(format!(
            "{}: milestone dependency handoffs mismatch",
            spec.name
        ));
    }
    if string_list(agent, "required_handoff_packages")
        != Some(expected_strings(spec.deps))
    {
        errors.push(format!(
            "{}: agent dependency handoffs mismatch",
            spec.name
        ));
    }
    if string_list(row, "milestone_ids")
        != Some(expected_strings(spec.milestones))
    {
        errors.push(format!("{}: milestone IDs/order mismatch", spec.name));
    }
    if boolean(row, "one_active_milestone") != Some(true)
        || boolean(row, "claimable") != Some(false)
    {
        errors.push(format!(
            "{}: active/claimability invariant failed",
            spec.name
        ));
    }

    let Some(packet) = string(row, "packet") else {
        errors.push(format!("{}: missing package checkpoint packet", spec.name));
        return;
    };
    let Ok(text) = read_text(root, packet) else {
        errors.push(format!("{}: missing package checkpoint packet", spec.name));
        return;
    };
    for &milestone in spec.milestones {
        if !text.contains(&format!("## {milestone} —")) {
            errors.push(format!("{}: missing checkpoint {milestone}", spec.name));
        }
    }
    if text.contains("docs/architecture/") || text.contains("/src/") {
        errors.push(format!(
            "{}: forbidden architecture/implementation read in packet",
            spec.name
        ));
    }
    if text.contains("docs/handoff/W1_IMPLEMENTATION_PACKET.md") {
        errors.push(format!("{}: prior W1 packet replayed", spec.name));
    }
    if spec.name != "eliot-searchd" && text.contains("Qdrant") {
        errors.push(format!(
            "{}: indexed dependency mentioned in W2 packet",
            spec.name
        ));
    }
}

fn validate_daemon_reentry(
    root: &Path,
    daemon_context: &Value,
    errors: &mut Vec<String>,
) {
    let daemon_sources = daemon_context
        .get("content")
        .and_then(Value::as_table)
        .and_then(|content| content.get("source_files"))
        .and_then(Value::as_array)
        .and_then(|items| {
            items
                .iter()
                .map(|item| item.as_str().map(str::to_owned))
                .collect::<Option<Vec<_>>>()
        })
        .unwrap_or_default();
    if !daemon_sources
        .iter()
        .any(|source| source == "docs/handoff/W2_DAEMON_REENTRY.md")
    {
        errors.push("daemon context lacks W2 re-entry boundary".to_owned());
    }
    if daemon_sources
        .iter()
        .any(|source| source == "docs/handoff/W1_IMPLEMENTATION_PACKET.md")
    {
        errors.push("daemon W2 context replays W1 implementation packet".to_owned());
    }

    match read_text(root, "docs/handoff/w2-packages/eliot-searchd.md") {
        Ok(text) => {
            for token in [
                "accepted prior W1 daemon",
                "Do not replay the W1 implementation packet",
                "DIRECT",
                "D20",
                "D23",
            ] {
                if !text.contains(token) {
                    errors.push(format!(
                        "daemon packet missing re-entry token: {token}"
                    ));
                }
            }
        }
        Err(_) => errors.push(
            "daemon packet missing re-entry token: unreadable packet".to_owned(),
        ),
    }
}

fn zero_state_matches(document: &Value) -> bool {
    let Some(state) = document.get("current_state").and_then(Value::as_table)
    else {
        return false;
    };
    state.len() == 7
        && state.get("accepted_G0").and_then(Value::as_bool) == Some(false)
        && state.get("accepted_W1").and_then(Value::as_bool) == Some(false)
        && state.get("materialized_contexts").and_then(Value::as_integer)
            == Some(0)
        && state.get("issued_tickets").and_then(Value::as_integer) == Some(0)
        && state.get("active_leases").and_then(Value::as_integer) == Some(0)
        && state
            .get("accepted_package_handoffs")
            .and_then(Value::as_integer)
            == Some(0)
        && state.get("W2_G1_receipt").and_then(Value::as_str)
            == Some("ABSENT")
}

fn validate_workflow(root: &Path, errors: &mut Vec<String>) {
    let Ok(workflow) =
        read_text(root, ".github/workflows/w2-milestone-packets.yml")
    else {
        errors.push("missing W2 milestone workflow".to_owned());
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
    for trigger in [
        "\n  push:",
        "\n  pull_request:",
        "\n  schedule:",
        "\n  workflow_run:",
    ] {
        if workflow.contains(trigger) {
            errors.push(format!(
                "automatic workflow trigger: {}",
                trigger.trim()
            ));
        }
    }
}

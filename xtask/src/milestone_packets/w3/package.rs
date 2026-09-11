use std::path::Path;

use toml::Value;

use super::super::{
    boolean, expected_strings, read_text, string, string_list,
};
use super::spec::PackageSpec;

pub(super) fn validate_package(
    root: &Path,
    spec: &PackageSpec,
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
        || string_list(agent, "required_handoff_packages")
            != Some(expected_strings(spec.deps))
    {
        errors.push(format!("{}: dependency handoff mismatch", spec.name));
    }
    if string_list(row, "milestone_ids")
        != Some(expected_strings(spec.milestones))
        || boolean(row, "one_active_milestone") != Some(true)
        || boolean(row, "claimable") != Some(false)
    {
        errors.push(format!(
            "{}: milestone or claimability mismatch",
            spec.name
        ));
    }
    if string(row, "phase") != string(agent, "phase") {
        errors.push(format!("{}: phase mismatch with agent registry", spec.name));
    }
    if string(agent, "write_scope") != Some(write_scope.as_str()) {
        errors.push(format!("{}: agent write scope mismatch", spec.name));
    }
    let expected_ticket = format!("swarm/ticket-drafts/w3/{}.toml", spec.name);
    let expected_context = format!("swarm/context-drafts/w3/{}.toml", spec.name);
    if string(agent, "ticket_draft") != Some(expected_ticket.as_str())
        || string(agent, "context_draft") != Some(expected_context.as_str())
    {
        errors.push(format!("{}: agent draft linkage mismatch", spec.name));
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
            errors.push(format!("{}: missing checkpoint {milestone}", spec.name));
        }
    }
    if text.contains("docs/architecture/") || text.contains("/src/") {
        errors.push(format!("{}: forbidden read path in packet", spec.name));
    }
    if text.contains("W1_IMPLEMENTATION_PACKET.md")
        || text.contains("W2_IMPLEMENTATION_PACKET.md")
    {
        errors.push(format!("{}: prior-stage packet replay", spec.name));
    }
    if !text.contains("submission candidate") {
        errors.push(format!("{}: exit claim ceiling missing", spec.name));
    }
}

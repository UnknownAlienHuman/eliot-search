use std::path::Path;

use toml::Value;

use super::super::{
    boolean, child_string, child_string_list, expected_strings, integer,
    require_regular_file, string, string_list,
};
use super::spec::PackageSpec;

#[allow(clippy::too_many_arguments)]
pub(super) fn validate_package(
    root: &Path,
    spec: &PackageSpec,
    packet: Option<&Value>,
    crate_row: Option<&Value>,
    function: Option<&Value>,
    ticket: &Value,
    context: &Value,
    errors: &mut Vec<String>,
) {
    let empty_packet = Value::Table(toml::map::Map::new());
    let packet = packet.unwrap_or(&empty_packet);
    let empty_crate = Value::Table(toml::map::Map::new());
    let crate_row = crate_row.unwrap_or(&empty_crate);
    let empty_function = Value::Table(toml::map::Map::new());
    let function = function.unwrap_or(&empty_function);
    let write_scope = format!("{}/**", spec.path);

    validate_files(root, spec, packet, errors);
    validate_packet(spec, packet, &write_scope, errors);
    validate_registries(spec, crate_row, function, &write_scope, errors);
    validate_ticket(spec, ticket, &write_scope, errors);
    validate_context(root, spec, context, errors);
}

fn validate_files(
    root: &Path,
    spec: &PackageSpec,
    packet: &Value,
    errors: &mut Vec<String>,
) {
    require_regular_file(root, spec.name, string(packet, "assignment"), errors);
    require_regular_file(root, spec.name, string(packet, "functions"), errors);
    for relative in [
        format!("{}/AGENTS.md", spec.path),
        format!("{}/Cargo.toml", spec.path),
        format!("{}/README.md", spec.path),
    ] {
        require_regular_file(root, spec.name, Some(&relative), errors);
    }
    for &relative in spec.config {
        require_regular_file(root, spec.name, Some(relative), errors);
    }
    for &relative in spec.qualification_sources {
        require_regular_file(root, spec.name, Some(relative), errors);
    }
}

fn validate_packet(
    spec: &PackageSpec,
    packet: &Value,
    write_scope: &str,
    errors: &mut Vec<String>,
) {
    if string(packet, "path") != Some(spec.path)
        || string(packet, "write_scope") != Some(write_scope)
    {
        errors.push(format!("{}: packet path/write scope mismatch", spec.name));
    }
    if string(packet, "phase") != Some(spec.phase)
        || string(packet, "execution_group") != Some(spec.group)
    {
        errors.push(format!("{}: phase/group mismatch", spec.name));
    }
    if string_list(packet, "required_handoff_packages")
        != Some(expected_strings(spec.deps))
        || string_list(packet, "config_packets") != Some(expected_strings(spec.config))
    {
        errors.push(format!("{}: dependency/config packet mismatch", spec.name));
    }
    if integer(packet, "soft_src_lines") != Some(spec.soft)
        || integer(packet, "split_review_total_lines") != Some(8_500)
        || integer(packet, "hard_total_lines") != Some(10_000)
        || boolean(packet, "one_active_writer") != Some(true)
        || boolean(packet, "claimable") != Some(false)
    {
        errors.push(format!("{}: line budget or claimability mismatch", spec.name));
    }
}

fn validate_registries(
    spec: &PackageSpec,
    crate_row: &Value,
    function: &Value,
    write_scope: &str,
    errors: &mut Vec<String>,
) {
    let expected_wave = if spec.name == "eliot-searchd" { 1 } else { 3 };
    if string(crate_row, "path") != Some(spec.path)
        || integer(crate_row, "wave") != Some(expected_wave)
    {
        errors.push(format!("{}: crate registry mismatch", spec.name));
    }
    if string(function, "write_scope") != Some(write_scope) {
        errors.push(format!("{}: function registry write scope mismatch", spec.name));
    }
}

fn validate_ticket(
    spec: &PackageSpec,
    ticket: &Value,
    write_scope: &str,
    errors: &mut Vec<String>,
) {
    if string(ticket, "status") != Some("DRAFT_ONLY_NOT_ISSUED")
        || boolean(ticket, "claimable") != Some(false)
        || boolean(ticket, "authorizes_implementation") != Some(false)
        || boolean(ticket, "creates_lease") != Some(false)
    {
        errors.push(format!("{}: ticket draft creates authority", spec.name));
    }
    if string(ticket, "stage") != Some("W3")
        || integer(ticket, "wave") != Some(3)
        || string(ticket, "phase") != Some(spec.phase)
    {
        errors.push(format!("{}: ticket stage mismatch", spec.name));
    }
    if child_string(ticket, "repository_fence", "write_scope") != Some(write_scope)
        || child_string_list(ticket, "dependencies", "required_handoff_packages")
            != Some(expected_strings(spec.deps))
    {
        errors.push(format!("{}: ticket scope/dependency mismatch", spec.name));
    }
    if child_string(ticket, "unresolved_identity", "base_commit") != Some("UNSELECTED")
        || child_string(ticket, "unresolved_identity", "writer") != Some("UNASSIGNED")
        || child_string(ticket, "unresolved_identity", "reviewer") != Some("UNASSIGNED")
    {
        errors.push(format!("{}: ticket identity prematurely resolved", spec.name));
    }
    if child_string(ticket, "stage_prerequisites", "status") != Some("UNAVAILABLE") {
        errors.push(format!("{}: stage prerequisites prematurely accepted", spec.name));
    }
    validate_qualification(spec, ticket, errors);
}

fn validate_qualification(
    spec: &PackageSpec,
    ticket: &Value,
    errors: &mut Vec<String>,
) {
    for key in [
        "artifact_status",
        "collection_schema",
        "mandatory_probe_evidence",
        "independent_reviewer_receipt",
    ] {
        if let Some(value) = child_string(ticket, "qualification", key) {
            if !matches!(
                value,
                "UNQUALIFIED" | "NOT_ACCEPTED" | "UNAVAILABLE" | "ABSENT"
            ) {
                errors.push(format!(
                    "{}: qualification field {key} is successful",
                    spec.name
                ));
            }
        }
    }
    for key in [
        "automatic_download",
        "automatic_upgrade",
        "indexed_mode_enabled",
    ] {
        if ticket
            .get("qualification")
            .and_then(Value::as_table)
            .and_then(|table| table.get(key))
            .and_then(Value::as_bool)
            == Some(true)
        {
            errors.push(format!("{}: ticket enables {key}", spec.name));
        }
    }
}

fn validate_context(
    root: &Path,
    spec: &PackageSpec,
    context: &Value,
    errors: &mut Vec<String>,
) {
    if string(context, "status") != Some("UNMATERIALIZED_DRAFT")
        || boolean(context, "claimable") != Some(false)
        || boolean(context, "authorizes_implementation") != Some(false)
    {
        errors.push(format!("{}: context draft creates authority", spec.name));
    }
    if string(context, "stage") != Some("W3")
        || integer(context, "wave") != Some(3)
        || string(context, "phase") != Some(spec.phase)
    {
        errors.push(format!("{}: context stage mismatch", spec.name));
    }

    let sources = child_string_list(context, "content", "source_files").unwrap_or_default();
    let selectors =
        child_string_list(context, "content", "registry_fragments").unwrap_or_default();
    let slots = child_string_list(context, "content", "accepted_handoff_slots")
        .unwrap_or_default();
    if integer(context, "source_file_count") != i64::try_from(sources.len()).ok()
        || sources.len() > 16
    {
        errors.push(format!("{}: source count/ceiling mismatch", spec.name));
    }
    if integer(context, "registry_fragment_count") != i64::try_from(selectors.len()).ok()
        || selectors.len() > 4
    {
        errors.push(format!("{}: selector count/ceiling mismatch", spec.name));
    }
    if integer(context, "accepted_handoff_slot_count") != i64::try_from(slots.len()).ok()
        || slots.len() != spec.deps.len()
    {
        errors.push(format!("{}: handoff slot count mismatch", spec.name));
    }

    let expected_slots: Vec<String> = spec
        .deps
        .iter()
        .map(|dependency| format!("{dependency}::accepted_package_and_api_handoff"))
        .collect();
    if slots != expected_slots {
        errors.push(format!("{}: handoff slots mismatch", spec.name));
    }
    let expected_selectors = vec![
        format!("swarm/crates.toml::package[name={}]", spec.name),
        format!("swarm/function-packets.toml::package[name={}]", spec.name),
        "swarm/stages.toml::stage[id=W3]".to_owned(),
    ];
    if selectors != expected_selectors {
        errors.push(format!("{}: selector set mismatch", spec.name));
    }

    for source in &sources {
        if !root.join(source).is_file() {
            errors.push(format!("{}: missing context source {source}", spec.name));
            continue;
        }
        if source.starts_with("docs/architecture/") || source.contains("/src/") {
            errors.push(format!(
                "{}: forbidden architecture/implementation source {source}",
                spec.name
            ));
        }
        if matches!(
            source.as_str(),
            "docs/handoff/W1_IMPLEMENTATION_PACKET.md"
                | "docs/handoff/W2_IMPLEMENTATION_PACKET.md"
        ) {
            errors.push(format!("{}: prior stage packet replayed", spec.name));
        }
    }

    if !sources
        .iter()
        .any(|source| source == "qualification/qdrant/W3_QUALIFICATION.md")
    {
        errors.push(format!(
            "{}: W3 qualification contract absent from context",
            spec.name
        ));
    }
    for &required in spec.qualification_sources {
        if !sources.iter().any(|source| source == required) {
            errors.push(format!(
                "{}: qualification source absent {required}",
                spec.name
            ));
        }
    }
    if spec.name == "eliot-searchd"
        && !sources
            .iter()
            .any(|source| source == "docs/handoff/W3_DAEMON_REENTRY.md")
    {
        errors.push("eliot-searchd: re-entry packet absent".to_owned());
    }
}

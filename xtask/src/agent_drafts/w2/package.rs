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
    validate_registries(spec, packet, crate_row, function, &write_scope, errors);
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
    for relative in spec.config {
        require_regular_file(root, spec.name, Some(relative), errors);
    }
}

fn validate_packet(
    spec: &PackageSpec,
    packet: &Value,
    write_scope: &str,
    errors: &mut Vec<String>,
) {
    if string(packet, "path") != Some(spec.path) {
        errors.push(format!("{}: packet package path mismatch", spec.name));
    }
    if string(packet, "write_scope") != Some(write_scope) {
        errors.push(format!("{}: packet write scope mismatch", spec.name));
    }
    if string(packet, "phase") != Some(spec.phase)
        || string(packet, "execution_group") != Some(spec.group)
    {
        errors.push(format!("{}: phase/execution group mismatch", spec.name));
    }
    if string_list(packet, "required_handoff_packages")
        != Some(expected_strings(spec.deps))
    {
        errors.push(format!("{}: packet dependency handoffs mismatch", spec.name));
    }
    if string_list(packet, "config_packets") != Some(expected_strings(spec.config)) {
        errors.push(format!("{}: configuration packet mismatch", spec.name));
    }
    if integer(packet, "soft_src_lines") != Some(spec.soft) {
        errors.push(format!("{}: soft line target mismatch", spec.name));
    }
    if integer(packet, "split_review_total_lines") != Some(8_500)
        || integer(packet, "hard_total_lines") != Some(10_000)
    {
        errors.push(format!("{}: split/hard line limits mismatch", spec.name));
    }
    if boolean(packet, "one_active_writer") != Some(true)
        || boolean(packet, "claimable") != Some(false)
    {
        errors.push(format!(
            "{}: packet writer/claimability invariant failed",
            spec.name
        ));
    }
}

fn validate_registries(
    spec: &PackageSpec,
    packet: &Value,
    crate_row: &Value,
    function: &Value,
    write_scope: &str,
    errors: &mut Vec<String>,
) {
    if string(crate_row, "path") != Some(spec.path)
        || integer(crate_row, "wave") != Some(spec.base_wave)
    {
        errors.push(format!("{}: crate registry path/base wave mismatch", spec.name));
    }
    if string(function, "write_scope") != Some(write_scope) {
        errors.push(format!("{}: function registry write scope mismatch", spec.name));
    }
    if function.get("assignment") != packet.get("assignment") {
        errors.push(format!("{}: assignment registry mismatch", spec.name));
    }
    if function.get("functions") != packet.get("functions") {
        errors.push(format!("{}: function packet registry mismatch", spec.name));
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
    {
        errors.push(format!("{}: ticket draft became claimable", spec.name));
    }
    if boolean(ticket, "authorizes_implementation") != Some(false)
        || boolean(ticket, "creates_lease") != Some(false)
    {
        errors.push(format!("{}: ticket draft creates authority", spec.name));
    }
    if string(ticket, "stage") != Some("W2")
        || integer(ticket, "wave") != Some(2)
        || string(ticket, "phase") != Some(spec.phase)
    {
        errors.push(format!("{}: ticket stage/phase mismatch", spec.name));
    }
    if child_string(ticket, "repository_fence", "write_scope") != Some(write_scope) {
        errors.push(format!("{}: ticket write scope mismatch", spec.name));
    }
    if child_string_list(ticket, "dependencies", "required_handoff_packages")
        != Some(expected_strings(spec.deps))
    {
        errors.push(format!("{}: ticket dependency handoffs mismatch", spec.name));
    }
    if child_string(ticket, "unresolved_identity", "base_commit") != Some("UNSELECTED")
        || child_string(ticket, "unresolved_identity", "writer") != Some("UNASSIGNED")
        || child_string(ticket, "unresolved_identity", "reviewer") != Some("UNASSIGNED")
    {
        errors.push(format!("{}: ticket identity prematurely resolved", spec.name));
    }
    if child_string_list(ticket, "stage_prerequisites", "required_gates")
        != Some(expected_strings(&["G0"]))
        || child_string_list(ticket, "stage_prerequisites", "required_receipts")
            != Some(expected_strings(&["W1"]))
        || child_string(ticket, "stage_prerequisites", "status") != Some("UNAVAILABLE")
    {
        errors.push(format!("{}: ticket stage prerequisites mismatch", spec.name));
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
    {
        errors.push(format!("{}: context draft became claimable", spec.name));
    }
    if boolean(context, "authorizes_implementation") != Some(false) {
        errors.push(format!("{}: context draft authorizes implementation", spec.name));
    }
    if string(context, "stage") != Some("W2")
        || integer(context, "wave") != Some(2)
        || string(context, "phase") != Some(spec.phase)
    {
        errors.push(format!("{}: context stage/phase mismatch", spec.name));
    }
    if child_string_list(context, "stage_prerequisites", "required_gates")
        != Some(expected_strings(&["G0"]))
        || child_string_list(context, "stage_prerequisites", "required_receipts")
            != Some(expected_strings(&["W1"]))
        || child_string(context, "stage_prerequisites", "status") != Some("UNAVAILABLE")
    {
        errors.push(format!("{}: context stage prerequisites mismatch", spec.name));
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
        errors.push(format!(
            "{}: registry fragment count/ceiling mismatch",
            spec.name
        ));
    }
    if integer(context, "accepted_handoff_slot_count") != i64::try_from(slots.len()).ok()
        || slots.len() != spec.deps.len()
    {
        errors.push(format!(
            "{}: accepted handoff slot count mismatch",
            spec.name
        ));
    }

    let expected_slots: Vec<String> = spec
        .deps
        .iter()
        .map(|dependency| format!("{dependency}::accepted_package_and_api_handoff"))
        .collect();
    if slots != expected_slots {
        errors.push(format!(
            "{}: accepted handoff slot set/order mismatch",
            spec.name
        ));
    }
    let expected_selectors = vec![
        format!("swarm/crates.toml::package[name={}]", spec.name),
        format!("swarm/function-packets.toml::package[name={}]", spec.name),
        "swarm/stages.toml::stage[id=W2]".to_owned(),
    ];
    if selectors != expected_selectors {
        errors.push(format!(
            "{}: registry selector set/order mismatch",
            spec.name
        ));
    }

    validate_sources(root, spec, &sources, errors);
    validate_forbidden_paths(spec, context, errors);

    if spec.name == "eliot-searchd" {
        if !sources
            .iter()
            .any(|source| source == "docs/handoff/W2_DAEMON_REENTRY.md")
        {
            errors.push("eliot-searchd: missing W2 re-entry boundary".to_owned());
        }
        if spec.deps.first().copied() != Some("eliot-searchd") {
            errors.push("eliot-searchd: prior W1 daemon handoff not first".to_owned());
        }
    }
}

fn validate_sources(
    root: &Path,
    spec: &PackageSpec,
    sources: &[String],
    errors: &mut Vec<String>,
) {
    for source in sources {
        if !root.join(source).is_file() {
            errors.push(format!("{}: missing context source {source}", spec.name));
            continue;
        }
        if source.starts_with("docs/architecture/") {
            errors.push(format!("{}: architecture master in context", spec.name));
        }
        if source.contains("/src/") || source.ends_with("/src") {
            errors.push(format!("{}: implementation source in context", spec.name));
        }
        if source == "docs/handoff/W1_IMPLEMENTATION_PACKET.md" {
            errors.push(format!("{}: W1 packet replayed in W2 context", spec.name));
        }
        let lowered = source.to_ascii_lowercase();
        if lowered.contains("qdrant") || lowered.contains("w3_implementation_packet") {
            errors.push(format!(
                "{}: indexed/Qdrant packet entered W2 context",
                spec.name
            ));
        }
    }
}

fn validate_forbidden_paths(
    spec: &PackageSpec,
    context: &Value,
    errors: &mut Vec<String>,
) {
    let forbidden = child_string_list(context, "content", "forbidden_paths")
        .unwrap_or_default();
    for required in [
        "docs/architecture/**",
        "docs/handoff/W1_IMPLEMENTATION_PACKET.md",
    ] {
        if !forbidden.iter().any(|value| value == required) {
            errors.push(format!("{}: missing forbidden path {required}", spec.name));
        }
    }
}

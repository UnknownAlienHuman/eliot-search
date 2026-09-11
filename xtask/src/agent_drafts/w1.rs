use std::collections::BTreeSet;
use std::path::Path;

use toml::Value;

use super::{
    AgentDraftReport, boolean, child_bool, child_string, child_string_list,
    expected_strings, indexed_rows, integer, load_doc, read_text,
    require_regular_file, string, string_list, symmetric_difference,
};

struct PackageSpec {
    name: &'static str,
    path: &'static str,
    phase: &'static str,
    soft: i64,
    deps: &'static [&'static str],
    config: &'static [&'static str],
    group: &'static str,
}

const PACKAGES: [PackageSpec; 7] = [
    PackageSpec {
        name: "search-config",
        path: "crates/search-config",
        phase: "P01",
        soft: 5_500,
        deps: &["search-contracts"],
        config: &[],
        group: "A",
    },
    PackageSpec {
        name: "search-runtime-owner",
        path: "crates/search-runtime/search-runtime-owner",
        phase: "P01",
        soft: 4_500,
        deps: &["search-contracts", "search-domain", "search-ports", "search-config"],
        config: &["config/sections/instance.md"],
        group: "B",
    },
    PackageSpec {
        name: "search-os-secrets",
        path: "crates/search-runtime/search-os-secrets",
        phase: "P01",
        soft: 3_500,
        deps: &["search-contracts", "search-domain", "search-ports", "search-config"],
        config: &["config/sections/secrets.md"],
        group: "B",
    },
    PackageSpec {
        name: "search-control-redb",
        path: "crates/search-control-redb",
        phase: "P02",
        soft: 7_500,
        deps: &["search-contracts", "search-domain", "search-ports", "search-config"],
        config: &["config/sections/control.md"],
        group: "B",
    },
    PackageSpec {
        name: "search-provider-protocol",
        path: "crates/search-provider-protocol",
        phase: "P02",
        soft: 7_500,
        deps: &["search-contracts", "search-domain", "search-ports", "search-config"],
        config: &["config/sections/protocol.md"],
        group: "B",
    },
    PackageSpec {
        name: "eliot-searchd",
        path: "bins/eliot-searchd",
        phase: "P02",
        soft: 6_500,
        deps: &[
            "search-contracts",
            "search-domain",
            "search-ports",
            "search-config",
            "search-runtime-owner",
            "search-os-secrets",
            "search-control-redb",
            "search-provider-protocol",
        ],
        config: &["config/sections/optional_profiles.md"],
        group: "C",
    },
    PackageSpec {
        name: "eliot-search",
        path: "bins/eliot-search",
        phase: "P02",
        soft: 4_500,
        deps: &["search-contracts", "search-ports", "search-config", "search-provider-protocol"],
        config: &[],
        group: "C",
    },
];

pub(super) fn validate_w1_agent_drafts(root: &Path) -> AgentDraftReport {
    match validate(root) {
        Ok(report) => report,
        Err(error) => AgentDraftReport::early(error),
    }
}

fn validate(root: &Path) -> Result<AgentDraftReport, String> {
    let packet_doc = load_doc(root, "swarm/w1-agent-packets.toml")?;
    let ticket_manifest = load_doc(root, "swarm/ticket-drafts/w1/manifest.toml")?;
    let context_manifest = load_doc(root, "swarm/context-drafts/w1/manifest.toml")?;
    let crates = indexed_rows(&load_doc(root, "swarm/crates.toml")?, "package", "name")?;
    let functions = indexed_rows(
        &load_doc(root, "swarm/function-packets.toml")?,
        "package",
        "name",
    )?;
    let stages = indexed_rows(&load_doc(root, "swarm/stages.toml")?, "stage", "id")?;
    let launch = load_doc(root, "swarm/launch-state.toml")?;
    let cases = load_doc(root, "qualification/w1-agent-drafts/cases-v1.toml")?;
    let packet_rows = indexed_rows(&packet_doc, "package", "name")?;
    let ticket_rows = indexed_rows(&ticket_manifest, "draft", "package")?;
    let context_rows = indexed_rows(&context_manifest, "draft", "package")?;

    let expected_names: BTreeSet<String> =
        PACKAGES.iter().map(|spec| spec.name.to_owned()).collect();
    let mut errors = Vec::new();
    for (label, rows) in [
        ("packet", &packet_rows),
        ("ticket", &ticket_rows),
        ("context", &context_rows),
    ] {
        let actual: BTreeSet<String> = rows.keys().cloned().collect();
        if actual != expected_names {
            errors.push(format!(
                "{label} package set mismatch: {:?}",
                symmetric_difference(&actual, &expected_names)
            ));
        }
    }

    if string(&packet_doc, "status") != Some("BLOCKED_ON_G0_AND_W0") {
        errors.push("W1 packet registry is not blocked".to_owned());
    }
    if string_list(&packet_doc, "requires_accepted_gates")
        != Some(expected_strings(&["G0"]))
        || string_list(&packet_doc, "requires_accepted_receipts")
            != Some(expected_strings(&["W0"]))
    {
        errors.push("W1 prerequisites mismatch".to_owned());
    }
    if boolean(&packet_doc, "one_writer_one_package") != Some(true) {
        errors.push("one-writer-one-package disabled".to_owned());
    }
    if boolean(&packet_doc, "parallel_milestones_within_package") != Some(false) {
        errors.push("parallel milestones within a package must be false".to_owned());
    }
    if boolean(&packet_doc, "implementation_authorized_by_this_registry") != Some(false) {
        errors.push("packet registry authorizes implementation".to_owned());
    }

    let empty = Value::Table(toml::map::Map::new());
    let w1 = stages.get("W1").unwrap_or(&empty);
    if string(w1, "status") != Some("BLOCKED")
        || string_list(w1, "packages")
            != Some(PACKAGES.iter().map(|spec| spec.name.to_owned()).collect())
    {
        errors.push("central W1 stage mismatch".to_owned());
    }
    if string_list(w1, "requires_accepted_gates") != Some(expected_strings(&["G0"]))
        || string_list(w1, "requires_accepted_receipts") != Some(expected_strings(&["W0"]))
    {
        errors.push("central W1 prerequisites mismatch".to_owned());
    }

    if string(&launch, "active_stage") != Some("P00")
        || integer(&launch, "active_wave") != Some(0)
    {
        errors.push("launch authority moved from P00/W0".to_owned());
    }
    if string_list(&launch, "authorized_packages")
        != Some(expected_strings(&["search-contracts"]))
    {
        errors.push("authorized package set changed".to_owned());
    }

    for spec in &PACKAGES {
        let empty_packet = Value::Table(toml::map::Map::new());
        let packet = packet_rows.get(spec.name).unwrap_or(&empty_packet);
        let empty_crate = Value::Table(toml::map::Map::new());
        let crate_row = crates.get(spec.name).unwrap_or(&empty_crate);
        let empty_function = Value::Table(toml::map::Map::new());
        let function = functions.get(spec.name).unwrap_or(&empty_function);
        let ticket = load_doc(
            root,
            &format!("swarm/ticket-drafts/w1/{}.toml", spec.name),
        )?;
        let context = load_doc(
            root,
            &format!("swarm/context-drafts/w1/{}.toml", spec.name),
        )?;

        require_regular_file(root, spec.name, string(packet, "assignment"), &mut errors);
        require_regular_file(root, spec.name, string(packet, "functions"), &mut errors);
        for relative in [
            format!("{}/AGENTS.md", spec.path),
            format!("{}/Cargo.toml", spec.path),
            format!("{}/README.md", spec.path),
        ] {
            require_regular_file(root, spec.name, Some(&relative), &mut errors);
        }
        for relative in spec.config {
            require_regular_file(root, spec.name, Some(relative), &mut errors);
        }

        if string(packet, "path") != Some(spec.path)
            || string(packet, "write_scope") != Some(&format!("{}/**", spec.path))
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
        {
            errors.push(format!("{}: packet dependency handoffs mismatch", spec.name));
        }
        if string_list(packet, "config_packets") != Some(expected_strings(spec.config)) {
            errors.push(format!("{}: config packet mismatch", spec.name));
        }
        if integer(packet, "soft_src_lines") != Some(spec.soft)
            || integer(packet, "hard_total_lines") != Some(10_000)
        {
            errors.push(format!("{}: line budget mismatch", spec.name));
        }
        if string(crate_row, "path") != Some(spec.path)
            || integer(crate_row, "wave") != Some(1)
        {
            errors.push(format!("{}: crate registry mismatch", spec.name));
        }
        if string(function, "write_scope") != Some(&format!("{}/**", spec.path)) {
            errors.push(format!("{}: function registry write scope mismatch", spec.name));
        }

        if string(&ticket, "status") != Some("DRAFT_ONLY_NOT_ISSUED")
            || boolean(&ticket, "claimable") != Some(false)
        {
            errors.push(format!("{}: ticket draft became claimable", spec.name));
        }
        if boolean(&ticket, "authorizes_implementation") != Some(false)
            || boolean(&ticket, "creates_lease") != Some(false)
        {
            errors.push(format!("{}: ticket draft creates authority", spec.name));
        }
        if string(&ticket, "stage") != Some("W1")
            || integer(&ticket, "wave") != Some(1)
            || string(&ticket, "phase") != Some(spec.phase)
        {
            errors.push(format!("{}: ticket stage mismatch", spec.name));
        }
        if child_string(&ticket, "repository_fence", "write_scope")
            != Some(&format!("{}/**", spec.path))
        {
            errors.push(format!("{}: ticket write scope mismatch", spec.name));
        }
        if child_string_list(&ticket, "dependencies", "required_handoff_packages")
            != Some(expected_strings(spec.deps))
        {
            errors.push(format!("{}: ticket dependency handoffs mismatch", spec.name));
        }
        if child_string(&ticket, "unresolved_identity", "base_commit") != Some("UNSELECTED")
            || child_string(&ticket, "unresolved_identity", "writer") != Some("UNASSIGNED")
            || child_string(&ticket, "unresolved_identity", "reviewer") != Some("UNASSIGNED")
        {
            errors.push(format!("{}: ticket identity prematurely resolved", spec.name));
        }
        if child_string(&ticket, "stage_prerequisites", "status") != Some("UNAVAILABLE") {
            errors.push(format!("{}: stage prerequisites prematurely accepted", spec.name));
        }

        if string(&context, "status") != Some("UNMATERIALIZED_DRAFT")
            || boolean(&context, "claimable") != Some(false)
        {
            errors.push(format!("{}: context draft became claimable", spec.name));
        }
        if boolean(&context, "authorizes_implementation") != Some(false) {
            errors.push(format!("{}: context draft authorizes implementation", spec.name));
        }
        if string(&context, "stage") != Some("W1")
            || integer(&context, "wave") != Some(1)
            || string(&context, "phase") != Some(spec.phase)
        {
            errors.push(format!("{}: context stage mismatch", spec.name));
        }

        let sources = child_string_list(&context, "content", "source_files").unwrap_or_default();
        let selectors =
            child_string_list(&context, "content", "registry_fragments").unwrap_or_default();
        let slots = child_string_list(&context, "content", "accepted_handoff_slots")
            .unwrap_or_default();
        if integer(&context, "source_file_count") != i64::try_from(sources.len()).ok()
            || sources.len() > 16
        {
            errors.push(format!("{}: source count/ceiling mismatch", spec.name));
        }
        if integer(&context, "registry_fragment_count")
            != i64::try_from(selectors.len()).ok()
            || selectors.len() > 4
        {
            errors.push(format!("{}: selector count/ceiling mismatch", spec.name));
        }
        if integer(&context, "accepted_handoff_slot_count")
            != i64::try_from(slots.len()).ok()
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
        for source in &sources {
            require_regular_file(root, spec.name, Some(source), &mut errors);
            if source.starts_with("docs/architecture/") {
                errors.push(format!("{}: architecture master in context", spec.name));
            }
            if source.contains("/src/") || source.ends_with("/src") {
                errors.push(format!("{}: implementation source in context", spec.name));
            }
        }
        let expected_selectors = vec![
            format!("swarm/crates.toml::package[name={}]", spec.name),
            format!("swarm/function-packets.toml::package[name={}]", spec.name),
            "swarm/stages.toml::stage[id=W1]".to_owned(),
        ];
        if selectors != expected_selectors {
            errors.push(format!("{}: selector set mismatch", spec.name));
        }
    }

    if integer(&ticket_manifest, "draft_count") != Some(7)
        || integer(&ticket_manifest, "issued_ticket_count") != Some(0)
    {
        errors.push("W1 ticket manifest counts invalid".to_owned());
    }
    if integer(&context_manifest, "draft_count") != Some(7)
        || integer(&context_manifest, "materialized_context_count") != Some(0)
    {
        errors.push("W1 context manifest counts invalid".to_owned());
    }
    let case_rows = cases.get("case").and_then(Value::as_array);
    if integer(&cases, "case_count") != Some(20)
        || case_rows.is_none_or(|rows| rows.len() != 20)
    {
        errors.push("qualification case inventory mismatch".to_owned());
    }

    match read_text(root, ".github/workflows/w1-agent-drafts.yml") {
        Ok(workflow) => {
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
        Err(_) => errors.push("missing manual workflow".to_owned()),
    }

    Ok(AgentDraftReport {
        complete: true,
        packages: PACKAGES.len(),
        ticket_drafts: ticket_rows.len(),
        context_drafts: context_rows.len(),
        qualification_cases: case_rows.map_or(0, Vec::len),
        launch_stage: string(&launch, "active_stage").map(str::to_owned),
        launch_wave: integer(&launch, "active_wave"),
        errors,
    })
}

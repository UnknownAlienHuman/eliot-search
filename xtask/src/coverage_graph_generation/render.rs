//! Deterministic coverage manifest and human report rendering.

use super::load::CoverageSnapshot;

pub(super) fn manifest(snapshot: &CoverageSnapshot) -> Result<String, String> {
    let mut text = upsert_after(
        &snapshot.manifest_text,
        "route_assignment_policy",
        "route_assignment_policy = \"reviewed_registry_only\"",
        "status",
    )?;
    text = upsert_after(
        &text,
        "heuristic_route_generation_allowed",
        "heuristic_route_generation_allowed = false",
        "route_assignment_policy",
    )?;
    for (key, value) in [
        ("exact_operation_module_count", snapshot.operations),
        ("documentation_source_file_count", snapshot.documentation_files),
        ("documentation_node_count", snapshot.documentation_nodes),
        ("dependency_edge_count", snapshot.dependency_edges),
        ("logical_module_count", snapshot.modules),
        ("weak_logical_module_count", snapshot.weak_modules.len()),
        ("integration_documentation_node_count", snapshot.governance_nodes),
    ] {
        text = replace_assignment(&text, key, &value.to_string())?;
    }
    Ok(with_terminal_lf(&text))
}

pub(super) fn human_report(snapshot: &CoverageSnapshot) -> String {
    let route_counts = serde_json::to_string_pretty(&snapshot.route_counts)
        .expect("coverage route counts are serializable");
    format!(
        concat!(
            "# Coverage graph v2\n\n",
            "This is the exact machine-checked ownership graph from architecture and package contracts to Cargo\n",
            "packages and package-local logical modules. It does not claim Rust implementation.\n\n",
            "## Closed relations\n\n",
            "- **{packages} Cargo packages** and **{modules} declared logical modules**;\n",
            "- **{operations} package-qualified operations** mapped to exactly one reviewed module in the same package;\n",
            "- **{documentation_nodes} Markdown heading nodes** across **{documentation_files} tracked documentation files**;\n",
            "- **{implementation_nodes} implementation/principle/qualification nodes** mapped to package modules;\n",
            "- **{governance_nodes} governance/navigation nodes** explicitly classified as non-crate-owned rather than forced into a fake product crate;\n",
            "- **{dependency_edges} Cargo dependency edges** mapped from a consumer module to the producer public entry;\n",
            "- **{progressive_edges} later-wave dependency edges** bound to exact progressive stage re-entry records;\n",
            "- **{weak_modules} weak implementation modules** after relation aggregation.\n\n",
            "## Route ownership policy\n\n",
            "Operation, documentation and dependency routes in `swarm/coverage/*.toml` are reviewed machine inputs.\n",
            "The Rust generator reconciles their derived counts and report; it does not guess ownership from names,\n",
            "word similarity or package heuristics. New or changed routes require an explicit reviewed registry change.\n\n",
            "## Operation routing quality\n\n",
            "```text\n{route_counts}\n```\n\n",
            "`public_facade` and `semantic_low` routes are merge-blocking. The committed operation registry records\n",
            "the exact source file, source section, selected module, routing class and score for review.\n\n",
            "## Reconciliation and validation\n\n",
            "```powershell\n",
            "cargo run --locked --quiet -p xtask -- generate coverage-graph --check --json\n",
            "cargo run --locked --quiet -p xtask -- generate package-maps --check --json\n",
            "cargo run --locked --quiet -p xtask -- validate coverage-graph --json\n",
            "cargo run --locked --quiet -p xtask -- validate package-maps --json\n",
            "cargo run --locked --quiet -p xtask -- validate architecture-coverage --json\n",
            "cargo run --locked --quiet -p xtask -- validate architecture-coverage-contracts --json\n",
            "```\n\n",
            "The validators reject missing or orphan operations, stale documentation headings, cross-package module\n",
            "routes, configuration/recipe/port owner drift, missing dependency or re-entry edges, weak implementation\n",
            "modules and any automatic trigger in the permanent validation workflow.\n\n",
            "## Authority ceiling\n\n",
            "These registries are design/ownership evidence only. They create no ticket, lease, accepted package\n",
            "handoff, gate receipt, wave receipt or implementation authority. Launch state remains P00/W0.\n"
        ),
        packages = snapshot.packages,
        modules = snapshot.modules,
        operations = snapshot.operations,
        documentation_nodes = snapshot.documentation_nodes,
        documentation_files = snapshot.documentation_files,
        implementation_nodes = snapshot.implementation_nodes,
        governance_nodes = snapshot.governance_nodes,
        dependency_edges = snapshot.dependency_edges,
        progressive_edges = snapshot.progressive_edges,
        weak_modules = snapshot.weak_modules.len(),
        route_counts = route_counts,
    )
}

fn replace_assignment(text: &str, key: &str, value: &str) -> Result<String, String> {
    let prefix = format!("{key} = ");
    let mut replacements = 0_usize;
    let mut lines = Vec::new();
    for line in text.lines() {
        if line.starts_with(&prefix) {
            replacements = replacements.saturating_add(1);
            lines.push(format!("{key} = {value}"));
        } else {
            lines.push(line.to_owned());
        }
    }
    if replacements != 1 {
        return Err(format!(
            "coverage manifest {key}: expected one assignment, found {replacements}"
        ));
    }
    Ok(lines.join("\n"))
}

fn upsert_after(
    text: &str,
    key: &str,
    assignment: &str,
    after_key: &str,
) -> Result<String, String> {
    let prefix = format!("{key} = ");
    let count = text.lines().filter(|line| line.starts_with(&prefix)).count();
    if count > 1 {
        return Err(format!(
            "coverage manifest {key}: expected at most one assignment, found {count}"
        ));
    }
    if count == 1 {
        let value = assignment
            .split_once(" = ")
            .map_or("", |(_, value)| value);
        return replace_assignment(text, key, value);
    }

    let after_prefix = format!("{after_key} = ");
    let mut inserted = false;
    let mut lines = Vec::new();
    for line in text.lines() {
        lines.push(line.to_owned());
        if line.starts_with(&after_prefix) {
            if inserted {
                return Err(format!(
                    "coverage manifest {after_key}: multiple insertion anchors"
                ));
            }
            lines.push(assignment.to_owned());
            inserted = true;
        }
    }
    if !inserted {
        return Err(format!(
            "coverage manifest {after_key}: insertion anchor missing"
        ));
    }
    Ok(lines.join("\n"))
}

fn with_terminal_lf(text: &str) -> String {
    let mut value = text.trim_end_matches(&['\r', '\n'][..]).to_owned();
    value.push('\n');
    value
}

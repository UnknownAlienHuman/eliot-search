//! Global package-map indexes and human navigation rendering.

use std::collections::{BTreeMap, BTreeSet};

use super::{array, finish, quote};
use super::super::model::{PackageMapModel, PackageModel};
use super::super::super::{DOC_INDEX_PATH, INDEX_PATH, INTEGRATION_PATH, bool_text};
use super::super::super::load::{integer, string, strings};

pub(super) fn render_integration(model: &PackageMapModel) -> String {
    let mut lines = vec![
        "schema_version = 1".to_owned(),
        "project = \"eliot-search\"".to_owned(),
        "status = \"EXPLICIT_NON_CRATE_DOCUMENTATION_MAP\"".to_owned(),
        format!("node_count = {}", model.integration.len()),
        "product_semantics_allowed = false".to_owned(),
        "implementation_authorized_by_this_map = false".to_owned(),
        String::new(),
    ];
    for row in &model.integration {
        lines.extend([
            "[[node]]".to_owned(),
            format!("id = {}", quote(string(row, "id").unwrap_or(""))),
            format!("path = {}", quote(string(row, "path").unwrap_or(""))),
            format!("line = {}", integer(row, "line").unwrap_or(0)),
            format!(
                "heading = {}",
                quote(string(row, "heading").unwrap_or(""))
            ),
            format!("kind = {}", quote(string(row, "kind").unwrap_or(""))),
            format!(
                "route_kind = {}",
                quote(string(row, "route_kind").unwrap_or(""))
            ),
            format!(
                "rationale = {}",
                quote(string(row, "rationale").unwrap_or(""))
            ),
            String::new(),
        ]);
    }
    finish(lines)
}

pub(super) fn render_document_index(model: &PackageMapModel) -> String {
    let mut lines = vec![
        "schema_version = 1".to_owned(),
        "project = \"eliot-search\"".to_owned(),
        "status = \"DOCUMENTATION_FILE_REVERSE_INDEX_NOT_IMPLEMENTED\"".to_owned(),
        format!("file_count = {}", model.documents_by_path.len()),
        format!("node_count = {}", model.stats.documents),
        "every_tracked_markdown_file_indexed = true".to_owned(),
        "implementation_authorized_by_this_index = false".to_owned(),
        String::new(),
    ];
    for (path, nodes) in &model.documents_by_path {
        let packages: BTreeSet<String> = nodes
            .iter()
            .flat_map(|node| strings(node, "packages"))
            .collect();
        let modules: BTreeSet<String> = nodes
            .iter()
            .flat_map(|node| strings(node, "modules"))
            .collect();
        let package_values: Vec<String> = packages.into_iter().collect();
        let module_values: Vec<String> = modules.into_iter().collect();
        let principles = nodes
            .iter()
            .filter(|node| string(node, "kind") == Some("principle_or_invariant"))
            .count();
        lines.extend([
            "[[file]]".to_owned(),
            format!("path = {}", quote(path)),
            format!("node_count = {}", nodes.len()),
            format!("principle_count = {principles}"),
            format!("packages = {}", array(&package_values)),
            format!("modules = {}", array(&module_values)),
            format!("non_crate_only = {}", bool_text(package_values.is_empty())),
            String::new(),
        ]);
    }
    finish(lines)
}

pub(super) fn render_package_index(
    model: &PackageMapModel,
    outputs: &BTreeMap<String, String>,
) -> String {
    let mut lines = vec![
        "schema_version = 1".to_owned(),
        "project = \"eliot-search\"".to_owned(),
        "status = \"BOUNDED_PACKAGE_MAP_INDEX_NOT_IMPLEMENTED\"".to_owned(),
        format!("package_count = {}", model.stats.packages),
        format!("map_file_count = {}", model.stats.map_files),
        format!("operation_count = {}", model.stats.operations),
        format!("documentation_node_count = {}", model.stats.documents),
        format!("dependency_edge_count = {}", model.stats.dependencies),
        format!("logical_module_count = {}", model.stats.modules),
        format!("integration_node_count = {}", model.stats.integration_nodes),
        format!("documentation_file_index = {}", quote(DOC_INDEX_PATH)),
        format!("integration_map = {}", quote(INTEGRATION_PATH)),
        "one_agent_reads_one_package_map = true".to_owned(),
        "global_architecture_read_required = false".to_owned(),
        "implementation_authorized_by_this_index = false".to_owned(),
        String::new(),
    ];
    for package in &model.packages {
        render_package_index_row(package, outputs, &mut lines);
    }
    finish(lines)
}

fn render_package_index_row(
    package: &PackageModel,
    outputs: &BTreeMap<String, String>,
    lines: &mut Vec<String>,
) {
    lines.extend([
        "[[package]]".to_owned(),
        format!("name = {}", quote(&package.name)),
        format!(
            "path = {}",
            quote(string(&package.row, "path").unwrap_or("None"))
        ),
        format!("wave = {}", integer(&package.row, "wave").unwrap_or(0)),
        format!(
            "family = {}",
            quote(string(&package.row, "family").unwrap_or("None"))
        ),
    ]);
    for (key, path) in [
        ("overview", package.paths.overview.as_str()),
        ("operations", package.paths.operations.as_str()),
        ("documents", package.paths.documents.as_str()),
        ("relations", package.paths.relations.as_str()),
    ] {
        lines.push(format!("{key}_map = {}", quote(path)));
        let digest = crate::coverage_graph::digest_text(
            outputs.get(path).map_or("", String::as_str),
        );
        lines.push(format!("{key}_sha256 = {}", quote(&digest)));
    }
    let counts = package.counts;
    for (key, value) in [
        ("modules", counts.modules),
        ("operations", counts.operations),
        ("documents", counts.documents),
        ("principles", counts.principles),
        ("outbound_dependencies", counts.outbound_dependencies),
        ("inbound_dependencies", counts.inbound_dependencies),
        ("architecture", counts.architecture),
        ("configuration", counts.configuration),
        ("recipes", counts.recipes),
        ("ports", counts.ports),
        ("schemas", counts.schemas),
    ] {
        lines.push(format!("{key}_count = {value}"));
    }
    lines.push(String::new());
}

pub(super) fn render_human_index(model: &PackageMapModel) -> String {
    let mut lines = vec![
        "# Bounded package map index v2".to_owned(),
        String::new(),
        "This index is the Swarm entry point after a package is assigned. A package writer reads only:".to_owned(),
        String::new(),
        "1. its assignment and issued context bundle;".to_owned(),
        "2. `overview.toml`;".to_owned(),
        "3. the package-local operation, documentation and relation maps linked by the overview;".to_owned(),
        "4. exact accepted dependency handoffs named by the relation map.".to_owned(),
        String::new(),
        "The maps do not authorize implementation and do not replace an issued ticket or lease.".to_owned(),
        String::new(),
        "| Package | Wave | Modules | Operations | Doc nodes | Dependencies | Map |".to_owned(),
        "|---|---:|---:|---:|---:|---:|---|".to_owned(),
    ];
    for package in &model.packages {
        lines.push(format!(
            "| `{}` | {} | {} | {} | {} | {} | [`overview`](/swarm/coverage/package-maps/{}/overview.toml) |",
            package.name,
            integer(&package.row, "wave").unwrap_or(0),
            package.counts.modules,
            package.counts.operations,
            package.counts.documents,
            package.counts.outbound_dependencies,
            package.name,
        ));
    }
    lines.extend([
        String::new(),
        "## Global reverse indexes".to_owned(),
        String::new(),
        format!("- `{INDEX_PATH}` — package-to-map index with exact digests."),
        format!("- `{DOC_INDEX_PATH}` — documentation file to package/module reverse index."),
        format!("- `{INTEGRATION_PATH}` — governance/navigation nodes explicitly outside product crates."),
        "- `swarm/coverage/operation-modules.toml` — operation to exact package-local module.".to_owned(),
        "- `swarm/coverage/dependency-edges.toml` — typed package/module dependency edges.".to_owned(),
        "- `swarm/coverage/module-coverage.toml` — reverse relation counts and structural roles.".to_owned(),
    ]);
    finish(lines)
}

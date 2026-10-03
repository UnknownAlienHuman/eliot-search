//! Per-package documentation-node renderer.

use super::super::{array, finish, quote};
use super::super::super::model::PackageModel;
use super::super::super::super::load::{integer, string, strings};

pub(super) fn render_documents(package: &PackageModel) -> String {
    let mut lines = vec![
        "schema_version = 1".to_owned(),
        "project = \"eliot-search\"".to_owned(),
        "status = \"PACKAGE_DOCUMENT_NODE_MAP_NOT_IMPLEMENTED\"".to_owned(),
        format!("package = {}", quote(&package.name)),
        format!("node_count = {}", package.documents.len()),
        format!("principle_count = {}", package.counts.principles),
        "every_node_has_package_local_module_route = true".to_owned(),
        "implementation_authorized_by_this_map = false".to_owned(),
        String::new(),
    ];
    let prefix = format!("{}:", package.name);
    for row in &package.documents {
        let mut modules: Vec<String> = strings(row, "modules")
            .into_iter()
            .filter(|module| module.starts_with(&prefix))
            .collect();
        modules.sort();
        lines.extend([
            "[[node]]".to_owned(),
            format!("id = {}", quote(string(row, "id").unwrap_or(""))),
            format!("path = {}", quote(string(row, "path").unwrap_or(""))),
            format!("line = {}", integer(row, "line").unwrap_or(0)),
            format!("level = {}", integer(row, "level").unwrap_or(0)),
            format!(
                "heading = {}",
                quote(string(row, "heading").unwrap_or(""))
            ),
            format!("kind = {}", quote(string(row, "kind").unwrap_or(""))),
            format!("modules = {}", array(&modules)),
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

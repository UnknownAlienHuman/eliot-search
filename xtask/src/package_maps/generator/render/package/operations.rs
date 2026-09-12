//! Per-package operation renderer.

use super::super::{array, finish, quote};
use super::super::super::model::PackageModel;
use super::super::super::super::load::{integer, string, strings};

pub(super) fn render_operations(package: &PackageModel) -> String {
    let mut lines = vec![
        "schema_version = 1".to_owned(),
        "project = \"eliot-search\"".to_owned(),
        "status = \"EXACT_PACKAGE_OPERATION_MAP_NOT_IMPLEMENTED\"".to_owned(),
        format!("package = {}", quote(&package.name)),
        format!("operation_count = {}", package.operations.len()),
        "one_internal_module_per_operation = true".to_owned(),
        "public_facade_or_low_confidence_routes_allowed = false".to_owned(),
        "implementation_authorized_by_this_map = false".to_owned(),
        String::new(),
    ];
    for row in &package.operations {
        lines.extend([
            "[[operation]]".to_owned(),
            format!("id = {}", quote(string(row, "id").unwrap_or(""))),
            format!(
                "name = {}",
                quote(string(row, "operation").unwrap_or(""))
            ),
            format!("module = {}", quote(string(row, "module").unwrap_or(""))),
            format!(
                "public_entry_module = {}",
                quote(string(row, "public_entry_module").unwrap_or(""))
            ),
            format!("sources = {}", array(&strings(row, "sources"))),
            format!(
                "source_contexts = {}",
                array(&strings(row, "source_contexts"))
            ),
            format!(
                "route_kind = {}",
                quote(string(row, "route_kind").unwrap_or(""))
            ),
            format!("score = {}", integer(row, "score").unwrap_or(0)),
            String::new(),
        ]);
    }
    finish(lines)
}

//! Per-package overview renderer.

use super::super::{array, finish, quote};
use super::super::super::model::PackageModel;
use super::super::super::super::load::{boolean, integer, string, strings};
use super::super::super::super::bool_text;

pub(super) fn render_overview(package: &PackageModel) -> String {
    let row = &package.row;
    let counts = package.counts;
    let mut lines = vec![
        "schema_version = 1".to_owned(),
        "project = \"eliot-search\"".to_owned(),
        "status = \"BOUNDED_PACKAGE_MAP_NOT_IMPLEMENTED\"".to_owned(),
        format!("package = {}", quote(&package.name)),
        format!("path = {}", quote(string(row, "path").unwrap_or("None"))),
        format!("kind = {}", quote(string(row, "kind").unwrap_or("None"))),
        format!("family = {}", quote(string(row, "family").unwrap_or("None"))),
        format!("cell = {}", quote(string(row, "cell").unwrap_or("None"))),
        format!("earliest_wave = {}", integer(row, "wave").unwrap_or(0)),
        format!("optional = {}", bool_text(boolean(row, "optional").unwrap_or(false))),
        format!(
            "soft_src_line_target = {}",
            integer(row, "soft_src_line_target").unwrap_or(0)
        ),
        format!(
            "assignment = {}",
            quote(string(row, "assignment").unwrap_or("None"))
        ),
        format!(
            "function_source = {}",
            quote(string(row, "functions").unwrap_or("FOUNDATION_CONTRACT"))
        ),
        format!(
            "qualification = {}",
            quote(string(row, "qualification").unwrap_or("NONE"))
        ),
        format!("config_sections = {}", array(&strings(row, "config_sections"))),
        format!("declared_dependencies = {}", array(&strings(row, "deps"))),
        format!("module_count = {}", counts.modules),
        format!("operation_count = {}", counts.operations),
        format!("documentation_node_count = {}", counts.documents),
        format!("principle_node_count = {}", counts.principles),
        format!(
            "outbound_dependency_count = {}",
            counts.outbound_dependencies
        ),
        format!("inbound_dependency_count = {}", counts.inbound_dependencies),
        format!("architecture_relation_count = {}", counts.architecture),
        format!("configuration_relation_count = {}", counts.configuration),
        format!("recipe_relation_count = {}", counts.recipes),
        format!("port_relation_count = {}", counts.ports),
        format!("schema_relation_count = {}", counts.schemas),
        format!("operations_map = {}", quote(&package.paths.operations)),
        format!("documents_map = {}", quote(&package.paths.documents)),
        format!("relations_map = {}", quote(&package.paths.relations)),
        "one_agent_one_package = true".to_owned(),
        "cross_package_reads_require_public_handoff = true".to_owned(),
        "implementation_authorized_by_this_map = false".to_owned(),
        String::new(),
    ];
    for module in &package.modules {
        lines.extend([
            "[[module]]".to_owned(),
            format!("name = {}", quote(string(module, "module").unwrap_or(""))),
            format!("role = {}", quote(string(module, "role").unwrap_or(""))),
            format!(
                "structural_rationale = {}",
                quote(string(module, "structural_rationale").unwrap_or(""))
            ),
            format!(
                "operation_count = {}",
                integer(module, "operation_count").unwrap_or(0)
            ),
            format!(
                "documentation_node_count = {}",
                integer(module, "documentation_node_count").unwrap_or(0)
            ),
            format!(
                "architecture_relation_count = {}",
                integer(module, "architecture_relation_count").unwrap_or(0)
            ),
            format!(
                "port_relation_count = {}",
                integer(module, "port_relation_count").unwrap_or(0)
            ),
            format!(
                "port_method_relation_count = {}",
                integer(module, "port_method_relation_count").unwrap_or(0)
            ),
            format!(
                "schema_relation_count = {}",
                integer(module, "schema_relation_count").unwrap_or(0)
            ),
            format!(
                "configuration_relation_count = {}",
                integer(module, "configuration_relation_count").unwrap_or(0)
            ),
            format!(
                "recipe_relation_count = {}",
                integer(module, "recipe_relation_count").unwrap_or(0)
            ),
            format!(
                "dependency_relation_count = {}",
                integer(module, "dependency_relation_count").unwrap_or(0)
            ),
            String::new(),
        ]);
    }
    finish(lines)
}

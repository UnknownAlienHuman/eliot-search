//! Per-package relation renderer.

use super::super::{array, finish, quote};
use super::super::super::model::PackageModel;
use super::super::super::super::bool_text;
use super::super::super::super::load::{boolean, integer, string, strings};

pub(super) fn render_relations(package: &PackageModel) -> String {
    let mut lines = vec![
        "schema_version = 1".to_owned(),
        "project = \"eliot-search\"".to_owned(),
        "status = \"PACKAGE_RELATION_MAP_NOT_IMPLEMENTED\"".to_owned(),
        format!("package = {}", quote(&package.name)),
        format!(
            "outbound_dependency_count = {}",
            package.outbound.len()
        ),
        format!("inbound_dependency_count = {}", package.inbound.len()),
        format!(
            "architecture_relation_count = {}",
            package.architecture.len()
        ),
        format!(
            "configuration_relation_count = {}",
            package.configurations.len()
        ),
        format!("recipe_relation_count = {}", package.recipes.len()),
        format!("port_relation_count = {}", package.ports.len()),
        format!("schema_relation_count = {}", package.schemas.len()),
        "dependency_targets_enter_through_public_entry = true".to_owned(),
        "implementation_authorized_by_this_map = false".to_owned(),
        String::new(),
    ];
    for (direction, values) in [
        ("outbound", package.outbound.as_slice()),
        ("inbound", package.inbound.as_slice()),
    ] {
        for row in values {
            lines.extend([
                "[[dependency]]".to_owned(),
                format!("direction = {}", quote(direction)),
                format!("id = {}", quote(string(row, "id").unwrap_or(""))),
                format!(
                    "consumer = {}",
                    quote(string(row, "consumer").unwrap_or(""))
                ),
                format!(
                    "consumer_module = {}",
                    quote(string(row, "consumer_module").unwrap_or(""))
                ),
                format!(
                    "consumer_earliest_wave = {}",
                    integer(row, "consumer_earliest_wave").unwrap_or(0)
                ),
                format!(
                    "producer = {}",
                    quote(string(row, "producer").unwrap_or(""))
                ),
                format!(
                    "producer_module = {}",
                    quote(string(row, "producer_module").unwrap_or(""))
                ),
                format!(
                    "producer_earliest_wave = {}",
                    integer(row, "producer_earliest_wave").unwrap_or(0)
                ),
                format!(
                    "relationship = {}",
                    quote(string(row, "relationship").unwrap_or(""))
                ),
                format!(
                    "contract_source = {}",
                    quote(string(row, "contract_source").unwrap_or(""))
                ),
                format!(
                    "requires_stage_reentry = {}",
                    bool_text(boolean(row, "requires_stage_reentry").unwrap_or(false))
                ),
                format!(
                    "reentry_stage = {}",
                    quote(string(row, "reentry_stage").unwrap_or(""))
                ),
                format!(
                    "exact_accepted_handoff_required = {}",
                    bool_text(
                        boolean(row, "exact_accepted_handoff_required")
                            .unwrap_or(false)
                    )
                ),
                String::new(),
            ]);
        }
    }

    let prefix = format!("{}:", package.name);
    for relation in &package.architecture {
        let mut modules: Vec<String> = relation
            .modules
            .iter()
            .filter(|module| module.starts_with(&prefix))
            .cloned()
            .collect();
        modules.sort();
        lines.extend([
            "[[architecture]]".to_owned(),
            format!("kind = {}", quote(&relation.kind)),
            format!("id = {}", quote(&relation.id)),
            format!("modules = {}", array(&modules)),
            format!(
                "required_outputs = {}",
                array(&relation.required_outputs)
            ),
            format!("exit_evidence = {}", array(&relation.exit_evidence)),
            String::new(),
        ]);
    }

    for (name, row) in &package.configurations {
        lines.extend([
            "[[configuration]]".to_owned(),
            format!("section = {}", quote(name)),
            format!(
                "module = {}",
                quote(string(row, "owner_module").unwrap_or(""))
            ),
            format!(
                "contract = {}",
                quote(string(row, "contract").unwrap_or(""))
            ),
            format!(
                "reload = {}",
                quote(string(row, "reload").unwrap_or("None"))
            ),
            String::new(),
        ]);
    }

    for recipe in &package.recipes {
        lines.extend([
            "[[recipe]]".to_owned(),
            format!("id = {}", quote(&recipe.id)),
            format!("module = {}", quote(&recipe.module)),
            format!("request_schema = {}", quote(&recipe.request_schema)),
            format!("result_schema = {}", quote(&recipe.result_schema)),
            String::new(),
        ]);
    }

    for port in &package.ports {
        lines.extend([
            "[[port]]".to_owned(),
            format!("name = {}", quote(string(port, "name").unwrap_or(""))),
            format!(
                "module = {}",
                quote(string(port, "implementation_module").unwrap_or("None"))
            ),
            format!("methods = {}", array(&strings(port, "methods"))),
            format!(
                "method_modules = {}",
                array(&strings(port, "method_modules"))
            ),
            String::new(),
        ]);
    }

    for schema in &package.schemas {
        lines.extend([
            "[[schema_group]]".to_owned(),
            format!("packet = {}", quote(&schema.packet)),
            format!("group = {}", quote(&schema.group)),
            format!("owner_roles = {}", array(&schema.owner_roles)),
            format!("modules = {}", array(&schema.modules)),
            format!("schemas = {}", array(&schema.schemas)),
            format!("source_files = {}", array(&schema.source_files)),
            String::new(),
        ]);
    }
    finish(lines)
}

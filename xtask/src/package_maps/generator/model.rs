//! Canonical in-memory package-map model derived from checked-in registries.

use std::collections::BTreeMap;

use toml::Value;

use super::super::load::{
    ArchitectureRelation, Inputs, SchemaOwner, SchemaRelation, string, strings,
};
use super::super::{PackagePaths, dependency_cycle, package_paths};

#[derive(Clone, Debug)]
pub(super) struct RecipeModel {
    pub(super) id: String,
    pub(super) module: String,
    pub(super) request_schema: String,
    pub(super) result_schema: String,
}

#[derive(Clone, Debug)]
pub(super) struct SchemaModel {
    pub(super) packet: String,
    pub(super) group: String,
    pub(super) owner_roles: Vec<String>,
    pub(super) modules: Vec<String>,
    pub(super) schemas: Vec<String>,
    pub(super) source_files: Vec<String>,
}

#[derive(Clone, Copy, Debug, Default)]
pub(super) struct PackageCounts {
    pub(super) modules: usize,
    pub(super) operations: usize,
    pub(super) documents: usize,
    pub(super) principles: usize,
    pub(super) outbound_dependencies: usize,
    pub(super) inbound_dependencies: usize,
    pub(super) architecture: usize,
    pub(super) configuration: usize,
    pub(super) recipes: usize,
    pub(super) ports: usize,
    pub(super) schemas: usize,
}

#[derive(Clone, Debug)]
pub(super) struct PackageModel {
    pub(super) name: String,
    pub(super) row: Value,
    pub(super) paths: PackagePaths,
    pub(super) modules: Vec<Value>,
    pub(super) operations: Vec<Value>,
    pub(super) documents: Vec<Value>,
    pub(super) outbound: Vec<Value>,
    pub(super) inbound: Vec<Value>,
    pub(super) architecture: Vec<ArchitectureRelation>,
    pub(super) configurations: Vec<(String, Value)>,
    pub(super) recipes: Vec<RecipeModel>,
    pub(super) ports: Vec<Value>,
    pub(super) schemas: Vec<SchemaModel>,
    pub(super) counts: PackageCounts,
}

#[derive(Clone, Debug)]
pub(super) struct GenerationStats {
    pub(super) packages: usize,
    pub(super) map_files: usize,
    pub(super) operations: usize,
    pub(super) documents: usize,
    pub(super) modules: usize,
    pub(super) dependencies: usize,
    pub(super) integration_nodes: usize,
    pub(super) cycle: Vec<String>,
}

#[derive(Clone, Debug)]
pub(super) struct PackageMapModel {
    pub(super) packages: Vec<PackageModel>,
    pub(super) integration: Vec<Value>,
    pub(super) documents_by_path: BTreeMap<String, Vec<Value>>,
    pub(super) stats: GenerationStats,
}

impl PackageMapModel {
    pub(super) fn derive(inputs: &Inputs) -> Self {
        let mut packages = Vec::new();
        for (name, row) in &inputs.package_rows {
            let modules: Vec<Value> = inputs
                .module_values
                .iter()
                .filter(|module| string(module, "package") == Some(name))
                .cloned()
                .collect();
            let operations: Vec<Value> = inputs
                .operation_values
                .iter()
                .filter(|operation| string(operation, "package") == Some(name))
                .cloned()
                .collect();
            let documents: Vec<Value> = inputs
                .document_values
                .iter()
                .filter(|document| {
                    strings(document, "packages").iter().any(|package| package == name)
                })
                .cloned()
                .collect();
            let outbound: Vec<Value> = inputs
                .dependency_values
                .iter()
                .filter(|edge| string(edge, "consumer") == Some(name))
                .cloned()
                .collect();
            let inbound: Vec<Value> = inputs
                .dependency_values
                .iter()
                .filter(|edge| string(edge, "producer") == Some(name))
                .cloned()
                .collect();
            let prefix = format!("{name}:");
            let architecture: Vec<ArchitectureRelation> = inputs
                .architecture
                .iter()
                .filter(|relation| {
                    relation.modules.iter().any(|module| module.starts_with(&prefix))
                })
                .cloned()
                .collect();
            let configurations: Vec<(String, Value)> = inputs
                .config_values
                .iter()
                .filter(|section| string(section, "owner") == Some(name))
                .filter_map(|section| {
                    string(section, "name")
                        .map(|section_name| (section_name.to_owned(), section.clone()))
                })
                .collect();
            let recipes = derive_recipes(inputs, name);
            let ports: Vec<Value> = inputs
                .port_values
                .iter()
                .filter(|port| string(port, "implementation_package") == Some(name))
                .cloned()
                .collect();
            let schemas = derive_schemas(&inputs.schemas, name);
            let counts = PackageCounts {
                modules: modules.len(),
                operations: operations.len(),
                documents: documents.len(),
                principles: documents
                    .iter()
                    .filter(|document| {
                        string(document, "kind") == Some("principle_or_invariant")
                    })
                    .count(),
                outbound_dependencies: outbound.len(),
                inbound_dependencies: inbound.len(),
                architecture: architecture.len(),
                configuration: configurations.len(),
                recipes: recipes.len(),
                ports: ports.len(),
                schemas: schemas.len(),
            };
            packages.push(PackageModel {
                name: name.clone(),
                row: row.clone(),
                paths: package_paths(name),
                modules,
                operations,
                documents,
                outbound,
                inbound,
                architecture,
                configurations,
                recipes,
                ports,
                schemas,
                counts,
            });
        }

        let integration: Vec<Value> = inputs
            .document_values
            .iter()
            .filter(|document| strings(document, "packages").is_empty())
            .cloned()
            .collect();
        let mut documents_by_path: BTreeMap<String, Vec<Value>> = BTreeMap::new();
        for document in &inputs.document_values {
            if let Some(path) = string(document, "path") {
                documents_by_path
                    .entry(path.to_owned())
                    .or_default()
                    .push(document.clone());
            }
        }
        let dependency_map: BTreeMap<String, Vec<String>> = inputs
            .package_rows
            .iter()
            .map(|(package, row)| (package.clone(), strings(row, "deps")))
            .collect();
        let stats = GenerationStats {
            packages: packages.len(),
            map_files: packages.len().saturating_mul(4),
            operations: inputs.operation_values.len(),
            documents: inputs.document_values.len(),
            modules: inputs.module_values.len(),
            dependencies: inputs.dependency_values.len(),
            integration_nodes: integration.len(),
            cycle: dependency_cycle(&dependency_map),
        };
        Self {
            packages,
            integration,
            documents_by_path,
            stats,
        }
    }
}

fn derive_recipes(inputs: &Inputs, package: &str) -> Vec<RecipeModel> {
    let prefix = format!("{package}:");
    let mut result = Vec::new();
    for recipe in &inputs.recipe_values {
        let Some(id) = string(recipe, "id") else {
            continue;
        };
        for reference in strings(recipe, "execution_modules") {
            let Some(module) = reference.strip_prefix(&prefix) else {
                continue;
            };
            result.push(RecipeModel {
                id: id.to_owned(),
                module: module.to_owned(),
                request_schema: string(recipe, "request_schema")
                    .unwrap_or("None")
                    .to_owned(),
                result_schema: string(recipe, "result_schema")
                    .unwrap_or("None")
                    .to_owned(),
            });
        }
    }
    result
}

fn derive_schemas(relations: &[SchemaRelation], package: &str) -> Vec<SchemaModel> {
    let mut result = Vec::new();
    for relation in relations {
        let owners: Vec<&SchemaOwner> = relation
            .owners
            .iter()
            .filter(|owner| owner.package == package)
            .collect();
        if owners.is_empty() {
            continue;
        }
        let mut owner_roles: Vec<String> =
            owners.iter().map(|owner| owner.kind.clone()).collect();
        let mut modules: Vec<String> =
            owners.iter().map(|owner| owner.module.clone()).collect();
        owner_roles.sort();
        modules.sort();
        modules.dedup();
        result.push(SchemaModel {
            packet: relation.packet.clone(),
            group: relation.group.clone(),
            owner_roles,
            modules,
            schemas: relation.schemas.clone(),
            source_files: relation.source_files.clone(),
        });
    }
    result
}

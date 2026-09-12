//! Loading and counting reviewed coverage graph registries.

use std::collections::{BTreeMap, BTreeSet};
use std::fs;
use std::path::Path;

use toml::Value;

pub(super) const MANIFEST_PATH: &str = "swarm/coverage/manifest.toml";
pub(super) const HUMAN_REPORT_PATH: &str = "docs/handoff/COVERAGE_GRAPH_V2.md";

pub(super) struct CoverageSnapshot {
    pub(super) manifest_text: String,
    pub(super) packages: usize,
    pub(super) operations: usize,
    pub(super) documentation_files: usize,
    pub(super) documentation_nodes: usize,
    pub(super) implementation_nodes: usize,
    pub(super) governance_nodes: usize,
    pub(super) dependency_edges: usize,
    pub(super) progressive_edges: usize,
    pub(super) modules: usize,
    pub(super) weak_modules: Vec<String>,
    pub(super) route_counts: BTreeMap<String, usize>,
    pub(super) derivation_errors: Vec<String>,
}

impl CoverageSnapshot {
    pub(super) fn load(root: &Path) -> Result<Self, String> {
        let manifest_text = read_text(root, MANIFEST_PATH)?;
        let manifest: Value = toml::from_str(&manifest_text)
            .map_err(|error| format!("{MANIFEST_PATH}: {error}"))?;
        let package_document = load_toml(
            root,
            manifest_path(&manifest, "package_registry", "swarm/crates.toml"),
        )?;
        let operation_document = load_toml(
            root,
            manifest_path(
                &manifest,
                "operation_module_registry",
                "swarm/coverage/operation-modules.toml",
            ),
        )?;
        let documentation_document = load_toml(
            root,
            manifest_path(
                &manifest,
                "documentation_node_registry",
                "swarm/coverage/documentation-nodes.toml",
            ),
        )?;
        let dependency_document = load_toml(
            root,
            manifest_path(
                &manifest,
                "dependency_edge_registry",
                "swarm/coverage/dependency-edges.toml",
            ),
        )?;
        let module_document = load_toml(
            root,
            manifest_path(
                &manifest,
                "module_coverage_registry",
                "swarm/coverage/module-coverage.toml",
            ),
        )?;

        let package_rows = rows(&package_document, "package")?;
        let operation_rows = rows(&operation_document, "operation")?;
        let documentation_rows = rows(&documentation_document, "node")?;
        let dependency_rows = rows(&dependency_document, "edge")?;
        let module_rows = rows(&module_document, "module")?;

        let mut route_counts = BTreeMap::new();
        for row in operation_rows {
            let route = string(row, "route_kind").unwrap_or("<missing>");
            *route_counts.entry(route.to_owned()).or_insert(0) += 1;
        }

        let mut documentation_files = BTreeSet::new();
        let mut governance_nodes = 0_usize;
        for row in documentation_rows {
            if let Some(path) = string(row, "path") {
                documentation_files.insert(path.to_owned());
            }
            if matches!(
                string(row, "kind"),
                Some("governance") | Some("navigation")
            ) {
                governance_nodes = governance_nodes.saturating_add(1);
            }
        }
        let documentation_nodes = documentation_rows.len();
        let implementation_nodes = documentation_nodes.saturating_sub(governance_nodes);
        let progressive_edges = dependency_rows
            .iter()
            .filter(|row| boolean(row, "requires_stage_reentry") == Some(true))
            .count();
        let mut weak_modules: Vec<String> = module_rows
            .iter()
            .filter(|row| boolean(row, "weakly_covered") == Some(true))
            .filter_map(|row| string(row, "id").map(str::to_owned))
            .collect();
        weak_modules.sort();
        let derivation_errors = super::derive::validate(
            root,
            &manifest,
            &package_document,
            &operation_document,
            &documentation_document,
            &dependency_document,
            &module_document,
        );

        Ok(Self {
            manifest_text,
            packages: package_rows.len(),
            operations: operation_rows.len(),
            documentation_files: documentation_files.len(),
            documentation_nodes,
            implementation_nodes,
            governance_nodes,
            dependency_edges: dependency_rows.len(),
            progressive_edges,
            modules: module_rows.len(),
            weak_modules,
            route_counts,
            derivation_errors,
        })
    }
}

fn load_toml(root: &Path, relative: &str) -> Result<Value, String> {
    let text = read_text(root, relative)?;
    toml::from_str(&text).map_err(|error| format!("{relative}: {error}"))
}

fn read_text(root: &Path, relative: &str) -> Result<String, String> {
    fs::read_to_string(root.join(relative))
        .map_err(|error| format!("{relative}: {error}"))
}

fn manifest_path<'a>(manifest: &'a Value, key: &str, fallback: &'a str) -> &'a str {
    string(manifest, key).unwrap_or(fallback)
}

fn rows<'a>(document: &'a Value, key: &str) -> Result<&'a [Value], String> {
    document
        .get(key)
        .and_then(Value::as_array)
        .map(Vec::as_slice)
        .ok_or_else(|| format!("{key} must be an array of tables"))
}

fn string<'a>(value: &'a Value, key: &str) -> Option<&'a str> {
    value.get(key).and_then(Value::as_str)
}

fn boolean(value: &Value, key: &str) -> Option<bool> {
    value.get(key).and_then(Value::as_bool)
}

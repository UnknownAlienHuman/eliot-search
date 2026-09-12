//! Rust ownership for bounded package-map helpers and validation.
//!
//! Package maps consume the canonical checked-in coverage registries. They do
//! not rebuild a second architecture graph and never issue implementation,
//! ticket, lease, handoff, gate, wave, or product authority.

mod helpers;
mod load;
mod validation;

use std::path::Path;

use serde_json::json;

pub use helpers::{
    DOC_INDEX_PATH, HUMAN_INDEX_PATH, INDEX_PATH, INTEGRATION_PATH, MAP_ROOT,
    PackagePaths, bool_text, dependency_cycle, package_paths,
    stale_package_files, string_list,
};

/// Stable report for the package-map closure validator.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct PackageMapsReport {
    /// Whether all mandatory inputs loaded.
    pub complete: bool,
    /// Registered product packages.
    pub packages: usize,
    /// Expected package-local map files.
    pub package_map_files: usize,
    /// Canonical logical modules.
    pub logical_modules: usize,
    /// Canonical operations.
    pub operations: usize,
    /// Canonical documentation nodes.
    pub documentation_nodes: usize,
    /// Documentation nodes routed to one or more product packages.
    pub product_documentation_nodes: usize,
    /// Explicit governance/navigation nodes outside product packages.
    pub integration_documentation_nodes: usize,
    /// Canonical package dependency edges.
    pub dependency_edges: usize,
    /// Residual dependency-cycle package names.
    pub dependency_cycles: Vec<String>,
    /// Canonical modules lacking a specific relation.
    pub weak_modules: Vec<String>,
    /// Non-blocking diagnostics.
    pub warnings: Vec<String>,
    /// Stable blocking diagnostics.
    pub errors: Vec<String>,
}

impl PackageMapsReport {
    /// Returns true only for a complete report without blocking diagnostics.
    #[must_use]
    pub const fn passed(&self) -> bool {
        self.complete && self.errors.is_empty()
    }

    pub(super) fn early(error: String) -> Self {
        Self {
            complete: false,
            packages: 0,
            package_map_files: 0,
            logical_modules: 0,
            operations: 0,
            documentation_nodes: 0,
            product_documentation_nodes: 0,
            integration_documentation_nodes: 0,
            dependency_edges: 0,
            dependency_cycles: Vec::new(),
            weak_modules: Vec::new(),
            warnings: Vec::new(),
            errors: vec![error],
        }
    }
}

/// Validates package-map closure against canonical coverage registries.
#[must_use]
pub fn validate_package_maps(root: &Path) -> PackageMapsReport {
    validation::validate(root)
}

/// Stable process exit code.
#[must_use]
pub const fn exit_code(report: &PackageMapsReport) -> i32 {
    if report.passed() { 0 } else { 1 }
}

/// Renders the stable machine-readable report.
#[must_use]
pub fn render_report_json(report: &PackageMapsReport) -> String {
    let value = if report.complete {
        json!({
            "status": if report.passed() { "PASS" } else { "FAIL" },
            "packages": report.packages,
            "package_map_files": report.package_map_files,
            "logical_modules": report.logical_modules,
            "operations": report.operations,
            "documentation_nodes": report.documentation_nodes,
            "product_documentation_nodes": report.product_documentation_nodes,
            "integration_documentation_nodes": report.integration_documentation_nodes,
            "dependency_edges": report.dependency_edges,
            "dependency_cycles": report.dependency_cycles,
            "weak_modules": report.weak_modules,
            "warnings": report.warnings,
            "errors": report.errors,
        })
    } else {
        json!({
            "status": "FAIL",
            "warnings": report.warnings,
            "errors": report.errors,
        })
    };
    serde_json::to_string_pretty(&value)
        .expect("serializing a bounded package-map report cannot fail")
}

//! Rust validation of the committed coverage graph v2 registries.
//!
//! Graph generation remains a separate deterministic boundary. This validator
//! checks the committed graph, package-map closure, progressive dependency
//! re-entry, authority ceilings and permanent workflow policy without Python.

mod content;
mod load;
mod policy;
mod relations;

use std::path::Path;

use serde_json::json;

use load::CoverageInputs;

/// Stable validation report for coverage graph v2.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct CoverageGraphReport {
    /// Whether every mandatory input loaded.
    pub complete: bool,
    /// Registered Cargo packages.
    pub packages: usize,
    /// Declared logical modules.
    pub logical_modules: usize,
    /// Package-qualified operations.
    pub operations: usize,
    /// Unique documentation source files.
    pub documentation_files: usize,
    /// Documentation heading nodes.
    pub documentation_nodes: usize,
    /// Principle/invariant documentation nodes.
    pub principle_or_invariant_nodes: usize,
    /// Governance/navigation documentation nodes.
    pub governance_or_navigation_nodes: usize,
    /// Cargo dependency edges.
    pub dependency_edges: usize,
    /// Operations still routed through an unreviewed public facade.
    pub public_facade_operations: usize,
    /// Operations with low-confidence semantic routing.
    pub semantic_low_operations: usize,
    /// Logical modules without a specific relation.
    pub weak_modules: Vec<String>,
    /// Non-blocking diagnostics.
    pub warnings: Vec<String>,
    /// Stable blocking diagnostics.
    pub errors: Vec<String>,
}

impl CoverageGraphReport {
    /// Returns true only for a complete report without blocking diagnostics.
    #[must_use]
    pub const fn passed(&self) -> bool {
        self.complete && self.errors.is_empty()
    }

    fn early(error: String) -> Self {
        Self {
            complete: false,
            packages: 0,
            logical_modules: 0,
            operations: 0,
            documentation_files: 0,
            documentation_nodes: 0,
            principle_or_invariant_nodes: 0,
            governance_or_navigation_nodes: 0,
            dependency_edges: 0,
            public_facade_operations: 0,
            semantic_low_operations: 0,
            weak_modules: Vec::new(),
            warnings: Vec::new(),
            errors: vec![error],
        }
    }
}

/// Validates the committed coverage graph and its package-map projections.
#[must_use]
pub fn validate_coverage_graph(root: &Path) -> CoverageGraphReport {
    let inputs = match CoverageInputs::load(root) {
        Ok(inputs) => inputs,
        Err(error) => return CoverageGraphReport::early(error),
    };

    let mut errors = Vec::new();
    let mut warnings = Vec::new();

    let package_maps = crate::package_maps::validate_package_maps(root);
    warnings.extend(
        package_maps
            .warnings
            .iter()
            .map(|warning| format!("package-map: {warning}")),
    );
    errors.extend(
        package_maps
            .errors
            .iter()
            .map(|error| format!("package-map: {error}")),
    );
    if !package_maps.complete {
        errors.push("package-map validation did not load all mandatory inputs".to_owned());
    }

    let counts = content::validate(&inputs, &mut errors);
    relations::validate(root, &inputs, &counts.public_entries, &mut errors);
    policy::validate(root, &inputs, &counts, &mut errors);

    CoverageGraphReport {
        complete: true,
        packages: inputs.package_rows.len(),
        logical_modules: inputs.module_rows.len(),
        operations: inputs.operation_rows.len(),
        documentation_files: counts.documentation_files,
        documentation_nodes: inputs.documentation_rows.len(),
        principle_or_invariant_nodes: counts.principles,
        governance_or_navigation_nodes: counts.governance,
        dependency_edges: inputs.dependency_rows.len(),
        public_facade_operations: counts.public_facades,
        semantic_low_operations: counts.semantic_low,
        weak_modules: counts.weak_modules,
        warnings,
        errors,
    }
}

/// Stable process exit code.
#[must_use]
pub const fn exit_code(report: &CoverageGraphReport) -> i32 {
    if report.passed() { 0 } else { 1 }
}

/// Renders the stable machine-readable report.
#[must_use]
pub fn render_report_json(report: &CoverageGraphReport) -> String {
    let value = if report.complete {
        json!({
            "status": if report.passed() { "PASS" } else { "FAIL" },
            "packages": report.packages,
            "logical_modules": report.logical_modules,
            "operations": report.operations,
            "documentation_files": report.documentation_files,
            "documentation_nodes": report.documentation_nodes,
            "principle_or_invariant_nodes": report.principle_or_invariant_nodes,
            "governance_or_navigation_nodes": report.governance_or_navigation_nodes,
            "dependency_edges": report.dependency_edges,
            "public_facade_operations": report.public_facade_operations,
            "semantic_low_operations": report.semantic_low_operations,
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
        .expect("serializing a bounded coverage-graph report cannot fail")
}

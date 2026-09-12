//! Rust ownership for coverage-graph reconciliation and derived reporting.
//!
//! Package/module routes are reviewed machine registries, not heuristic output.
//! This generator derives source identities, reconciles reviewed ownership,
//! updates derived counts/reporting and runs the Rust closure validators. It
//! never invents a route, mutates control roots or grants implementation authority.

mod derive;
mod io;
mod load;
mod render;

use std::path::Path;

use serde_json::json;

use load::CoverageSnapshot;

/// Coverage-graph generation mode.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum CoverageGraphGenerationMode {
    /// Compare derived files with the committed repository.
    Check,
    /// Rewrite only the derived manifest metadata and human report.
    Write,
}

impl CoverageGraphGenerationMode {
    const fn label(self) -> &'static str {
        match self {
            Self::Check => "CHECK",
            Self::Write => "WRITE",
        }
    }
}

/// Stable result for one reconciliation run.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct CoverageGraphGenerationReport {
    /// Whether all mandatory inputs loaded.
    pub complete: bool,
    /// Requested mode.
    pub mode: &'static str,
    /// Registered packages.
    pub packages: usize,
    /// Reviewed operation routes.
    pub operations: usize,
    /// Documentation source files.
    pub documentation_files: usize,
    /// Documentation heading nodes.
    pub documentation_nodes: usize,
    /// Implementation/principle/qualification nodes.
    pub implementation_nodes: usize,
    /// Governance/navigation nodes.
    pub governance_nodes: usize,
    /// Cargo dependency edges.
    pub dependency_edges: usize,
    /// Later-wave progressive re-entry edges.
    pub progressive_edges: usize,
    /// Declared logical modules.
    pub modules: usize,
    /// Modules without a specific reviewed relation.
    pub weak_modules: Vec<String>,
    /// Derived files whose committed bytes differ.
    pub stale: Vec<String>,
    /// Non-blocking diagnostics.
    pub warnings: Vec<String>,
    /// Stable blocking diagnostics.
    pub errors: Vec<String>,
}

impl CoverageGraphGenerationReport {
    /// Returns true only for a complete, current and error-free run.
    #[must_use]
    pub const fn passed(&self) -> bool {
        self.complete && self.errors.is_empty() && self.stale.is_empty()
    }

    fn early(mode: CoverageGraphGenerationMode, error: String) -> Self {
        Self {
            complete: false,
            mode: mode.label(),
            packages: 0,
            operations: 0,
            documentation_files: 0,
            documentation_nodes: 0,
            implementation_nodes: 0,
            governance_nodes: 0,
            dependency_edges: 0,
            progressive_edges: 0,
            modules: 0,
            weak_modules: Vec::new(),
            stale: Vec::new(),
            warnings: Vec::new(),
            errors: vec![error],
        }
    }
}

/// Reconciles reviewed coverage registries into derived repository files.
#[must_use]
pub fn generate_coverage_graph(
    root: &Path,
    mode: CoverageGraphGenerationMode,
) -> CoverageGraphGenerationReport {
    let snapshot = match CoverageSnapshot::load(root) {
        Ok(snapshot) => snapshot,
        Err(error) => return CoverageGraphGenerationReport::early(mode, error),
    };
    let expected_manifest = match render::manifest(&snapshot) {
        Ok(value) => value,
        Err(error) => return CoverageGraphGenerationReport::early(mode, error),
    };
    let expected_report = render::human_report(&snapshot);
    let expected = [
        (load::MANIFEST_PATH, expected_manifest.as_str()),
        (load::HUMAN_REPORT_PATH, expected_report.as_str()),
    ];

    let mut warnings = Vec::new();
    let mut errors: Vec<String> = snapshot
        .derivation_errors
        .iter()
        .map(|error| format!("source-derivation: {error}"))
        .collect();
    collect_architecture_validation(root, &mut warnings, &mut errors);

    if matches!(mode, CoverageGraphGenerationMode::Write) && errors.is_empty() {
        if let Err(error) = io::write_all(root, &expected) {
            errors.push(error);
        }
    }

    if errors.is_empty() {
        let report = crate::coverage_graph_validation::validate_coverage_graph(root);
        warnings.extend(
            report
                .warnings
                .iter()
                .map(|warning| format!("coverage-graph: {warning}")),
        );
        errors.extend(
            report
                .errors
                .iter()
                .map(|error| format!("coverage-graph: {error}")),
        );
        if !report.complete {
            errors.push(
                "coverage-graph validator did not load all mandatory inputs"
                    .to_owned(),
            );
        }
    }

    let stale = io::stale(root, &expected);
    CoverageGraphGenerationReport {
        complete: true,
        mode: mode.label(),
        packages: snapshot.packages,
        operations: snapshot.operations,
        documentation_files: snapshot.documentation_files,
        documentation_nodes: snapshot.documentation_nodes,
        implementation_nodes: snapshot.implementation_nodes,
        governance_nodes: snapshot.governance_nodes,
        dependency_edges: snapshot.dependency_edges,
        progressive_edges: snapshot.progressive_edges,
        modules: snapshot.modules,
        weak_modules: snapshot.weak_modules,
        stale,
        warnings,
        errors,
    }
}

fn collect_architecture_validation(
    root: &Path,
    warnings: &mut Vec<String>,
    errors: &mut Vec<String>,
) {
    let architecture =
        crate::architecture_coverage::validate_architecture_coverage(root);
    warnings.extend(
        architecture
            .warnings
            .iter()
            .map(|warning| format!("architecture: {warning}")),
    );
    errors.extend(
        architecture
            .errors
            .iter()
            .map(|error| format!("architecture: {error}")),
    );
    if !architecture.complete {
        errors.push("architecture validator did not load all inputs".to_owned());
    }

    let contracts =
        crate::architecture_coverage_contracts::validate_architecture_coverage_contracts(root);
    errors.extend(
        contracts
            .errors
            .iter()
            .map(|error| format!("architecture-contracts: {error}")),
    );
    if !contracts.complete {
        errors.push(
            "architecture contract validator did not load all inputs".to_owned(),
        );
    }
}

/// Stable process exit code.
#[must_use]
pub const fn generation_exit_code(
    report: &CoverageGraphGenerationReport,
) -> i32 {
    if report.passed() { 0 } else { 1 }
}

/// Renders a deterministic machine-readable generation report.
#[must_use]
pub fn render_generation_report_json(
    report: &CoverageGraphGenerationReport,
) -> String {
    let status = if report.passed() {
        if report.mode == "WRITE" { "GENERATED" } else { "PASS" }
    } else {
        "FAIL"
    };
    serde_json::to_string_pretty(&json!({
        "status": status,
        "mode": report.mode,
        "packages": report.packages,
        "operations": report.operations,
        "documentation_files": report.documentation_files,
        "documentation_nodes": report.documentation_nodes,
        "implementation_nodes": report.implementation_nodes,
        "governance_nodes": report.governance_nodes,
        "dependency_edges": report.dependency_edges,
        "progressive_edges": report.progressive_edges,
        "modules": report.modules,
        "weak_modules": report.weak_modules,
        "stale": report.stale,
        "warnings": report.warnings,
        "errors": report.errors,
    }))
    .expect("serializing a bounded coverage generation report cannot fail")
}

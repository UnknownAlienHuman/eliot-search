//! Full Rust architecture-to-package coverage validator.
//!
//! This is the schema-v2 replacement for `validate-architecture-coverage.py`.
//! It validates static ownership and registry closure only; it never grants
//! implementation authority or claims runtime/qualification success.

mod control;
mod load;
mod markdown;
mod schemas;
mod topology;

use std::path::Path;

use serde_json::json;

use load::Inputs;

/// Architecture coverage report compatible with the retired Python entrypoint.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ArchitectureCoverageReport {
    /// Whether all mandatory inputs loaded successfully.
    pub complete: bool,
    /// Registered packages.
    pub packages: usize,
    /// Package assignment files.
    pub assignments: usize,
    /// Non-foundation function sources.
    pub function_sources: usize,
    /// Source-derived package-qualified operations.
    pub derived_package_qualified_operations: usize,
    /// Declared logical modules.
    pub modules: usize,
    /// Architecture sections.
    pub architecture_sections: usize,
    /// Capability cells.
    pub capability_cells: usize,
    /// Architecture invariants.
    pub invariants: usize,
    /// Shared ports.
    pub ports: usize,
    /// Configuration sections.
    pub configuration_sections: usize,
    /// P00 schema/type symbols.
    pub schema_and_type_symbols: usize,
    /// Public recipes.
    pub recipes: usize,
    /// Closed reason codes.
    pub reason_codes: usize,
    /// Delivery slices.
    pub delivery_slices: usize,
    /// Current launch stage.
    pub launch_stage: Option<String>,
    /// Current launch wave.
    pub launch_wave: Option<i64>,
    /// Non-blocking diagnostics. Kept for report compatibility.
    pub warnings: Vec<String>,
    /// Stable blocking diagnostics.
    pub errors: Vec<String>,
}

impl ArchitectureCoverageReport {
    /// Returns true only for a complete report without errors.
    #[must_use]
    pub const fn passed(&self) -> bool {
        self.complete && self.errors.is_empty()
    }

    fn early(error: String) -> Self {
        Self {
            complete: false,
            packages: 0,
            assignments: 0,
            function_sources: 0,
            derived_package_qualified_operations: 0,
            modules: 0,
            architecture_sections: 0,
            capability_cells: 0,
            invariants: 0,
            ports: 0,
            configuration_sections: 0,
            schema_and_type_symbols: 0,
            recipes: 0,
            reason_codes: 0,
            delivery_slices: 0,
            launch_stage: None,
            launch_wave: None,
            warnings: Vec::new(),
            errors: vec![error],
        }
    }
}

/// Validates full static architecture coverage closure.
#[must_use]
pub fn validate_architecture_coverage(root: &Path) -> ArchitectureCoverageReport {
    let inputs = match Inputs::load(root) {
        Ok(inputs) => inputs,
        Err(error) => return ArchitectureCoverageReport::early(error),
    };
    let mut errors = Vec::new();
    let topology = topology::validate(root, &inputs, &mut errors);
    let schemas = schemas::validate(root, &inputs, &topology, &mut errors);
    let delivery_slices =
        control::validate(root, &inputs, &topology, &schemas, &mut errors);

    ArchitectureCoverageReport {
        complete: true,
        packages: topology.packages.len(),
        assignments: topology.assignment_paths.len(),
        function_sources: topology.function_rows.len(),
        derived_package_qualified_operations: topology.operation_count,
        modules: topology.module_total,
        architecture_sections: topology.section_count,
        capability_cells: topology.capability_count,
        invariants: topology.invariant_count,
        ports: topology.port_count,
        configuration_sections: topology.config_count,
        schema_and_type_symbols: schemas.schema_total,
        recipes: schemas.recipe_count,
        reason_codes: schemas.reason_count,
        delivery_slices,
        launch_stage: load::string(&inputs.launch, "active_stage").map(str::to_owned),
        launch_wave: load::integer(&inputs.launch, "active_wave"),
        warnings: Vec::new(),
        errors,
    }
}

/// Stable process exit code.
#[must_use]
pub const fn exit_code(report: &ArchitectureCoverageReport) -> i32 {
    if report.passed() { 0 } else { 1 }
}

/// Renders the stable machine-readable report.
#[must_use]
pub fn render_report_json(report: &ArchitectureCoverageReport) -> String {
    let value = if report.complete {
        json!({
            "status": if report.passed() { "PASS" } else { "FAIL" },
            "packages": report.packages,
            "assignments": report.assignments,
            "function_sources": report.function_sources,
            "derived_package_qualified_operations": report.derived_package_qualified_operations,
            "modules": report.modules,
            "architecture_sections": report.architecture_sections,
            "capability_cells": report.capability_cells,
            "invariants": report.invariants,
            "ports": report.ports,
            "configuration_sections": report.configuration_sections,
            "schema_and_type_symbols": report.schema_and_type_symbols,
            "recipes": report.recipes,
            "reason_codes": report.reason_codes,
            "delivery_slices": report.delivery_slices,
            "launch_stage": report.launch_stage,
            "launch_wave": report.launch_wave,
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
        .expect("serializing a bounded architecture coverage report cannot fail")
}

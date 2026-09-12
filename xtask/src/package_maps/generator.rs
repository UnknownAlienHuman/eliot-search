//! Deterministic Rust package-map generator and drift checker.

mod io;
mod model;
mod render;

use std::path::Path;

use serde_json::json;

use super::load::{Inputs, read_text};
use io::{check_outputs, patch_manifest_text, write_outputs};
use model::PackageMapModel;
use render::render_outputs;

/// Generator execution mode.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum PackageMapGenerationMode {
    /// Derive all outputs and report drift without writing.
    Check,
    /// Replace generated outputs and patch the manifest count block.
    Write,
}

/// Stable generator/check report.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct PackageMapGenerationReport {
    /// Whether all canonical inputs loaded and rendering completed.
    pub complete: bool,
    /// Selected execution mode.
    pub mode: PackageMapGenerationMode,
    /// Number of registered packages.
    pub packages: usize,
    /// Number of package-local maps.
    pub map_files: usize,
    /// Number of operation routes.
    pub operations: usize,
    /// Number of documentation nodes.
    pub documents: usize,
    /// Number of logical modules.
    pub modules: usize,
    /// Number of dependency edges.
    pub dependencies: usize,
    /// Number of explicit integration documentation nodes.
    pub integration_nodes: usize,
    /// Number of rendered output files, excluding the patched manifest.
    pub output_count: usize,
    /// Derived outputs that differ from the repository.
    pub stale_files: Vec<String>,
    /// Blocking derivation/write diagnostics.
    pub errors: Vec<String>,
}

impl PackageMapGenerationReport {
    /// Returns true only when derivation completed and no drift/error remains.
    #[must_use]
    pub const fn passed(&self) -> bool {
        self.complete && self.errors.is_empty() && self.stale_files.is_empty()
    }

    fn early(mode: PackageMapGenerationMode, error: String) -> Self {
        Self {
            complete: false,
            mode,
            packages: 0,
            map_files: 0,
            operations: 0,
            documents: 0,
            modules: 0,
            dependencies: 0,
            integration_nodes: 0,
            output_count: 0,
            stale_files: Vec::new(),
            errors: vec![error],
        }
    }
}

/// Derives every package-map artifact from canonical checked-in registries.
#[must_use]
pub fn generate_package_maps(
    root: &Path,
    mode: PackageMapGenerationMode,
) -> PackageMapGenerationReport {
    let inputs = match Inputs::load(root) {
        Ok(inputs) => inputs,
        Err(error) => return PackageMapGenerationReport::early(mode, error),
    };
    let model = PackageMapModel::derive(&inputs);
    let outputs = render_outputs(&model);
    let manifest = match read_text(root, "swarm/coverage/manifest.toml") {
        Ok(text) => text,
        Err(error) => return PackageMapGenerationReport::early(mode, error),
    };
    let patched_manifest = match patch_manifest_text(&manifest, &model.stats) {
        Ok(text) => text,
        Err(error) => return PackageMapGenerationReport::early(mode, error),
    };
    let mut errors = Vec::new();
    if !model.stats.cycle.is_empty() {
        errors.push(format!("package dependency cycle: {:?}", model.stats.cycle));
    }
    let stale_files = match mode {
        PackageMapGenerationMode::Check => {
            check_outputs(root, &outputs, &patched_manifest)
        }
        PackageMapGenerationMode::Write => {
            if let Err(error) = write_outputs(root, &outputs, &patched_manifest) {
                errors.push(error);
            }
            Vec::new()
        }
    };
    PackageMapGenerationReport {
        complete: true,
        mode,
        packages: model.stats.packages,
        map_files: model.stats.map_files,
        operations: model.stats.operations,
        documents: model.stats.documents,
        modules: model.stats.modules,
        dependencies: model.stats.dependencies,
        integration_nodes: model.stats.integration_nodes,
        output_count: outputs.len(),
        stale_files,
        errors,
    }
}

/// Stable process exit code for generation/check mode.
#[must_use]
pub const fn generation_exit_code(report: &PackageMapGenerationReport) -> i32 {
    if report.passed() { 0 } else { 1 }
}

/// Stable machine-readable generation/check report.
#[must_use]
pub fn render_generation_report_json(report: &PackageMapGenerationReport) -> String {
    let mode = match report.mode {
        PackageMapGenerationMode::Check => "CHECK",
        PackageMapGenerationMode::Write => "WRITE",
    };
    let value = if report.complete {
        json!({
            "status": if report.passed() { "PASS" } else { "FAIL" },
            "mode": mode,
            "packages": report.packages,
            "map_files": report.map_files,
            "operations": report.operations,
            "documents": report.documents,
            "modules": report.modules,
            "dependencies": report.dependencies,
            "integration_nodes": report.integration_nodes,
            "output_count": report.output_count,
            "stale_files": report.stale_files,
            "errors": report.errors,
        })
    } else {
        json!({
            "status": "FAIL",
            "mode": mode,
            "stale_files": report.stale_files,
            "errors": report.errors,
        })
    };
    serde_json::to_string_pretty(&value)
        .expect("serializing a bounded package-map generation report cannot fail")
}

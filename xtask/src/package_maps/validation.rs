//! Full package-map closure validation.

mod content;
mod files;
mod policy;
#[allow(unused_must_use)]
mod relations;

use std::collections::BTreeMap;
use std::path::Path;

use super::load::{Inputs, boolean, strings};
use super::{PackageMapsReport, dependency_cycle};

pub(super) fn validate(root: &Path) -> PackageMapsReport {
    let inputs = match Inputs::load(root) {
        Ok(inputs) => inputs,
        Err(error) => return PackageMapsReport::early(error),
    };
    let mut errors = Vec::new();
    let mut warnings = Vec::new();

    files::validate(root, &inputs, &mut warnings, &mut errors);
    content::validate(root, &inputs, &mut errors);
    relations::validate(root, &inputs, &mut errors);
    policy::validate(root, &inputs, &mut errors);

    let dependency_map: BTreeMap<String, Vec<String>> = inputs
        .package_rows
        .iter()
        .map(|(package, row)| (package.clone(), strings(row, "deps")))
        .collect();
    let dependency_cycles = dependency_cycle(&dependency_map);
    if !dependency_cycles.is_empty() {
        errors.push(format!(
            "package dependency cycle: {dependency_cycles:?}"
        ));
    }

    let weak_modules: Vec<String> = inputs
        .module_rows
        .iter()
        .filter(|(_, row)| boolean(row, "weakly_covered") == Some(true))
        .map(|(id, _)| id.clone())
        .collect();
    if !weak_modules.is_empty() {
        errors.push(format!(
            "weak implementation modules remain: {weak_modules:?}"
        ));
    }

    let product_documentation_nodes = inputs
        .document_rows
        .values()
        .filter(|row| !strings(row, "packages").is_empty())
        .count();
    let integration_documentation_nodes = inputs
        .document_rows
        .len()
        .saturating_sub(product_documentation_nodes);

    PackageMapsReport {
        complete: true,
        packages: inputs.package_rows.len(),
        package_map_files: inputs.package_rows.len().saturating_mul(4),
        logical_modules: inputs.module_rows.len(),
        operations: inputs.operation_rows.len(),
        documentation_nodes: inputs.document_rows.len(),
        product_documentation_nodes,
        integration_documentation_nodes,
        dependency_edges: inputs.dependency_rows.len(),
        dependency_cycles,
        weak_modules,
        warnings,
        errors,
    }
}

//! Authority ceilings, port closure and permanent workflow policy.

use std::path::Path;

use super::super::load::{
    Inputs, boolean, integer, load_toml, read_text, string, strings,
};
use super::super::{INTEGRATION_PATH, package_paths};

pub(super) fn validate(root: &Path, inputs: &Inputs, errors: &mut Vec<String>) {
    validate_authority(root, inputs, errors);
    validate_ports(inputs, errors);
    validate_structural_modules(inputs, errors);
    validate_workflow(root, errors);
}

fn validate_authority(root: &Path, inputs: &Inputs, errors: &mut Vec<String>) {
    if boolean(&inputs.index, "implementation_authorized_by_this_index") != Some(false) {
        errors.push("package-map index grants implementation authority".to_owned());
    }
    match load_toml(root, INTEGRATION_PATH) {
        Ok(integration) => {
            if boolean(&integration, "product_semantics_allowed") != Some(false)
                || boolean(&integration, "implementation_authorized_by_this_map") != Some(false)
            {
                errors.push("integration documentation map authority changed".to_owned());
            }
        }
        Err(error) => errors.push(error),
    }
    for package in inputs.package_rows.keys() {
        let paths = package_paths(package);
        for relative in [
            paths.overview,
            paths.operations,
            paths.documents,
            paths.relations,
        ] {
            match load_toml(root, &relative) {
                Ok(document) => {
                    if boolean(&document, "implementation_authorized_by_this_map")
                        != Some(false)
                    {
                        errors.push(format!("{relative}: implementation authority changed"));
                    }
                }
                Err(error) => errors.push(error),
            }
        }
    }
}

fn validate_ports(inputs: &Inputs, errors: &mut Vec<String>) {
    if integer(&inputs.port_document, "schema_version") != Some(2) {
        errors.push("port registry must be schema v2".to_owned());
    }
    if integer(&inputs.port_document, "port_count")
        != i64::try_from(inputs.port_rows.len()).ok()
    {
        errors.push("port registry count mismatch".to_owned());
    }
    let method_count: usize = inputs
        .port_rows
        .values()
        .map(|row| strings(row, "methods").len())
        .sum();
    if integer(&inputs.port_document, "method_count") != i64::try_from(method_count).ok() {
        errors.push("port method count mismatch".to_owned());
    }
    for (port, row) in &inputs.port_rows {
        let methods = strings(row, "methods");
        let modules = strings(row, "method_modules");
        let package = string(row, "implementation_package").unwrap_or_default();
        if methods.len() != modules.len() {
            errors.push(format!("{port}: one method module per method required"));
        }
        for (method, module) in methods.iter().zip(modules.iter()) {
            if !inputs
                .module_rows
                .contains_key(&format!("{package}:{module}"))
            {
                errors.push(format!(
                    "{port}.{method}: invalid package-local method module"
                ));
            }
        }
    }
}

fn validate_structural_modules(inputs: &Inputs, errors: &mut Vec<String>) {
    for (id, row) in &inputs.module_rows {
        if matches!(
            string(row, "role"),
            Some("public_entry" | "structural_boundary" | "structural_support")
        ) && string(row, "structural_rationale")
            .is_none_or(|value| value.trim().is_empty())
        {
            errors.push(format!("{id}: structural rationale missing"));
        }
    }
}

fn validate_workflow(root: &Path, errors: &mut Vec<String>) {
    let path = ".github/workflows/package-map-coverage-v2.yml";
    let workflow = match read_text(root, path) {
        Ok(workflow) => workflow,
        Err(error) => {
            errors.push(error);
            return;
        }
    };
    for token in [
        "workflow_dispatch:",
        "contents: read",
        "persist-credentials: false",
    ] {
        if !workflow.contains(token) {
            errors.push(format!("permanent workflow missing {token}"));
        }
    }
    for token in [
        "\n  push:",
        "\n  pull_request:",
        "\n  schedule:",
        "\n  workflow_run:",
        "\n  repository_dispatch:",
    ] {
        if workflow.contains(token) {
            errors.push(format!(
                "permanent workflow has automatic trigger {}",
                token.trim()
            ));
        }
    }
    if workflow.contains("validate-package-maps-v2.py") {
        errors.push("permanent workflow still invokes retired Python validator".to_owned());
    }
    if !workflow.contains("validate package-maps --json") {
        errors.push("permanent workflow does not invoke Rust package-map validator".to_owned());
    }
}

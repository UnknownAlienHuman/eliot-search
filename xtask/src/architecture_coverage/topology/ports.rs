//! Shared-port method ownership and configuration-section ownership.

use std::collections::BTreeSet;
use std::path::Path;

use toml::Value;

use super::rows_or_empty;
use super::super::load::{
    Inputs, ModuleMap, read_text, require, string, string_list,
    validate_module_ref, validate_owner_pair,
};
use super::super::markdown::port_methods;

pub(super) fn validate(
    root: &Path,
    inputs: &Inputs,
    packages: &BTreeSet<String>,
    modules: &ModuleMap,
    errors: &mut Vec<String>,
) -> (usize, usize) {
    let port_count = validate_ports(root, inputs, packages, modules, errors);
    let config_count = validate_config(root, inputs, packages, modules, errors);
    (port_count, config_count)
}

fn validate_ports(
    root: &Path,
    inputs: &Inputs,
    packages: &BTreeSet<String>,
    modules: &ModuleMap,
    errors: &mut Vec<String>,
) -> usize {
    let source = match read_text(root, "docs/contracts/p00/PORT_OPERATIONS.md") {
        Ok(text) => port_methods(&text),
        Err(error) => {
            errors.push(error);
            Default::default()
        }
    };
    let rows = rows_or_empty(&inputs.port_doc, "port", "name", errors);
    require(
        errors,
        source.len() == 23,
        "PORT_OPERATIONS must define 23 ports",
    );
    require(
        errors,
        rows.keys().cloned().collect::<BTreeSet<_>>()
            == source.keys().cloned().collect(),
        "port registry set mismatch",
    );
    require(
        errors,
        inputs.port_doc.get("schema_version").and_then(Value::as_integer)
            == Some(2),
        "port registry must be schema v2",
    );
    let source_method_count: usize = source.values().map(Vec::len).sum();
    require(
        errors,
        inputs.port_doc.get("method_count").and_then(Value::as_integer)
            == i64::try_from(source_method_count).ok(),
        "port method total mismatch",
    );

    for (port, row) in &rows {
        let methods = string_list(row, "methods");
        let method_modules = string_list(row, "method_modules");
        require(
            errors,
            methods.as_ref() == source.get(port),
            format!("{port}: method inventory mismatch"),
        );
        require(
            errors,
            method_modules.as_ref().zip(methods.as_ref()).is_some_and(
                |(module_list, method_list)| module_list.len() == method_list.len(),
            ),
            format!("{port}: one method module per method required"),
        );
        let package = string(row, "implementation_package");
        let module = string(row, "implementation_module");
        require(
            errors,
            package.is_some_and(|value| packages.contains(value)),
            format!("{port}: unknown implementation package"),
        );
        validate_owner_pair(errors, package, module, modules, port, false);
        if let (Some(methods), Some(method_modules), Some(package)) =
            (methods, method_modules, package)
        {
            for (method, module) in methods.iter().zip(method_modules.iter()) {
                validate_owner_pair(
                    errors,
                    Some(package),
                    Some(module),
                    modules,
                    &format!("{port}.{method}"),
                    false,
                );
            }
        }
    }
    require(
        errors,
        rows.get("ResidencyPolicyPort")
            .and_then(|row| string(row, "implementation_package"))
            == Some("search-revision-store"),
        "ResidencyPolicyPort must be implemented by search-revision-store",
    );
    require(
        errors,
        rows.get("ClockPort")
            .and_then(|row| string(row, "implementation_package"))
            == Some("eliot-searchd"),
        "ClockPort must be the daemon private adapter",
    );
    rows.len()
}

fn validate_config(
    root: &Path,
    inputs: &Inputs,
    packages: &BTreeSet<String>,
    modules: &ModuleMap,
    errors: &mut Vec<String>,
) -> usize {
    let rows = rows_or_empty(&inputs.config_doc, "section", "name", errors);
    require(
        errors,
        rows.len() == 20,
        "configuration registry must contain 20 sections",
    );
    let mut contracts = BTreeSet::new();
    for (name, row) in &rows {
        let owner = string(row, "owner");
        let owner_module = string(row, "owner_module");
        let contract = string(row, "contract");
        require(
            errors,
            owner.is_some_and(|value| packages.contains(value)),
            format!("config {name}: unknown owner {}", owner.unwrap_or("<missing>")),
        );
        require(
            errors,
            owner_module.is_some(),
            format!("config {name}: owner module missing"),
        );
        if let (Some(owner), Some(module)) = (owner, owner_module) {
            validate_module_ref(
                errors,
                &format!("{owner}:{module}"),
                modules,
                &format!("config {name}"),
            );
        }
        require(
            errors,
            contract.is_some_and(|relative| root.join(relative).is_file()),
            format!(
                "config {name}: missing contract {}",
                contract.unwrap_or("<missing>")
            ),
        );
        if let Some(relative) = contract {
            require(
                errors,
                contracts.insert(relative.to_owned()),
                format!("config {name}: duplicate contract path {relative}"),
            );
        }
    }
    rows.len()
}

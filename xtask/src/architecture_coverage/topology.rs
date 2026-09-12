//! Package/module topology and architecture relation closure.

mod packages;
mod ports;
mod relations;

use std::collections::{BTreeMap, BTreeSet};
use std::path::Path;

use toml::Value;

use super::load::{Inputs, ModuleMap, RowMap};

pub(super) struct TopologySummary {
    pub(super) package_rows: RowMap,
    pub(super) packages: BTreeSet<String>,
    pub(super) foundation_rows: RowMap,
    pub(super) function_rows: RowMap,
    pub(super) assignment_paths: BTreeSet<String>,
    pub(super) modules: ModuleMap,
    pub(super) module_rows: RowMap,
    pub(super) module_total: usize,
    pub(super) operation_count: usize,
    pub(super) section_count: usize,
    pub(super) capability_count: usize,
    pub(super) invariant_count: usize,
    pub(super) port_count: usize,
    pub(super) config_count: usize,
}

pub(super) fn validate(
    root: &Path,
    inputs: &Inputs,
    errors: &mut Vec<String>,
) -> TopologySummary {
    let package = packages::validate(root, inputs, errors);
    let (section_count, capability_count, invariant_count) =
        relations::validate(inputs, &package.packages, &package.modules, errors);
    let (port_count, config_count) =
        ports::validate(root, inputs, &package.packages, &package.modules, errors);
    TopologySummary {
        package_rows: package.package_rows,
        packages: package.packages,
        foundation_rows: package.foundation_rows,
        function_rows: package.function_rows,
        assignment_paths: package.assignment_paths,
        modules: package.modules,
        module_rows: package.module_rows,
        module_total: package.module_total,
        operation_count: package.operation_count,
        section_count,
        capability_count,
        invariant_count,
        port_count,
        config_count,
    }
}

pub(super) fn rows_or_empty(
    document: &Value,
    key: &str,
    identity: &str,
    errors: &mut Vec<String>,
) -> BTreeMap<String, Value> {
    match super::load::indexed_rows(document, key, identity) {
        Ok(rows) => rows,
        Err(error) => {
            errors.push(error);
            BTreeMap::new()
        }
    }
}

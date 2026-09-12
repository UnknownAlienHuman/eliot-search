//! Derivation of package dependency and logical-module identity sets.

use std::collections::BTreeSet;
use std::path::Path;

use toml::Value;

use super::source::{
    compare_identity_sets, indexed_or_empty, load_toml, string_array,
};

pub(super) fn validate_dependencies(
    package_document: &Value,
    dependency_document: &Value,
    errors: &mut Vec<String>,
) {
    let packages = indexed_or_empty(package_document, "package", "name", errors);
    let mut expected = BTreeSet::new();
    for (consumer, row) in packages {
        for producer in string_array(&row, "deps") {
            expected.insert(format!("{consumer}->{producer}"));
        }
    }
    let actual = indexed_or_empty(dependency_document, "edge", "id", errors);
    compare_identity_sets(
        "package dependency",
        expected,
        actual.keys().cloned().collect(),
        errors,
    );
}

pub(super) fn validate_modules(
    root: &Path,
    manifest: &Value,
    module_document: &Value,
    errors: &mut Vec<String>,
) {
    let registry_path = manifest
        .get("module_registry")
        .and_then(Value::as_str)
        .unwrap_or("swarm/module-packets.toml");
    let registry = match load_toml(root, registry_path) {
        Ok(document) => document,
        Err(error) => {
            errors.push(error);
            return;
        }
    };
    let mut expected = BTreeSet::new();
    let Some(packets) = registry.get("packet").and_then(Value::as_array) else {
        errors.push(format!("{registry_path}: packet must be an array"));
        return;
    };
    for packet in packets {
        let Some(path) = packet.get("path").and_then(Value::as_str) else {
            errors.push(format!("{registry_path}: packet path missing"));
            continue;
        };
        let document = match load_toml(root, path) {
            Ok(document) => document,
            Err(error) => {
                errors.push(error);
                continue;
            }
        };
        let rows = indexed_or_empty(&document, "package", "name", errors);
        for (package, row) in rows {
            for module in string_array(&row, "modules") {
                expected.insert(format!("{package}:{module}"));
            }
        }
    }
    let actual = indexed_or_empty(module_document, "module", "id", errors);
    compare_identity_sets(
        "logical module",
        expected,
        actual.keys().cloned().collect(),
        errors,
    );
}

//! Coverage graph v2 input loading.

use std::collections::BTreeMap;
use std::fs;
use std::path::Path;

use toml::Value;

pub(super) type RowMap = BTreeMap<String, Value>;

pub(super) struct CoverageInputs {
    pub(super) manifest: Value,
    pub(super) package_rows: RowMap,
    pub(super) module_document: Value,
    pub(super) module_rows: RowMap,
    pub(super) operation_document: Value,
    pub(super) operation_rows: RowMap,
    pub(super) documentation_document: Value,
    pub(super) documentation_rows: RowMap,
    pub(super) dependency_document: Value,
    pub(super) dependency_rows: RowMap,
    pub(super) override_rows: RowMap,
    pub(super) launch_state: Value,
}

impl CoverageInputs {
    pub(super) fn load(root: &Path) -> Result<Self, String> {
        let manifest = load_toml(root, "swarm/coverage/manifest.toml")?;
        let package_document = load_toml(
            root,
            manifest_path(&manifest, "package_registry", "swarm/crates.toml"),
        )?;
        let module_document = load_toml(
            root,
            manifest_path(
                &manifest,
                "module_coverage_registry",
                "swarm/coverage/module-coverage.toml",
            ),
        )?;
        let operation_document = load_toml(
            root,
            manifest_path(
                &manifest,
                "operation_module_registry",
                "swarm/coverage/operation-modules.toml",
            ),
        )?;
        let documentation_document = load_toml(
            root,
            manifest_path(
                &manifest,
                "documentation_node_registry",
                "swarm/coverage/documentation-nodes.toml",
            ),
        )?;
        let dependency_document = load_toml(
            root,
            manifest_path(
                &manifest,
                "dependency_edge_registry",
                "swarm/coverage/dependency-edges.toml",
            ),
        )?;
        let stage_readsets = load_toml(root, "swarm/stage-readsets.toml")?;
        let launch_state = load_toml(root, "swarm/launch-state.toml")?;

        Ok(Self {
            package_rows: indexed_rows(&package_document, "package", "name")?,
            module_rows: indexed_rows(&module_document, "module", "id")?,
            operation_rows: indexed_rows(&operation_document, "operation", "id")?,
            documentation_rows: indexed_rows(&documentation_document, "node", "id")?,
            dependency_rows: indexed_rows(&dependency_document, "edge", "id")?,
            override_rows: indexed_rows(&stage_readsets, "override", "id")?,
            manifest,
            module_document,
            operation_document,
            documentation_document,
            dependency_document,
            launch_state,
        })
    }
}

pub(super) fn load_toml(root: &Path, relative: &str) -> Result<Value, String> {
    let bytes = fs::read(root.join(relative))
        .map_err(|error| format!("{relative}: {error}"))?;
    let text = String::from_utf8(bytes)
        .map_err(|error| format!("{relative}: {error}"))?;
    toml::from_str(&text).map_err(|error| format!("{relative}: {error}"))
}

pub(super) fn read_text(root: &Path, relative: &str) -> Result<String, String> {
    fs::read_to_string(root.join(relative))
        .map_err(|error| format!("{relative}: {error}"))
}

pub(super) fn indexed_rows(
    document: &Value,
    key: &str,
    identity: &str,
) -> Result<RowMap, String> {
    let rows = array(document, key)
        .ok_or_else(|| format!("{key} must be an array of tables"))?;
    let mut result = BTreeMap::new();
    for row in rows {
        let name = string(row, identity)
            .ok_or_else(|| format!("{key}: row missing {identity}"))?;
        if result.insert(name.to_owned(), row.clone()).is_some() {
            return Err(format!("{key}: duplicate identity {name}"));
        }
    }
    Ok(result)
}

pub(super) fn manifest_path<'a>(
    manifest: &'a Value,
    key: &str,
    fallback: &'a str,
) -> &'a str {
    string(manifest, key).unwrap_or(fallback)
}

pub(super) fn array<'a>(value: &'a Value, key: &str) -> Option<&'a [Value]> {
    value.get(key)?.as_array().map(Vec::as_slice)
}

pub(super) fn string<'a>(value: &'a Value, key: &str) -> Option<&'a str> {
    value.get(key).and_then(Value::as_str)
}

pub(super) fn integer(value: &Value, key: &str) -> Option<i64> {
    value.get(key).and_then(Value::as_integer)
}

pub(super) fn boolean(value: &Value, key: &str) -> Option<bool> {
    value.get(key).and_then(Value::as_bool)
}

pub(super) fn strings(value: &Value, key: &str) -> Vec<String> {
    array(value, key)
        .unwrap_or_default()
        .iter()
        .filter_map(Value::as_str)
        .map(str::to_owned)
        .collect()
}

pub(super) fn ref_package(reference: &str) -> &str {
    reference.split_once(':').map_or("", |(package, _)| package)
}

//! Repository loading and closed TOML helpers for architecture coverage.

use std::collections::BTreeMap;
use std::path::Path;

use toml::Value;

pub(super) type RowMap = BTreeMap<String, Value>;
pub(super) type ModuleMap = BTreeMap<String, std::collections::BTreeSet<String>>;

pub(super) struct Inputs {
    pub(super) manifest: Value,
    pub(super) package_doc: Value,
    pub(super) function_doc: Value,
    pub(super) module_doc: Value,
    pub(super) section_doc: Value,
    pub(super) capability_doc: Value,
    pub(super) invariant_doc: Value,
    pub(super) port_doc: Value,
    pub(super) schema_doc: Value,
    pub(super) recipe_doc: Value,
    pub(super) delivery_doc: Value,
    pub(super) reason_doc: Value,
    pub(super) config_doc: Value,
    pub(super) launch: Value,
    pub(super) p00_manifest: Value,
    pub(super) architecture: String,
}

impl Inputs {
    pub(super) fn load(root: &Path) -> Result<Self, String> {
        let manifest = load_toml(root, "swarm/coverage/manifest.toml")?;
        let package_doc = load_manifest_doc(root, &manifest, "package_registry")?;
        let function_doc = load_manifest_doc(root, &manifest, "function_registry")?;
        let module_doc = load_manifest_doc(root, &manifest, "module_registry")?;
        let section_doc =
            load_manifest_doc(root, &manifest, "architecture_section_registry")?;
        let capability_doc = load_manifest_doc(root, &manifest, "capability_registry")?;
        let invariant_doc = load_manifest_doc(root, &manifest, "invariant_registry")?;
        let port_doc = load_manifest_doc(root, &manifest, "port_registry")?;
        let schema_doc = load_manifest_doc(root, &manifest, "schema_registry")?;
        let recipe_doc = load_manifest_doc(root, &manifest, "recipe_registry")?;
        let delivery_doc = load_manifest_doc(root, &manifest, "delivery_registry")?;
        let reason_doc = load_manifest_doc(root, &manifest, "reason_registry")?;
        let config_doc = load_manifest_doc(root, &manifest, "configuration_registry")?;
        let architecture_path = manifest_string(&manifest, "architecture_master")?;
        let architecture = read_text(root, architecture_path)?;
        Ok(Self {
            manifest,
            package_doc,
            function_doc,
            module_doc,
            section_doc,
            capability_doc,
            invariant_doc,
            port_doc,
            schema_doc,
            recipe_doc,
            delivery_doc,
            reason_doc,
            config_doc,
            launch: load_toml(root, "swarm/launch-state.toml")?,
            p00_manifest: load_toml(root, "docs/contracts/p00/manifest.toml")?,
            architecture,
        })
    }
}

fn load_manifest_doc(
    root: &Path,
    manifest: &Value,
    key: &str,
) -> Result<Value, String> {
    load_toml(root, manifest_string(manifest, key)?)
}

pub(super) fn manifest_string<'a>(
    manifest: &'a Value,
    key: &str,
) -> Result<&'a str, String> {
    manifest
        .get(key)
        .and_then(Value::as_str)
        .ok_or_else(|| format!("swarm/coverage/manifest.toml: missing {key}"))
}

pub(super) fn load_toml(root: &Path, relative: &str) -> Result<Value, String> {
    let text = read_text(root, relative)?;
    toml::from_str(&text).map_err(|error| format!("{relative}: {error}"))
}

pub(super) fn read_text(root: &Path, relative: &str) -> Result<String, String> {
    std::fs::read_to_string(root.join(relative))
        .map_err(|error| format!("{relative}: {error}"))
}

pub(super) fn indexed_rows(
    document: &Value,
    key: &str,
    identity: &str,
) -> Result<RowMap, String> {
    let rows = document
        .get(key)
        .and_then(Value::as_array)
        .ok_or_else(|| format!("{key} must be an array of tables"))?;
    let mut result = RowMap::new();
    for row in rows {
        let name = row
            .as_table()
            .and_then(|table| table.get(identity))
            .and_then(Value::as_str)
            .ok_or_else(|| format!("invalid {key} row"))?;
        if result.insert(name.to_owned(), row.clone()).is_some() {
            return Err(format!("duplicate {key} row {name}"));
        }
    }
    Ok(result)
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

pub(super) fn string_list(value: &Value, key: &str) -> Option<Vec<String>> {
    value
        .get(key)?
        .as_array()?
        .iter()
        .map(|item| item.as_str().map(str::to_owned))
        .collect()
}

pub(super) fn require(
    errors: &mut Vec<String>,
    condition: bool,
    message: impl Into<String>,
) {
    if !condition {
        errors.push(message.into());
    }
}

pub(super) fn validate_module_ref(
    errors: &mut Vec<String>,
    reference: &str,
    modules: &ModuleMap,
    owner: &str,
) {
    let Some((package, module)) = reference.split_once(':') else {
        errors.push(format!("{owner}: invalid module ref {reference}"));
        return;
    };
    match modules.get(package) {
        None => errors.push(format!(
            "{owner}: unknown package in module ref {reference}"
        )),
        Some(names) if !names.contains(module) => errors.push(format!(
            "{owner}: unknown module in ref {reference}"
        )),
        Some(_) => {}
    }
}

pub(super) fn validate_owner_pair(
    errors: &mut Vec<String>,
    package: Option<&str>,
    module: Option<&str>,
    modules: &ModuleMap,
    owner: &str,
    allow_none: bool,
) {
    if allow_none && package == Some("NONE") && module == Some("NONE") {
        return;
    }
    let (Some(package), Some(module)) = (package, module) else {
        errors.push(format!("{owner}: owner package/module must be strings"));
        return;
    };
    validate_module_ref(errors, &format!("{package}:{module}"), modules, owner);
}

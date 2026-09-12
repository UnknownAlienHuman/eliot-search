//! Package-map input loading over canonical checked-in coverage registries.

use std::collections::BTreeMap;
use std::fs;
use std::path::Path;

use toml::Value;

pub(super) type RowMap = BTreeMap<String, Value>;

#[derive(Clone, Debug)]
pub(super) struct ArchitectureRelation {
    pub(super) kind: String,
    pub(super) id: String,
    pub(super) modules: Vec<String>,
    pub(super) required_outputs: Vec<String>,
    pub(super) exit_evidence: Vec<String>,
}

#[derive(Clone, Debug)]
pub(super) struct SchemaOwner {
    pub(super) kind: String,
    pub(super) package: String,
    pub(super) module: String,
}

#[derive(Clone, Debug)]
pub(super) struct SchemaRelation {
    pub(super) packet: String,
    pub(super) group: String,
    pub(super) schemas: Vec<String>,
    pub(super) source_files: Vec<String>,
    pub(super) owners: Vec<SchemaOwner>,
}

pub(super) struct Inputs {
    pub(super) manifest: Value,
    pub(super) root_cargo: Value,
    pub(super) package_rows: RowMap,
    pub(super) index: Value,
    pub(super) index_rows: RowMap,
    pub(super) module_rows: RowMap,
    pub(super) operation_rows: RowMap,
    pub(super) document_rows: RowMap,
    pub(super) dependency_rows: RowMap,
    pub(super) integration_rows: RowMap,
    pub(super) port_document: Value,
    pub(super) port_rows: RowMap,
    pub(super) config_rows: RowMap,
    pub(super) recipe_rows: RowMap,
    pub(super) override_rows: RowMap,
    pub(super) architecture: Vec<ArchitectureRelation>,
    pub(super) schemas: Vec<SchemaRelation>,
}

impl Inputs {
    pub(super) fn load(root: &Path) -> Result<Self, String> {
        let manifest = load_toml(root, "swarm/coverage/manifest.toml")?;
        let package_document = load_toml(
            root,
            manifest_path(&manifest, "package_registry", "swarm/crates.toml"),
        )?;
        let index = load_toml(root, super::INDEX_PATH)?;
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
        let document_document = load_toml(
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
        let integration_document = load_toml(root, super::INTEGRATION_PATH)?;
        let port_document = load_toml(
            root,
            manifest_path(&manifest, "port_registry", "swarm/coverage/ports.toml"),
        )?;
        let config_document = load_toml(
            root,
            manifest_path(&manifest, "configuration_registry", "config/sections.toml"),
        )?;
        let recipe_document = load_toml(
            root,
            manifest_path(&manifest, "recipe_registry", "swarm/coverage/recipes.toml"),
        )?;
        let stage_readsets = load_toml(root, "swarm/stage-readsets.toml")?;
        let root_cargo = load_toml(root, "Cargo.toml")?;
        let architecture = load_architecture(root, &manifest)?;
        let schemas = load_schemas(root, &manifest)?;

        Ok(Self {
            manifest,
            root_cargo,
            package_rows: indexed_rows(&package_document, "package", "name")?,
            index_rows: indexed_rows(&index, "package", "name")?,
            index,
            module_rows: indexed_rows(&module_document, "module", "id")?,
            operation_rows: indexed_rows(&operation_document, "operation", "id")?,
            document_rows: indexed_rows(&document_document, "node", "id")?,
            dependency_rows: indexed_rows(&dependency_document, "edge", "id")?,
            integration_rows: indexed_rows(&integration_document, "node", "id")?,
            port_rows: indexed_rows(&port_document, "port", "name")?,
            port_document,
            config_rows: indexed_rows(&config_document, "section", "name")?,
            recipe_rows: indexed_rows(&recipe_document, "recipe", "id")?,
            override_rows: indexed_rows(&stage_readsets, "override", "id")?,
            architecture,
            schemas,
        })
    }
}

fn manifest_path<'a>(manifest: &'a Value, key: &str, fallback: &'a str) -> &'a str {
    string(manifest, key).unwrap_or(fallback)
}

fn load_architecture(
    root: &Path,
    manifest: &Value,
) -> Result<Vec<ArchitectureRelation>, String> {
    let specs = [
        (
            "architecture_section",
            "architecture_section_registry",
            "swarm/coverage/architecture-sections.toml",
            "section",
        ),
        (
            "capability",
            "capability_registry",
            "swarm/coverage/capabilities.toml",
            "cell",
        ),
        (
            "invariant",
            "invariant_registry",
            "swarm/coverage/invariants.toml",
            "invariant",
        ),
        (
            "delivery",
            "delivery_registry",
            "swarm/coverage/delivery-slices.toml",
            "slice",
        ),
    ];
    let mut result = Vec::new();
    for (kind, manifest_key, fallback, table) in specs {
        let document = load_toml(root, manifest_path(manifest, manifest_key, fallback))?;
        for (id, row) in indexed_rows(&document, table, "id")? {
            result.push(ArchitectureRelation {
                kind: kind.to_owned(),
                id,
                modules: strings(&row, "modules"),
                required_outputs: strings(&row, "required_outputs"),
                exit_evidence: strings(&row, "exit_evidence"),
            });
        }
    }
    result.sort_by(|left, right| {
        (&left.kind, &left.id).cmp(&(&right.kind, &right.id))
    });
    Ok(result)
}

fn load_schemas(root: &Path, manifest: &Value) -> Result<Vec<SchemaRelation>, String> {
    let registry = load_toml(
        root,
        manifest_path(manifest, "schema_registry", "swarm/coverage/schemas.toml"),
    )?;
    let packets = array(&registry, "packet")
        .ok_or_else(|| "schema registry packet must be an array".to_owned())?;
    let mut result = Vec::new();
    for packet in packets {
        let path = string(packet, "path")
            .ok_or_else(|| "schema packet path missing".to_owned())?;
        let document = load_toml(root, path)?;
        let groups = array(&document, "group")
            .ok_or_else(|| format!("{path}: group must be an array"))?;
        for group in groups {
            let group_id = string(group, "id").unwrap_or("UNNAMED").to_owned();
            let mut owners = Vec::new();
            for owner_kind in ["shape_owner", "meaning_owner", "state_owner"] {
                let package_key = format!("{owner_kind}_package");
                let module_key = format!("{owner_kind}_module");
                let package = string(group, &package_key).unwrap_or("NONE");
                let module = string(group, &module_key).unwrap_or("NONE");
                if package != "NONE" && module != "NONE" {
                    owners.push(SchemaOwner {
                        kind: owner_kind.to_owned(),
                        package: package.to_owned(),
                        module: module.to_owned(),
                    });
                }
            }
            result.push(SchemaRelation {
                packet: path.to_owned(),
                group: group_id,
                schemas: strings(group, "schemas"),
                source_files: strings(group, "source_files"),
                owners,
            });
        }
    }
    result.sort_by(|left, right| {
        (&left.packet, &left.group).cmp(&(&right.packet, &right.group))
    });
    Ok(result)
}

pub(super) fn read_text(root: &Path, relative: &str) -> Result<String, String> {
    fs::read_to_string(root.join(relative)).map_err(|error| format!("{relative}: {error}"))
}

pub(super) fn load_toml(root: &Path, relative: &str) -> Result<Value, String> {
    let text = read_text(root, relative)?;
    toml::from_str(&text).map_err(|error| format!("{relative}: {error}"))
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
            .ok_or_else(|| format!("{key}: missing {identity}"))?;
        if result.insert(name.to_owned(), row.clone()).is_some() {
            return Err(format!("{key}: duplicate {identity} {name}"));
        }
    }
    Ok(result)
}

pub(super) fn array<'a>(document: &'a Value, key: &str) -> Option<&'a [Value]> {
    document.get(key)?.as_array().map(Vec::as_slice)
}

pub(super) fn table<'a>(document: &'a Value, key: &str) -> Option<&'a toml::map::Map<String, Value>> {
    document.get(key)?.as_table()
}

pub(super) fn string<'a>(document: &'a Value, key: &str) -> Option<&'a str> {
    document.get(key)?.as_str()
}

pub(super) fn integer(document: &Value, key: &str) -> Option<i64> {
    document.get(key)?.as_integer()
}

pub(super) fn boolean(document: &Value, key: &str) -> Option<bool> {
    document.get(key)?.as_bool()
}

pub(super) fn strings(document: &Value, key: &str) -> Vec<String> {
    array(document, key)
        .map(|items| {
            items
                .iter()
                .filter_map(|item| item.as_str().map(str::to_owned))
                .collect()
        })
        .unwrap_or_default()
}

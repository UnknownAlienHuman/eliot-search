//! Package-local dependency and architecture relation closure.

use std::collections::{BTreeMap, BTreeSet};
use std::path::Path;

use toml::Value;

use super::super::load::{
    Inputs, SchemaOwner, array, boolean, integer, load_toml, string, strings,
};
use super::super::package_paths;

pub(super) fn validate(root: &Path, inputs: &Inputs, errors: &mut Vec<String>) {
    for package in inputs.package_rows.keys() {
        validate_package(root, inputs, package, errors);
    }
}

fn validate_package(
    root: &Path,
    inputs: &Inputs,
    package: &str,
    errors: &mut Vec<String>,
) {
    let path = package_paths(package).relations;
    let document = match load_toml(root, &path) {
        Ok(document) => document,
        Err(error) => {
            errors.push(error);
            return;
        }
    };

    let expected_dependencies = expected_dependencies(inputs, package);
    let actual_dependencies = keyed_rows(
        &document,
        "dependency",
        |row| {
            Some(format!(
                "{}|{}",
                string(row, "direction")?,
                string(row, "id")?
            ))
        },
        errors,
    );
    compare_keys(
        package,
        "dependency",
        &actual_dependencies,
        &expected_dependencies,
        errors,
    );
    for (key, expected) in &expected_dependencies {
        let Some(actual) = actual_dependencies.get(key) else {
            continue;
        };
        for field in [
            "direction",
            "id",
            "consumer",
            "consumer_module",
            "producer",
            "producer_module",
            "relationship",
            "contract_source",
            "reentry_stage",
        ] {
            if string(actual, field) != string(expected, field) {
                errors.push(format!("{package}:{key}: dependency {field} mismatch"));
            }
        }
        for field in ["consumer_earliest_wave", "producer_earliest_wave"] {
            if integer(actual, field) != integer(expected, field) {
                errors.push(format!("{package}:{key}: dependency {field} mismatch"));
            }
        }
        for field in [
            "requires_stage_reentry",
            "exact_accepted_handoff_required",
        ] {
            if boolean(actual, field) != boolean(expected, field) {
                errors.push(format!("{package}:{key}: dependency {field} mismatch"));
            }
        }
    }

    let expected_architecture = expected_architecture(inputs, package);
    let actual_architecture = keyed_rows(
        &document,
        "architecture",
        |row| {
            Some(format!(
                "{}|{}",
                string(row, "kind")?,
                string(row, "id")?
            ))
        },
        errors,
    );
    compare_keys(
        package,
        "architecture",
        &actual_architecture,
        &expected_architecture,
        errors,
    );
    for (key, expected) in &expected_architecture {
        let Some(actual) = actual_architecture.get(key) else {
            continue;
        };
        for field in ["modules", "required_outputs", "exit_evidence"] {
            if strings(actual, field) != strings(expected, field) {
                errors.push(format!("{package}:{key}: architecture {field} mismatch"));
            }
        }
    }

    let expected_configs: BTreeMap<String, Value> = inputs
        .config_rows
        .iter()
        .filter(|(_, row)| string(row, "owner") == Some(package))
        .map(|(name, row)| (name.clone(), row.clone()))
        .collect();
    let actual_configs = keyed_rows(
        &document,
        "configuration",
        |row| string(row, "section").map(str::to_owned),
        errors,
    );
    compare_keys(
        package,
        "configuration",
        &actual_configs,
        &expected_configs,
        errors,
    );
    for (name, expected) in &expected_configs {
        let Some(actual) = actual_configs.get(name) else {
            continue;
        };
        if string(actual, "module") != string(expected, "owner_module")
            || string(actual, "contract") != string(expected, "contract")
            || string(actual, "reload") != string(expected, "reload")
        {
            errors.push(format!("{package}:{name}: configuration mismatch"));
        }
    }

    let expected_recipes = expected_recipes(inputs, package);
    let actual_recipes = keyed_rows(
        &document,
        "recipe",
        |row| {
            Some(format!(
                "{}|{}",
                string(row, "id")?,
                string(row, "module")?
            ))
        },
        errors,
    );
    compare_keys(
        package,
        "recipe",
        &actual_recipes,
        &expected_recipes,
        errors,
    );
    for (key, expected) in &expected_recipes {
        let Some(actual) = actual_recipes.get(key) else {
            continue;
        };
        for field in ["request_schema", "result_schema"] {
            if string(actual, field) != string(expected, field) {
                errors.push(format!("{package}:{key}: recipe {field} mismatch"));
            }
        }
    }

    let expected_ports: BTreeMap<String, Value> = inputs
        .port_rows
        .iter()
        .filter(|(_, row)| string(row, "implementation_package") == Some(package))
        .map(|(name, row)| (name.clone(), row.clone()))
        .collect();
    let actual_ports = keyed_rows(
        &document,
        "port",
        |row| string(row, "name").map(str::to_owned),
        errors,
    );
    compare_keys(package, "port", &actual_ports, &expected_ports, errors);
    for (name, expected) in &expected_ports {
        let Some(actual) = actual_ports.get(name) else {
            continue;
        };
        if string(actual, "module") != string(expected, "implementation_module")
            || strings(actual, "methods") != strings(expected, "methods")
            || strings(actual, "method_modules") != strings(expected, "method_modules")
        {
            errors.push(format!("{package}:{name}: port relation mismatch"));
        }
    }

    let expected_schemas = expected_schemas(inputs, package);
    let actual_schemas = keyed_rows(
        &document,
        "schema_group",
        |row| {
            Some(format!(
                "{}|{}",
                string(row, "packet")?,
                string(row, "group")?
            ))
        },
        errors,
    );
    compare_keys(
        package,
        "schema",
        &actual_schemas,
        &expected_schemas,
        errors,
    );
    for (key, expected) in &expected_schemas {
        let Some(actual) = actual_schemas.get(key) else {
            continue;
        };
        for field in ["owner_roles", "modules", "schemas", "source_files"] {
            if strings(actual, field) != strings(expected, field) {
                errors.push(format!("{package}:{key}: schema {field} mismatch"));
            }
        }
    }

    let outbound_count = expected_dependencies
        .keys()
        .filter(|key| key.starts_with("outbound|"))
        .count();
    let inbound_count = expected_dependencies
        .keys()
        .filter(|key| key.starts_with("inbound|"))
        .count();
    let architecture_count = expected_architecture.len();
    let configuration_count = expected_configs.len();
    let recipe_count = expected_recipes.len();
    let port_count = expected_ports.len();
    let schema_count = expected_schemas.len();

    for (field, expected) in [
        ("outbound_dependency_count", outbound_count),
        ("inbound_dependency_count", inbound_count),
        ("architecture_relation_count", architecture_count),
        ("configuration_relation_count", configuration_count),
        ("recipe_relation_count", recipe_count),
        ("port_relation_count", port_count),
        ("schema_relation_count", schema_count),
    ] {
        if integer(&document, field) != i64::try_from(expected).ok() {
            errors.push(format!("{package}: relation {field} mismatch"));
        }
    }
    if let Some(index) = inputs.index_rows.get(package) {
        for (field, expected) in [
            ("outbound_dependencies_count", outbound_count),
            ("inbound_dependencies_count", inbound_count),
            ("architecture_count", architecture_count),
            ("configuration_count", configuration_count),
            ("recipes_count", recipe_count),
            ("ports_count", port_count),
            ("schemas_count", schema_count),
        ] {
            if integer(index, field) != i64::try_from(expected).ok() {
                errors.push(format!("{package}: index {field} mismatch"));
            }
        }
    }
}

fn expected_dependencies(inputs: &Inputs, package: &str) -> BTreeMap<String, Value> {
    let mut result = BTreeMap::new();
    for (id, row) in &inputs.dependency_rows {
        if string(row, "consumer") == Some(package) {
            let mut mapped = row.clone();
            if let Some(table) = mapped.as_table_mut() {
                table.insert(
                    "direction".to_owned(),
                    Value::String("outbound".to_owned()),
                );
            }
            result.insert(format!("outbound|{id}"), mapped);
        }
        if string(row, "producer") == Some(package) {
            let mut mapped = row.clone();
            if let Some(table) = mapped.as_table_mut() {
                table.insert(
                    "direction".to_owned(),
                    Value::String("inbound".to_owned()),
                );
            }
            result.insert(format!("inbound|{id}"), mapped);
        }
    }
    result
}

fn expected_architecture(inputs: &Inputs, package: &str) -> BTreeMap<String, Value> {
    let prefix = format!("{package}:");
    let mut result = BTreeMap::new();
    for relation in &inputs.architecture {
        let mut modules: Vec<String> = relation
            .modules
            .iter()
            .filter(|module| module.starts_with(&prefix))
            .cloned()
            .collect();
        if modules.is_empty() {
            continue;
        }
        modules.sort();
        let value = object([
            ("kind", Value::String(relation.kind.clone())),
            ("id", Value::String(relation.id.clone())),
            ("modules", string_array(modules)),
            (
                "required_outputs",
                string_array(relation.required_outputs.clone()),
            ),
            (
                "exit_evidence",
                string_array(relation.exit_evidence.clone()),
            ),
        ]);
        result.insert(format!("{}|{}", relation.kind, relation.id), value);
    }
    result
}

fn expected_recipes(inputs: &Inputs, package: &str) -> BTreeMap<String, Value> {
    let prefix = format!("{package}:");
    let mut result = BTreeMap::new();
    for (id, row) in &inputs.recipe_rows {
        for reference in strings(row, "execution_modules") {
            let Some(module) = reference.strip_prefix(&prefix) else {
                continue;
            };
            let value = object([
                ("id", Value::String(id.clone())),
                ("module", Value::String(module.to_owned())),
                (
                    "request_schema",
                    Value::String(
                        string(row, "request_schema")
                            .unwrap_or("None")
                            .to_owned(),
                    ),
                ),
                (
                    "result_schema",
                    Value::String(
                        string(row, "result_schema")
                            .unwrap_or("None")
                            .to_owned(),
                    ),
                ),
            ]);
            result.insert(format!("{id}|{module}"), value);
        }
    }
    result
}

fn expected_schemas(inputs: &Inputs, package: &str) -> BTreeMap<String, Value> {
    let mut result = BTreeMap::new();
    for relation in &inputs.schemas {
        let owners: Vec<&SchemaOwner> = relation
            .owners
            .iter()
            .filter(|owner| owner.package == package)
            .collect();
        if owners.is_empty() {
            continue;
        }
        let mut owner_roles: Vec<String> =
            owners.iter().map(|owner| owner.kind.clone()).collect();
        let mut modules: Vec<String> =
            owners.iter().map(|owner| owner.module.clone()).collect();
        owner_roles.sort();
        modules.sort();
        modules.dedup();
        let value = object([
            ("packet", Value::String(relation.packet.clone())),
            ("group", Value::String(relation.group.clone())),
            ("owner_roles", string_array(owner_roles)),
            ("modules", string_array(modules)),
            ("schemas", string_array(relation.schemas.clone())),
            ("source_files", string_array(relation.source_files.clone())),
        ]);
        result.insert(format!("{}|{}", relation.packet, relation.group), value);
    }
    result
}

fn object<const N: usize>(entries: [(&str, Value); N]) -> Value {
    let mut table = toml::map::Map::new();
    for (key, value) in entries {
        table.insert(key.to_owned(), value);
    }
    Value::Table(table)
}

fn string_array(values: Vec<String>) -> Value {
    Value::Array(values.into_iter().map(Value::String).collect())
}

fn keyed_rows(
    document: &Value,
    table: &str,
    key: impl Fn(&Value) -> Option<String>,
    errors: &mut Vec<String>,
) -> BTreeMap<String, Value> {
    let mut result = BTreeMap::new();
    let Some(rows) = array(document, table) else {
        return result;
    };
    for row in rows {
        let Some(identity) = key(row) else {
            errors.push(format!("{table}: row identity missing"));
            continue;
        };
        if result.insert(identity.clone(), row.clone()).is_some() {
            errors.push(format!("{table}: duplicate row {identity}"));
        }
    }
    result
}

fn compare_keys(
    package: &str,
    label: &str,
    actual: &BTreeMap<String, Value>,
    expected: &BTreeMap<String, Value>,
    errors: &mut Vec<String>,
) {
    let actual: BTreeSet<String> = actual.keys().cloned().collect();
    let expected: BTreeSet<String> = expected.keys().cloned().collect();
    if actual != expected {
        let difference: Vec<String> = actual
            .symmetric_difference(&expected)
            .cloned()
            .collect();
        errors.push(format!(
            "{package}: {label} relation closure mismatch: {difference:?}"
        ));
    }
}

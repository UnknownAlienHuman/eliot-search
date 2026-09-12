//! Package-local module, operation and documentation-map closure.

use std::collections::BTreeSet;
use std::path::Path;

use toml::Value;

use super::super::load::{
    Inputs, RowMap, indexed_rows, integer, load_toml, string, strings,
};
use super::super::package_paths;

pub(super) fn validate(root: &Path, inputs: &Inputs, errors: &mut Vec<String>) {
    for package in inputs.package_rows.keys() {
        validate_package(root, inputs, package, errors);
    }
    validate_integration(inputs, errors);
}

fn validate_package(
    root: &Path,
    inputs: &Inputs,
    package: &str,
    errors: &mut Vec<String>,
) {
    let paths = package_paths(package);
    let overview = load_map(root, &paths.overview, errors);
    let operations = load_map(root, &paths.operations, errors);
    let documents = load_map(root, &paths.documents, errors);

    let actual_modules = rows_or_empty(&overview, "module", "name", errors);
    let expected_modules: RowMap = inputs
        .module_rows
        .iter()
        .filter(|(_, row)| string(row, "package") == Some(package))
        .map(|(id, row)| {
            (
                string(row, "module").unwrap_or(id).to_owned(),
                row.clone(),
            )
        })
        .collect();
    compare_keys(
        package,
        "module",
        &actual_modules,
        &expected_modules,
        errors,
    );
    for (name, expected) in &expected_modules {
        let Some(actual) = actual_modules.get(name) else {
            continue;
        };
        compare_string(actual, "role", expected, "role", package, name, errors);
        compare_string(
            actual,
            "structural_rationale",
            expected,
            "structural_rationale",
            package,
            name,
            errors,
        );
        for key in [
            "operation_count",
            "documentation_node_count",
            "architecture_relation_count",
            "port_relation_count",
            "port_method_relation_count",
            "schema_relation_count",
            "configuration_relation_count",
            "recipe_relation_count",
            "dependency_relation_count",
        ] {
            if integer(actual, key) != integer(expected, key) {
                errors.push(format!("{package}:{name}: module {key} mismatch"));
            }
        }
    }

    let actual_operations = rows_or_empty(&operations, "operation", "id", errors);
    let expected_operations: RowMap = inputs
        .operation_rows
        .iter()
        .filter(|(_, row)| string(row, "package") == Some(package))
        .map(|(id, row)| (id.clone(), row.clone()))
        .collect();
    compare_keys(
        package,
        "operation",
        &actual_operations,
        &expected_operations,
        errors,
    );
    for (id, expected) in &expected_operations {
        let Some(actual) = actual_operations.get(id) else {
            continue;
        };
        for (actual_key, expected_key) in [
            ("name", "operation"),
            ("module", "module"),
            ("public_entry_module", "public_entry_module"),
            ("route_kind", "route_kind"),
        ] {
            compare_string(
                actual,
                actual_key,
                expected,
                expected_key,
                package,
                id,
                errors,
            );
        }
        for key in ["sources", "source_contexts"] {
            if strings(actual, key) != strings(expected, key) {
                errors.push(format!("{id}: operation {key} mismatch"));
            }
        }
        if integer(actual, "score") != integer(expected, "score") {
            errors.push(format!("{id}: operation score mismatch"));
        }
        if matches!(string(actual, "route_kind"), Some("public_facade" | "semantic_low")) {
            errors.push(format!("{id}: unreviewed operation route"));
        }
        let module = string(actual, "module").unwrap_or_default();
        if !inputs
            .module_rows
            .contains_key(&format!("{package}:{module}"))
        {
            errors.push(format!("{id}: package map module invalid"));
        }
    }

    let actual_documents = rows_or_empty(&documents, "node", "id", errors);
    let expected_documents: RowMap = inputs
        .document_rows
        .iter()
        .filter(|(_, row)| strings(row, "packages").iter().any(|value| value == package))
        .map(|(id, row)| (id.clone(), row.clone()))
        .collect();
    compare_keys(
        package,
        "document",
        &actual_documents,
        &expected_documents,
        errors,
    );
    for (id, expected) in &expected_documents {
        let Some(actual) = actual_documents.get(id) else {
            continue;
        };
        for key in ["path", "heading", "kind", "route_kind", "rationale"] {
            compare_string(actual, key, expected, key, package, id, errors);
        }
        for key in ["line", "level"] {
            if integer(actual, key) != integer(expected, key) {
                errors.push(format!("{package}:{id}: document {key} mismatch"));
            }
        }
        let mut expected_modules: Vec<String> = strings(expected, "modules")
            .into_iter()
            .filter(|module| module.starts_with(&format!("{package}:")))
            .collect();
        expected_modules.sort();
        let actual_modules = strings(actual, "modules");
        if actual_modules != expected_modules {
            errors.push(format!("{package}:{id}: document module route mismatch"));
        }
        if actual_modules.is_empty() {
            errors.push(format!("{package}:{id}: document route empty"));
        }
        if actual_modules
            .iter()
            .any(|module| !module.starts_with(&format!("{package}:")))
        {
            errors.push(format!("{package}:{id}: foreign module in package map"));
        }
    }

    let principles = expected_documents
        .values()
        .filter(|row| string(row, "kind") == Some("principle_or_invariant"))
        .count();
    let Some(index_row) = inputs.index_rows.get(package) else {
        return;
    };
    for (key, expected) in [
        ("modules_count", expected_modules.len()),
        ("operations_count", expected_operations.len()),
        ("documents_count", expected_documents.len()),
        ("principles_count", principles),
    ] {
        if integer(index_row, key) != i64::try_from(expected).ok() {
            errors.push(format!("{package}: index {key} mismatch"));
        }
    }
    for (key, expected) in [
        ("module_count", expected_modules.len()),
        ("operation_count", expected_operations.len()),
        ("documentation_node_count", expected_documents.len()),
        ("principle_node_count", principles),
    ] {
        if integer(&overview, key) != i64::try_from(expected).ok() {
            errors.push(format!("{package}: overview {key} mismatch"));
        }
    }
    if integer(&operations, "operation_count")
        != i64::try_from(expected_operations.len()).ok()
    {
        errors.push(format!("{package}: operations count mismatch"));
    }
    if integer(&documents, "node_count")
        != i64::try_from(expected_documents.len()).ok()
        || integer(&documents, "principle_count") != i64::try_from(principles).ok()
    {
        errors.push(format!("{package}: documents count mismatch"));
    }
}

fn validate_integration(inputs: &Inputs, errors: &mut Vec<String>) {
    let expected: BTreeSet<String> = inputs
        .document_rows
        .iter()
        .filter(|(_, row)| strings(row, "packages").is_empty())
        .map(|(id, _)| id.clone())
        .collect();
    let actual: BTreeSet<String> = inputs.integration_rows.keys().cloned().collect();
    if actual != expected {
        let difference: Vec<String> = actual
            .symmetric_difference(&expected)
            .cloned()
            .collect();
        errors.push(format!("integration documentation map closure mismatch: {difference:?}"));
    }
    for id in &actual {
        let Some(canonical) = inputs.document_rows.get(id) else {
            continue;
        };
        if !matches!(string(canonical, "kind"), Some("governance" | "navigation")) {
            errors.push(format!("{id}: product-bearing node misclassified as integration"));
        }
        if let Some(mapped) = inputs.integration_rows.get(id) {
            for key in ["path", "heading", "kind", "route_kind", "rationale"] {
                if string(mapped, key) != string(canonical, key) {
                    errors.push(format!("{id}: integration {key} mismatch"));
                }
            }
            if integer(mapped, "line") != integer(canonical, "line") {
                errors.push(format!("{id}: integration line mismatch"));
            }
        }
    }
}

fn load_map(root: &Path, path: &str, errors: &mut Vec<String>) -> Value {
    match load_toml(root, path) {
        Ok(document) => document,
        Err(error) => {
            errors.push(error);
            Value::Table(toml::map::Map::new())
        }
    }
}

fn rows_or_empty(
    document: &Value,
    table: &str,
    identity: &str,
    errors: &mut Vec<String>,
) -> RowMap {
    match indexed_rows(document, table, identity) {
        Ok(rows) => rows,
        Err(error) => {
            errors.push(error);
            RowMap::new()
        }
    }
}

fn compare_keys(
    package: &str,
    label: &str,
    actual: &RowMap,
    expected: &RowMap,
    errors: &mut Vec<String>,
) {
    let actual: BTreeSet<String> = actual.keys().cloned().collect();
    let expected: BTreeSet<String> = expected.keys().cloned().collect();
    if actual != expected {
        let difference: Vec<String> = actual
            .symmetric_difference(&expected)
            .cloned()
            .collect();
        errors.push(format!("{package}: {label} closure mismatch: {difference:?}"));
    }
}

#[allow(clippy::too_many_arguments)]
fn compare_string(
    actual: &Value,
    actual_key: &str,
    expected: &Value,
    expected_key: &str,
    package: &str,
    identity: &str,
    errors: &mut Vec<String>,
) {
    if string(actual, actual_key) != string(expected, expected_key) {
        errors.push(format!(
            "{package}:{identity}: {actual_key}/{expected_key} mismatch"
        ));
    }
}

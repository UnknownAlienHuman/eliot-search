//! Generated-file, manifest and Cargo/dependency closure checks.

use std::collections::BTreeSet;
use std::fs;
use std::path::Path;

use toml::Value;

use super::super::load::{
    Inputs, boolean, integer, load_toml, read_text, string, strings, table,
};
use super::super::{
    DOC_INDEX_PATH, INDEX_PATH, INTEGRATION_PATH, MAP_ROOT, package_paths,
};

pub(super) fn validate(
    root: &Path,
    inputs: &Inputs,
    warnings: &mut Vec<String>,
    errors: &mut Vec<String>,
) {
    validate_manifest(inputs, errors);
    validate_index_and_files(root, inputs, errors);
    validate_workspace(root, inputs, warnings, errors);
    validate_package_manifests(root, inputs, errors);
}

fn validate_manifest(inputs: &Inputs, errors: &mut Vec<String>) {
    for (key, expected) in [
        ("package_map_index", INDEX_PATH),
        ("documentation_file_index", DOC_INDEX_PATH),
        ("integration_documentation_map", INTEGRATION_PATH),
    ] {
        if string(&inputs.manifest, key) != Some(expected) {
            errors.push(format!("manifest {key} mismatch"));
        }
    }
    let counts = [
        ("package_map_count", inputs.package_rows.len()),
        (
            "package_map_file_count",
            inputs.package_rows.len().saturating_mul(4),
        ),
        ("exact_operation_module_count", inputs.operation_rows.len()),
        ("documentation_node_count", inputs.document_rows.len()),
        ("dependency_edge_count", inputs.dependency_rows.len()),
        ("logical_module_count", inputs.module_rows.len()),
        (
            "integration_documentation_node_count",
            inputs.integration_rows.len(),
        ),
    ];
    for (key, expected) in counts {
        if integer(&inputs.manifest, key) != i64::try_from(expected).ok() {
            errors.push(format!("manifest {key} mismatch"));
        }
    }
    if integer(&inputs.manifest, "weak_logical_module_count") != Some(0) {
        errors.push("weak module count must be zero".to_owned());
    }
    for key in [
        "implementation_authorized_by_this_manifest",
        "package_acceptance_claimed",
        "gate_or_wave_acceptance_claimed",
        "runtime_evidence_available",
        "product_acceptance_claimed",
    ] {
        if boolean(&inputs.manifest, key) != Some(false) {
            errors.push(format!("manifest authority changed: {key}"));
        }
    }
}

fn validate_index_and_files(root: &Path, inputs: &Inputs, errors: &mut Vec<String>) {
    let expected_packages: BTreeSet<String> = inputs.package_rows.keys().cloned().collect();
    let indexed_packages: BTreeSet<String> = inputs.index_rows.keys().cloned().collect();
    if indexed_packages != expected_packages {
        let difference: Vec<String> = indexed_packages
            .symmetric_difference(&expected_packages)
            .cloned()
            .collect();
        errors.push(format!("package-map index package closure mismatch: {difference:?}"));
    }
    let index_counts = [
        ("package_count", inputs.package_rows.len()),
        (
            "map_file_count",
            inputs.package_rows.len().saturating_mul(4),
        ),
        ("operation_count", inputs.operation_rows.len()),
        ("documentation_node_count", inputs.document_rows.len()),
        ("dependency_edge_count", inputs.dependency_rows.len()),
        ("logical_module_count", inputs.module_rows.len()),
        ("integration_node_count", inputs.integration_rows.len()),
    ];
    for (key, expected) in index_counts {
        if integer(&inputs.index, key) != i64::try_from(expected).ok() {
            errors.push(format!("package-map index {key} mismatch"));
        }
    }

    let mut expected_files = BTreeSet::new();
    for (package, package_row) in &inputs.package_rows {
        let paths = package_paths(package);
        let Some(index_row) = inputs.index_rows.get(package) else {
            continue;
        };
        if string(index_row, "path") != string(package_row, "path")
            || integer(index_row, "wave") != integer(package_row, "wave")
            || string(index_row, "family") != string(package_row, "family")
        {
            errors.push(format!("{package}: index identity mismatch"));
        }
        for (kind, relative) in [
            ("overview", paths.overview.as_str()),
            ("operations", paths.operations.as_str()),
            ("documents", paths.documents.as_str()),
            ("relations", paths.relations.as_str()),
        ] {
            expected_files.insert(relative.to_owned());
            let map_key = format!("{kind}_map");
            let digest_key = format!("{kind}_sha256");
            if string(index_row, &map_key) != Some(relative) {
                errors.push(format!("{package}: {kind} map path mismatch"));
            }
            match read_text(root, relative) {
                Ok(actual) => {
                    if actual.lines().count().saturating_add(1) >= 10_000 {
                        errors.push(format!("map exceeds 10k lines: {relative}"));
                    }
                    let digest = crate::coverage_graph::digest_text(&actual);
                    if string(index_row, &digest_key) != Some(digest.as_str()) {
                        errors.push(format!("{package}: {kind} map digest mismatch"));
                    }
                    match toml::from_str::<Value>(&actual) {
                        Ok(document) => {
                            if string(&document, "package") != Some(package) {
                                errors.push(format!("{package}: {kind} map package mismatch"));
                            }
                            if kind == "overview" {
                                if string(&document, "operations_map")
                                    != Some(paths.operations.as_str())
                                    || string(&document, "documents_map")
                                        != Some(paths.documents.as_str())
                                    || string(&document, "relations_map")
                                        != Some(paths.relations.as_str())
                                {
                                    errors.push(format!("{package}: overview links mismatch"));
                                }
                            }
                        }
                        Err(error) => errors.push(format!("{relative}: {error}")),
                    }
                }
                Err(_) => errors.push(format!("missing generated map {relative}")),
            }
        }
    }

    let mut actual_files = BTreeSet::new();
    let map_root = root.join(MAP_ROOT);
    if map_root.exists() {
        collect_toml_files(root, &map_root, &mut actual_files, errors);
    }
    if actual_files != expected_files {
        let difference: Vec<String> = actual_files
            .symmetric_difference(&expected_files)
            .cloned()
            .collect();
        errors.push(format!("orphan/missing package maps: {difference:?}"));
    }
}

fn collect_toml_files(
    root: &Path,
    directory: &Path,
    output: &mut BTreeSet<String>,
    errors: &mut Vec<String>,
) {
    let entries = match fs::read_dir(directory) {
        Ok(entries) => entries,
        Err(error) => {
            errors.push(format!("{}: {error}", directory.display()));
            return;
        }
    };
    for entry in entries.flatten() {
        let path = entry.path();
        match entry.file_type() {
            Ok(file_type) if file_type.is_dir() => {
                collect_toml_files(root, &path, output, errors);
            }
            Ok(file_type)
                if file_type.is_file()
                    && path.extension().and_then(|value| value.to_str()) == Some("toml") =>
            {
                if let Ok(relative) = path.strip_prefix(root) {
                    output.insert(relative.to_string_lossy().replace('\\', "/"));
                }
            }
            Ok(_) => {}
            Err(error) => errors.push(format!("{}: {error}", path.display())),
        }
    }
}

fn validate_workspace(
    _root: &Path,
    inputs: &Inputs,
    warnings: &mut Vec<String>,
    errors: &mut Vec<String>,
) {
    let Some(workspace) = table(&inputs.root_cargo, "workspace") else {
        errors.push("root Cargo.toml lacks [workspace]".to_owned());
        return;
    };
    let members: BTreeSet<String> = workspace
        .get("members")
        .and_then(Value::as_array)
        .map(|items| {
            items
                .iter()
                .filter_map(|item| item.as_str().map(str::to_owned))
                .collect()
        })
        .unwrap_or_default();
    let registered_paths: BTreeSet<String> = inputs
        .package_rows
        .values()
        .filter_map(|row| string(row, "path").map(str::to_owned))
        .collect();
    let missing: Vec<String> = registered_paths.difference(&members).cloned().collect();
    if !missing.is_empty() {
        errors.push(format!("registered packages missing from workspace: {missing:?}"));
    }
    let extras: Vec<String> = members.difference(&registered_paths).cloned().collect();
    if !extras.is_empty() {
        warnings.push(format!("workspace-only members: {extras:?}"));
    }

    let known: BTreeSet<String> = inputs.package_rows.keys().cloned().collect();
    let internal_workspace_dependencies: BTreeSet<String> = workspace
        .get("dependencies")
        .and_then(Value::as_table)
        .map(|dependencies| {
            dependencies
                .keys()
                .filter(|name| known.contains(*name))
                .cloned()
                .collect()
        })
        .unwrap_or_default();
    let library_packages: BTreeSet<String> = inputs
        .package_rows
        .iter()
        .filter(|(_, row)| string(row, "kind") == Some("lib"))
        .map(|(name, _)| name.clone())
        .collect();
    if internal_workspace_dependencies != library_packages {
        let difference: Vec<String> = internal_workspace_dependencies
            .symmetric_difference(&library_packages)
            .cloned()
            .collect();
        errors.push(format!("workspace dependency/package library mismatch: {difference:?}"));
    }
}

fn validate_package_manifests(root: &Path, inputs: &Inputs, errors: &mut Vec<String>) {
    let known: BTreeSet<String> = inputs.package_rows.keys().cloned().collect();
    for (package, row) in &inputs.package_rows {
        let Some(package_path) = string(row, "path") else {
            errors.push(format!("{package}: package path missing"));
            continue;
        };
        let manifest_path = format!("{package_path}/Cargo.toml");
        let package_manifest = match load_toml(root, &manifest_path) {
            Ok(document) => document,
            Err(error) => {
                errors.push(error);
                continue;
            }
        };
        let cargo_name = table(&package_manifest, "package")
            .and_then(|section| section.get("name"))
            .and_then(Value::as_str);
        if cargo_name != Some(package) {
            errors.push(format!("{package}: Cargo package name mismatch"));
        }
        let declared: BTreeSet<String> = strings(row, "deps").into_iter().collect();
        let actual = internal_dependencies(&package_manifest, &known);
        if actual != declared {
            errors.push(format!(
                "{package}: Cargo/registry dependency mismatch: cargo={actual:?} registry={declared:?}"
            ));
        }
        let consumer_wave = integer(row, "wave").unwrap_or(0);
        for producer in declared {
            let producer_wave = inputs
                .package_rows
                .get(&producer)
                .and_then(|value| integer(value, "wave"))
                .unwrap_or(0);
            if producer_wave <= consumer_wave {
                continue;
            }
            let override_id = format!("W{producer_wave}.{package}");
            let Some(override_row) = inputs.override_rows.get(&override_id) else {
                errors.push(format!(
                    "{package}: later-wave dependency {producer} lacks exact {override_id} reentry"
                ));
                continue;
            };
            if string(override_row, "package") != Some(package)
                || integer(override_row, "wave") != Some(producer_wave)
                || boolean(override_row, "replace_previous_stage_context") != Some(true)
                || boolean(override_row, "accepted_prior_stage_handoff_only") != Some(true)
                || boolean(override_row, "dependency_implementation_reads_allowed") != Some(false)
            {
                errors.push(format!("{package}: invalid {override_id} reentry"));
            }
        }
    }
}

fn internal_dependencies(document: &Value, known: &BTreeSet<String>) -> BTreeSet<String> {
    let mut result = BTreeSet::new();
    for table_name in ["dependencies", "dev-dependencies", "build-dependencies"] {
        if let Some(dependencies) = table(document, table_name) {
            result.extend(
                dependencies
                    .keys()
                    .filter(|name| known.contains(*name))
                    .cloned(),
            );
        }
    }
    if let Some(targets) = table(document, "target") {
        for target in targets.values().filter_map(Value::as_table) {
            for table_name in ["dependencies", "dev-dependencies", "build-dependencies"] {
                if let Some(dependencies) = target.get(table_name).and_then(Value::as_table) {
                    result.extend(
                        dependencies
                            .keys()
                            .filter(|name| known.contains(*name))
                            .cloned(),
                    );
                }
            }
        }
    }
    result
}

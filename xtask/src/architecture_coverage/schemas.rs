//! P00 schema/type, recipe and reason-code closure.

use std::collections::BTreeSet;
use std::path::Path;

use toml::Value;

use super::{
    load::{
        Inputs, integer, load_toml, read_text, require, string,
        string_list, validate_module_ref, validate_owner_pair,
    },
    markdown::{
        exact_type_registry_symbols, normalize_type_name, recipe_ids,
        reason_codes, top_level_yaml_labels,
    },
    topology::{TopologySummary, rows_or_empty},
};

const COMPLETIONS: [&str; 5] = [
    "RecipeIdV1",
    "RecipeBodyV1",
    "ComparisonAxis",
    "ProtocolRange",
    "PackageOpaque",
];

const EXPECTED_RECIPES: [&str; 11] = [
    "locate@1",
    "find_text@1",
    "inspect_entity@1",
    "compare_implementations@1",
    "explore_entity@1",
    "corpus_profile@1",
    "corpus_delta@1",
    "provenance@1",
    "compile_exact_scan@1",
    "execute_exact_scan@1",
    "expand_handle@1",
];

pub(super) struct SchemaSummary {
    pub(super) schema_total: usize,
    pub(super) type_registry_symbols: usize,
    pub(super) completion_symbols: usize,
    pub(super) primitive_families: usize,
    pub(super) recipe_count: usize,
    pub(super) reason_count: usize,
}

pub(super) fn validate(
    root: &Path,
    inputs: &Inputs,
    topology: &TopologySummary,
    errors: &mut Vec<String>,
) -> SchemaSummary {
    let mut schema_names = BTreeSet::new();
    let mut primitive_registered = BTreeSet::new();
    let mut schema_total = 0_usize;
    let mut primitive_families = 0_usize;

    let Some(packets) = inputs.schema_doc.get("packet").and_then(Value::as_array) else {
        errors.push("schema packet registry missing".to_owned());
        return SchemaSummary {
            schema_total: 0,
            type_registry_symbols: 0,
            completion_symbols: COMPLETIONS.len(),
            primitive_families: 0,
            recipe_count: 0,
            reason_count: 0,
        };
    };

    for packet in packets {
        let Some(path) = packet.get("path").and_then(Value::as_str) else {
            errors.push("invalid schema packet entry".to_owned());
            continue;
        };
        require(
            errors,
            root.join(path).is_file(),
            format!("missing schema packet {path}"),
        );
        let document = match load_toml(root, path) {
            Ok(document) => document,
            Err(error) => {
                errors.push(error);
                continue;
            }
        };
        let Some(groups) = document.get("group").and_then(Value::as_array) else {
            errors.push(format!("{path}: group array missing"));
            continue;
        };
        let mut packet_count = 0_usize;
        for group in groups {
            let Some(group_table) = group.as_table() else {
                errors.push(format!("{path}: invalid group"));
                continue;
            };
            let group_id = group_table
                .get("id")
                .and_then(Value::as_str)
                .unwrap_or("<missing>");
            let names = group_table
                .get("schemas")
                .and_then(Value::as_array)
                .and_then(|items| {
                    items
                        .iter()
                        .map(|item| item.as_str().map(str::to_owned))
                        .collect::<Option<Vec<_>>>()
                });
            require(
                errors,
                names.as_ref().is_some_and(|items| !items.is_empty()),
                format!("{path}:{group_id}: schemas missing"),
            );
            let names = names.unwrap_or_default();
            for name in &names {
                require(
                    errors,
                    !name.is_empty(),
                    format!("{path}:{group_id}: invalid schema name"),
                );
                require(
                    errors,
                    schema_names.insert(name.clone()),
                    format!("duplicate schema/type name {name}"),
                );
                if packet.get("id").and_then(Value::as_str) == Some("primitives") {
                    primitive_registered.insert(name.clone());
                }
                packet_count = packet_count.saturating_add(1);
            }

            let source_files = group_table
                .get("source_files")
                .and_then(Value::as_array)
                .and_then(|items| {
                    items
                        .iter()
                        .map(|item| item.as_str().map(str::to_owned))
                        .collect::<Option<Vec<_>>>()
                });
            require(
                errors,
                source_files.as_ref().is_some_and(|items| !items.is_empty()),
                format!("{path}:{group_id}: source files missing"),
            );
            let mut combined = String::new();
            for source in source_files.unwrap_or_default() {
                require(
                    errors,
                    root.join(&source).is_file(),
                    format!("{path}:{group_id}: missing schema source {source}"),
                );
                if let Ok(text) = read_text(root, &source) {
                    combined.push('\n');
                    combined.push_str(&text);
                }
            }
            for name in &names {
                require(
                    errors,
                    combined.contains(normalize_type_name(name)),
                    format!("{path}:{group_id}: {name} absent from declared sources"),
                );
            }

            for (package_key, module_key) in [
                ("shape_owner_package", "shape_owner_module"),
                ("meaning_owner_package", "meaning_owner_module"),
                ("state_owner_package", "state_owner_module"),
            ] {
                validate_owner_pair(
                    errors,
                    group_table.get(package_key).and_then(Value::as_str),
                    group_table.get(module_key).and_then(Value::as_str),
                    &topology.modules,
                    &format!("schema {group_id}"),
                    true,
                );
            }
            for package in group_table
                .get("secondary_state_owner_packages")
                .and_then(Value::as_array)
                .into_iter()
                .flatten()
                .filter_map(Value::as_str)
            {
                require(
                    errors,
                    topology.packages.contains(package),
                    format!("schema {group_id}: unknown secondary owner {package}"),
                );
            }
        }
        require(
            errors,
            integer(&document, "schema_count") == i64::try_from(packet_count).ok(),
            format!("{path}: schema count mismatch"),
        );
        require(
            errors,
            packet.get("schema_count").and_then(Value::as_integer)
                == i64::try_from(packet_count).ok(),
            format!("{path}: summary schema count mismatch"),
        );
        schema_total = schema_total.saturating_add(packet_count);
        primitive_families = primitive_families.saturating_add(
            document
                .get("primitive_family")
                .and_then(Value::as_array)
                .map_or(0, Vec::len),
        );
    }

    require(
        errors,
        schema_total == 217
            && integer(&inputs.schema_doc, "schema_or_registry_count") == Some(217),
        "schema/type total must be 217",
    );
    require(
        errors,
        integer(&inputs.schema_doc, "type_registry_named_symbol_count") == Some(115),
        "TYPE_REGISTRY symbol count must be 115",
    );
    require(
        errors,
        integer(&inputs.schema_doc, "completion_symbol_count") == Some(5),
        "type completion count must be 5",
    );
    require(
        errors,
        primitive_families == 12
            && integer(&inputs.schema_doc, "canonical_primitive_family_count") == Some(12),
        "canonical primitive family count must be 12",
    );

    let type_registry_symbols = match read_text(root, "docs/contracts/p00/TYPE_REGISTRY.md") {
        Ok(text) => match exact_type_registry_symbols(&text) {
            Ok(symbols) => symbols,
            Err(error) => {
                errors.push(error);
                BTreeSet::new()
            }
        },
        Err(error) => {
            errors.push(error);
            BTreeSet::new()
        }
    };
    require(
        errors,
        type_registry_symbols.len() == 115,
        format!(
            "TYPE_REGISTRY derived symbol count is {}, expected 115",
            type_registry_symbols.len()
        ),
    );
    let completion_symbols: BTreeSet<String> =
        COMPLETIONS.iter().map(|value| (*value).to_owned()).collect();
    let expected_primitives: BTreeSet<String> = type_registry_symbols
        .union(&completion_symbols)
        .cloned()
        .collect();
    require(
        errors,
        expected_primitives == primitive_registered,
        format!(
            "primitive registry mismatch: {:?}",
            expected_primitives
                .symmetric_difference(&primitive_registered)
                .cloned()
                .collect::<Vec<_>>()
        ),
    );

    validate_completion_contract(root, &inputs.p00_manifest, errors);
    let recipe_count = validate_recipes(
        root,
        inputs,
        topology,
        &schema_names,
        errors,
    );
    let reason_count = validate_reasons(root, &inputs.reason_doc, errors);

    SchemaSummary {
        schema_total,
        type_registry_symbols: type_registry_symbols.len(),
        completion_symbols: completion_symbols.len(),
        primitive_families,
        recipe_count,
        reason_count,
    }
}

fn validate_completion_contract(
    root: &Path,
    p00_manifest: &Value,
    errors: &mut Vec<String>,
) {
    let completion_text = read_text(root, "docs/contracts/p00/TYPE_COMPLETIONS.md")
        .unwrap_or_else(|error| {
            errors.push(error);
            String::new()
        });
    for symbol in COMPLETIONS {
        require(
            errors,
            completion_text.contains(symbol),
            format!("TYPE_COMPLETIONS missing {symbol}"),
        );
    }
    let required_files = string_list(p00_manifest, "required_files");
    require(
        errors,
        required_files.as_ref().is_some_and(|files| files.len() == 13),
        "P00 required_files must contain 13 entries",
    );
    require(
        errors,
        required_files
            .as_ref()
            .is_some_and(|files| files.iter().any(|file| file == "TYPE_COMPLETIONS.md")),
        "P00 manifest does not include TYPE_COMPLETIONS.md",
    );
}

fn validate_recipes(
    root: &Path,
    inputs: &Inputs,
    topology: &TopologySummary,
    schema_names: &BTreeSet<String>,
    errors: &mut Vec<String>,
) -> usize {
    let source_files = [
        "docs/contracts/p00/SUPPORT_SCHEMAS.md",
        "docs/contracts/p00/SOURCE_GRAPH.md",
        "docs/contracts/p00/RECIPES.md",
        "docs/contracts/p00/QUERY_AND_RESULTS.md",
        "docs/contracts/p00/RECIPE_RESULTS.md",
        "docs/contracts/p00/PROTOCOL_AND_LIFECYCLE.md",
        "docs/contracts/p00/TYPE_COMPLETIONS.md",
    ];
    let mut source_labels = BTreeSet::new();
    for relative in source_files {
        match read_text(root, relative) {
            Ok(text) => source_labels.extend(top_level_yaml_labels(&text)),
            Err(error) => errors.push(error),
        }
    }
    let recipes_text = read_text(root, "docs/contracts/p00/RECIPES.md")
        .unwrap_or_else(|error| {
            errors.push(error);
            String::new()
        });
    let source_recipes = recipe_ids(&recipes_text);
    let expected: BTreeSet<String> =
        EXPECTED_RECIPES.iter().map(|value| (*value).to_owned()).collect();
    require(
        errors,
        source_recipes == expected,
        "RECIPES.md exact recipe set mismatch",
    );
    let unregistered: BTreeSet<String> = source_labels
        .into_iter()
        .filter(|label| !schema_names.contains(label) && !source_recipes.contains(label))
        .collect();
    require(
        errors,
        unregistered.is_empty(),
        format!("unregistered top-level P00 schema labels: {unregistered:?}"),
    );

    let rows = rows_or_empty(&inputs.recipe_doc, "recipe", "id", errors);
    require(
        errors,
        rows.len() == 11,
        "recipe registry must contain 11 recipes",
    );
    require(
        errors,
        rows.keys().cloned().collect::<BTreeSet<_>>() == source_recipes,
        "recipe registry mismatch",
    );
    for (recipe, row) in &rows {
        require(
            errors,
            string(row, "request_schema").is_some_and(|name| schema_names.contains(name)),
            format!("{recipe}: unknown request schema"),
        );
        require(
            errors,
            string(row, "result_schema").is_some_and(|name| schema_names.contains(name)),
            format!("{recipe}: unknown result schema"),
        );
        let owners = string_list(row, "primary_execution_packages");
        let refs = string_list(row, "execution_modules");
        require(
            errors,
            owners.as_ref().is_some_and(|items| !items.is_empty()),
            format!("{recipe}: execution owners missing"),
        );
        require(
            errors,
            refs.as_ref().zip(owners.as_ref()).is_some_and(|(refs, owners)| refs.len() == owners.len()),
            format!("{recipe}: one execution module per owner required"),
        );
        let mut referenced_packages = BTreeSet::new();
        for reference in refs.unwrap_or_default() {
            validate_module_ref(errors, &reference, &topology.modules, recipe);
            if let Some((package, _)) = reference.split_once(':') {
                referenced_packages.insert(package.to_owned());
            }
        }
        let owner_set: BTreeSet<String> = owners.unwrap_or_default().into_iter().collect();
        for package in &owner_set {
            require(
                errors,
                topology.packages.contains(package),
                format!("{recipe}: unknown execution package {package}"),
            );
        }
        require(
            errors,
            referenced_packages == owner_set,
            format!("{recipe}: execution module/package mismatch"),
        );
    }
    rows.len()
}

fn validate_reasons(
    root: &Path,
    reason_doc: &Value,
    errors: &mut Vec<String>,
) -> usize {
    let source = read_text(root, "docs/contracts/p00/REASON_CODES.md")
        .map(|text| reason_codes(&text))
        .unwrap_or_else(|error| {
            errors.push(error);
            BTreeSet::new()
        });
    let mut registered = BTreeSet::new();
    for (key, expected_count) in [
        ("search_reason_codes", 31_usize),
        ("protocol_error_codes", 10),
        ("contract_error_codes", 10),
    ] {
        let values = reason_doc
            .get(key)
            .and_then(Value::as_table)
            .and_then(|table| table.get("values"))
            .and_then(Value::as_array)
            .and_then(|items| {
                items
                    .iter()
                    .map(|item| item.as_str().map(str::to_owned))
                    .collect::<Option<Vec<_>>>()
            });
        require(
            errors,
            values.is_some(),
            format!("reason registry {key} missing"),
        );
        require(
            errors,
            values.as_ref().is_some_and(|items| items.len() == expected_count),
            format!("reason registry {key} count mismatch"),
        );
        registered.extend(values.unwrap_or_default());
    }
    require(
        errors,
        source == registered,
        format!(
            "reason code registry mismatch: {:?}",
            source
                .symmetric_difference(&registered)
                .cloned()
                .collect::<Vec<_>>()
        ),
    );
    require(
        errors,
        registered.len() == 51,
        "reason code total must be 51",
    );
    registered.len()
}

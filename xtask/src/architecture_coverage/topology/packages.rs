//! Package, assignment, module and function-source topology.

use std::collections::{BTreeSet, VecDeque};
use std::path::Path;

use toml::Value;

use super::rows_or_empty;
use super::super::load::{
    Inputs, ModuleMap, RowMap, boolean, integer, load_toml, require, string,
    string_list,
};
use super::super::markdown::operation_names;

const FOUNDATION: [&str; 3] = ["search-contracts", "search-domain", "search-ports"];

pub(super) struct PackageClosure {
    pub(super) package_rows: RowMap,
    pub(super) packages: BTreeSet<String>,
    pub(super) foundation_rows: RowMap,
    pub(super) function_rows: RowMap,
    pub(super) assignment_paths: BTreeSet<String>,
    pub(super) modules: ModuleMap,
    pub(super) module_rows: RowMap,
    pub(super) module_total: usize,
    pub(super) operation_count: usize,
}

pub(super) fn validate(
    root: &Path,
    inputs: &Inputs,
    errors: &mut Vec<String>,
) -> PackageClosure {
    let package_rows = rows_or_empty(&inputs.package_doc, "package", "name", errors);
    let packages: BTreeSet<String> = package_rows.keys().cloned().collect();
    require(
        errors,
        integer(&inputs.package_doc, "package_count") == Some(45)
            && packages.len() == 45,
        "package registry must contain 45 packages",
    );
    require(
        errors,
        integer(&inputs.manifest, "package_count") == Some(45),
        "coverage manifest package count mismatch",
    );

    let foundation_rows = rows_or_empty(&inputs.function_doc, "foundation", "package", errors);
    let function_rows = rows_or_empty(&inputs.function_doc, "package", "name", errors);
    let foundation: BTreeSet<String> =
        FOUNDATION.iter().map(|value| (*value).to_owned()).collect();
    require(
        errors,
        foundation_rows.keys().cloned().collect::<BTreeSet<_>>() == foundation,
        "foundation package set mismatch",
    );
    require(
        errors,
        function_rows.keys().cloned().collect::<BTreeSet<_>>()
            == packages.difference(&foundation).cloned().collect(),
        "package function source set mismatch",
    );
    require(
        errors,
        function_rows.len() == 42,
        "expected 42 package-local function sources",
    );

    let assignment_paths = validate_assignments(root, &package_rows, errors);
    let (module_rows, modules, module_total) = validate_modules(
        root,
        inputs,
        &package_rows,
        &foundation_rows,
        &function_rows,
        &packages,
        errors,
    );
    let operation_count = validate_functions(
        root,
        &package_rows,
        &function_rows,
        errors,
    );

    PackageClosure {
        package_rows,
        packages,
        foundation_rows,
        function_rows,
        assignment_paths,
        modules,
        module_rows,
        module_total,
        operation_count,
    }
}

fn validate_assignments(
    root: &Path,
    package_rows: &RowMap,
    errors: &mut Vec<String>,
) -> BTreeSet<String> {
    let mut assignment_paths = BTreeSet::new();
    for (package, row) in package_rows {
        let assignment = string(row, "assignment");
        require(
            errors,
            assignment.is_some(),
            format!("{package}: assignment path missing"),
        );
        let Some(relative) = assignment else {
            continue;
        };
        require(
            errors,
            root.join(relative).is_file(),
            format!("{package}: assignment file missing: {relative}"),
        );
        require(
            errors,
            assignment_paths.insert(relative.to_owned()),
            format!("duplicate assignment path {relative}"),
        );
        if let Ok(text) = std::fs::read_to_string(root.join(relative)) {
            require(
                errors,
                text.contains(package.as_str()),
                format!("{package}: assignment does not name package"),
            );
            require(
                errors,
                text.trim().len() > 100,
                format!("{package}: assignment is empty/underspecified"),
            );
        }
    }

    let actual = top_level_markdown_files(root, "swarm/assignments", "README.md");
    require(
        errors,
        actual == assignment_paths,
        format!(
            "orphan/missing assignments: {:?}",
            actual
                .symmetric_difference(&assignment_paths)
                .cloned()
                .collect::<Vec<_>>()
        ),
    );
    assignment_paths
}

#[allow(clippy::too_many_arguments)]
fn validate_modules(
    root: &Path,
    inputs: &Inputs,
    package_rows: &RowMap,
    foundation_rows: &RowMap,
    function_rows: &RowMap,
    packages: &BTreeSet<String>,
    errors: &mut Vec<String>,
) -> (RowMap, ModuleMap, usize) {
    let Some(packets) = inputs.module_doc.get("packet").and_then(Value::as_array) else {
        errors.push("module registry packet list missing".to_owned());
        return (RowMap::new(), ModuleMap::new(), 0);
    };
    let mut module_rows = RowMap::new();
    let mut modules = ModuleMap::new();
    let mut module_total = 0_usize;
    let maximum = integer(&inputs.module_doc, "max_modules_per_package")
        .and_then(|value| usize::try_from(value).ok())
        .unwrap_or(0);

    for packet in packets {
        let Some(path) = packet.get("path").and_then(Value::as_str) else {
            errors.push("invalid module packet entry".to_owned());
            continue;
        };
        require(
            errors,
            root.join(path).is_file(),
            format!("missing module packet {path}"),
        );
        let document = match load_toml(root, path) {
            Ok(document) => document,
            Err(error) => {
                errors.push(error);
                continue;
            }
        };
        let entries = rows_or_empty(&document, "package", "name", errors);
        require(
            errors,
            integer(&document, "package_count")
                == i64::try_from(entries.len()).ok(),
            format!("{path}: package count mismatch"),
        );
        let mut packet_module_total = 0_usize;
        for (package, entry) in entries {
            if module_rows.contains_key(&package) {
                errors.push(format!("duplicate module packet for {package}"));
                continue;
            }
            let names = string_list(&entry, "modules").unwrap_or_else(|| {
                errors.push(format!(
                    "{package}: modules must be a string array"
                ));
                Vec::new()
            });
            let unique: BTreeSet<String> = names.iter().cloned().collect();
            require(
                errors,
                unique.len() == names.len(),
                format!("{package}: duplicate module name"),
            );
            require(
                errors,
                integer(&entry, "module_count")
                    == i64::try_from(names.len()).ok(),
                format!("{package}: module count mismatch"),
            );
            require(
                errors,
                names.len() <= maximum,
                format!("{package}: module count exceeds maximum"),
            );
            require(
                errors,
                string(&entry, "public_entry_module")
                    .is_some_and(|entry_name| unique.contains(entry_name)),
                format!("{package}: public entry module missing"),
            );
            for name in &names {
                require(
                    errors,
                    valid_module_name(name),
                    format!("{package}: invalid module name {name}"),
                );
            }
            packet_module_total = packet_module_total.saturating_add(names.len());
            module_total = module_total.saturating_add(names.len());
            modules.insert(package.clone(), unique);
            module_rows.insert(package, entry);
        }
        require(
            errors,
            integer(&document, "module_count")
                == i64::try_from(packet_module_total).ok(),
            format!("{path}: declared module count mismatch"),
        );
        require(
            errors,
            packet.get("package_count").and_then(Value::as_integer)
                == i64::try_from(
                    document
                        .get("package")
                        .and_then(Value::as_array)
                        .map_or(0, Vec::len),
                )
                .ok(),
            format!("{path}: summary package count mismatch"),
        );
        require(
            errors,
            packet.get("module_count").and_then(Value::as_integer)
                == i64::try_from(packet_module_total).ok(),
            format!("{path}: summary module count mismatch"),
        );
    }

    let actual_packages: BTreeSet<String> = module_rows.keys().cloned().collect();
    require(
        errors,
        &actual_packages == packages,
        format!(
            "module package closure mismatch: {:?}",
            actual_packages
                .symmetric_difference(packages)
                .cloned()
                .collect::<Vec<_>>()
        ),
    );
    require(
        errors,
        integer(&inputs.module_doc, "package_count") == Some(45),
        "module registry package count mismatch",
    );
    require(
        errors,
        integer(&inputs.module_doc, "module_count") == Some(479)
            && module_total == 479,
        "module total must be 479",
    );
    require(
        errors,
        maximum == 15,
        "module ceiling must remain 15",
    );
    require(
        errors,
        boolean(
            &inputs.module_doc,
            "implementation_authorized_by_this_registry",
        ) == Some(false),
        "module registry authorizes implementation",
    );

    for (package, entry) in &module_rows {
        let registry = package_rows.get(package);
        require(
            errors,
            string(entry, "path") == registry.and_then(|row| string(row, "path")),
            format!("{package}: module path differs from package registry"),
        );
        let expected_source = foundation_rows
            .get(package)
            .and_then(|row| string(row, "primary_contract"))
            .or_else(|| {
                function_rows
                    .get(package)
                    .and_then(|row| string(row, "functions"))
            });
        require(
            errors,
            string(entry, "operation_source") == expected_source,
            format!("{package}: module operation source mismatch"),
        );
        for (key, message) in [
            (
                "all_public_operations_enter_through_public_entry",
                "public entry invariant disabled",
            ),
            (
                "package_state_must_remain_inside_declared_modules",
                "state containment invariant disabled",
            ),
            (
                "cross_package_module_imports_require_public_handoff",
                "cross-package handoff invariant disabled",
            ),
        ] {
            require(
                errors,
                boolean(entry, key) == Some(true),
                format!("{package}: {message}"),
            );
        }
    }
    (module_rows, modules, module_total)
}

fn validate_functions(
    root: &Path,
    package_rows: &RowMap,
    function_rows: &RowMap,
    errors: &mut Vec<String>,
) -> usize {
    let mut qualified = BTreeSet::new();
    let mut registered_paths = BTreeSet::new();
    let mut operation_count = 0_usize;
    for (package, row) in function_rows {
        let path = string(row, "functions");
        require(
            errors,
            path.is_some(),
            format!("{package}: function source missing"),
        );
        let Some(relative) = path else {
            continue;
        };
        registered_paths.insert(relative.to_owned());
        let package_path = package_rows
            .get(package)
            .and_then(|package_row| string(package_row, "path"));
        require(
            errors,
            package_path.is_some_and(|owner| {
                relative.starts_with(&format!("{owner}/"))
            }),
            format!("{package}: function source is not package-local"),
        );
        require(
            errors,
            relative.ends_with("/FUNCTIONS.md"),
            format!("{package}: function source must be FUNCTIONS.md"),
        );
        require(
            errors,
            root.join(relative).is_file(),
            format!("{package}: function source does not exist"),
        );
        let Ok(text) = std::fs::read_to_string(root.join(relative)) else {
            continue;
        };
        let operations = operation_names(&text);
        require(
            errors,
            !operations.is_empty(),
            format!("{package}: no source-derived operations"),
        );
        for operation in operations {
            let identity = format!("{package}::{operation}");
            require(
                errors,
                qualified.insert(identity.clone()),
                format!("duplicate qualified operation {identity}"),
            );
            operation_count = operation_count.saturating_add(1);
        }
    }

    let actual = recursive_named_files(root, &["crates", "bins"], "FUNCTIONS.md");
    require(
        errors,
        actual == registered_paths,
        format!(
            "orphan/missing function packets: {:?}",
            actual
                .symmetric_difference(&registered_paths)
                .cloned()
                .collect::<Vec<_>>()
        ),
    );
    operation_count
}

fn top_level_markdown_files(
    root: &Path,
    directory: &str,
    ignored_name: &str,
) -> BTreeSet<String> {
    let mut result = BTreeSet::new();
    let Ok(entries) = std::fs::read_dir(root.join(directory)) else {
        return result;
    };
    for entry in entries.flatten() {
        let path = entry.path();
        if !path.is_file()
            || path.file_name().and_then(|value| value.to_str()) == Some(ignored_name)
            || path.extension().and_then(|value| value.to_str()) != Some("md")
        {
            continue;
        }
        if let Ok(relative) = path.strip_prefix(root) {
            result.insert(relative.to_string_lossy().replace('\\', "/"));
        }
    }
    result
}

fn recursive_named_files(
    root: &Path,
    roots: &[&str],
    name: &str,
) -> BTreeSet<String> {
    let mut result = BTreeSet::new();
    let mut pending: VecDeque<std::path::PathBuf> = roots
        .iter()
        .map(|relative| root.join(relative))
        .filter(|path| path.exists())
        .collect();
    while let Some(directory) = pending.pop_front() {
        let Ok(entries) = std::fs::read_dir(&directory) else {
            continue;
        };
        for entry in entries.flatten() {
            let path = entry.path();
            if path.is_dir() {
                pending.push_back(path);
            } else if path.file_name().and_then(|value| value.to_str()) == Some(name)
                && let Ok(relative) = path.strip_prefix(root)
            {
                result.insert(relative.to_string_lossy().replace('\\', "/"));
            }
        }
    }
    result
}

fn valid_module_name(value: &str) -> bool {
    value
        .as_bytes()
        .first()
        .is_some_and(u8::is_ascii_lowercase)
        && value
            .bytes()
            .all(|byte| byte.is_ascii_lowercase() || byte.is_ascii_digit() || byte == b'_')
}

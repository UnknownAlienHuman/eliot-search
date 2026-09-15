use toml::Value;

use super::{BRIDGE_MANIFEST, ROOT_MANIFEST, VENDOR_CRATE};

pub(super) fn collect_vendor_dependency_declarations(
    value: &Value,
    path: &mut Vec<String>,
    declarations: &mut Vec<String>,
) {
    let Some(table) = value.as_table() else {
        return;
    };
    for (key, child) in table {
        let dependency_table = is_dependency_section(key)
            || key == "replace"
            || path.last().is_some_and(|parent| parent == "patch");
        if dependency_table && let Some(dependencies) = child.as_table() {
            for (name, specification) in dependencies {
                let package = specification
                    .as_table()
                    .and_then(|entry| entry.get("package"))
                    .and_then(Value::as_str);
                // `replace` keys include the version; renamed dependencies
                // identify their real crate through `package`, not the key.
                let replaced_name = name.split(':').next();
                if replaced_name == Some(VENDOR_CRATE)
                    || package == Some(VENDOR_CRATE)
                {
                    let mut location = path.clone();
                    location.push(key.clone());
                    location.push(name.clone());
                    declarations.push(location.join("."));
                }
            }
        }
        path.push(key.clone());
        collect_vendor_dependency_declarations(child, path, declarations);
        path.pop();
    }
}

pub(super) fn validate_workspace_dependency(
    document: &Value,
    errors: &mut Vec<String>,
) -> Option<String> {
    let Some(entry) = value_at(
        document,
        &["workspace", "dependencies", VENDOR_CRATE],
    ) else {
        errors.push(format!(
            "{ROOT_MANIFEST}: workspace.dependencies.{VENDOR_CRATE} is missing"
        ));
        return None;
    };
    let Some(table) = entry.as_table() else {
        errors.push(format!(
            "{ROOT_MANIFEST}: workspace {VENDOR_CRATE} dependency \
             must use a table"
        ));
        return None;
    };
    let Some(version) = table.get("version").and_then(Value::as_str) else {
        errors.push(format!(
            "{ROOT_MANIFEST}: workspace {VENDOR_CRATE} version is missing"
        ));
        return None;
    };
    let Some(exact_version) = version.strip_prefix('=') else {
        errors.push(format!(
            "{ROOT_MANIFEST}: workspace {VENDOR_CRATE} version must be an \
             exact =major.minor.patch pin"
        ));
        return None;
    };
    if exact_version.is_empty() {
        errors.push(format!(
            "{ROOT_MANIFEST}: workspace {VENDOR_CRATE} exact version is empty"
        ));
        return None;
    }
    for key in SOURCE_OVERRIDE_KEYS {
        if table.contains_key(*key) {
            errors.push(format!(
                "{ROOT_MANIFEST}: workspace {VENDOR_CRATE} must not \
                 override the qualified registry source with {key}"
            ));
        }
    }
    if table.get("package").is_some_and(|package| {
        package.as_str() != Some(VENDOR_CRATE)
    }) {
        errors.push(format!(
            "{ROOT_MANIFEST}: workspace {VENDOR_CRATE} must name the \
             qualified package"
        ));
    }
    if table.get("default-features").and_then(Value::as_bool) != Some(false) {
        errors.push(format!(
            "{ROOT_MANIFEST}: workspace {VENDOR_CRATE} must keep \
             default-features = false"
        ));
    }
    Some(exact_version.to_owned())
}

pub(super) fn validate_bridge_dependency(
    document: &Value,
    errors: &mut Vec<String>,
) {
    let Some(entry) = value_at(document, &["dependencies", VENDOR_CRATE])
    else {
        errors.push(format!(
            "{BRIDGE_MANIFEST}: dependencies.{VENDOR_CRATE} is missing"
        ));
        return;
    };
    let Some(table) = entry.as_table() else {
        errors.push(format!(
            "{BRIDGE_MANIFEST}: {VENDOR_CRATE} must inherit the workspace \
             dependency"
        ));
        return;
    };
    if table.get("workspace").and_then(Value::as_bool) != Some(true) {
        errors.push(format!(
            "{BRIDGE_MANIFEST}: {VENDOR_CRATE}.workspace must be true"
        ));
    }
    if table.contains_key("version") {
        errors.push(format!(
            "{BRIDGE_MANIFEST}: {VENDOR_CRATE} must not override the \
             workspace version"
        ));
    }
    if table.contains_key("default-features") || table.contains_key("features") {
        errors.push(format!(
            "{BRIDGE_MANIFEST}: {VENDOR_CRATE} must not override workspace \
             features"
        ));
    }
    for key in SOURCE_OVERRIDE_KEYS.iter().copied().chain(["package"]) {
        if table.contains_key(key) {
            errors.push(format!(
                "{BRIDGE_MANIFEST}: {VENDOR_CRATE} must not override \
                 the workspace source with {key}"
            ));
        }
    }
}

pub(super) fn lockfile_package_version(
    document: &Value,
    package: &str,
) -> Option<String> {
    let mut packages = value_at(document, &["package"])?
        .as_array()?
        .iter()
        .filter_map(Value::as_table)
        .filter(|row| row.get("name").and_then(Value::as_str) == Some(package));
    let record = packages.next()?;
    // Even equal versions from different sources are not one qualified client.
    if packages.next().is_some() {
        return None;
    }
    record.get("version")?.as_str().map(str::to_owned)
}

pub(super) fn value_at<'a>(
    document: &'a Value,
    path: &[&str],
) -> Option<&'a Value> {
    let mut current = document;
    for key in path {
        current = current.as_table()?.get(*key)?;
    }
    Some(current)
}

fn is_dependency_section(key: &str) -> bool {
    matches!(
        key,
        "dependencies" | "dev-dependencies" | "build-dependencies"
    )
}

const SOURCE_OVERRIDE_KEYS: &[&str] = &[
    "git", "path", "registry", "registry-index", "branch", "tag", "rev",
];

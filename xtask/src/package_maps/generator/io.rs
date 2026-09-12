//! Check/write boundary for deterministic package-map outputs.

use std::collections::{BTreeMap, BTreeSet};
use std::fs;
use std::path::Path;

use super::model::GenerationStats;
use super::super::{DOC_INDEX_PATH, INTEGRATION_PATH, MAP_ROOT};

pub(super) fn patch_manifest_text(
    text: &str,
    stats: &GenerationStats,
) -> Result<String, String> {
    let mut lines: Vec<String> = text.lines().map(str::to_owned).collect();
    if !lines.iter().any(|line| line.starts_with("package_map_index = ")) {
        let marker = lines
            .iter()
            .position(|line| {
                line == "module_coverage_registry = \"swarm/coverage/module-coverage.toml\""
            })
            .ok_or_else(|| {
                "manifest module coverage marker missing or duplicated".to_owned()
            })?;
        let _ = lines.splice(
            marker + 1..marker + 1,
            [
                "package_map_index = \"swarm/coverage/package-map-index.toml\""
                    .to_owned(),
                format!("documentation_file_index = \"{DOC_INDEX_PATH}\""),
                format!("integration_documentation_map = \"{INTEGRATION_PATH}\""),
            ],
        );
    }

    let weak = lines
        .iter()
        .position(|line| line.starts_with("weak_logical_module_count = "))
        .ok_or_else(|| "manifest map count block missing".to_owned())?;
    let mut end = weak + 1;
    while end < lines.len()
        && [
            "package_map_count = ",
            "package_map_file_count = ",
            "integration_documentation_node_count = ",
        ]
        .iter()
        .any(|prefix| lines[end].starts_with(prefix))
    {
        end += 1;
    }
    let _ = lines.splice(
        weak..end,
        [
            "weak_logical_module_count = 0".to_owned(),
            format!("package_map_count = {}", stats.packages),
            format!("package_map_file_count = {}", stats.map_files),
            format!(
                "integration_documentation_node_count = {}",
                stats.integration_nodes
            ),
        ],
    );
    let mut output = lines.join("\n");
    output.push('\n');
    Ok(output)
}

pub(super) fn check_outputs(
    root: &Path,
    outputs: &BTreeMap<String, String>,
    expected_manifest: &str,
) -> Vec<String> {
    let mut stale = Vec::new();
    for (relative, expected) in outputs {
        match fs::read_to_string(root.join(relative)) {
            Ok(actual) if actual == *expected => {}
            Ok(_) => stale.push(relative.clone()),
            Err(_) => stale.push(relative.clone()),
        }
    }
    let expected_package_files: BTreeSet<String> = outputs
        .keys()
        .filter(|path| path.starts_with(&format!("{MAP_ROOT}/")))
        .cloned()
        .collect();
    let actual_package_files = package_files(root);
    stale.extend(
        actual_package_files
            .symmetric_difference(&expected_package_files)
            .cloned(),
    );
    match fs::read_to_string(root.join("swarm/coverage/manifest.toml")) {
        Ok(actual) if actual == expected_manifest => {}
        _ => stale.push("swarm/coverage/manifest.toml".to_owned()),
    }
    stale.sort();
    stale.dedup();
    stale
}

pub(super) fn write_outputs(
    root: &Path,
    outputs: &BTreeMap<String, String>,
    manifest: &str,
) -> Result<(), String> {
    let package_root = root.join(MAP_ROOT);
    for relative in package_files(root) {
        fs::remove_file(root.join(&relative))
            .map_err(|error| format!("{relative}: {error}"))?;
    }
    if package_root.exists() {
        remove_empty_directories(&package_root)?;
    }
    for (relative, content) in outputs {
        let path = root.join(relative);
        let parent = path
            .parent()
            .ok_or_else(|| format!("{relative}: output parent missing"))?;
        fs::create_dir_all(parent)
            .map_err(|error| format!("{}: {error}", parent.display()))?;
        fs::write(&path, content.as_bytes())
            .map_err(|error| format!("{relative}: {error}"))?;
    }
    fs::write(root.join("swarm/coverage/manifest.toml"), manifest.as_bytes())
        .map_err(|error| format!("swarm/coverage/manifest.toml: {error}"))
}

fn package_files(root: &Path) -> BTreeSet<String> {
    let mut output = BTreeSet::new();
    collect_files(root, &root.join(MAP_ROOT), &mut output);
    output
}

fn collect_files(root: &Path, directory: &Path, output: &mut BTreeSet<String>) {
    let Ok(entries) = fs::read_dir(directory) else {
        return;
    };
    for entry in entries.flatten() {
        let path = entry.path();
        let Ok(kind) = entry.file_type() else {
            continue;
        };
        if kind.is_dir() {
            collect_files(root, &path, output);
        } else if kind.is_file()
            && path.extension().and_then(|value| value.to_str()) == Some("toml")
            && let Ok(relative) = path.strip_prefix(root)
        {
            let _ = output.insert(relative.to_string_lossy().replace('\\', "/"));
        }
    }
}

fn remove_empty_directories(directory: &Path) -> Result<(), String> {
    let mut directories = Vec::new();
    collect_directories(directory, &mut directories);
    directories.sort_by_key(|path| std::cmp::Reverse(path.components().count()));
    for path in directories {
        if fs::read_dir(&path)
            .map_err(|error| format!("{}: {error}", path.display()))?
            .next()
            .is_none()
        {
            fs::remove_dir(&path)
                .map_err(|error| format!("{}: {error}", path.display()))?;
        }
    }
    Ok(())
}

fn collect_directories(directory: &Path, output: &mut Vec<std::path::PathBuf>) {
    let Ok(entries) = fs::read_dir(directory) else {
        return;
    };
    for entry in entries.flatten() {
        let path = entry.path();
        if entry.file_type().is_ok_and(|kind| kind.is_dir()) {
            collect_directories(&path, output);
            output.push(path);
        }
    }
}

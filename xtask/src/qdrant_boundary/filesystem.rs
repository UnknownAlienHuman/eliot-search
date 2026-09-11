use std::fs;
use std::path::{Path, PathBuf};

use toml::Value;

pub(super) fn collect_files(
    root: &Path,
    directory: &Path,
    files: &mut Vec<PathBuf>,
    errors: &mut Vec<String>,
) {
    let entries = match fs::read_dir(directory) {
        Ok(entries) => entries,
        Err(error) => {
            errors.push(format!(
                "{}: unable to read directory: {error}",
                relative_path(root, directory)
            ));
            return;
        }
    };

    let mut entries = match entries.collect::<Result<Vec<_>, _>>() {
        Ok(entries) => entries,
        Err(error) => {
            errors.push(format!(
                "{}: unable to enumerate directory: {error}",
                relative_path(root, directory)
            ));
            return;
        }
    };
    entries.sort_by_key(std::fs::DirEntry::file_name);

    for entry in entries {
        let path = entry.path();
        let file_type = match entry.file_type() {
            Ok(file_type) => file_type,
            Err(error) => {
                errors.push(format!(
                    "{}: unable to inspect file type: {error}",
                    relative_path(root, &path)
                ));
                continue;
            }
        };
        if file_type.is_symlink() {
            continue;
        }
        if file_type.is_dir() {
            let name = entry.file_name();
            if matches!(
                name.to_str(),
                Some(
                    ".git"
                        | "target"
                        | ".venv"
                        | "__pycache__"
                        | "node_modules"
                )
            ) {
                continue;
            }
            collect_files(root, &path, files, errors);
        } else if file_type.is_file() {
            files.push(path);
        }
    }
}

pub(super) fn relative_path(root: &Path, path: &Path) -> String {
    let relative = path.strip_prefix(root).unwrap_or(path);
    let rendered = relative.to_string_lossy().replace('\\', "/");
    if rendered.is_empty() {
        ".".to_owned()
    } else {
        rendered
    }
}

pub(super) fn read_text(
    path: &Path,
    label: &str,
    errors: &mut Vec<String>,
) -> Option<String> {
    match fs::read_to_string(path) {
        Ok(text) => Some(text),
        Err(error) => {
            errors.push(format!(
                "{label}: unable to read UTF-8 text: {error}"
            ));
            None
        }
    }
}

pub(super) fn read_toml(
    path: &Path,
    label: &str,
    errors: &mut Vec<String>,
) -> Option<Value> {
    let text = read_text(path, label, errors)?;
    match text.parse::<Value>() {
        Ok(document) => Some(document),
        Err(error) => {
            errors.push(format!("{label}: invalid TOML: {error}"));
            None
        }
    }
}

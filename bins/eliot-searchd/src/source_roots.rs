//! Persistent bounded source-root catalog for the development daemon.

use std::collections::BTreeSet;
use std::fs::{self, File, OpenOptions};
use std::io::{self, Read, Write};
use std::path::{Path, PathBuf};

pub(crate) const MAX_SOURCE_ROOTS: usize = 32;
pub(crate) const MAX_SOURCE_ROOT_FILE_BYTES: usize = 64 * 1024;
pub(crate) const MAX_SOURCE_ROOT_PATH_BYTES: usize = 512;

#[derive(Clone, Copy, Debug, Eq, Ord, PartialEq, PartialOrd)]
pub(crate) enum SourceRootState {
    Available,
    Missing,
    NotDirectory,
    Unsafe,
    Unverifiable,
}

impl SourceRootState {
    pub(crate) const fn code(self) -> &'static str {
        match self {
            Self::Available => "available",
            Self::Missing => "missing",
            Self::NotDirectory => "not_directory",
            Self::Unsafe => "unsafe",
            Self::Unverifiable => "unverifiable",
        }
    }
}

#[derive(Clone, Debug, Eq, Ord, PartialEq, PartialOrd)]
struct SourceRootEntry {
    configured_path: PathBuf,
    state: SourceRootState,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) struct SourceRootView {
    pub(crate) index: usize,
    pub(crate) path: String,
    pub(crate) state: SourceRootState,
}

#[derive(Clone, Debug)]
pub(crate) struct SourceRootCatalog {
    config_path: PathBuf,
    entries: Vec<SourceRootEntry>,
}

impl SourceRootCatalog {
    pub(crate) fn load(
        config_path: PathBuf,
        command_roots: &[PathBuf],
    ) -> Result<Self, SourceRootError> {
        if command_roots.len() > MAX_SOURCE_ROOTS {
            return Err(SourceRootError::RootLimitExceeded);
        }
        recover_interrupted_update(&config_path)?;
        let mut configured = load_configured_paths(&config_path)?;
        let had_command_roots = !command_roots.is_empty();
        for root in command_roots {
            let canonical = canonicalize_new_root(root)?;
            if !configured.contains(&canonical) {
                configured.push(canonical);
            }
        }
        canonicalize_configured_set(&mut configured)?;
        let mut catalog = Self {
            config_path,
            entries: configured
                .into_iter()
                .map(|configured_path| SourceRootEntry {
                    configured_path,
                    state: SourceRootState::Unverifiable,
                })
                .collect(),
        };
        catalog.refresh();
        if had_command_roots {
            catalog.persist()?;
        }
        Ok(catalog)
    }

    pub(crate) fn configured_count(&self) -> usize {
        self.entries.len()
    }

    pub(crate) fn available_count(&self) -> usize {
        self.entries
            .iter()
            .filter(|entry| entry.state == SourceRootState::Available)
            .count()
    }

    pub(crate) fn unavailable_count(&self) -> usize {
        self.configured_count().saturating_sub(self.available_count())
    }

    pub(crate) fn refresh(&mut self) -> bool {
        let was_available = self.available_count();
        for entry in &mut self.entries {
            entry.state = probe_root(&entry.configured_path);
        }
        was_available != self.available_count()
    }

    pub(crate) fn available_paths(&self) -> Vec<(usize, &Path)> {
        self.entries
            .iter()
            .enumerate()
            .filter(|(_, entry)| entry.state == SourceRootState::Available)
            .map(|(index, entry)| (index, entry.configured_path.as_path()))
            .collect()
    }

    pub(crate) fn views(&self) -> Result<Vec<SourceRootView>, SourceRootError> {
        self.entries
            .iter()
            .enumerate()
            .map(|(index, entry)| {
                Ok(SourceRootView {
                    index,
                    path: path_text(&entry.configured_path)?.to_owned(),
                    state: entry.state,
                })
            })
            .collect()
    }

    pub(crate) fn add(&mut self, requested: &Path) -> Result<SourceRootView, SourceRootError> {
        let canonical = canonicalize_new_root(requested)?;
        if let Some(index) = self
            .entries
            .iter()
            .position(|entry| entry.configured_path == canonical)
        {
            self.entries[index].state = probe_root(&canonical);
            return self.view(index);
        }
        if self.entries.len() >= MAX_SOURCE_ROOTS {
            return Err(SourceRootError::RootLimitExceeded);
        }
        ensure_no_overlap(
            self.entries.iter().map(|entry| &entry.configured_path),
            &canonical,
        )?;

        let mut staged = self.entries.clone();
        staged.push(SourceRootEntry {
            configured_path: canonical,
            state: SourceRootState::Available,
        });
        staged.sort_by(|left, right| left.configured_path.cmp(&right.configured_path));
        persist_entries(&self.config_path, &staged)?;
        self.entries = staged;
        let index = self
            .entries
            .iter()
            .position(|entry| entry.configured_path == canonical)
            .ok_or(SourceRootError::CatalogCorrupt)?;
        self.view(index)
    }

    pub(crate) fn remove(&mut self, requested: &Path) -> Result<String, SourceRootError> {
        let requested = normalize_remove_target(requested)?;
        let Some(index) = self
            .entries
            .iter()
            .position(|entry| entry.configured_path == requested)
        else {
            return Err(SourceRootError::RootNotFound);
        };
        let removed = path_text(&self.entries[index].configured_path)?.to_owned();
        let mut staged = self.entries.clone();
        staged.remove(index);
        persist_entries(&self.config_path, &staged)?;
        self.entries = staged;
        Ok(removed)
    }

    fn persist(&self) -> Result<(), SourceRootError> {
        persist_entries(&self.config_path, &self.entries)
    }

    fn view(&self, index: usize) -> Result<SourceRootView, SourceRootError> {
        let entry = self
            .entries
            .get(index)
            .ok_or(SourceRootError::CatalogCorrupt)?;
        Ok(SourceRootView {
            index,
            path: path_text(&entry.configured_path)?.to_owned(),
            state: entry.state,
        })
    }
}

fn canonicalize_configured_set(paths: &mut Vec<PathBuf>) -> Result<(), SourceRootError> {
    if paths.len() > MAX_SOURCE_ROOTS {
        return Err(SourceRootError::RootLimitExceeded);
    }
    paths.sort();
    paths.dedup();
    for path in paths.iter() {
        validate_persisted_path(path)?;
    }
    for (index, path) in paths.iter().enumerate() {
        ensure_no_overlap(paths[..index].iter(), path)?;
    }
    Ok(())
}

fn ensure_no_overlap<'a>(
    existing: impl IntoIterator<Item = &'a PathBuf>,
    candidate: &Path,
) -> Result<(), SourceRootError> {
    if existing.into_iter().any(|root| {
        candidate != root.as_path()
            && (candidate.starts_with(root) || root.starts_with(candidate))
    }) {
        Err(SourceRootError::RootOverlap)
    } else {
        Ok(())
    }
}

fn canonicalize_new_root(path: &Path) -> Result<PathBuf, SourceRootError> {
    reject_symlink(path)?;
    let canonical = fs::canonicalize(path).map_err(SourceRootError::RootIo)?;
    reject_symlink(&canonical)?;
    let metadata = fs::metadata(&canonical).map_err(SourceRootError::RootIo)?;
    if !metadata.is_dir() {
        return Err(SourceRootError::RootNotDirectory);
    }
    validate_persisted_path(&canonical)?;
    Ok(canonical)
}

fn normalize_remove_target(path: &Path) -> Result<PathBuf, SourceRootError> {
    if path.exists() {
        canonicalize_new_root(path)
    } else {
        validate_persisted_path(path)?;
        Ok(path.to_path_buf())
    }
}

fn probe_root(path: &Path) -> SourceRootState {
    let metadata = match fs::symlink_metadata(path) {
        Ok(metadata) => metadata,
        Err(error) if error.kind() == io::ErrorKind::NotFound => {
            return SourceRootState::Missing;
        }
        Err(_) => return SourceRootState::Unverifiable,
    };
    if metadata.file_type().is_symlink() {
        return SourceRootState::Unsafe;
    }
    if !metadata.is_dir() {
        return SourceRootState::NotDirectory;
    }
    match fs::canonicalize(path) {
        Ok(canonical) if canonical == path => SourceRootState::Available,
        Ok(_) => SourceRootState::Unsafe,
        Err(_) => SourceRootState::Unverifiable,
    }
}

fn validate_persisted_path(path: &Path) -> Result<(), SourceRootError> {
    if !path.is_absolute() {
        return Err(SourceRootError::RootPathNotAbsolute);
    }
    let value = path_text(path)?;
    if value.is_empty()
        || value.as_bytes().len() > MAX_SOURCE_ROOT_PATH_BYTES
        || value
            .chars()
            .any(|character| matches!(character, '\r' | '\n' | '\0'))
    {
        return Err(SourceRootError::InvalidRootPath);
    }
    Ok(())
}

fn path_text(path: &Path) -> Result<&str, SourceRootError> {
    path.to_str().ok_or(SourceRootError::RootPathNotUtf8)
}

fn load_configured_paths(path: &Path) -> Result<Vec<PathBuf>, SourceRootError> {
    reject_symlink(path)?;
    if !path.exists() {
        return Ok(Vec::new());
    }
    let mut file = File::open(path).map_err(SourceRootError::ConfigIo)?;
    let mut bytes = Vec::new();
    file.by_ref()
        .take(
            u64::try_from(MAX_SOURCE_ROOT_FILE_BYTES)
                .expect("source-root file ceiling fits u64")
                .saturating_add(1),
        )
        .read_to_end(&mut bytes)
        .map_err(SourceRootError::ConfigIo)?;
    if bytes.len() > MAX_SOURCE_ROOT_FILE_BYTES {
        return Err(SourceRootError::ConfigTooLarge);
    }
    let text = std::str::from_utf8(&bytes).map_err(|_| SourceRootError::ConfigNotUtf8)?;
    let mut paths = Vec::new();
    for line in text.lines() {
        let line = line.trim();
        if line.is_empty() || line.starts_with('#') {
            continue;
        }
        if paths.len() >= MAX_SOURCE_ROOTS {
            return Err(SourceRootError::RootLimitExceeded);
        }
        let path = PathBuf::from(line);
        validate_persisted_path(&path)?;
        paths.push(path);
    }
    Ok(paths)
}

fn persist_entries(path: &Path, entries: &[SourceRootEntry]) -> Result<(), SourceRootError> {
    if entries.len() > MAX_SOURCE_ROOTS {
        return Err(SourceRootError::RootLimitExceeded);
    }
    let parent = path.parent().ok_or(SourceRootError::InvalidConfigPath)?;
    fs::create_dir_all(parent).map_err(SourceRootError::ConfigIo)?;
    reject_symlink(parent)?;
    reject_symlink(path)?;

    let mut body = String::from("# ELIOT Search source roots v1\n");
    for entry in entries {
        validate_persisted_path(&entry.configured_path)?;
        body.push_str(path_text(&entry.configured_path)?);
        body.push('\n');
    }
    if body.len() > MAX_SOURCE_ROOT_FILE_BYTES {
        return Err(SourceRootError::ConfigTooLarge);
    }

    let temporary = path.with_extension("tmp");
    let backup = path.with_extension("bak");
    reject_symlink(&temporary)?;
    reject_symlink(&backup)?;
    remove_plain_file_if_present(&temporary)?;
    let mut file = OpenOptions::new()
        .write(true)
        .create_new(true)
        .open(&temporary)
        .map_err(SourceRootError::ConfigIo)?;
    file.write_all(body.as_bytes())
        .and_then(|()| file.sync_all())
        .map_err(SourceRootError::ConfigIo)?;
    drop(file);

    remove_plain_file_if_present(&backup)?;
    if path.exists() {
        fs::rename(path, &backup).map_err(SourceRootError::ConfigIo)?;
    }
    if let Err(error) = fs::rename(&temporary, path) {
        if backup.exists() && !path.exists() {
            let _ = fs::rename(&backup, path);
        }
        let _ = fs::remove_file(&temporary);
        return Err(SourceRootError::ConfigIo(error));
    }
    remove_plain_file_if_present(&backup)?;
    Ok(())
}

fn recover_interrupted_update(path: &Path) -> Result<(), SourceRootError> {
    let backup = path.with_extension("bak");
    let temporary = path.with_extension("tmp");
    reject_symlink(path)?;
    reject_symlink(&backup)?;
    reject_symlink(&temporary)?;
    if !path.exists() && backup.exists() {
        fs::rename(&backup, path).map_err(SourceRootError::ConfigIo)?;
    } else if path.exists() && backup.exists() {
        remove_plain_file_if_present(&backup)?;
    }
    if temporary.exists() {
        remove_plain_file_if_present(&temporary)?;
    }
    Ok(())
}

fn remove_plain_file_if_present(path: &Path) -> Result<(), SourceRootError> {
    match fs::symlink_metadata(path) {
        Ok(metadata) if metadata.file_type().is_symlink() => Err(SourceRootError::SymlinkDenied),
        Ok(metadata) if metadata.is_file() => {
            fs::remove_file(path).map_err(SourceRootError::ConfigIo)
        }
        Ok(_) => Err(SourceRootError::InvalidConfigPath),
        Err(error) if error.kind() == io::ErrorKind::NotFound => Ok(()),
        Err(error) => Err(SourceRootError::ConfigIo(error)),
    }
}

fn reject_symlink(path: &Path) -> Result<(), SourceRootError> {
    match fs::symlink_metadata(path) {
        Ok(metadata) if metadata.file_type().is_symlink() => Err(SourceRootError::SymlinkDenied),
        Ok(_) => Ok(()),
        Err(error) if error.kind() == io::ErrorKind::NotFound => Ok(()),
        Err(error) => Err(SourceRootError::ConfigIo(error)),
    }
}

#[derive(Debug)]
pub(crate) enum SourceRootError {
    RootLimitExceeded,
    RootNotFound,
    RootNotDirectory,
    RootOverlap,
    RootPathNotAbsolute,
    RootPathNotUtf8,
    InvalidRootPath,
    InvalidConfigPath,
    ConfigTooLarge,
    ConfigNotUtf8,
    SymlinkDenied,
    CatalogCorrupt,
    RootIo(io::Error),
    ConfigIo(io::Error),
}

impl SourceRootError {
    pub(crate) const fn code(&self) -> &'static str {
        match self {
            Self::RootLimitExceeded => "SOURCE_ROOT_LIMIT",
            Self::RootNotFound => "SOURCE_ROOT_NOT_FOUND",
            Self::RootNotDirectory => "SOURCE_ROOT_NOT_DIRECTORY",
            Self::RootOverlap => "SOURCE_ROOT_OVERLAP",
            Self::RootPathNotAbsolute => "SOURCE_ROOT_PATH_NOT_ABSOLUTE",
            Self::RootPathNotUtf8 => "SOURCE_ROOT_PATH_NOT_UTF8",
            Self::InvalidRootPath => "SOURCE_ROOT_PATH_INVALID",
            Self::InvalidConfigPath => "SOURCE_ROOT_CONFIG_PATH_INVALID",
            Self::ConfigTooLarge => "SOURCE_ROOT_CONFIG_TOO_LARGE",
            Self::ConfigNotUtf8 => "SOURCE_ROOT_CONFIG_NOT_UTF8",
            Self::SymlinkDenied => "SOURCE_ROOT_SYMLINK_DENIED",
            Self::CatalogCorrupt => "SOURCE_ROOT_CATALOG_CORRUPT",
            Self::RootIo(_) => "SOURCE_ROOT_IO_FAILED",
            Self::ConfigIo(_) => "SOURCE_ROOT_CONFIG_IO_FAILED",
        }
    }
}

impl std::fmt::Display for SourceRootError {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter.write_str(self.code())
    }
}

impl std::error::Error for SourceRootError {}

#[cfg(test)]
mod tests {
    use super::*;
    use std::time::{SystemTime, UNIX_EPOCH};

    fn temporary_directory(name: &str) -> PathBuf {
        let unique = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .expect("system time after epoch")
            .as_nanos();
        std::env::temp_dir().join(format!(
            "eliot-search-source-roots-{name}-{}-{unique}",
            std::process::id()
        ))
    }

    #[test]
    fn persists_reloads_adds_and_removes() {
        let root = temporary_directory("lifecycle");
        let first = root.join("first");
        let second = root.join("second");
        fs::create_dir_all(&first).expect("create first root");
        fs::create_dir_all(&second).expect("create second root");
        let config = root.join("state").join("source-roots.txt");

        let mut catalog = SourceRootCatalog::load(config.clone(), std::slice::from_ref(&first))
            .expect("initial catalog");
        assert_eq!(catalog.configured_count(), 1);
        assert_eq!(catalog.available_count(), 1);
        catalog.add(&second).expect("add second root");
        assert_eq!(catalog.configured_count(), 2);

        let reloaded = SourceRootCatalog::load(config.clone(), &[]).expect("reload catalog");
        assert_eq!(reloaded.configured_count(), 2);
        let removed = catalog.remove(
            &fs::canonicalize(&first).expect("canonical first root"),
        );
        assert!(removed.is_ok());
        assert_eq!(catalog.configured_count(), 1);

        let _ = fs::remove_dir_all(root);
    }

    #[test]
    fn missing_root_is_retained_but_unavailable() {
        let root = temporary_directory("missing");
        let source = root.join("source");
        fs::create_dir_all(&source).expect("create root");
        let config = root.join("state").join("source-roots.txt");
        let catalog = SourceRootCatalog::load(config.clone(), std::slice::from_ref(&source))
            .expect("persist catalog");
        assert_eq!(catalog.available_count(), 1);
        fs::remove_dir_all(&source).expect("remove root");

        let reloaded = SourceRootCatalog::load(config, &[]).expect("reload missing root");
        assert_eq!(reloaded.configured_count(), 1);
        assert_eq!(reloaded.available_count(), 0);
        assert_eq!(reloaded.unavailable_count(), 1);

        let _ = fs::remove_dir_all(root);
    }
}

//! Persistent bounded observation-root catalog for the primary daemon.
//!
//! The caller holds the data-root owner lock for this catalog's entire lifetime.
//! Registration is observation configuration, not a source identity, access grant,
//! current-workspace proof, or purge instruction.

use std::collections::BTreeSet;
use std::fs::{self, File, Metadata, OpenOptions};
use std::io::{self, Read, Write};
use std::path::{Component, Path, PathBuf};

pub(crate) const MAX_SOURCE_ROOTS: usize = 32;
pub(crate) const MAX_SOURCE_ROOT_FILE_BYTES: usize = 64 * 1024;
pub(crate) const MAX_SOURCE_ROOT_PATH_BYTES: usize = 512;
const HEADER: &str = "# ELIOT Search source roots v1";

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

#[derive(Debug)]
pub(crate) struct SourceRootCatalog {
    config_path: PathBuf,
    entries: Vec<SourceRootEntry>,
    excluded_data_root: Option<PathBuf>,
    needs_reopen: bool,
}

impl SourceRootCatalog {
    /// Restores registration while the primary runtime owns the data root.
    pub(crate) fn load_owned(data_root: &Path) -> Result<Self, SourceRootError> {
        reject_symlink(data_root)?;
        let canonical = fs::canonicalize(data_root).map_err(SourceRootError::RootIo)?;
        if !fs::metadata(&canonical).map_err(SourceRootError::RootIo)?.is_dir() {
            return Err(SourceRootError::InvalidConfigPath);
        }
        let control = canonical.join("control");
        reject_symlink(&control)?;
        if !control.try_exists().map_err(SourceRootError::ConfigIo)? {
            fs::create_dir(&control).map_err(SourceRootError::ConfigIo)?;
            sync_directory(&canonical)?;
        }
        if !fs::symlink_metadata(&control).map_err(SourceRootError::ConfigIo)?.is_dir()
            || fs::canonicalize(&control).map_err(SourceRootError::ConfigIo)? != control
        {
            return Err(SourceRootError::InvalidConfigPath);
        }
        let mut catalog = Self::load(control.join("source-roots.v1"), &[])?;
        for entry in &catalog.entries {
            ensure_outside_data_root(&entry.configured_path, &canonical)?;
        }
        catalog.excluded_data_root = Some(canonical);
        Ok(catalog)
    }

    pub(crate) fn load(
        config_path: PathBuf,
        command_roots: &[PathBuf],
    ) -> Result<Self, SourceRootError> {
        if command_roots.len() > MAX_SOURCE_ROOTS {
            return Err(SourceRootError::RootLimitExceeded);
        }
        recover_interrupted_update(&config_path)?;
        let mut configured = load_configured_paths(&config_path)?;
        for root in command_roots {
            let canonical = canonicalize_new_root(root)?;
            if !configured.contains(&canonical) {
                configured.push(canonical);
            }
        }
        canonicalize_configured_set(&mut configured)?;
        let mut catalog = Self {
            config_path,
            entries: configured.into_iter().map(|configured_path| SourceRootEntry {
                configured_path,
                state: SourceRootState::Unverifiable,
            }).collect(),
            excluded_data_root: None,
            needs_reopen: false,
        };
        catalog.refresh();
        if !command_roots.is_empty() {
            persist_entries(&catalog.config_path, &catalog.entries)?;
        }
        Ok(catalog)
    }

    pub(crate) fn configured_count(&self) -> usize {
        self.entries.len()
    }

    pub(crate) fn available_count(&self) -> usize {
        if self.needs_reopen {
            return 0;
        }
        self.entries.iter().filter(|entry| entry.state == SourceRootState::Available).count()
    }

    pub(crate) fn unavailable_count(&self) -> usize {
        self.configured_count().saturating_sub(self.available_count())
    }

    /// Detects each root's transition, including swaps with unchanged totals.
    pub(crate) fn refresh(&mut self) -> bool {
        if self.needs_reopen {
            return false;
        }
        let mut changed = false;
        for entry in &mut self.entries {
            let observed = probe_root(&entry.configured_path);
            changed |= entry.state != observed;
            entry.state = observed;
        }
        changed
    }

    pub(crate) fn available_paths(&self) -> Vec<(usize, &Path)> {
        if self.needs_reopen {
            return Vec::new();
        }
        self.entries.iter().enumerate()
            .filter(|(_, entry)| entry.state == SourceRootState::Available)
            .map(|(index, entry)| (index, entry.configured_path.as_path()))
            .collect()
    }

    pub(crate) fn views(&self) -> Result<Vec<SourceRootView>, SourceRootError> {
        self.ensure_usable()?;
        (0..self.entries.len()).map(|index| self.view(index)).collect()
    }

    pub(crate) fn add(&mut self, requested: &Path) -> Result<SourceRootView, SourceRootError> {
        self.ensure_usable()?;
        let canonical = canonicalize_new_root(requested)?;
        if let Some(data_root) = &self.excluded_data_root {
            ensure_outside_data_root(&canonical, data_root)?;
        }
        if let Some(index) = self.entries.iter().position(|entry| entry.configured_path == canonical) {
            self.entries[index].state = probe_root(&canonical);
            return self.view(index);
        }
        if self.entries.len() >= MAX_SOURCE_ROOTS {
            return Err(SourceRootError::RootLimitExceeded);
        }
        ensure_no_overlap(self.entries.iter().map(|entry| &entry.configured_path), &canonical)?;
        // Compute the insertion position before moving the owned path.
        let index = self.entries.partition_point(|entry| entry.configured_path < canonical);
        let mut staged = self.entries.clone();
        staged.insert(index, SourceRootEntry {
            configured_path: canonical,
            state: SourceRootState::Available,
        });
        self.commit(staged)?;
        self.view(index)
    }

    /// Removes observation registration, including a missing or replaced locator.
    /// Retained source revisions and access policy are deliberately unchanged.
    pub(crate) fn remove(&mut self, requested: &Path) -> Result<String, SourceRootError> {
        self.ensure_usable()?;
        let index = self.entries.iter().position(|entry| entry.configured_path == requested);
        let index = match index {
            Some(index) => index,
            None => {
                let canonical = canonicalize_new_root(requested)?;
                self.entries.iter().position(|entry| entry.configured_path == canonical)
                    .ok_or(SourceRootError::RootNotFound)?
            }
        };
        let removed = path_text(&self.entries[index].configured_path)?.to_owned();
        let mut staged = self.entries.clone();
        staged.remove(index);
        self.commit(staged)?;
        Ok(removed)
    }

    fn commit(&mut self, staged: Vec<SourceRootEntry>) -> Result<(), SourceRootError> {
        if let Err(error) = persist_entries(&self.config_path, &staged) {
            self.needs_reopen = true;
            return Err(error);
        }
        self.entries = staged;
        Ok(())
    }

    fn ensure_usable(&self) -> Result<(), SourceRootError> {
        if self.needs_reopen {
            Err(SourceRootError::UpdateOutcomeUnknown)
        } else {
            Ok(())
        }
    }

    fn view(&self, index: usize) -> Result<SourceRootView, SourceRootError> {
        self.ensure_usable()?;
        let entry = self.entries.get(index).ok_or(SourceRootError::CatalogCorrupt)?;
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
    for (index, path) in paths.iter().enumerate() {
        validate_persisted_path(path)?;
        ensure_no_overlap(paths[..index].iter(), path)?;
    }
    Ok(())
}

fn ensure_no_overlap<'a>(
    existing: impl IntoIterator<Item = &'a PathBuf>,
    candidate: &Path,
) -> Result<(), SourceRootError> {
    if existing.into_iter().any(|root| candidate != root.as_path()
        && (candidate.starts_with(root) || root.starts_with(candidate)))
    {
        Err(SourceRootError::RootOverlap)
    } else {
        Ok(())
    }
}

fn ensure_outside_data_root(candidate: &Path, data_root: &Path) -> Result<(), SourceRootError> {
    if candidate.starts_with(data_root) || data_root.starts_with(candidate) {
        Err(SourceRootError::DataRootOverlap)
    } else {
        Ok(())
    }
}

fn canonicalize_new_root(path: &Path) -> Result<PathBuf, SourceRootError> {
    reject_symlink(path)?;
    let canonical = fs::canonicalize(path).map_err(SourceRootError::RootIo)?;
    reject_symlink(&canonical)?;
    if !fs::metadata(&canonical).map_err(SourceRootError::RootIo)?.is_dir() {
        return Err(SourceRootError::RootNotDirectory);
    }
    validate_persisted_path(&canonical)?;
    Ok(canonical)
}

fn probe_root(path: &Path) -> SourceRootState {
    let metadata = match fs::symlink_metadata(path) {
        Ok(metadata) => metadata,
        Err(error) if error.kind() == io::ErrorKind::NotFound => return SourceRootState::Missing,
        Err(_) => return SourceRootState::Unverifiable,
    };
    if metadata.file_type().is_symlink() || is_reparse(&metadata) {
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
    if value.is_empty() || value.len() > MAX_SOURCE_ROOT_PATH_BYTES
        || value.chars().any(char::is_control)
        || path.components().any(|component| matches!(component, Component::ParentDir))
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
    let mut file = match File::open(path) {
        Ok(file) => file,
        Err(error) if error.kind() == io::ErrorKind::NotFound => return Ok(Vec::new()),
        Err(error) => return Err(SourceRootError::ConfigIo(error)),
    };
    let metadata = file.metadata().map_err(SourceRootError::ConfigIo)?;
    if !metadata.is_file() || is_reparse(&metadata) {
        return Err(SourceRootError::InvalidConfigPath);
    }
    let mut bytes = Vec::new();
    Read::by_ref(&mut file)
        .take((MAX_SOURCE_ROOT_FILE_BYTES + 1) as u64)
        .read_to_end(&mut bytes)
        .map_err(SourceRootError::ConfigIo)?;
    if bytes.len() > MAX_SOURCE_ROOT_FILE_BYTES {
        return Err(SourceRootError::ConfigTooLarge);
    }
    let text = std::str::from_utf8(&bytes).map_err(|_| SourceRootError::ConfigNotUtf8)?;
    if !text.ends_with('\n') {
        return Err(SourceRootError::CatalogCorrupt);
    }
    let mut lines = text.split_terminator('\n');
    if lines.next() != Some(HEADER) {
        return Err(SourceRootError::CatalogCorrupt);
    }
    let mut paths = Vec::new();
    let mut seen = BTreeSet::new();
    for line in lines {
        if paths.len() >= MAX_SOURCE_ROOTS {
            return Err(SourceRootError::RootLimitExceeded);
        }
        // Whitespace belongs to the path; trimming would admit a different root.
        let path = PathBuf::from(line);
        validate_persisted_path(&path)?;
        if !seen.insert(path.clone()) {
            return Err(SourceRootError::CatalogCorrupt);
        }
        paths.push(path);
    }
    Ok(paths)
}

fn persist_entries(path: &Path, entries: &[SourceRootEntry]) -> Result<(), SourceRootError> {
    if entries.len() > MAX_SOURCE_ROOTS {
        return Err(SourceRootError::RootLimitExceeded);
    }
    let parent = path.parent().ok_or(SourceRootError::InvalidConfigPath)?;
    reject_symlink(parent)?;
    fs::create_dir_all(parent).map_err(SourceRootError::ConfigIo)?;
    reject_symlink(path)?;
    let mut body = format!("{HEADER}\n");
    let mut expected = Vec::new();
    for entry in entries {
        validate_persisted_path(&entry.configured_path)?;
        body.push_str(path_text(&entry.configured_path)?);
        body.push('\n');
        expected.push(entry.configured_path.clone());
    }
    if body.len() > MAX_SOURCE_ROOT_FILE_BYTES {
        return Err(SourceRootError::ConfigTooLarge);
    }
    let temporary = path.with_extension("tmp");
    let backup = path.with_extension("bak");
    remove_plain_file_if_present(&temporary)?;
    reject_symlink(&backup)?;
    let mut file = OpenOptions::new().write(true).create_new(true)
        .open(&temporary).map_err(SourceRootError::ConfigIo)?;
    file.write_all(body.as_bytes()).and_then(|()| file.sync_all())
        .map_err(SourceRootError::ConfigIo)?;
    drop(file);
    if load_configured_paths(&temporary)? != expected {
        return Err(SourceRootError::CatalogCorrupt);
    }
    remove_plain_file_if_present(&backup)?;
    if path.try_exists().map_err(SourceRootError::ConfigIo)? {
        fs::rename(path, &backup).map_err(SourceRootError::ConfigIo)?;
    }
    // From this point the previous current path may have moved. Any failure
    // requires reopening/recovery; continuing with old in-memory roots is unsafe.
    fs::rename(&temporary, path).map_err(|_| SourceRootError::UpdateOutcomeUnknown)?;
    sync_directory(parent).map_err(|_| SourceRootError::UpdateOutcomeUnknown)?;
    if load_configured_paths(path).map_err(|_| SourceRootError::UpdateOutcomeUnknown)? != expected {
        return Err(SourceRootError::UpdateOutcomeUnknown);
    }
    remove_plain_file_if_present(&backup).map_err(|_| SourceRootError::UpdateOutcomeUnknown)?;
    sync_directory(parent).map_err(|_| SourceRootError::UpdateOutcomeUnknown)
}

fn recover_interrupted_update(path: &Path) -> Result<(), SourceRootError> {
    let backup = path.with_extension("bak");
    let temporary = path.with_extension("tmp");
    reject_symlink(path)?;
    reject_symlink(&backup)?;
    reject_symlink(&temporary)?;
    let current_exists = path.try_exists().map_err(SourceRootError::ConfigIo)?;
    let backup_exists = backup.try_exists().map_err(SourceRootError::ConfigIo)?;
    if current_exists {
        // Never replace a corrupt current catalog with a silently older one.
        load_configured_paths(path)?;
        remove_plain_file_if_present(&backup)?;
    } else if backup_exists {
        load_configured_paths(&backup)?;
        fs::rename(&backup, path).map_err(SourceRootError::ConfigIo)?;
        sync_directory(path.parent().ok_or(SourceRootError::InvalidConfigPath)?)?;
    }
    remove_plain_file_if_present(&temporary)
}

fn remove_plain_file_if_present(path: &Path) -> Result<(), SourceRootError> {
    match fs::symlink_metadata(path) {
        Ok(metadata) if metadata.file_type().is_symlink() || is_reparse(&metadata) => {
            Err(SourceRootError::SymlinkDenied)
        }
        Ok(metadata) if metadata.is_file() => fs::remove_file(path).map_err(SourceRootError::ConfigIo),
        Ok(_) => Err(SourceRootError::InvalidConfigPath),
        Err(error) if error.kind() == io::ErrorKind::NotFound => Ok(()),
        Err(error) => Err(SourceRootError::ConfigIo(error)),
    }
}

fn reject_symlink(path: &Path) -> Result<(), SourceRootError> {
    match fs::symlink_metadata(path) {
        Ok(metadata) if metadata.file_type().is_symlink() || is_reparse(&metadata) => {
            Err(SourceRootError::SymlinkDenied)
        }
        Ok(_) => Ok(()),
        Err(error) if error.kind() == io::ErrorKind::NotFound => Ok(()),
        Err(error) => Err(SourceRootError::ConfigIo(error)),
    }
}

#[cfg(windows)]
fn is_reparse(metadata: &Metadata) -> bool {
    use std::os::windows::fs::MetadataExt;
    metadata.file_attributes() & 0x400 != 0
}

#[cfg(not(windows))]
fn is_reparse(_metadata: &Metadata) -> bool {
    false
}

#[cfg(unix)]
fn sync_directory(path: &Path) -> Result<(), SourceRootError> {
    File::open(path).and_then(|file| file.sync_all()).map_err(SourceRootError::ConfigIo)
}

#[cfg(not(unix))]
fn sync_directory(_path: &Path) -> Result<(), SourceRootError> {
    // Windows power-loss durability needs native qualification; no such receipt
    // is emitted by this observation-registration adapter.
    Ok(())
}

#[derive(Debug)]
pub(crate) enum SourceRootError {
    RootLimitExceeded,
    RootNotFound,
    RootNotDirectory,
    RootOverlap,
    DataRootOverlap,
    RootPathNotAbsolute,
    RootPathNotUtf8,
    InvalidRootPath,
    InvalidConfigPath,
    ConfigTooLarge,
    ConfigNotUtf8,
    SymlinkDenied,
    CatalogCorrupt,
    UpdateOutcomeUnknown,
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
            Self::DataRootOverlap => "SOURCE_ROOT_DATA_ROOT_OVERLAP",
            Self::RootPathNotAbsolute => "SOURCE_ROOT_PATH_NOT_ABSOLUTE",
            Self::RootPathNotUtf8 => "SOURCE_ROOT_PATH_NOT_UTF8",
            Self::InvalidRootPath => "SOURCE_ROOT_PATH_INVALID",
            Self::InvalidConfigPath => "SOURCE_ROOT_CONFIG_PATH_INVALID",
            Self::ConfigTooLarge => "SOURCE_ROOT_CONFIG_TOO_LARGE",
            Self::ConfigNotUtf8 => "SOURCE_ROOT_CONFIG_NOT_UTF8",
            Self::SymlinkDenied => "SOURCE_ROOT_SYMLINK_DENIED",
            Self::CatalogCorrupt => "SOURCE_ROOT_CATALOG_CORRUPT",
            Self::UpdateOutcomeUnknown => "SOURCE_ROOT_UPDATE_OUTCOME_UNKNOWN",
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

impl std::error::Error for SourceRootError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            Self::RootIo(error) | Self::ConfigIo(error) => Some(error),
            _ => None,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::atomic::{AtomicU64, Ordering};
    use std::time::{SystemTime, UNIX_EPOCH};

    struct Sandbox(PathBuf);
    impl Sandbox {
        fn new() -> Self {
            static SEQUENCE: AtomicU64 = AtomicU64::new(0);
            let stamp = SystemTime::now().duration_since(UNIX_EPOCH).unwrap().as_nanos();
            let path = std::env::temp_dir().join(format!(
                "eliot-roots-{}-{stamp}-{}", std::process::id(), SEQUENCE.fetch_add(1, Ordering::Relaxed)
            ));
            fs::create_dir(&path).unwrap();
            Self(fs::canonicalize(path).unwrap())
        }
        fn directory(&self, name: &str) -> PathBuf {
            let path = self.0.join(name);
            fs::create_dir(&path).unwrap();
            fs::canonicalize(path).unwrap()
        }
    }
    impl Drop for Sandbox {
        fn drop(&mut self) { let _ = fs::remove_dir_all(&self.0); }
    }

    #[test]
    fn persists_reloads_adds_and_removes() {
        let sandbox = Sandbox::new();
        let first = sandbox.directory("first");
        let second = sandbox.directory("second");
        let config = sandbox.0.join("state/source-roots.txt");
        let mut catalog = SourceRootCatalog::load(config.clone(), std::slice::from_ref(&first)).unwrap();
        assert_eq!(catalog.available_count(), 1);
        catalog.add(&second).unwrap();
        assert_eq!(SourceRootCatalog::load(config.clone(), &[]).unwrap().configured_count(), 2);
        catalog.remove(&first).unwrap();
        let reopened = SourceRootCatalog::load(config, &[]).unwrap();
        assert_eq!(reopened.configured_count(), 1);
        assert_eq!(reopened.available_paths()[0].1, second);
    }

    #[test]
    fn missing_root_is_retained_but_unavailable() {
        let sandbox = Sandbox::new();
        let source = sandbox.directory("source");
        let config = sandbox.0.join("state/source-roots.txt");
        SourceRootCatalog::load(config.clone(), std::slice::from_ref(&source)).unwrap();
        fs::remove_dir(&source).unwrap();
        let mut reopened = SourceRootCatalog::load(config.clone(), &[]).unwrap();
        assert_eq!(reopened.configured_count(), 1);
        assert_eq!(reopened.unavailable_count(), 1);
        reopened.remove(&source).unwrap();
        assert_eq!(SourceRootCatalog::load(config, &[]).unwrap().configured_count(), 0);
    }

    #[test]
    fn refresh_reports_swapped_availability_with_unchanged_count() {
        let sandbox = Sandbox::new();
        let first = sandbox.directory("first");
        let second = sandbox.directory("second");
        let mut catalog = SourceRootCatalog::load(sandbox.0.join("roots.txt"), &[first.clone(), second.clone()]).unwrap();
        fs::remove_dir(&second).unwrap();
        assert!(catalog.refresh());
        fs::remove_dir(&first).unwrap();
        fs::create_dir(&second).unwrap();
        assert!(catalog.refresh());
        assert_eq!(catalog.available_count(), 1);
        assert!(!catalog.refresh());
    }

    #[test]
    fn owned_catalog_rejects_data_root_in_both_overlap_directions() {
        let sandbox = Sandbox::new();
        let data = sandbox.directory("data");
        let nested = data.join("nested");
        fs::create_dir(&nested).unwrap();
        let sibling = sandbox.directory("data-other");
        let mut catalog = SourceRootCatalog::load_owned(&data).unwrap();
        for path in [&data, &nested, &sandbox.0] {
            assert!(matches!(catalog.add(path), Err(SourceRootError::DataRootOverlap)));
        }
        catalog.add(&sibling).unwrap();
        assert_eq!(SourceRootCatalog::load_owned(&data).unwrap().configured_count(), 1);
    }

    #[test]
    fn malformed_current_catalog_is_not_replaced_by_valid_backup() {
        let sandbox = Sandbox::new();
        let source = sandbox.directory("source");
        let config = sandbox.0.join("roots.v1");
        SourceRootCatalog::load(config.clone(), &[source]).unwrap();
        fs::copy(&config, config.with_extension("bak")).unwrap();
        fs::write(&config, b"truncated").unwrap();
        assert!(SourceRootCatalog::load(config.clone(), &[]).is_err());
        assert_eq!(fs::read(&config).unwrap(), b"truncated");
        assert!(config.with_extension("bak").exists());
    }

    #[test]
    fn interrupted_replacement_restores_last_current_catalog() {
        let sandbox = Sandbox::new();
        let source = sandbox.directory("source");
        let config = sandbox.0.join("roots.v1");
        SourceRootCatalog::load(config.clone(), &[source]).unwrap();
        fs::rename(&config, config.with_extension("bak")).unwrap();
        fs::write(config.with_extension("tmp"), b"incomplete").unwrap();
        let reopened = SourceRootCatalog::load(config.clone(), &[]).unwrap();
        assert_eq!(reopened.configured_count(), 1);
        assert!(!config.with_extension("tmp").exists());
    }

    #[test]
    fn replacement_by_regular_file_does_not_prevent_unregistering() {
        let sandbox = Sandbox::new();
        let source = sandbox.directory("source");
        let mut catalog = SourceRootCatalog::load(sandbox.0.join("roots.v1"), std::slice::from_ref(&source)).unwrap();
        fs::remove_dir(&source).unwrap();
        fs::write(&source, b"not a directory").unwrap();
        assert!(catalog.refresh());
        assert_eq!(catalog.views().unwrap()[0].state, SourceRootState::NotDirectory);
        catalog.remove(&source).unwrap();
    }

    #[test]
    fn nested_sources_are_rejected_and_duplicates_are_idempotent() {
        let sandbox = Sandbox::new();
        let source = sandbox.directory("source");
        let nested = source.join("nested");
        fs::create_dir(&nested).unwrap();
        let mut catalog = SourceRootCatalog::load(sandbox.0.join("roots.v1"), std::slice::from_ref(&source)).unwrap();
        catalog.add(&source).unwrap();
        assert_eq!(catalog.configured_count(), 1);
        assert!(matches!(catalog.add(&nested), Err(SourceRootError::RootOverlap)));
    }

    #[cfg(unix)]
    #[test]
    fn trailing_spaces_are_part_of_the_persisted_path() {
        let sandbox = Sandbox::new();
        let source = sandbox.directory("source ");
        let config = sandbox.0.join("roots.v1");
        SourceRootCatalog::load(config.clone(), std::slice::from_ref(&source)).unwrap();
        let reopened = SourceRootCatalog::load(config, &[]).unwrap();
        assert_eq!(reopened.available_paths()[0].1, source);
    }

    #[cfg(unix)]
    #[test]
    fn symbolic_source_or_config_links_are_rejected() {
        use std::os::unix::fs::symlink;
        let sandbox = Sandbox::new();
        let source = sandbox.directory("source");
        let link = sandbox.0.join("link");
        symlink(&source, &link).unwrap();
        assert!(SourceRootCatalog::load(sandbox.0.join("roots.v1"), &[link]).is_err());
        let target = sandbox.0.join("target");
        fs::write(&target, format!("{HEADER}\n")).unwrap();
        let config = sandbox.0.join("config.v1");
        symlink(&target, &config).unwrap();
        assert!(SourceRootCatalog::load(config, &[]).is_err());
    }
}

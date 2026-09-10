//! Persistent bounded observation-root catalog for the primary daemon.
//!
//! The caller holds the data-root owner lock for this catalog's entire lifetime.
//! Registration is observation configuration, not a source identity, access grant,
//! current-workspace proof, or purge instruction.

use std::collections::BTreeSet;
use std::fs::{self, File, Metadata, OpenOptions};
use std::io::{self, Read, Write};
use std::path::{Component, Path, PathBuf};

pub const MAX_SOURCE_ROOTS: usize = 32;
pub const MAX_SOURCE_ROOT_FILE_BYTES: usize = 64 * 1024;
pub const MAX_SOURCE_ROOT_PATH_BYTES: usize = 512;
/// Maximum watcher hints retained in memory. Hints are dirty markers only;
/// overflow becomes an explicit gap that forces a full refresh, never a silent drop.
///
/// Live-daemon bound: one-shot CLI never fills this queue (see
/// [`SourceRootCatalog::note_watcher_hint`]).
#[allow(dead_code)]
pub const MAX_WATCHER_HINTS: usize = 64;
/// Maximum observation gaps reported in one truth snapshot. One per
/// configured root plus watcher-overflow and outcome-unknown fences.
pub const MAX_OBSERVATION_GAPS: usize = 40;
const HEADER: &str = "# ELIOT Search source roots v1";

#[derive(Clone, Copy, Debug, Eq, Ord, PartialEq, PartialOrd)]
pub enum SourceRootState {
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
pub struct SourceRootView {
    pub(crate) index: usize,
    pub(crate) path: String,
    pub(crate) state: SourceRootState,
}

#[derive(Debug)]
pub struct SourceRootCatalog {
    config_path: PathBuf,
    entries: Vec<SourceRootEntry>,
    excluded_data_root: Option<PathBuf>,
    needs_reopen: bool,
    watcher_sequence: u64,
    pending_hints: Vec<WatcherHint>,
    watcher_overflowed: bool,
    reconciliation_generation: u64,
    last_synced_generation: Option<u64>,
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
            #[cfg(unix)]
            sync_directory(&canonical)?;
            #[cfg(not(unix))]
            sync_directory(&canonical);
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
            watcher_sequence: 0,
            pending_hints: Vec::new(),
            watcher_overflowed: false,
            reconciliation_generation: 0,
            last_synced_generation: None,
        };
        catalog.refresh();
        if !command_roots.is_empty() {
            persist_entries(&catalog.config_path, &catalog.entries)?;
        }
        Ok(catalog)
    }

    pub(crate) const fn configured_count(&self) -> usize {
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
    /// A watcher hint never changes availability by itself; only this explicit
    /// refresh reconciles hints into probed states. Consumed hints are drained;
    /// an overflowed watcher forces a generation bump so currentness is lost
    /// until the next successful multi-root sync re-proves it.
    pub(crate) fn refresh(&mut self) -> bool {
        if self.needs_reopen {
            return false;
        }
        let had_overflow = self.watcher_overflowed;
        let had_hints = !self.pending_hints.is_empty();
        let mut changed = false;
        for entry in &mut self.entries {
            let observed = probe_root(&entry.configured_path);
            changed |= entry.state != observed;
            entry.state = observed;
        }
        self.pending_hints.clear();
        self.watcher_overflowed = false;
        if changed || had_overflow || had_hints {
            self.reconciliation_generation = self.reconciliation_generation.wrapping_add(1);
            // The active set or its observability moved: a previous sync no
            // longer proves the current workspace.
            self.last_synced_generation = None;
        }
        changed || had_overflow || had_hints
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
            let observed = probe_root(&canonical);
            if self.entries[index].state != observed {
                self.entries[index].state = observed;
                self.reconciliation_generation = self.reconciliation_generation.wrapping_add(1);
                self.last_synced_generation = None;
            }
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
        let index = if let Some(index) = index {
            index
        } else {
            let canonical = canonicalize_new_root(requested)?;
            self.entries.iter().position(|entry| entry.configured_path == canonical)
                .ok_or(SourceRootError::RootNotFound)?
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
        // The registered active set changed: prior sync proof no longer covers
        // the current workspace. Unregistration is explicit and never revokes
        // retained revisions, but it still blocks currentness until re-sync.
        self.reconciliation_generation = self.reconciliation_generation.wrapping_add(1);
        self.last_synced_generation = None;
        Ok(())
    }

    const fn ensure_usable(&self) -> Result<(), SourceRootError> {
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

    /// Records one watcher notification as a hint only. Hints never change
    /// availability; they only mark the catalog dirty so the next explicit
    /// [`Self::refresh`] reconciles authoritative probe results. Bounded:
    /// beyond [`MAX_WATCHER_HINTS`] the catalog keeps an explicit overflow
    /// fence instead of growing or silently dropping.
    ///
    /// Returns `true` when the watcher fence is overflowed after this hint.
    ///
    /// Live-daemon only: one-shot CLI processes start with an empty hint
    /// queue and reconcile via explicit `refresh`, so this entry stays
    /// unused on the CLI path by construction.
    #[allow(dead_code)]
    pub(crate) fn note_watcher_hint(
        &mut self,
        position: usize,
        kind: WatcherHintKind,
    ) -> Result<bool, SourceRootError> {
        self.ensure_usable()?;
        if position >= self.entries.len() {
            return Err(SourceRootError::RootNotFound);
        }
        self.watcher_sequence = self.watcher_sequence.wrapping_add(1);
        if self.pending_hints.len() >= MAX_WATCHER_HINTS {
            self.watcher_overflowed = true;
            return Ok(true);
        }
        self.pending_hints.push(WatcherHint {
            position,
            kind,
            sequence: self.watcher_sequence,
        });
        Ok(self.watcher_overflowed)
    }

    /// Drains pending watcher hints for one explicit reconciliation pass.
    /// The caller must still call [`Self::refresh`]; draining alone proves nothing.
    ///
    /// Live-daemon only: see [`Self::note_watcher_hint`].
    #[allow(dead_code)]
    pub(crate) fn drain_watcher_hints(&mut self) -> Vec<WatcherHint> {
        std::mem::take(&mut self.pending_hints)
    }

    /// Whether the bounded watcher fence overflowed since the last refresh.
    ///
    /// Live-daemon only: see [`Self::note_watcher_hint`].
    #[allow(dead_code)]
    pub(crate) const fn watcher_overflowed(&self) -> bool {
        self.watcher_overflowed
    }

    /// Explicit observation gaps blocking currentness. A missing, replaced,
    /// unsafe or unverifiable root is a typed gap, never an empty inventory.
    /// A watcher overflow is its own gap and forces a full refresh. When the
    /// catalog needs reopening, the single outcome-unknown gap blocks every
    /// current claim. Bounded by [`MAX_OBSERVATION_GAPS`]; excess is a fence,
    /// never a silent truncation.
    pub(crate) fn observation_gaps(&self) -> Vec<ObservationGap> {
        let mut gaps = Vec::new();
        if self.needs_reopen {
            gaps.push(ObservationGap {
                position: 0,
                reason: ObservationGapReason::UpdateOutcomeUnknown,
                state: SourceRootState::Unverifiable,
            });
            return gaps;
        }
        for (position, entry) in self.entries.iter().enumerate() {
            let reason = match entry.state {
                SourceRootState::Available => continue,
                SourceRootState::Missing => ObservationGapReason::Missing,
                SourceRootState::NotDirectory => ObservationGapReason::NotDirectory,
                SourceRootState::Unsafe => ObservationGapReason::Unsafe,
                SourceRootState::Unverifiable => ObservationGapReason::Unverifiable,
            };
            if gaps.len() >= MAX_OBSERVATION_GAPS {
                break;
            }
            gaps.push(ObservationGap {
                position,
                reason,
                state: entry.state,
            });
        }
        if self.watcher_overflowed && gaps.len() < MAX_OBSERVATION_GAPS {
            gaps.push(ObservationGap {
                position: self.entries.len(),
                reason: ObservationGapReason::WatcherOverflow,
                state: SourceRootState::Unverifiable,
            });
        }
        gaps
    }

    /// Bounded reconciliation cursor for canonical control-state diagnostics.
    /// The cursor is a dirty marker, not proof: only [`Self::refresh`] plus a
    /// successful multi-root [`Self::mark_reconciled_synced`] can re-prove
    /// currentness.
    pub(crate) const fn reconciliation_cursor(&self) -> ReconciliationCursor {
        ReconciliationCursor {
            generation: self.reconciliation_generation,
            pending_hints: self.pending_hints.len(),
            overflowed: self.watcher_overflowed,
            hint_sequence: self.watcher_sequence,
            last_synced_generation: self.last_synced_generation,
        }
    }

    /// Independent source/workspace currentness snapshot. Source truth is the
    /// probed active set; workspace truth additionally requires the last
    /// successful multi-root sync to cover the current reconciliation
    /// generation. Index truth lives outside this catalog (qualified
    /// Qdrant/artifacts/routes) and always blocks the final proven claim in
    /// this shell via the provider roots-section.
    pub(crate) fn current_workspace_truth(&self) -> CurrentWorkspaceTruth {
        let gaps = self.observation_gaps();
        let configured = self.configured_count();
        let available = self.available_count();
        let unavailable = self.unavailable_count();
        let source_current =
            !self.needs_reopen && configured > 0 && gaps.is_empty() && available == configured;
        let workspace_current = source_current
            && self.last_synced_generation == Some(self.reconciliation_generation);
        CurrentWorkspaceTruth {
            configured,
            available,
            unavailable,
            gap_count: gaps.len(),
            reconciliation_generation: self.reconciliation_generation,
            last_synced_generation: self.last_synced_generation,
            source_current,
            workspace_current,
        }
    }

    /// Marks the current reconciliation generation as sync-proven. Fails
    /// closed: with any unresolved gap, no configured root, or a poisoned
    /// catalog the mark is refused and currentness stays unproven. A later
    /// `add`/`remove`/`refresh` invalidates the mark via generation bump.
    pub(crate) fn mark_reconciled_synced(&mut self) -> bool {
        if self.needs_reopen || self.entries.is_empty() || !self.observation_gaps().is_empty() {
            return false;
        }
        self.last_synced_generation = Some(self.reconciliation_generation);
        true
    }
}

/// Closed watcher-hint kind. Hints are dirty markers only; they carry no
/// inventory contents and never authorize a current claim.
///
/// Live-daemon only: one-shot CLI reconciles via explicit `refresh`, so these
/// variants stay unused on the CLI path by construction.
#[allow(dead_code)]
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum WatcherHintKind {
    Modified,
    Created,
    Removed,
    Rescan,
    Overflow,
}

impl WatcherHintKind {
    /// Stable wire spelling for diagnostics.
    #[must_use]
    #[allow(dead_code)]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Modified => "modified",
            Self::Created => "created",
            Self::Removed => "removed",
            Self::Rescan => "rescan",
            Self::Overflow => "overflow",
        }
    }
}

/// One bounded watcher hint: which registered position looked dirty and why.
/// The sequence orders hints inside one catalog lifetime; it is never
/// persisted and never proves currency.
///
/// Live-daemon only: see [`WatcherHintKind`].
#[allow(dead_code)]
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct WatcherHint {
    pub(crate) position: usize,
    pub(crate) kind: WatcherHintKind,
    pub(crate) sequence: u64,
}

/// Typed observation-gap reason. Every non-available root and every
/// watcher/poison fence maps to exactly one closed reason.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ObservationGapReason {
    Missing,
    NotDirectory,
    Unsafe,
    Unverifiable,
    WatcherOverflow,
    UpdateOutcomeUnknown,
}

impl ObservationGapReason {
    /// Stable machine-readable gap code.
    #[must_use]
    pub const fn code(self) -> &'static str {
        match self {
            Self::Missing => "OBSERVATION_GAP_MISSING",
            Self::NotDirectory => "OBSERVATION_GAP_NOT_DIRECTORY",
            Self::Unsafe => "OBSERVATION_GAP_UNSAFE",
            Self::Unverifiable => "OBSERVATION_GAP_UNVERIFIABLE",
            Self::WatcherOverflow => "OBSERVATION_GAP_WATCHER_OVERFLOW",
            Self::UpdateOutcomeUnknown => "OBSERVATION_GAP_UPDATE_OUTCOME_UNKNOWN",
        }
    }
}

/// One explicit observation gap blocking currentness. Positions reference the
/// sorted registration order; reasons are closed codes without paths or bytes.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct ObservationGap {
    pub(crate) position: usize,
    pub(crate) reason: ObservationGapReason,
    pub(crate) state: SourceRootState,
}

/// Bounded reconciliation cursor snapshot for control-state diagnostics.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct ReconciliationCursor {
    pub(crate) generation: u64,
    pub(crate) pending_hints: usize,
    pub(crate) overflowed: bool,
    pub(crate) hint_sequence: u64,
    pub(crate) last_synced_generation: Option<u64>,
}

/// Independent source/workspace truth snapshot. `source_current` covers the
/// probed active set; `workspace_current` additionally requires a successful
/// multi-root sync over the current generation. The final proven claim also
/// needs index/barrier truth from the provider roots-section.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct CurrentWorkspaceTruth {
    pub(crate) configured: usize,
    pub(crate) available: usize,
    pub(crate) unavailable: usize,
    pub(crate) gap_count: usize,
    pub(crate) reconciliation_generation: u64,
    pub(crate) last_synced_generation: Option<u64>,
    pub(crate) source_current: bool,
    pub(crate) workspace_current: bool,
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
    read_configured_bytes(path)?.as_deref().map(decode_configured_paths)
        .transpose().map(Option::unwrap_or_default)
}

fn read_configured_bytes(path: &Path) -> Result<Option<Vec<u8>>, SourceRootError> {
    reject_symlink(path)?;
    let mut file = match File::open(path) {
        Ok(file) => file,
        Err(error) if error.kind() == io::ErrorKind::NotFound => return Ok(None),
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
    let after = file.metadata().map_err(SourceRootError::ConfigIo)?;
    if bytes.len() as u64 != metadata.len() || metadata.len() != after.len()
        || metadata.modified().ok() != after.modified().ok()
    {
        return Err(SourceRootError::CatalogCorrupt);
    }
    Ok(Some(bytes))
}

fn decode_configured_paths(bytes: &[u8]) -> Result<Vec<PathBuf>, SourceRootError> {
    let text = std::str::from_utf8(bytes).map_err(|_| SourceRootError::ConfigNotUtf8)?;
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
    #[cfg(unix)]
    sync_directory(parent).map_err(|_| SourceRootError::UpdateOutcomeUnknown)?;
    #[cfg(not(unix))]
    sync_directory(parent);
    if load_configured_paths(path).map_err(|_| SourceRootError::UpdateOutcomeUnknown)? != expected {
        return Err(SourceRootError::UpdateOutcomeUnknown);
    }
    remove_plain_file_if_present(&backup).map_err(|_| SourceRootError::UpdateOutcomeUnknown)?;
    #[cfg(unix)]
    {
        sync_directory(parent).map_err(|_| SourceRootError::UpdateOutcomeUnknown)
    }
    #[cfg(not(unix))]
    {
        sync_directory(parent);
        Ok(())
    }
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
        #[cfg(unix)]
        sync_directory(path.parent().ok_or(SourceRootError::InvalidConfigPath)?)?;
        #[cfg(not(unix))]
        sync_directory(path.parent().ok_or(SourceRootError::InvalidConfigPath)?);
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
const fn sync_directory(_path: &Path) {
    // Windows power-loss durability needs native qualification; no such receipt
    // is emitted by this observation-registration adapter.
}

#[derive(Debug)]
pub enum SourceRootError {
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

/// Exact persisted registration input. No current-path probes, recovery or new owner.
/// Paths are retained internally for a future importer; diagnostics must redact them.
#[derive(Eq, PartialEq)]
pub struct RootMigrationInput {
    pub(crate) paths: Vec<PathBuf>,
    pub(crate) file_bytes: Option<Vec<u8>>,
}

/// Reads registration without invoking `load_owned/load` or recovering .tmp/.bak files.
/// Absence is explicit and is not proof that no registration existed previously.
pub fn migration_input(data_root: &Path) -> Result<RootMigrationInput, SourceRootError> {
    reject_symlink(data_root)?;
    let canonical = fs::canonicalize(data_root).map_err(SourceRootError::RootIo)?;
    let control = canonical.join("control");
    reject_symlink(&control)?;
    if !fs::symlink_metadata(&control).map_err(SourceRootError::ConfigIo)?.is_dir()
        || fs::canonicalize(&control).map_err(SourceRootError::ConfigIo)? != control
    {
        return Err(SourceRootError::InvalidConfigPath);
    }
    let path = control.join("source-roots.v1");
    for pending in [path.with_extension("tmp"), path.with_extension("bak")] {
        match fs::symlink_metadata(pending) {
            Ok(_) => return Err(SourceRootError::UpdateOutcomeUnknown),
            Err(error) if error.kind() == io::ErrorKind::NotFound => {}
            Err(error) => return Err(SourceRootError::ConfigIo(error)),
        }
    }
    let file_bytes = read_configured_bytes(&path)?;
    let mut paths = file_bytes.as_deref().map(decode_configured_paths)
        .transpose()?.unwrap_or_default();
    canonicalize_configured_set(&mut paths)?;
    for path in &paths { ensure_outside_data_root(path, &canonical)?; }
    Ok(RootMigrationInput { paths, file_bytes })
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

    #[test]
    fn watcher_hints_are_bounded_and_never_prove_availability() {
        let sandbox = Sandbox::new();
        let source = sandbox.directory("source");
        let mut catalog =
            SourceRootCatalog::load(sandbox.0.join("roots.v1"), std::slice::from_ref(&source))
                .unwrap();
        assert_eq!(WatcherHintKind::Modified.as_str(), "modified");
        assert_eq!(WatcherHintKind::Created.as_str(), "created");
        assert_eq!(WatcherHintKind::Removed.as_str(), "removed");
        assert_eq!(WatcherHintKind::Rescan.as_str(), "rescan");
        assert_eq!(WatcherHintKind::Overflow.as_str(), "overflow");
        assert!(!catalog.watcher_overflowed());
        assert_eq!(catalog.observation_gaps().len(), 0);
        // Hints alone change nothing observable until an explicit refresh.
        assert!(!catalog.note_watcher_hint(0, WatcherHintKind::Modified).unwrap());
        assert!(!catalog.note_watcher_hint(0, WatcherHintKind::Created).unwrap());
        assert!(!catalog.note_watcher_hint(0, WatcherHintKind::Removed).unwrap());
        assert!(!catalog.note_watcher_hint(0, WatcherHintKind::Rescan).unwrap());
        assert_eq!(catalog.available_count(), 1);
        assert_eq!(catalog.observation_gaps().len(), 0);
        assert_eq!(catalog.drain_watcher_hints().len(), 4);
        // Unknown positions fail closed instead of allocating.
        assert!(matches!(
            catalog.note_watcher_hint(7, WatcherHintKind::Removed),
            Err(SourceRootError::RootNotFound)
        ));
        // Overflow becomes an explicit gap, never a silent drop or growth.
        for _ in 0..(MAX_WATCHER_HINTS + 4) {
            let _ = catalog.note_watcher_hint(0, WatcherHintKind::Modified);
        }
        assert!(catalog.watcher_overflowed());
        assert!(
            catalog
                .observation_gaps()
                .iter()
                .any(|gap| gap.reason == ObservationGapReason::WatcherOverflow)
        );
        assert!(!catalog.current_workspace_truth().source_current);
        // An explicit refresh reconciles and clears the fence.
        assert!(catalog.refresh());
        assert!(!catalog.watcher_overflowed());
        assert_eq!(catalog.observation_gaps().len(), 0);
    }

    #[test]
    fn missing_and_replaced_roots_are_gaps_and_block_sync_proof() {
        let sandbox = Sandbox::new();
        let source = sandbox.directory("source");
        let mut catalog =
            SourceRootCatalog::load(sandbox.0.join("roots.v1"), std::slice::from_ref(&source))
                .unwrap();
        assert!(!catalog.current_workspace_truth().workspace_current);
        assert!(catalog.mark_reconciled_synced());
        assert!(catalog.current_workspace_truth().workspace_current);
        // Missing is retained but gapped; sync proof is refused and invalidated.
        fs::remove_dir(&source).unwrap();
        assert!(catalog.refresh());
        let gaps = catalog.observation_gaps();
        assert_eq!(gaps.len(), 1);
        assert_eq!(gaps[0].reason, ObservationGapReason::Missing);
        assert_eq!(gaps[0].reason.code(), "OBSERVATION_GAP_MISSING");
        assert!(!catalog.mark_reconciled_synced());
        assert!(!catalog.current_workspace_truth().workspace_current);
        assert!(!catalog.current_workspace_truth().source_current);
        // Replaced-by-file keeps the same gap discipline with its own reason.
        fs::write(&source, b"not a directory").unwrap();
        assert!(catalog.refresh());
        let gaps = catalog.observation_gaps();
        assert_eq!(gaps.len(), 1);
        assert_eq!(gaps[0].reason, ObservationGapReason::NotDirectory);
        // Explicit unregistration clears the gap without revoking history;
        // an empty catalog still never claims current.
        catalog.remove(&source).unwrap();
        assert_eq!(catalog.observation_gaps().len(), 0);
        assert!(!catalog.mark_reconciled_synced());
        assert!(!catalog.current_workspace_truth().workspace_current);
    }

    #[test]
    fn active_set_mutation_invalidates_sync_proof() {
        let sandbox = Sandbox::new();
        let first = sandbox.directory("first");
        let second = sandbox.directory("second");
        let mut catalog = SourceRootCatalog::load(
            sandbox.0.join("roots.v1"),
            &[first.clone(), second],
        )
        .unwrap();
        assert!(catalog.mark_reconciled_synced());
        let generation = catalog.reconciliation_cursor().generation;
        catalog.remove(&first).unwrap();
        assert_ne!(catalog.reconciliation_cursor().generation, generation);
        assert!(!catalog.current_workspace_truth().workspace_current);
        // Re-adding re-proves only after an explicit sync mark with no gaps.
        let third = sandbox.directory("third");
        catalog.add(&third).unwrap();
        assert!(!catalog.current_workspace_truth().workspace_current);
        assert!(catalog.mark_reconciled_synced());
        assert!(catalog.current_workspace_truth().workspace_current);
    }
}

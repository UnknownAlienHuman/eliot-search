//! Live observation catalog state, watcher hints and currentness proof.

use std::fs;
use std::path::{Path, PathBuf};

use super::error::SourceRootError;
use super::model::{
    CurrentWorkspaceTruth, ObservationGap, ObservationGapReason,
    ReconciliationCursor, SourceRootEntry, SourceRootState, SourceRootView,
    WatcherHint, WatcherHintKind,
};
use super::path::{
    canonicalize_configured_set, canonicalize_new_root,
    ensure_no_overlap, ensure_outside_data_root, path_text, probe_root,
    reject_symlink, sync_directory,
};
use super::registry::{
    load_configured_paths, persist_entries, recover_interrupted_update,
};
use super::spec::{MAX_OBSERVATION_GAPS, MAX_SOURCE_ROOTS, MAX_WATCHER_HINTS};

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
        if !fs::metadata(&canonical)
            .map_err(SourceRootError::RootIo)?
            .is_dir()
        {
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
        if !fs::symlink_metadata(&control)
            .map_err(SourceRootError::ConfigIo)?
            .is_dir()
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
            entries: configured
                .into_iter()
                .map(|configured_path| SourceRootEntry {
                    configured_path,
                    state: SourceRootState::Unverifiable,
                })
                .collect(),
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
        self.entries
            .iter()
            .filter(|entry| entry.state == SourceRootState::Available)
            .count()
    }

    pub(crate) fn unavailable_count(&self) -> usize {
        self.configured_count().saturating_sub(self.available_count())
    }

    /// Reconciles watcher hints against authoritative root probes.
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
            self.reconciliation_generation =
                self.reconciliation_generation.wrapping_add(1);
            self.last_synced_generation = None;
        }
        changed || had_overflow || had_hints
    }

    pub(crate) fn available_paths(&self) -> Vec<(usize, &Path)> {
        if self.needs_reopen {
            return Vec::new();
        }
        self.entries
            .iter()
            .enumerate()
            .filter(|(_, entry)| entry.state == SourceRootState::Available)
            .map(|(index, entry)| (index, entry.configured_path.as_path()))
            .collect()
    }

    pub(crate) fn views(&self) -> Result<Vec<SourceRootView>, SourceRootError> {
        self.ensure_usable()?;
        (0..self.entries.len())
            .map(|index| self.view(index))
            .collect()
    }

    pub(crate) fn add(
        &mut self,
        requested: &Path,
    ) -> Result<SourceRootView, SourceRootError> {
        self.ensure_usable()?;
        let canonical = canonicalize_new_root(requested)?;
        if let Some(data_root) = &self.excluded_data_root {
            ensure_outside_data_root(&canonical, data_root)?;
        }
        if let Some(index) = self
            .entries
            .iter()
            .position(|entry| entry.configured_path == canonical)
        {
            let observed = probe_root(&canonical);
            if self.entries[index].state != observed {
                self.entries[index].state = observed;
                self.reconciliation_generation =
                    self.reconciliation_generation.wrapping_add(1);
                self.last_synced_generation = None;
            }
            return self.view(index);
        }
        if self.entries.len() >= MAX_SOURCE_ROOTS {
            return Err(SourceRootError::RootLimitExceeded);
        }
        ensure_no_overlap(
            self.entries.iter().map(|entry| &entry.configured_path),
            &canonical,
        )?;
        let index = self
            .entries
            .partition_point(|entry| entry.configured_path < canonical);
        let mut staged = self.entries.clone();
        staged.insert(
            index,
            SourceRootEntry {
                configured_path: canonical,
                state: SourceRootState::Available,
            },
        );
        self.commit(staged)?;
        self.view(index)
    }

    /// Removes observation registration without revoking retained revisions.
    pub(crate) fn remove(&mut self, requested: &Path) -> Result<String, SourceRootError> {
        self.ensure_usable()?;
        let index = self
            .entries
            .iter()
            .position(|entry| entry.configured_path == requested);
        let index = if let Some(index) = index {
            index
        } else {
            let canonical = canonicalize_new_root(requested)?;
            self.entries
                .iter()
                .position(|entry| entry.configured_path == canonical)
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

    /// Records one watcher notification as a bounded dirty hint only.
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

    #[allow(dead_code)]
    pub(crate) fn drain_watcher_hints(&mut self) -> Vec<WatcherHint> {
        core::mem::take(&mut self.pending_hints)
    }

    #[allow(dead_code)]
    pub(crate) const fn watcher_overflowed(&self) -> bool {
        self.watcher_overflowed
    }

    /// Returns explicit bounded gaps blocking currentness.
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

    pub(crate) const fn reconciliation_cursor(&self) -> ReconciliationCursor {
        ReconciliationCursor {
            generation: self.reconciliation_generation,
            pending_hints: self.pending_hints.len(),
            overflowed: self.watcher_overflowed,
            hint_sequence: self.watcher_sequence,
            last_synced_generation: self.last_synced_generation,
        }
    }

    /// Computes source/workspace truth without index truth.
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

    /// Marks the current reconciliation generation as sync-proven.
    pub(crate) fn mark_reconciled_synced(&mut self) -> bool {
        if self.needs_reopen
            || self.entries.is_empty()
            || !self.observation_gaps().is_empty()
        {
            return false;
        }
        self.last_synced_generation = Some(self.reconciliation_generation);
        true
    }
}

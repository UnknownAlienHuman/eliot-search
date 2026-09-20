//! Live observation catalog I/O composed with package-owned currentness state.

use std::fs;
use std::path::{Path, PathBuf};

use search_source_registry::{
    CurrentWorkspaceTruth, ObservationGap, ReconciliationCursor,
    SourceRootCurrentness, SourceRootCurrentnessError, SourceRootState,
    WatcherHint, WatcherHintKind,
};

use super::error::SourceRootError;
use super::model::{SourceRootEntry, SourceRootView};
use super::path::{
    canonicalize_configured_set, canonicalize_new_root,
    ensure_no_overlap, ensure_outside_data_root, path_text, probe_root,
    reject_symlink, sync_directory,
};
use super::registry::{
    load_configured_paths, persist_entries, recover_interrupted_update,
};
use super::spec::MAX_SOURCE_ROOTS;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum RegistrationMutation {
    Insert {
        position: usize,
        state: SourceRootState,
    },
    Remove {
        position: usize,
    },
}

#[derive(Debug)]
pub struct SourceRootCatalog {
    config_path: PathBuf,
    entries: Vec<SourceRootEntry>,
    excluded_data_root: Option<PathBuf>,
    currentness: SourceRootCurrentness,
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
        let entries = configured
            .into_iter()
            .map(|configured_path| SourceRootEntry { configured_path })
            .collect::<Vec<_>>();
        let currentness = SourceRootCurrentness::new(entries.len())
            .map_err(map_currentness_error)?;
        let mut catalog = Self {
            config_path,
            entries,
            excluded_data_root: None,
            currentness,
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
        self.currentness.available_count()
    }

    pub(crate) fn unavailable_count(&self) -> usize {
        self.currentness.unavailable_count()
    }

    /// Reconciles watcher hints against authoritative root probes.
    pub(crate) fn refresh(&mut self) -> bool {
        if self.currentness.update_outcome_unknown() {
            return false;
        }
        let observed = self
            .entries
            .iter()
            .map(|entry| probe_root(&entry.configured_path))
            .collect::<Vec<_>>();
        self.currentness.reconcile(&observed)
    }

    pub(crate) fn available_paths(&self) -> Vec<(usize, &Path)> {
        if self.currentness.update_outcome_unknown() {
            return Vec::new();
        }
        self.entries
            .iter()
            .enumerate()
            .filter(|(index, _)| {
                self.currentness.state(*index) == Some(SourceRootState::Available)
            })
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
            if let Err(error) = self.currentness.observe(index, observed) {
                self.currentness.mark_update_outcome_unknown();
                return Err(map_currentness_error(error));
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
            },
        );
        self.commit(
            staged,
            RegistrationMutation::Insert {
                position: index,
                state: SourceRootState::Available,
            },
        )?;
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
        self.commit(staged, RegistrationMutation::Remove { position: index })?;
        Ok(removed)
    }

    fn commit(
        &mut self,
        staged: Vec<SourceRootEntry>,
        mutation: RegistrationMutation,
    ) -> Result<(), SourceRootError> {
        if let Err(error) = persist_entries(&self.config_path, &staged) {
            self.currentness.mark_update_outcome_unknown();
            return Err(error);
        }
        self.entries = staged;
        let result = match mutation {
            RegistrationMutation::Insert { position, state } => {
                self.currentness.insert(position, state)
            }
            RegistrationMutation::Remove { position } => {
                self.currentness.remove(position)
            }
        };
        if let Err(error) = result {
            self.currentness.mark_update_outcome_unknown();
            return Err(map_currentness_error(error));
        }
        Ok(())
    }

    fn ensure_usable(&self) -> Result<(), SourceRootError> {
        if self.currentness.update_outcome_unknown() {
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
        let state = self
            .currentness
            .state(index)
            .ok_or(SourceRootError::CatalogCorrupt)?;
        Ok(SourceRootView {
            index,
            path: path_text(&entry.configured_path)?.to_owned(),
            state,
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
        self.currentness
            .note_watcher_hint(position, kind)
            .map_err(map_currentness_error)
    }

    #[allow(dead_code)]
    pub(crate) fn drain_watcher_hints(&mut self) -> Vec<WatcherHint> {
        self.currentness.drain_watcher_hints()
    }

    #[allow(dead_code)]
    pub(crate) const fn watcher_overflowed(&self) -> bool {
        self.currentness.watcher_overflowed()
    }

    /// Returns explicit bounded gaps blocking currentness.
    pub(crate) fn observation_gaps(&self) -> Vec<ObservationGap> {
        self.currentness.observation_gaps()
    }

    pub(crate) fn reconciliation_cursor(&self) -> ReconciliationCursor {
        self.currentness.reconciliation_cursor()
    }

    /// Computes source/workspace truth without index truth.
    pub(crate) fn current_workspace_truth(&self) -> CurrentWorkspaceTruth {
        self.currentness.current_workspace_truth()
    }

    /// Marks the current reconciliation generation as sync-proven.
    pub(crate) fn mark_reconciled_synced(&mut self) -> bool {
        self.currentness.mark_reconciled_synced()
    }
}

fn map_currentness_error(error: SourceRootCurrentnessError) -> SourceRootError {
    match error {
        SourceRootCurrentnessError::RootLimitExceeded => {
            SourceRootError::RootLimitExceeded
        }
        SourceRootCurrentnessError::PositionOutOfRange => {
            SourceRootError::CatalogCorrupt
        }
        SourceRootCurrentnessError::UpdateOutcomeUnknown => {
            SourceRootError::UpdateOutcomeUnknown
        }
    }
}

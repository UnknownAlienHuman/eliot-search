use super::*;

use std::fs;
use std::path::PathBuf;
use std::sync::atomic::{AtomicU64, Ordering};
use std::time::{SystemTime, UNIX_EPOCH};

struct Sandbox(PathBuf);

impl Sandbox {
    fn new() -> Self {
        static SEQUENCE: AtomicU64 = AtomicU64::new(0);
        let stamp = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_nanos();
        let path = std::env::temp_dir().join(format!(
            "eliot-roots-{}-{stamp}-{}",
            std::process::id(),
            SEQUENCE.fetch_add(1, Ordering::Relaxed)
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
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.0);
    }
}

#[test]
fn persists_reloads_adds_and_removes() {
    let sandbox = Sandbox::new();
    let first = sandbox.directory("first");
    let second = sandbox.directory("second");
    let config = sandbox.0.join("state/source-roots.txt");
    let mut catalog =
        SourceRootCatalog::load(config.clone(), std::slice::from_ref(&first)).unwrap();
    assert_eq!(catalog.available_count(), 1);
    catalog.add(&second).unwrap();
    assert_eq!(
        SourceRootCatalog::load(config.clone(), &[])
            .unwrap()
            .configured_count(),
        2
    );
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
    assert_eq!(
        SourceRootCatalog::load(config, &[])
            .unwrap()
            .configured_count(),
        0
    );
}

#[test]
fn refresh_reports_swapped_availability_with_unchanged_count() {
    let sandbox = Sandbox::new();
    let first = sandbox.directory("first");
    let second = sandbox.directory("second");
    let mut catalog = SourceRootCatalog::load(
        sandbox.0.join("roots.txt"),
        &[first.clone(), second.clone()],
    )
    .unwrap();
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
        assert!(matches!(
            catalog.add(path),
            Err(SourceRootError::DataRootOverlap)
        ));
    }
    catalog.add(&sibling).unwrap();
    assert_eq!(
        SourceRootCatalog::load_owned(&data)
            .unwrap()
            .configured_count(),
        1
    );
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
    let mut catalog = SourceRootCatalog::load(
        sandbox.0.join("roots.v1"),
        std::slice::from_ref(&source),
    )
    .unwrap();
    fs::remove_dir(&source).unwrap();
    fs::write(&source, b"not a directory").unwrap();
    assert!(catalog.refresh());
    assert_eq!(
        catalog.views().unwrap()[0].state,
        SourceRootState::NotDirectory
    );
    catalog.remove(&source).unwrap();
}

#[test]
fn nested_sources_are_rejected_and_duplicates_are_idempotent() {
    let sandbox = Sandbox::new();
    let source = sandbox.directory("source");
    let nested = source.join("nested");
    fs::create_dir(&nested).unwrap();
    let mut catalog = SourceRootCatalog::load(
        sandbox.0.join("roots.v1"),
        std::slice::from_ref(&source),
    )
    .unwrap();
    catalog.add(&source).unwrap();
    assert_eq!(catalog.configured_count(), 1);
    assert!(matches!(
        catalog.add(&nested),
        Err(SourceRootError::RootOverlap)
    ));
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
    let mut catalog = SourceRootCatalog::load(
        sandbox.0.join("roots.v1"),
        std::slice::from_ref(&source),
    )
    .unwrap();
    assert_eq!(WatcherHintKind::Modified.as_str(), "modified");
    assert_eq!(WatcherHintKind::Created.as_str(), "created");
    assert_eq!(WatcherHintKind::Removed.as_str(), "removed");
    assert_eq!(WatcherHintKind::Rescan.as_str(), "rescan");
    assert_eq!(WatcherHintKind::Overflow.as_str(), "overflow");
    assert!(!catalog.watcher_overflowed());
    assert_eq!(catalog.observation_gaps().len(), 0);
    assert!(!catalog
        .note_watcher_hint(0, WatcherHintKind::Modified)
        .unwrap());
    assert!(!catalog
        .note_watcher_hint(0, WatcherHintKind::Created)
        .unwrap());
    assert!(!catalog
        .note_watcher_hint(0, WatcherHintKind::Removed)
        .unwrap());
    assert!(!catalog
        .note_watcher_hint(0, WatcherHintKind::Rescan)
        .unwrap());
    assert_eq!(catalog.available_count(), 1);
    assert_eq!(catalog.observation_gaps().len(), 0);
    assert_eq!(catalog.drain_watcher_hints().len(), 4);
    assert!(matches!(
        catalog.note_watcher_hint(7, WatcherHintKind::Removed),
        Err(SourceRootError::RootNotFound)
    ));
    for _ in 0..(MAX_WATCHER_HINTS + 4) {
        let _ = catalog.note_watcher_hint(0, WatcherHintKind::Modified);
    }
    assert!(catalog.watcher_overflowed());
    assert!(catalog
        .observation_gaps()
        .iter()
        .any(|gap| gap.reason == ObservationGapReason::WatcherOverflow));
    assert!(!catalog.current_workspace_truth().source_current);
    assert!(catalog.refresh());
    assert!(!catalog.watcher_overflowed());
    assert_eq!(catalog.observation_gaps().len(), 0);
}

#[test]
fn missing_and_replaced_roots_are_gaps_and_block_sync_proof() {
    let sandbox = Sandbox::new();
    let source = sandbox.directory("source");
    let mut catalog = SourceRootCatalog::load(
        sandbox.0.join("roots.v1"),
        std::slice::from_ref(&source),
    )
    .unwrap();
    assert!(!catalog.current_workspace_truth().workspace_current);
    assert!(catalog.mark_reconciled_synced());
    assert!(catalog.current_workspace_truth().workspace_current);
    fs::remove_dir(&source).unwrap();
    assert!(catalog.refresh());
    let gaps = catalog.observation_gaps();
    assert_eq!(gaps.len(), 1);
    assert_eq!(gaps[0].reason, ObservationGapReason::Missing);
    assert_eq!(gaps[0].reason.code(), "OBSERVATION_GAP_MISSING");
    assert!(!catalog.mark_reconciled_synced());
    assert!(!catalog.current_workspace_truth().workspace_current);
    assert!(!catalog.current_workspace_truth().source_current);
    fs::write(&source, b"not a directory").unwrap();
    assert!(catalog.refresh());
    let gaps = catalog.observation_gaps();
    assert_eq!(gaps.len(), 1);
    assert_eq!(gaps[0].reason, ObservationGapReason::NotDirectory);
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
    let third = sandbox.directory("third");
    catalog.add(&third).unwrap();
    assert!(!catalog.current_workspace_truth().workspace_current);
    assert!(catalog.mark_reconciled_synced());
    assert!(catalog.current_workspace_truth().workspace_current);
}

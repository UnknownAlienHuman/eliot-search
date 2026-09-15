use std::fs::{self, File};
use std::io::{self, ErrorKind, Read};
use std::path::{Path, PathBuf};

use super::fingerprint::{count_lines, fingerprint};
use super::manifest::publish_manifest;
use super::model::{
    CaptureBatch, FileStamp, SnapshotEntry, SnapshotIndex, SnapshotLimits,
    SnapshotStats, StableRead, StableReadFailure,
};
use super::policy::{
    is_link_or_reparse, is_textual_utf8, policy_denies_file,
    should_skip_directory,
};
use super::spec::FINGERPRINT_ALGORITHM;
use super::storage::store_revision;

impl SnapshotIndex {
    /// Captures a complete bounded snapshot and publishes its immutable manifest.
    pub(crate) fn capture(
        data_root: &Path,
        source_roots: &[PathBuf],
        limits: SnapshotLimits,
    ) -> io::Result<Self> {
        let limits = limits.validate()?;
        fs::create_dir_all(data_root)?;
        let data_root = fs::canonicalize(data_root)?;
        let revisions_root = data_root.join("revisions").join(FINGERPRINT_ALGORITHM);
        let manifests_root = data_root.join("snapshots");
        fs::create_dir_all(&revisions_root)?;
        fs::create_dir_all(&manifests_root)?;

        let mut entries = Vec::new();
        let mut stats = SnapshotStats::default();
        let mut stack = source_roots
            .iter()
            .enumerate()
            .rev()
            .map(|(root_index, root)| (root_index, root.clone()))
            .collect::<Vec<_>>();

        while let Some((root_index, path)) = stack.pop() {
            if entries.len() >= limits.files || stats.total_bytes >= limits.total_bytes {
                stats.truncated = true;
                break;
            }
            if path.starts_with(&data_root) {
                stats.skipped_policy = stats.skipped_policy.saturating_add(1);
                continue;
            }

            let Ok(metadata) = fs::symlink_metadata(&path) else {
                stats.unreadable_files = stats.unreadable_files.saturating_add(1);
                continue;
            };
            if is_link_or_reparse(&metadata) {
                stats.skipped_links = stats.skipped_links.saturating_add(1);
                continue;
            }
            if metadata.is_dir() {
                if should_skip_directory(&path, &source_roots[root_index]) {
                    stats.skipped_policy = stats.skipped_policy.saturating_add(1);
                    continue;
                }
                let Ok(children) = fs::read_dir(&path) else {
                    stats.unreadable_files = stats.unreadable_files.saturating_add(1);
                    continue;
                };
                let mut children = children
                    .filter_map(Result::ok)
                    .map(|entry| entry.path())
                    .collect::<Vec<_>>();
                children.sort();
                for child in children.into_iter().rev() {
                    stack.push((root_index, child));
                }
                continue;
            }
            if !metadata.is_file() {
                stats.skipped_policy = stats.skipped_policy.saturating_add(1);
                continue;
            }
            if policy_denies_file(&path) || metadata.len() > limits.file_bytes {
                stats.skipped_policy = stats.skipped_policy.saturating_add(1);
                continue;
            }
            if !Self::capture_one_file(
                root_index,
                &source_roots[root_index],
                &path,
                &data_root,
                &revisions_root,
                &limits,
                &mut CaptureBatch {
                    entries: &mut entries,
                    stats: &mut stats,
                },
            )? {
                break;
            }
        }

        let (snapshot_id, manifest_fingerprint, manifest_path) =
            publish_manifest(source_roots.len(), &mut entries, &stats, &manifests_root)?;

        Ok(Self {
            snapshot_id,
            manifest_fingerprint,
            manifest_path,
            entries,
            stats,
            limits,
        })
    }

    fn capture_one_file(
        root_index: usize,
        root: &Path,
        path: &Path,
        data_root: &Path,
        revisions_root: &Path,
        limits: &SnapshotLimits,
        batch: &mut CaptureBatch<'_>,
    ) -> io::Result<bool> {
        let Some(read) =
            Self::read_candidate_file(root, data_root, path, limits.file_bytes, batch.stats)
        else {
            return Ok(true);
        };
        Self::store_candidate(root_index, read, revisions_root, limits, batch)
    }

    fn read_candidate_file(
        root: &Path,
        data_root: &Path,
        path: &Path,
        file_bytes: u64,
        stats: &mut SnapshotStats,
    ) -> Option<StableRead> {
        match stable_read(root, data_root, path, file_bytes) {
            Ok(read) => Some(read),
            Err(StableReadFailure::LinkOrEscape) => {
                stats.skipped_links = stats.skipped_links.saturating_add(1);
                None
            }
            Err(StableReadFailure::Binary) => {
                stats.skipped_binary = stats.skipped_binary.saturating_add(1);
                None
            }
            Err(StableReadFailure::Unstable) => {
                stats.unstable_files = stats.unstable_files.saturating_add(1);
                None
            }
            Err(StableReadFailure::Unreadable) => {
                stats.unreadable_files = stats.unreadable_files.saturating_add(1);
                None
            }
        }
    }

    fn store_candidate(
        root_index: usize,
        read: StableRead,
        revisions_root: &Path,
        limits: &SnapshotLimits,
        batch: &mut CaptureBatch<'_>,
    ) -> io::Result<bool> {
        let stats = &mut *batch.stats;
        let entries = &mut *batch.entries;
        let next_total = stats
            .total_bytes
            .checked_add(u64::try_from(read.bytes.len()).map_err(|_| {
                io::Error::new(
                    ErrorKind::InvalidData,
                    "snapshot byte accounting overflow",
                )
            })?)
            .ok_or_else(|| {
                io::Error::new(
                    ErrorKind::InvalidData,
                    "snapshot byte accounting overflow",
                )
            })?;
        if next_total > limits.total_bytes {
            stats.truncated = true;
            return Ok(false);
        }

        let revision_fingerprint = fingerprint(&read.bytes);
        let (revision_path, reused) = store_revision(
            revisions_root,
            revision_fingerprint,
            &read.bytes,
            limits.file_bytes,
        )?;
        if reused {
            stats.reused_revisions = stats.reused_revisions.saturating_add(1);
        } else {
            stats.written_revisions = stats.written_revisions.saturating_add(1);
        }
        stats.total_bytes = next_total;
        stats.indexed_files = stats.indexed_files.saturating_add(1);
        entries.push(SnapshotEntry {
            root_index,
            relative_path: read.relative_path,
            revision_fingerprint,
            revision_path,
            byte_length: u64::try_from(read.bytes.len()).map_err(|_| {
                io::Error::new(ErrorKind::InvalidData, "revision length overflow")
            })?,
            line_count: count_lines(&read.bytes),
        });
        Ok(true)
    }
}

fn stable_read(
    root: &Path,
    data_root: &Path,
    path: &Path,
    max_bytes: u64,
) -> Result<StableRead, StableReadFailure> {
    let canonical = fs::canonicalize(path).map_err(|_| StableReadFailure::Unreadable)?;
    if !canonical.starts_with(root) || canonical.starts_with(data_root) {
        return Err(StableReadFailure::LinkOrEscape);
    }
    let final_metadata =
        fs::symlink_metadata(&canonical).map_err(|_| StableReadFailure::Unreadable)?;
    if is_link_or_reparse(&final_metadata) || !final_metadata.is_file() {
        return Err(StableReadFailure::LinkOrEscape);
    }
    let mut file = File::open(&canonical).map_err(|_| StableReadFailure::Unreadable)?;
    let before = FileStamp::observe(&file)?;
    if before.length() > max_bytes {
        return Err(StableReadFailure::Unreadable);
    }
    let mut bytes = Vec::with_capacity(
        usize::try_from(before.length()).map_err(|_| StableReadFailure::Unreadable)?,
    );
    (&mut file)
        .take(max_bytes.saturating_add(1))
        .read_to_end(&mut bytes)
        .map_err(|_| StableReadFailure::Unreadable)?;
    if u64::try_from(bytes.len()).unwrap_or(u64::MAX) > max_bytes {
        return Err(StableReadFailure::Unreadable);
    }
    let after = FileStamp::observe(&file)?;
    if before != after || u64::try_from(bytes.len()).ok() != Some(before.length()) {
        return Err(StableReadFailure::Unstable);
    }
    if !is_textual_utf8(&bytes) {
        return Err(StableReadFailure::Binary);
    }
    let relative = canonical
        .strip_prefix(root)
        .map_err(|_| StableReadFailure::LinkOrEscape)?;
    let relative_path = relative
        .to_str()
        .ok_or(StableReadFailure::Unreadable)?
        .replace('\\', "/");
    Ok(StableRead {
        bytes,
        relative_path,
    })
}

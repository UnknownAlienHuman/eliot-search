use std::fs::File;
use std::io::{self, ErrorKind};
use std::path::{Path, PathBuf};

use super::policy::{is_link_or_reparse, system_time_nanos};
use super::spec::FINGERPRINT_ALGORITHM;

/// Finite capture and query limits.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct SnapshotLimits {
    pub(crate) files: usize,
    pub(crate) file_bytes: u64,
    pub(crate) total_bytes: u64,
    pub(crate) results: usize,
    pub(crate) excerpt_chars: usize,
}

impl SnapshotLimits {
    pub(crate) fn validate(self) -> io::Result<Self> {
        if self.files == 0
            || self.file_bytes == 0
            || self.total_bytes == 0
            || self.results == 0
            || self.excerpt_chars == 0
            || self.file_bytes > self.total_bytes
        {
            return Err(io::Error::new(
                ErrorKind::InvalidInput,
                "invalid snapshot limits",
            ));
        }
        Ok(self)
    }
}

/// Content-free capture accounting.
#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub struct SnapshotStats {
    pub(crate) indexed_files: usize,
    pub(crate) total_bytes: u64,
    pub(crate) written_revisions: usize,
    pub(crate) reused_revisions: usize,
    pub(crate) skipped_links: usize,
    pub(crate) skipped_policy: usize,
    pub(crate) skipped_binary: usize,
    pub(crate) unreadable_files: usize,
    pub(crate) unstable_files: usize,
    pub(crate) truncated: bool,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub(super) struct SnapshotEntry {
    pub(super) root_index: usize,
    pub(super) relative_path: String,
    pub(super) revision_fingerprint: [u8; 32],
    pub(super) revision_path: PathBuf,
    pub(super) byte_length: u64,
    pub(super) line_count: usize,
}

/// Frozen retained-revision snapshot used by the legacy harness hot path.
#[derive(Clone, Debug)]
pub struct SnapshotIndex {
    pub(super) snapshot_id: String,
    pub(super) manifest_fingerprint: [u8; 32],
    pub(super) manifest_path: PathBuf,
    pub(super) entries: Vec<SnapshotEntry>,
    pub(super) stats: SnapshotStats,
    pub(super) limits: SnapshotLimits,
}

/// Mutable entry batch shared by one snapshot-capture walk.
pub(super) struct CaptureBatch<'a> {
    pub(super) entries: &'a mut Vec<SnapshotEntry>,
    pub(super) stats: &'a mut SnapshotStats,
}

impl SnapshotIndex {
    pub(crate) fn snapshot_id(&self) -> &str {
        &self.snapshot_id
    }

    pub(crate) const fn manifest_fingerprint(&self) -> [u8; 32] {
        self.manifest_fingerprint
    }

    pub(crate) fn manifest_path(&self) -> &Path {
        &self.manifest_path
    }

    pub(crate) const fn stats(&self) -> &SnapshotStats {
        &self.stats
    }

    pub(crate) const fn fingerprint_algorithm() -> &'static str {
        FINGERPRINT_ALGORITHM
    }
}

/// One source-backed result from an exact retained revision.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct SnapshotMatch {
    pub(crate) root_index: usize,
    pub(crate) relative_path: String,
    pub(crate) revision_fingerprint: [u8; 32],
    pub(crate) line: usize,
    pub(crate) column_bytes: usize,
    pub(crate) byte_start: usize,
    pub(crate) byte_end: usize,
    pub(crate) excerpt: String,
}

/// Search result with explicit frozen denominator and gaps.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct SnapshotSearchResult {
    pub(crate) snapshot_id: String,
    pub(crate) manifest_fingerprint: [u8; 32],
    pub(crate) fingerprint_algorithm: &'static str,
    pub(crate) matches: Vec<SnapshotMatch>,
    pub(crate) scanned_revisions: usize,
    pub(crate) unavailable_revisions: usize,
    pub(crate) denominator_files: usize,
    pub(crate) complete: bool,
    pub(crate) truncated: bool,
}

#[derive(Debug)]
pub(super) struct StableRead {
    pub(super) bytes: Vec<u8>,
    pub(super) relative_path: String,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(super) enum StableReadFailure {
    LinkOrEscape,
    Binary,
    Unstable,
    Unreadable,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub(super) struct FileStamp {
    length: u64,
    modified_nanos: Option<u128>,
    created_nanos: Option<u128>,
    readonly: bool,
}

impl FileStamp {
    pub(super) fn observe(file: &File) -> Result<Self, StableReadFailure> {
        let metadata = file
            .metadata()
            .map_err(|_| StableReadFailure::Unreadable)?;
        if !metadata.is_file() || is_link_or_reparse(&metadata) {
            return Err(StableReadFailure::LinkOrEscape);
        }
        Ok(Self {
            length: metadata.len(),
            modified_nanos: system_time_nanos(metadata.modified().ok()),
            created_nanos: system_time_nanos(metadata.created().ok()),
            readonly: metadata.permissions().readonly(),
        })
    }

    pub(super) const fn length(&self) -> u64 {
        self.length
    }
}

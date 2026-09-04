//! Immutable development snapshots for the local DIRECT path.
//!
//! The snapshotter reads each candidate through one open file handle, verifies
//! stable metadata before and after the read, stores exact UTF-8 bytes under a
//! content fingerprint, writes a frozen manifest, and serves queries only from
//! those retained revision objects. The current fingerprint is collision-checked
//! but is not advertised as a cryptographic digest.

use std::collections::BTreeSet;
use std::fs::{self, File, OpenOptions};
use std::io::{self, ErrorKind, Read, Write};
use std::path::{Component, Path, PathBuf};
use std::sync::atomic::{AtomicU64, Ordering};
use std::time::{SystemTime, UNIX_EPOCH};

const FINGERPRINT_ALGORITHM: &str = "eliot-fnv4-v1";
const MAX_MANIFEST_BYTES: usize = 64 * 1024 * 1024;
static SNAPSHOT_SEQUENCE: AtomicU64 = AtomicU64::new(1);
static TEMP_SEQUENCE: AtomicU64 = AtomicU64::new(1);

/// Finite capture and query limits.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) struct SnapshotLimits {
    pub(crate) max_files: usize,
    pub(crate) max_file_bytes: u64,
    pub(crate) max_total_bytes: u64,
    pub(crate) max_results: usize,
    pub(crate) max_excerpt_chars: usize,
}

impl SnapshotLimits {
    pub(crate) fn validate(self) -> io::Result<Self> {
        if self.max_files == 0
            || self.max_file_bytes == 0
            || self.max_total_bytes == 0
            || self.max_results == 0
            || self.max_excerpt_chars == 0
            || self.max_file_bytes > self.max_total_bytes
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
pub(crate) struct SnapshotStats {
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
struct SnapshotEntry {
    root_index: usize,
    relative_path: String,
    revision_fingerprint: [u8; 32],
    revision_path: PathBuf,
    byte_length: u64,
    line_count: usize,
}

/// Frozen retained-revision snapshot used by the daemon hot path.
#[derive(Clone, Debug)]
pub(crate) struct SnapshotIndex {
    snapshot_id: String,
    manifest_fingerprint: [u8; 32],
    manifest_path: PathBuf,
    entries: Vec<SnapshotEntry>,
    stats: SnapshotStats,
    limits: SnapshotLimits,
}

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
            if entries.len() >= limits.max_files || stats.total_bytes >= limits.max_total_bytes {
                stats.truncated = true;
                break;
            }
            if path.starts_with(&data_root) {
                stats.skipped_policy = stats.skipped_policy.saturating_add(1);
                continue;
            }

            let metadata = match fs::symlink_metadata(&path) {
                Ok(metadata) => metadata,
                Err(_) => {
                    stats.unreadable_files = stats.unreadable_files.saturating_add(1);
                    continue;
                }
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
                let mut children = match fs::read_dir(&path) {
                    Ok(children) => children
                        .filter_map(Result::ok)
                        .map(|entry| entry.path())
                        .collect::<Vec<_>>(),
                    Err(_) => {
                        stats.unreadable_files = stats.unreadable_files.saturating_add(1);
                        continue;
                    }
                };
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
            if policy_denies_file(&path) {
                stats.skipped_policy = stats.skipped_policy.saturating_add(1);
                continue;
            }
            if metadata.len() > limits.max_file_bytes {
                stats.skipped_policy = stats.skipped_policy.saturating_add(1);
                continue;
            }

            let root = &source_roots[root_index];
            let read = match stable_read(root, &data_root, &path, limits.max_file_bytes) {
                Ok(read) => read,
                Err(StableReadFailure::LinkOrEscape) => {
                    stats.skipped_links = stats.skipped_links.saturating_add(1);
                    continue;
                }
                Err(StableReadFailure::Binary) => {
                    stats.skipped_binary = stats.skipped_binary.saturating_add(1);
                    continue;
                }
                Err(StableReadFailure::Unstable) => {
                    stats.unstable_files = stats.unstable_files.saturating_add(1);
                    continue;
                }
                Err(StableReadFailure::Unreadable) => {
                    stats.unreadable_files = stats.unreadable_files.saturating_add(1);
                    continue;
                }
            };
            let next_total = stats
                .total_bytes
                .checked_add(u64::try_from(read.bytes.len()).map_err(|_| {
                    io::Error::new(ErrorKind::InvalidData, "snapshot byte accounting overflow")
                })?)
                .ok_or_else(|| {
                    io::Error::new(ErrorKind::InvalidData, "snapshot byte accounting overflow")
                })?;
            if next_total > limits.max_total_bytes {
                stats.truncated = true;
                break;
            }

            let revision_fingerprint = fingerprint(&read.bytes);
            let (revision_path, reused) = store_revision(
                &revisions_root,
                revision_fingerprint,
                &read.bytes,
                limits.max_file_bytes,
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
        }

        entries.sort_by(|left, right| {
            left.root_index
                .cmp(&right.root_index)
                .then_with(|| left.relative_path.cmp(&right.relative_path))
                .then_with(|| left.revision_fingerprint.cmp(&right.revision_fingerprint))
        });
        let snapshot_id = new_snapshot_id()?;
        let manifest = render_manifest(&snapshot_id, source_roots.len(), &entries, &stats)?;
        if manifest.len() > MAX_MANIFEST_BYTES {
            return Err(io::Error::new(
                ErrorKind::InvalidData,
                "snapshot manifest exceeds its finite ceiling",
            ));
        }
        let manifest_fingerprint = fingerprint(&manifest);
        let manifest_path = manifests_root.join(format!("{snapshot_id}.manifest"));
        write_unique_verified(
            &manifest_path,
            &manifest,
            u64::try_from(MAX_MANIFEST_BYTES).unwrap_or(u64::MAX),
        )?;

        Ok(Self {
            snapshot_id,
            manifest_fingerprint,
            manifest_path,
            entries,
            stats,
            limits,
        })
    }

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

    pub(crate) const fn fingerprint_algorithm(&self) -> &'static str {
        FINGERPRINT_ALGORITHM
    }

    /// Reopens exact retained revisions and performs a bounded DIRECT search.
    pub(crate) fn search(&self, query: &str) -> io::Result<SnapshotSearchResult> {
        if query.is_empty() || query.len() > 1_024 {
            return Err(io::Error::new(
                ErrorKind::InvalidInput,
                "query must be non-empty and at most 1024 UTF-8 bytes",
            ));
        }
        let mut matches = Vec::new();
        let mut scanned_revisions = 0_usize;
        let mut unavailable_revisions = 0_usize;
        let mut truncated = false;

        for entry in &self.entries {
            if matches.len() >= self.limits.max_results {
                truncated = true;
                break;
            }
            let bytes = match read_verified_revision(
                &entry.revision_path,
                entry.revision_fingerprint,
                entry.byte_length,
                self.limits.max_file_bytes,
            ) {
                Ok(bytes) => bytes,
                Err(_) => {
                    unavailable_revisions = unavailable_revisions.saturating_add(1);
                    continue;
                }
            };
            scanned_revisions = scanned_revisions.saturating_add(1);
            let text = String::from_utf8(bytes).map_err(|_| {
                io::Error::new(ErrorKind::InvalidData, "retained revision is not UTF-8")
            })?;
            for line in lines_with_offsets(&text) {
                let Some(column) = find_query(line.text, query) else {
                    continue;
                };
                let byte_start = line
                    .byte_start
                    .checked_add(column)
                    .ok_or_else(|| io::Error::new(ErrorKind::InvalidData, "offset overflow"))?;
                let byte_end = byte_start
                    .checked_add(query.len())
                    .ok_or_else(|| io::Error::new(ErrorKind::InvalidData, "offset overflow"))?;
                matches.push(SnapshotMatch {
                    root_index: entry.root_index,
                    relative_path: entry.relative_path.clone(),
                    revision_fingerprint: entry.revision_fingerprint,
                    line: line.number.saturating_add(1),
                    column_bytes: column,
                    byte_start,
                    byte_end,
                    excerpt: truncate_chars(line.text.trim(), self.limits.max_excerpt_chars),
                });
                if matches.len() >= self.limits.max_results {
                    truncated = true;
                    break;
                }
            }
        }

        Ok(SnapshotSearchResult {
            snapshot_id: self.snapshot_id.clone(),
            manifest_fingerprint: self.manifest_fingerprint,
            fingerprint_algorithm: FINGERPRINT_ALGORITHM,
            matches,
            scanned_revisions,
            unavailable_revisions,
            denominator_files: self.entries.len(),
            complete: !truncated && !self.stats.truncated && unavailable_revisions == 0,
            truncated,
        })
    }
}

/// One source-backed result from an exact retained revision.
#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) struct SnapshotMatch {
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
pub(crate) struct SnapshotSearchResult {
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
struct StableRead {
    bytes: Vec<u8>,
    relative_path: String,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum StableReadFailure {
    LinkOrEscape,
    Binary,
    Unstable,
    Unreadable,
}

#[derive(Clone, Debug, Eq, PartialEq)]
struct FileStamp {
    length: u64,
    modified_nanos: Option<u128>,
    created_nanos: Option<u128>,
    readonly: bool,
}

impl FileStamp {
    fn observe(file: &File) -> Result<Self, StableReadFailure> {
        let metadata = file.metadata().map_err(|_| StableReadFailure::Unreadable)?;
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
    let final_metadata = fs::symlink_metadata(&canonical)
        .map_err(|_| StableReadFailure::Unreadable)?;
    if is_link_or_reparse(&final_metadata) || !final_metadata.is_file() {
        return Err(StableReadFailure::LinkOrEscape);
    }
    let mut file = File::open(&canonical).map_err(|_| StableReadFailure::Unreadable)?;
    let before = FileStamp::observe(&file)?;
    if before.length > max_bytes {
        return Err(StableReadFailure::Unreadable);
    }
    let mut bytes = Vec::with_capacity(
        usize::try_from(before.length).map_err(|_| StableReadFailure::Unreadable)?,
    );
    (&mut file)
        .take(max_bytes.saturating_add(1))
        .read_to_end(&mut bytes)
        .map_err(|_| StableReadFailure::Unreadable)?;
    if u64::try_from(bytes.len()).unwrap_or(u64::MAX) > max_bytes {
        return Err(StableReadFailure::Unreadable);
    }
    let after = FileStamp::observe(&file)?;
    if before != after || u64::try_from(bytes.len()).ok() != Some(before.length) {
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

fn store_revision(
    revisions_root: &Path,
    revision_fingerprint: [u8; 32],
    bytes: &[u8],
    max_bytes: u64,
) -> io::Result<(PathBuf, bool)> {
    let digest = hex32(revision_fingerprint);
    let directory = revisions_root.join(&digest[..2]);
    fs::create_dir_all(&directory)?;
    let final_path = directory.join(format!("{digest}.utf8"));
    if final_path.exists() {
        verify_exact_file(&final_path, bytes, revision_fingerprint, max_bytes)?;
        return Ok((final_path, true));
    }

    let sequence = TEMP_SEQUENCE.fetch_add(1, Ordering::Relaxed);
    let temporary = directory.join(format!(
        ".{digest}.{}.{}.tmp",
        std::process::id(),
        sequence
    ));
    {
        let mut file = OpenOptions::new()
            .write(true)
            .create_new(true)
            .open(&temporary)?;
        file.write_all(bytes)?;
        file.sync_all()?;
    }
    match fs::rename(&temporary, &final_path) {
        Ok(()) => {}
        Err(error) if final_path.exists() => {
            let _ = fs::remove_file(&temporary);
            verify_exact_file(&final_path, bytes, revision_fingerprint, max_bytes)?;
            return Ok((final_path, true));
        }
        Err(error) => {
            let _ = fs::remove_file(&temporary);
            return Err(error);
        }
    }
    sync_directory(&directory)?;
    verify_exact_file(&final_path, bytes, revision_fingerprint, max_bytes)?;
    Ok((final_path, false))
}

fn verify_exact_file(
    path: &Path,
    expected_bytes: &[u8],
    expected_fingerprint: [u8; 32],
    max_bytes: u64,
) -> io::Result<()> {
    let actual = read_bounded_file(path, max_bytes)?;
    if actual != expected_bytes || fingerprint(&actual) != expected_fingerprint {
        return Err(io::Error::new(
            ErrorKind::InvalidData,
            "revision fingerprint collision or durable readback mismatch",
        ));
    }
    Ok(())
}

fn read_verified_revision(
    path: &Path,
    expected_fingerprint: [u8; 32],
    expected_length: u64,
    max_bytes: u64,
) -> io::Result<Vec<u8>> {
    let bytes = read_bounded_file(path, max_bytes)?;
    if u64::try_from(bytes.len()).ok() != Some(expected_length)
        || fingerprint(&bytes) != expected_fingerprint
    {
        return Err(io::Error::new(
            ErrorKind::InvalidData,
            "retained revision readback mismatch",
        ));
    }
    Ok(bytes)
}

fn read_bounded_file(path: &Path, max_bytes: u64) -> io::Result<Vec<u8>> {
    let metadata = fs::symlink_metadata(path)?;
    if is_link_or_reparse(&metadata) || !metadata.is_file() || metadata.len() > max_bytes {
        return Err(io::Error::new(
            ErrorKind::InvalidData,
            "invalid retained revision object",
        ));
    }
    let mut file = File::open(path)?;
    let mut bytes = Vec::with_capacity(usize::try_from(metadata.len()).unwrap_or(0));
    (&mut file)
        .take(max_bytes.saturating_add(1))
        .read_to_end(&mut bytes)?;
    if u64::try_from(bytes.len()).unwrap_or(u64::MAX) > max_bytes {
        return Err(io::Error::new(
            ErrorKind::InvalidData,
            "retained revision exceeds its finite ceiling",
        ));
    }
    Ok(bytes)
}

fn write_unique_verified(path: &Path, bytes: &[u8], max_bytes: u64) -> io::Result<()> {
    let mut file = OpenOptions::new()
        .write(true)
        .create_new(true)
        .open(path)?;
    file.write_all(bytes)?;
    file.sync_all()?;
    let parent = path.parent().ok_or_else(|| {
        io::Error::new(ErrorKind::InvalidInput, "manifest has no parent directory")
    })?;
    sync_directory(parent)?;
    let readback = read_bounded_file(path, max_bytes)?;
    if readback != bytes {
        return Err(io::Error::new(
            ErrorKind::InvalidData,
            "manifest durable readback mismatch",
        ));
    }
    Ok(())
}

fn render_manifest(
    snapshot_id: &str,
    root_count: usize,
    entries: &[SnapshotEntry],
    stats: &SnapshotStats,
) -> io::Result<Vec<u8>> {
    let mut output = String::new();
    output.push_str("ELIOT_SEARCH_SNAPSHOT_V1\n");
    output.push_str(&format!("snapshot_id={snapshot_id}\n"));
    output.push_str(&format!("fingerprint_algorithm={FINGERPRINT_ALGORITHM}\n"));
    output.push_str(&format!("source_roots={root_count}\n"));
    output.push_str(&format!("entries={}\n", entries.len()));
    output.push_str(&format!("total_bytes={}\n", stats.total_bytes));
    output.push_str(&format!("capture_truncated={}\n", stats.truncated));
    output.push_str(&format!("skipped_links={}\n", stats.skipped_links));
    output.push_str(&format!("skipped_policy={}\n", stats.skipped_policy));
    output.push_str(&format!("skipped_binary={}\n", stats.skipped_binary));
    output.push_str(&format!("unreadable_files={}\n", stats.unreadable_files));
    output.push_str(&format!("unstable_files={}\n", stats.unstable_files));
    output.push_str("--\n");
    for entry in entries {
        let path_hex = hex_bytes(entry.relative_path.as_bytes());
        output.push_str(&format!(
            "{}\t{}\t{}\t{}\t{}\n",
            entry.root_index,
            path_hex,
            hex32(entry.revision_fingerprint),
            entry.byte_length,
            entry.line_count,
        ));
        if output.len() > MAX_MANIFEST_BYTES {
            return Err(io::Error::new(
                ErrorKind::InvalidData,
                "snapshot manifest exceeds its finite ceiling",
            ));
        }
    }
    Ok(output.into_bytes())
}

fn count_lines(bytes: &[u8]) -> usize {
    if bytes.is_empty() {
        0
    } else {
        bytes
            .iter()
            .filter(|byte| **byte == b'\n')
            .count()
            .saturating_add(1)
    }
}

#[derive(Clone, Copy)]
struct LineRef<'a> {
    number: usize,
    byte_start: usize,
    text: &'a str,
}

fn lines_with_offsets(text: &str) -> Vec<LineRef<'_>> {
    if text.is_empty() {
        return Vec::new();
    }
    let bytes = text.as_bytes();
    let mut output = Vec::new();
    let mut start = 0_usize;
    let mut number = 0_usize;
    for (index, byte) in bytes.iter().copied().enumerate() {
        if byte != b'\n' {
            continue;
        }
        let mut end = index;
        if end > start && bytes[end - 1] == b'\r' {
            end -= 1;
        }
        output.push(LineRef {
            number,
            byte_start: start,
            text: &text[start..end],
        });
        start = index.saturating_add(1);
        number = number.saturating_add(1);
    }
    if start < text.len() {
        output.push(LineRef {
            number,
            byte_start: start,
            text: &text[start..],
        });
    }
    output
}

fn find_query(line: &str, query: &str) -> Option<usize> {
    if let Some(offset) = line.find(query) {
        return Some(offset);
    }
    if !query.is_ascii() {
        return None;
    }
    let query = query.as_bytes();
    if query.len() > line.len() {
        return None;
    }
    line.as_bytes()
        .windows(query.len())
        .position(|window| window.eq_ignore_ascii_case(query))
}

fn truncate_chars(value: &str, limit: usize) -> String {
    let mut output = value.chars().take(limit).collect::<String>();
    if value.chars().count() > limit {
        output.push('…');
    }
    output
}

fn is_textual_utf8(bytes: &[u8]) -> bool {
    if bytes.contains(&0) || std::str::from_utf8(bytes).is_err() {
        return false;
    }
    let sample = bytes.iter().take(32 * 1024);
    let mut controls = 0_usize;
    let mut observed = 0_usize;
    for byte in sample {
        observed = observed.saturating_add(1);
        if *byte < 0x20 && !matches!(*byte, b'\t' | b'\n' | b'\r' | 0x0c) {
            controls = controls.saturating_add(1);
        }
    }
    controls <= observed.div_ceil(100).max(4)
}

fn should_skip_directory(path: &Path, root: &Path) -> bool {
    if path == root {
        return false;
    }
    let Some(name) = path.file_name().and_then(|name| name.to_str()) else {
        return true;
    };
    matches!(
        name.to_ascii_lowercase().as_str(),
        ".git"
            | ".hg"
            | ".svn"
            | ".eliot-search"
            | ".cache"
            | ".tox"
            | ".venv"
            | "venv"
            | "node_modules"
            | "target"
            | "dist"
            | "build"
            | "vendor"
            | "__pycache__"
            | ".ssh"
            | ".gnupg"
    )
}

fn policy_denies_file(path: &Path) -> bool {
    let Some(name) = path.file_name().and_then(|name| name.to_str()) else {
        return true;
    };
    let lower = name.to_ascii_lowercase();
    if lower == ".env"
        || lower.starts_with(".env.")
        || matches!(
            lower.as_str(),
            "id_rsa"
                | "id_dsa"
                | "id_ecdsa"
                | "id_ed25519"
                | "credentials"
                | "credentials.json"
                | "secrets.json"
                | "wallet.dat"
    )
    {
        return true;
    }
    let denied_extensions = [
        "exe", "dll", "so", "dylib", "bin", "obj", "o", "a", "lib", "class",
        "jar", "war", "zip", "7z", "rar", "gz", "bz2", "xz", "tar", "pdf",
        "png", "jpg", "jpeg", "gif", "webp", "bmp", "ico", "mp3", "wav", "flac",
        "mp4", "mkv", "mov", "avi", "db", "sqlite", "sqlite3", "mdb", "pdb",
        "key", "pem", "pfx", "p12", "jks", "keystore", "kdbx", "der", "crt",
    ];
    path.extension()
        .and_then(|extension| extension.to_str())
        .is_some_and(|extension| {
            denied_extensions.contains(&extension.to_ascii_lowercase().as_str())
        })
}

fn is_link_or_reparse(metadata: &fs::Metadata) -> bool {
    if metadata.file_type().is_symlink() {
        return true;
    }
    #[cfg(windows)]
    {
        use std::os::windows::fs::MetadataExt;
        const FILE_ATTRIBUTE_REPARSE_POINT: u32 = 0x0400;
        return metadata.file_attributes() & FILE_ATTRIBUTE_REPARSE_POINT != 0;
    }
    #[cfg(not(windows))]
    {
        false
    }
}

fn system_time_nanos(value: Option<SystemTime>) -> Option<u128> {
    value
        .and_then(|time| time.duration_since(UNIX_EPOCH).ok())
        .map(|duration| duration.as_nanos())
}

fn new_snapshot_id() -> io::Result<String> {
    let millis = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map_err(|_| io::Error::other("system clock precedes the Unix epoch"))?
        .as_millis();
    let sequence = SNAPSHOT_SEQUENCE.fetch_add(1, Ordering::Relaxed);
    Ok(format!(
        "{millis:032x}-{:08x}-{sequence:016x}",
        std::process::id()
    ))
}

/// Deterministic 256-bit development fingerprint with independent lanes.
pub(crate) fn fingerprint(bytes: &[u8]) -> [u8; 32] {
    let mut lanes = [
        0xcbf2_9ce4_8422_2325_u64,
        0x8422_2325_cbf2_9ce4,
        0x9e37_79b9_7f4a_7c15,
        0xc2b2_ae3d_27d4_eb4f,
    ];
    for (index, byte) in bytes.iter().copied().enumerate() {
        for (lane_index, lane) in lanes.iter_mut().enumerate() {
            let mixed = byte.wrapping_add(
                u8::try_from((index + lane_index * 29) & 0xff).unwrap_or(0),
            );
            *lane ^= u64::from(mixed);
            *lane = lane
                .wrapping_mul(0x0000_0100_0000_01b3_u64.wrapping_add(
                    u64::try_from(lane_index * 2).unwrap_or(0),
                ))
                .rotate_left(u32::try_from(11 + lane_index * 7).unwrap_or(11));
        }
    }
    let length = u64::try_from(bytes.len()).unwrap_or(u64::MAX);
    for (index, lane) in lanes.iter_mut().enumerate() {
        *lane ^= length.rotate_left(u32::try_from(index * 13).unwrap_or(0));
        *lane = avalanche(*lane);
    }
    let mut output = [0_u8; 32];
    for (index, lane) in lanes.into_iter().enumerate() {
        output[index * 8..index * 8 + 8].copy_from_slice(&lane.to_be_bytes());
    }
    output
}

fn avalanche(mut value: u64) -> u64 {
    value ^= value >> 33;
    value = value.wrapping_mul(0xff51_afd7_ed55_8ccd);
    value ^= value >> 33;
    value = value.wrapping_mul(0xc4ce_b9fe_1a85_ec53);
    value ^ (value >> 33)
}

pub(crate) fn hex32(bytes: [u8; 32]) -> String {
    hex_bytes(&bytes)
}

fn hex_bytes(bytes: &[u8]) -> String {
    const HEX: &[u8; 16] = b"0123456789abcdef";
    let mut output = String::with_capacity(bytes.len().saturating_mul(2));
    for byte in bytes {
        output.push(char::from(HEX[usize::from(byte >> 4)]));
        output.push(char::from(HEX[usize::from(byte & 0x0f)]));
    }
    output
}

#[cfg(unix)]
fn sync_directory(path: &Path) -> io::Result<()> {
    File::open(path)?.sync_all()
}

#[cfg(not(unix))]
fn sync_directory(_path: &Path) -> io::Result<()> {
    Ok(())
}

#[allow(dead_code)]
fn path_has_parent_escape(path: &Path) -> bool {
    path.components().any(|component| matches!(component, Component::ParentDir))
}

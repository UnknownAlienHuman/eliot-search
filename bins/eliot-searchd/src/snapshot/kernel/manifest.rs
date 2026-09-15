use std::fmt::Write as _;
use std::io::{self, ErrorKind};
use std::path::{Path, PathBuf};
use std::sync::atomic::Ordering;
use std::time::{SystemTime, UNIX_EPOCH};

use super::fingerprint::{fingerprint, hex32, hex_bytes};
use super::model::{SnapshotEntry, SnapshotStats};
use super::spec::{
    FINGERPRINT_ALGORITHM, MAX_MANIFEST_BYTES, SNAPSHOT_SEQUENCE,
};
use super::storage::write_unique_verified;

/// Sorts captured entries, renders the frozen manifest, and publishes it
/// with exact readback verification.
pub(super) fn publish_manifest(
    source_root_count: usize,
    entries: &mut [SnapshotEntry],
    stats: &SnapshotStats,
    manifests_root: &Path,
) -> io::Result<(String, [u8; 32], PathBuf)> {
    entries.sort_by(|left, right| {
        left.root_index
            .cmp(&right.root_index)
            .then_with(|| left.relative_path.cmp(&right.relative_path))
            .then_with(|| left.revision_fingerprint.cmp(&right.revision_fingerprint))
    });
    let snapshot_id = new_snapshot_id()?;
    let manifest = render_manifest(&snapshot_id, source_root_count, entries, stats)?;
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
    Ok((snapshot_id, manifest_fingerprint, manifest_path))
}

fn render_manifest(
    snapshot_id: &str,
    root_count: usize,
    entries: &[SnapshotEntry],
    stats: &SnapshotStats,
) -> io::Result<Vec<u8>> {
    let mut output = String::new();
    output.push_str("ELIOT_SEARCH_SNAPSHOT_V1\n");
    let _ = writeln!(output, "snapshot_id={snapshot_id}");
    let _ = writeln!(output, "fingerprint_algorithm={FINGERPRINT_ALGORITHM}");
    let _ = writeln!(output, "source_roots={root_count}");
    let _ = writeln!(output, "entries={}", entries.len());
    let _ = writeln!(output, "total_bytes={}", stats.total_bytes);
    let _ = writeln!(output, "capture_truncated={}", stats.truncated);
    let _ = writeln!(output, "skipped_links={}", stats.skipped_links);
    let _ = writeln!(output, "skipped_policy={}", stats.skipped_policy);
    let _ = writeln!(output, "skipped_binary={}", stats.skipped_binary);
    let _ = writeln!(output, "unreadable_files={}", stats.unreadable_files);
    let _ = writeln!(output, "unstable_files={}", stats.unstable_files);
    output.push_str("--\n");
    for entry in entries {
        let path_hex = hex_bytes(entry.relative_path.as_bytes());
        let _ = writeln!(
            output,
            "{}\t{}\t{}\t{}\t{}",
            entry.root_index,
            path_hex,
            hex32(entry.revision_fingerprint),
            entry.byte_length,
            entry.line_count
        );
        if output.len() > MAX_MANIFEST_BYTES {
            return Err(io::Error::new(
                ErrorKind::InvalidData,
                "snapshot manifest exceeds its finite ceiling",
            ));
        }
    }
    Ok(output.into_bytes())
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

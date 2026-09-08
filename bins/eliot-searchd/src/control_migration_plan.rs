//! Stage a reusable imported-source mapping artifact, not a replacement authority.
//! Canonical IDs/occurrences are compiled once per pass through the existing replay.
//! No original file, policy, namespace owner or visible source state is modified.

use std::fs::{self, File, OpenOptions};
use std::io::{BufRead, BufReader, BufWriter, Read, Write};
use std::path::{Path, PathBuf};
use std::time::{Duration, Instant, SystemTime, UNIX_EPOCH};

use search_contracts::SourceNamespaceId;

use super::{DirectStore, check_deadline, json_string, sha256};
use super::super::storage_io::{ensure_child_directory, ensure_directory, sync_directory};
use crate::development::DataRootGuard;

const MAX_PLAN_BYTES: u64 = 512 * 1024 * 1024;
const MAX_ROW_BYTES: usize = 8 * 1024;
const PLAN_DEADLINE: Duration = Duration::from_secs(120);

/// Temp names are not evidence or state. An error never removes a published plan.
struct StagingFile {
    path: PathBuf,
    file: Option<File>,
}

impl StagingFile {
    fn create(directory: &Path) -> Result<Self, String> {
        let stamp = SystemTime::now().duration_since(UNIX_EPOCH)
            .map_err(|_| "DIRECT_MIGRATION_CLOCK_INVALID".to_owned())?.as_nanos();
        let path = directory.join(format!(".source-map.{}.{stamp}.tmp", std::process::id()));
        let file = OpenOptions::new().read(true).write(true).create_new(true).open(&path)
            .map_err(|_| "DIRECT_MIGRATION_PLAN_CREATE_FAILED".to_owned())?;
        Ok(Self { path, file: Some(file) })
    }

    fn file_mut(&mut self) -> Result<&mut File, String> {
        self.file.as_mut().ok_or_else(|| "DIRECT_MIGRATION_PLAN_CLOSED".to_owned())
    }

    fn remove(mut self) -> Result<(), String> {
        drop(self.file.take());
        fs::remove_file(&self.path).map_err(|_| "DIRECT_MIGRATION_PLAN_CLEANUP_FAILED".to_owned())
    }
}

impl Drop for StagingFile {
    fn drop(&mut self) {
        drop(self.file.take());
        let _ = fs::remove_file(&self.path);
    }
}

impl DirectStore {
    /// Store a deterministic, content-free source mapping draft. The explicit UUID
    /// chooses the *import target*, not the original namespace or live NTFS identity.
    /// Same source history/target/profile gives the same file and IDs. No timestamps
    /// or random data enter its contents, so a lost acknowledgement can be retried.
    pub(crate) fn stage_source_migration_plan(
        &self, owner: &DataRootGuard, target: SourceNamespaceId,
    ) -> Result<String, String> {
        let deadline = Instant::now().checked_add(PLAN_DEADLINE)
            .ok_or_else(|| "DIRECT_MIGRATION_DEADLINE_EXCEEDED".to_owned())?;
        if owner.canonical_root() != self.root.as_path() {
            return Err("DIRECT_MIGRATION_ROOT_OWNER_MISMATCH".to_owned());
        }
        crate::catalog_presence::require_existing(&self.root)?;
        let header = self.inner.source_mapping_header(target)?;
        // Check admitted identity/history before creating even an inert artifact.
        if self.inner.verify_migration_snapshot(deadline)? != header.catalog_snapshot {
            return Err("DIRECT_CONTROL_READBACK_MISMATCH".to_owned());
        }
        let control = self.root.join("control");
        ensure_directory(&control)?;
        let directory = control.join("migration-plans");
        ensure_child_directory(&directory)?;
        sync_directory(&control)?;
        let mut staging = StagingFile::create(&directory)?;
        let mut bytes = 0_u64;
        let mut hash = PlanDigest::new();
        let summary = {
            let mut writer = BufWriter::new(staging.file_mut()?);
            let summary = self.inner.compile_source_mapping(target, deadline, |row| {
                check_deadline(Some(deadline))?;
                bytes = reserve(bytes, row.len())?;
                writer.write_all(row).map_err(|_| "DIRECT_MIGRATION_PLAN_WRITE_FAILED".to_owned())?;
                hash.push(row)
            })?;
            writer.flush().map_err(|_| "DIRECT_MIGRATION_PLAN_WRITE_FAILED".to_owned())?;
            summary
        };
        staging.file_mut()?.sync_all().map_err(|_| "DIRECT_MIGRATION_PLAN_SYNC_FAILED".to_owned())?;
        drop(staging.file.take());
        let digest = hash.finish();

        // Recompile from the fully revalidated source history and compare exact
        // encoded bytes. No generated-record parser is allowed to weaken replay.
        let mut reader = BufReader::new(open_plan(&staging.path, bytes)?);
        let mut compared = 0_u64;
        let mut buffer = [0_u8; MAX_ROW_BYTES];
        let readback = self.inner.compile_source_mapping(target, deadline, |row| {
            compared = reserve(compared, row.len())?;
            reader.read_exact(&mut buffer[..row.len()])
                .map_err(|_| "DIRECT_MIGRATION_PLAN_READBACK_FAILED".to_owned())?;
            if buffer[..row.len()] != *row {
                return Err("DIRECT_MIGRATION_PLAN_READBACK_MISMATCH".to_owned());
            }
            Ok(())
        })?;
        let mut extra = [0_u8; 1];
        if compared != bytes || readback != summary
            || reader.read(&mut extra).map_err(|_| "DIRECT_MIGRATION_PLAN_READBACK_FAILED".to_owned())? != 0
        {
            return Err("DIRECT_MIGRATION_PLAN_READBACK_MISMATCH".to_owned());
        }
        drop(reader);
        check_deadline(Some(deadline))?;
        let name = format!("{}.source-map.v1", sha256::hex(&digest));
        let path = directory.join(&name);
        // Hard-link publication is no-clobber, unlike rename on Unix. A prior
        // artifact is reused only after exact encoded fingerprint/length readback.
        let reused = match fs::hard_link(&staging.path, &path) {
            Ok(()) => false,
            Err(error) if error.kind() == std::io::ErrorKind::AlreadyExists => true,
            Err(_) => return Err("DIRECT_MIGRATION_PLAN_PUBLISH_OUTCOME_UNKNOWN".to_owned()),
        };
        sync_directory(&directory)?;
        if fingerprint(&path, bytes, deadline)? != digest {
            return Err("DIRECT_MIGRATION_PLAN_IMMUTABLE_CONFLICT".to_owned());
        }
        staging.remove()?;
        sync_directory(&directory)?;
        check_deadline(Some(deadline))?;
        Ok(format!(concat!(
            "{{\"event\":\"source_migration_plan_staged\",\"schema\":\"eliot.source-mapping.v1\",",
            "\"target_namespace_id\":\"{}\",\"catalog_snapshot_sha256\":\"{}\",",
            "\"plan_locator\":{},\"plan_chain_sha256\":\"{}\",\"digest_scheme\":\"sha256-record-chain-v1\",\"plan_bytes\":{},\"reused\":{},",
            "\"events\":{},\"sources\":{},\"revision_occurrences\":{},\"retained_revision_events\":{},",
            "\"retirements\":{},\"all_source_events_mapped\":true,\"canonical_records_materialized\":false,",
            "\"redb_imported\":false,\"cutover_authorized\":false}}"
        ), target, sha256::hex(&header.catalog_snapshot), json_string(&format!("control/migration-plans/{name}")),
            sha256::hex(&digest), bytes, reused, summary.events, summary.sources, summary.occurrences,
            summary.path_only_events, summary.retirements))
    }
}

fn reserve(current: u64, next: usize) -> Result<u64, String> {
    if next > MAX_ROW_BYTES { return Err("DIRECT_MIGRATION_PLAN_ROW_TOO_LARGE".to_owned()); }
    current.checked_add(next as u64).filter(|total| *total <= MAX_PLAN_BYTES)
        .ok_or_else(|| "DIRECT_MIGRATION_PLAN_TOO_LARGE".to_owned())
}

fn open_plan(path: &Path, expected: u64) -> Result<File, String> {
    let invalid = || "DIRECT_MIGRATION_PLAN_OBJECT_INVALID".to_owned();
    ensure_directory(path.parent().ok_or_else(invalid)?)?;
    let before = fs::symlink_metadata(path).map_err(|_| invalid())?;
    if !regular(&before) || before.len() != expected || expected > MAX_PLAN_BYTES { return Err(invalid()); }
    let file = File::open(path).map_err(|_| invalid())?;
    let opened = file.metadata().map_err(|_| invalid())?;
    if !regular(&opened) || opened.len() != expected || opened.modified().ok() != before.modified().ok() {
        return Err(invalid());
    }
    Ok(file)
}

fn regular(metadata: &fs::Metadata) -> bool {
    #[cfg(windows)]
    let reparse = {
        use std::os::windows::fs::MetadataExt;
        metadata.file_attributes() & 0x400 != 0
    };
    #[cfg(not(windows))]
    let reparse = false;
    metadata.is_file() && !metadata.file_type().is_symlink() && !reparse
}

/// Canonical record-chain digest, explicitly not the raw file's SHA-256.
/// Existing SHA-256/multipart primitives remain unchanged; no new hash algorithm.
struct PlanDigest { chain: [u8; 32], rows: u64, bytes: u64 }
impl PlanDigest {
    fn new() -> Self {
        Self { chain: sha256::digest_parts(b"eliot-search/source-map-chain/v1", &[]), rows: 0, bytes: 0 }
    }
    fn push(&mut self, row: &[u8]) -> Result<(), String> {
        self.bytes = reserve(self.bytes, row.len())?;
        if row.last() != Some(&b'\n') || row[..row.len() - 1].contains(&b'\n') {
            return Err("DIRECT_MIGRATION_PLAN_ROW_INVALID".to_owned());
        }
        self.rows = self.rows.checked_add(1).ok_or_else(|| "DIRECT_MIGRATION_PLAN_TOO_LARGE".to_owned())?;
        self.chain = sha256::digest_parts(b"eliot-search/source-map-row/v1", &[
            &self.chain, &self.rows.to_be_bytes(), row,
        ]);
        Ok(())
    }
    fn finish(self) -> [u8; 32] {
        sha256::digest_parts(b"eliot-search/source-map-end/v1", &[
            &self.chain, &self.rows.to_be_bytes(), &self.bytes.to_be_bytes(),
        ])
    }
}

fn fingerprint(path: &Path, length: u64, deadline: Instant) -> Result<[u8; 32], String> {
    let file = open_plan(path, length)?;
    let before = file.metadata().map_err(|_| "DIRECT_MIGRATION_PLAN_READ_FAILED".to_owned())?;
    let mut reader = BufReader::new(file);
    let mut row = Vec::new();
    let mut digest = PlanDigest::new();
    loop {
        check_deadline(Some(deadline))?;
        row.clear();
        let read = Read::take(&mut reader, MAX_ROW_BYTES as u64 + 1).read_until(b'\n', &mut row)
            .map_err(|_| "DIRECT_MIGRATION_PLAN_READ_FAILED".to_owned())?;
        if read == 0 { break; }
        digest.push(&row)?;
    }
    let after = reader.get_ref().metadata().map_err(|_| "DIRECT_MIGRATION_PLAN_READ_FAILED".to_owned())?;
    if digest.bytes != length || after.len() != length || before.modified().ok() != after.modified().ok() {
        return Err("DIRECT_MIGRATION_PLAN_READBACK_MISMATCH".to_owned());
    }
    Ok(digest.finish())
}

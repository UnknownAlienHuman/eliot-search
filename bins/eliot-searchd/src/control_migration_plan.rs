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

#[path = "control_migration_redb.rs"]
mod redb_import;
#[path = "control_migration_content.rs"]
mod content_readback;
#[path = "control_migration_cutover.rs"]
mod cutover;

const MAX_PLAN_BYTES: u64 = 512 * 1024 * 1024;
const MAX_ROW_BYTES: usize = 8 * 1024;
const PLAN_DEADLINE: Duration = Duration::from_secs(120);

/// Temp names are not evidence or state. An error never removes a published plan.
struct StagingFile {
    path: PathBuf,
    file: Option<File>,
    identity: (u64, u64),
    cleanup_armed: bool,
}

impl StagingFile {
    fn create(directory: &Path) -> Result<Self, String> {
        let stamp = SystemTime::now().duration_since(UNIX_EPOCH)
            .map_err(|_| "DIRECT_MIGRATION_CLOCK_INVALID".to_owned())?.as_nanos();
        let path = directory.join(format!(".source-map.{}.{stamp}.tmp", std::process::id()));
        let file = OpenOptions::new().read(true).write(true).create_new(true).open(&path)
            .map_err(|_| "DIRECT_MIGRATION_PLAN_CREATE_FAILED".to_owned())?;
        let identity = redb_import::native_identity(&file)?;
        redb_import::verify_locator(&file, &path)?;
        Ok(Self { path, file: Some(file), identity, cleanup_armed: true })
    }

    fn file_mut(&mut self) -> Result<&mut File, String> {
        self.file.as_mut().ok_or_else(|| "DIRECT_MIGRATION_PLAN_CLOSED".to_owned())
    }

    fn remove(mut self) -> Result<(), String> {
        self.discard_owned()
    }

    fn discard_owned(&mut self) -> Result<(), String> {
        // Disarm before any fallible cleanup. Explicit removal and Drop must
        // never unlink the same locator twice or retry a failed identity check.
        if !std::mem::replace(&mut self.cleanup_armed, false) { return Ok(()); }
        drop(self.file.take());
        let invalid = || "DIRECT_MIGRATION_PLAN_CLEANUP_IDENTITY_CHANGED".to_owned();
        let metadata = match fs::symlink_metadata(&self.path) {
            Ok(metadata) => metadata,
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => return Ok(()),
            Err(_) => return Err(invalid()),
        };
        if !regular(&metadata) { return Err(invalid()); }
        let current = File::open(&self.path).map_err(|_| invalid())?;
        if redb_import::native_identity(&current)? != self.identity { return Err(invalid()); }
        redb_import::verify_locator(&current, &self.path)?;
        drop(current);
        fs::remove_file(&self.path).map_err(|_| "DIRECT_MIGRATION_PLAN_CLEANUP_FAILED".to_owned())
    }
}

impl Drop for StagingFile {
    fn drop(&mut self) {
        // An ambiguous replacement is retained, not deleted by its old name.
        let _ = self.discard_owned();
    }
}

/// Typed result of one deterministic staging pass. The JSON report rendered
/// from it is byte-identical to the historical T10 output; the struct lets the
/// atomic cutover bind exact chains without reparsing its own receipt.
pub(super) struct StagedPlan {
    pub(super) target: SourceNamespaceId,
    pub(super) catalog_snapshot: [u8; 32],
    pub(super) plan_name: String,
    pub(super) plan_chain: [u8; 32],
    pub(super) plan_bytes: u64,
    pub(super) plan_reused: bool,
    pub(super) events: u64,
    pub(super) sources: u64,
    pub(super) occurrences: u64,
    pub(super) path_only_events: u64,
    pub(super) retirements: u64,
    pub(super) locator_prefix: &'static str,
    pub(super) location: &'static str,
    pub(super) database_name: String,
    pub(super) database_reused: bool,
    pub(super) content_name: String,
    pub(super) content_chain: [u8; 32],
    pub(super) content_records: u64,
    pub(super) content_source_bytes: u64,
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
        #[cfg(unix)]
        sync_directory(&control)?;
        #[cfg(not(unix))]
        sync_directory(&control);
        Self::stage_mapping_artifact(&self.inner, &self.root, target, &directory, "control/migration-plans/", deadline)
    }

    /// Shared artifact writer for the live owner and offline entrypoint. The caller
    /// holds `source_root`'s ordinary lock and opened source from that exact root.
    /// No source store is initialized. Payload readback resolves existing Windows
    /// credentials only; no credential or source object is created or converted.
    /// A returned locator is relative to the explicitly named location scope.
    /// The typed form below carries the same values for the atomic cutover.
    pub(crate) fn stage_mapping_artifact(
        source: &crate::plaintext_direct_store::DirectStore, source_root: &Path,
        target: SourceNamespaceId, directory: &Path, locator_prefix: &'static str,
        deadline: Instant,
    ) -> Result<String, String> {
        let plan = Self::stage_mapping_artifact_typed(
            source, source_root, target, directory, locator_prefix, deadline,
        )?;
        Ok(staged_plan_json(&plan))
    }

    /// Typed staging pass shared by the verify-only report and the atomic
    /// cutover. Every byte guarantee of the historical writer holds here; only
    /// the final JSON rendering moves to [`staged_plan_json`].
    pub(super) fn stage_mapping_artifact_typed(
        source: &crate::plaintext_direct_store::DirectStore, source_root: &Path,
        target: SourceNamespaceId, directory: &Path, locator_prefix: &'static str,
        deadline: Instant,
    ) -> Result<StagedPlan, String> {
        let location = match locator_prefix {
            "" => "explicit_output_directory",
            "control/migration-plans/" | "control/" => "data_root",
            _ => return Err("DIRECT_MIGRATION_OUTPUT_INVALID".to_owned()),
        };
        check_deadline(Some(deadline))?;
        ensure_directory(directory)?;
        let header = source.source_mapping_header(target)?;
        if source.verify_migration_snapshot(deadline)? != header.catalog_snapshot {
            return Err("DIRECT_CONTROL_READBACK_MISMATCH".to_owned());
        }
        // A committed cutover freezes the migrated history: restaging the same
        // target/snapshot reproduces evidence, anything else is superseded and
        // a torn marker quarantines instead of staging over it silently.
        cutover::gate_staging_against_marker(source_root, target, header.catalog_snapshot)?;
        let mut staging = StagingFile::create(directory)?;
        let mut bytes = 0_u64;
        let mut hash = PlanDigest::new();
        let summary = {
            let mut writer = BufWriter::new(staging.file_mut()?);
            let summary = source.compile_source_mapping(target, deadline, |row| {
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
        let readback = source.compile_source_mapping(target, deadline, |row| {
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
        #[cfg(unix)]
        sync_directory(directory)?;
        #[cfg(not(unix))]
        sync_directory(directory);
        if fingerprint(&path, bytes, deadline)? != digest {
            return Err("DIRECT_MIGRATION_PLAN_IMMUTABLE_CONFLICT".to_owned());
        }
        staging.remove()?;
        #[cfg(unix)]
        sync_directory(directory)?;
        #[cfg(not(unix))]
        sync_directory(directory);
        let content = content_readback::stage(source, source_root, target, digest, directory, deadline)?;
        let (database_name, database_reused) = redb_import::store(
            source, target, directory, digest, summary.import_counts(), &content, deadline,
        )?;
        if source.verify_migration_snapshot(deadline)? != header.catalog_snapshot {
            return Err("DIRECT_CONTROL_READBACK_MISMATCH".to_owned());
        }
        // Source replay and target import can take substantial time. Do not
        // acknowledge text/content locators only checked before those steps.
        if fingerprint(&path, bytes, deadline)? != digest
            || fingerprint(&directory.join(&content.name), content.encoded_bytes, deadline)? != content.chain
        {
            return Err("DIRECT_MIGRATION_PLAN_READBACK_MISMATCH".to_owned());
        }
        check_deadline(Some(deadline))?;
        Ok(StagedPlan {
            target,
            catalog_snapshot: header.catalog_snapshot,
            plan_name: name,
            plan_chain: digest,
            plan_bytes: bytes,
            plan_reused: reused,
            events: summary.events,
            sources: summary.sources,
            occurrences: summary.occurrences,
            path_only_events: summary.path_only_events,
            retirements: summary.retirements,
            locator_prefix,
            location,
            database_name,
            database_reused,
            content_name: content.name,
            content_chain: content.chain,
            content_records: content.records,
            content_source_bytes: content.source_bytes,
        })
    }
}

fn staged_plan_json(plan: &StagedPlan) -> String {
    format!(concat!(
        "{{\"event\":\"source_migration_plan_staged\",\"schema\":\"eliot.source-mapping.v1\",",
        "\"target_namespace_id\":\"{}\",\"catalog_snapshot_sha256\":\"{}\",",
        "\"plan_locator\":{},\"plan_chain_sha256\":\"{}\",\"digest_scheme\":\"sha256-record-chain-v1\",\"plan_bytes\":{},\"reused\":{},",
        "\"events\":{},\"sources\":{},\"revision_occurrences\":{},\"retained_revision_events\":{},",
        "\"retirements\":{},\"all_source_events_mapped\":true,\"canonical_records_materialized\":false,",
        "\"plan_location\":\"{}\",\"staged_database_locator\":{},\"staged_database_reused\":{},",
        "\"source_mapping_imported_to_redb\":true,\"staged_database_verified\":true,",
        "\"staged_database_schema\":\"source-map-content-v2\",\"content_manifest_bound_to_redb\":true,",
        "\"content_manifest_locator\":{},\"content_manifest_chain_sha256\":\"{}\",",
        "\"content_objects_verified\":{},\"content_bytes_verified\":{},\"content_blake3_verified\":true,",
        "\"redb_imported\":false,\"active_control_imported\":false,\"cutover_authorized\":false}}"
    ), plan.target, sha256::hex(&plan.catalog_snapshot),
        json_string(&format!("{}{}", plan.locator_prefix, plan.plan_name)),
        sha256::hex(&plan.plan_chain), plan.plan_bytes, plan.plan_reused,
        plan.events, plan.sources, plan.occurrences,
        plan.path_only_events, plan.retirements, plan.location,
        json_string(&format!("{}{}", plan.locator_prefix, plan.database_name)),
        plan.database_reused,
        json_string(&format!("{}{}", plan.locator_prefix, plan.content_name)),
        sha256::hex(&plan.content_chain),
        plan.content_records, plan.content_source_bytes)
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
    redb_import::verify_locator(&file, path)?;
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
    redb_import::verify_locator(reader.get_ref(), path)?;
    check_deadline(Some(deadline))?;
    Ok(digest.finish())
}

//! Atomic cutover of the primary control authority from the file journal to
//! the verified redb mapping.
//!
//! The T10 verify-only flow stages a deterministic plan, content manifest and
//! redb import without activating any authority (`cutover_authorized:false`).
//! This module owns the single explicit operation that promotes that staging
//! into the serving data root and publishes exactly one cutover marker:
//!
//! - exact readback of the file journal before staging (invariant 18);
//! - deterministic staging into `control/` with full re-verification;
//! - exact readback after staging; any mismatch quarantines, never silent;
//! - one atomic marker publication binding owner, snapshot and chains;
//! - ambiguous publish outcomes report `OUTCOME_UNKNOWN` with a recovery read.
//!
//! The file journal stays on disk as preserved evidence. Serve-path query and
//! mutation rerouting consumes this marker in a follow-up wiring step; until
//! then ordinary requests keep appending the legacy journal and the receipt
//! says so explicitly instead of relabelling partial progress as success.

use std::fs::{self, OpenOptions};
use std::io::Write;
use std::path::Path;
use std::time::{Duration, Instant};

use search_contracts::{DataRootId, InstallationIncarnationId, SourceNamespaceId};

use super::{DirectStore, StagedPlan, check_deadline, json_string, sha256};
use crate::development::DataRootGuard;

const CUTOVER_DEADLINE: Duration = Duration::from_secs(120);
/// Exact marker name inside `control/`. One file, one authority, no sidecars.
pub(super) const CUTOVER_MARKER_FILE: &str = "control-cutover.v1";
const CUTOVER_MARKER_TMP: &str = "control-cutover.tmp";
const MAX_MARKER_BYTES: usize = 2048;
const MARKER_MAGIC: &str = "ELIOT-SEARCH-CONTROL-CUTOVER-V1";
const MARKER_VERSION_LINE: &str = "format_version=1";
/// Exact staged schema the marker may bind. Never a prefix match.
pub(super) const STAGED_DATABASE_SCHEMA: &str = "source-map-content-v2";
const DATABASE_SUFFIX: &str = ".source-map.v2.redb";

const CUTOVER_OWNER_MISMATCH: &str = "DIRECT_MIGRATION_CUTOVER_OWNER_MISMATCH";
const CUTOVER_ALREADY_COMMITTED: &str = "DIRECT_MIGRATION_CUTOVER_ALREADY_COMMITTED";
const CUTOVER_SUPERSEDED: &str = "DIRECT_MIGRATION_CUTOVER_SUPERSEDED";
const CUTOVER_CORRUPT: &str = "DIRECT_MIGRATION_CUTOVER_CORRUPT";
const CUTOVER_CREATE_FAILED: &str = "DIRECT_MIGRATION_CUTOVER_CREATE_FAILED";
const CUTOVER_OUTCOME_UNKNOWN: &str = "DIRECT_MIGRATION_CUTOVER_PUBLISH_OUTCOME_UNKNOWN";
const CUTOVER_READBACK_MISMATCH: &str = "DIRECT_MIGRATION_CUTOVER_READBACK_MISMATCH";

/// Exact authority bound by one committed marker.
#[derive(Clone, Debug, Eq, PartialEq)]
pub(super) struct CutoverMarker {
    pub(super) target: SourceNamespaceId,
    pub(super) incarnation: InstallationIncarnationId,
    pub(super) root: DataRootId,
    pub(super) epoch: u64,
    pub(super) catalog_snapshot: [u8; 32],
    pub(super) plan_chain: [u8; 32],
    pub(super) content_chain: [u8; 32],
    /// Single file name inside `control/`, never a path.
    pub(super) database_file_name: String,
    pub(super) database_schema: String,
}

impl CutoverMarker {
    /// Canonical bytes. Deterministic: no timestamps, no randomness, so a lost
    /// acknowledgement can replay the exact operation and compare byte equality.
    pub(super) fn encode(&self) -> Vec<u8> {
        let mut body = String::new();
        push_line(&mut body, MARKER_MAGIC);
        push_line(&mut body, MARKER_VERSION_LINE);
        push_field(&mut body, "target_namespace_id", &self.target.to_string());
        push_field(
            &mut body,
            "installation_incarnation_id",
            &self.incarnation.to_string(),
        );
        push_field(&mut body, "data_root_id", &self.root.to_string());
        push_field(&mut body, "owner_epoch", &self.epoch.to_string());
        push_field(
            &mut body,
            "catalog_snapshot_sha256",
            &sha256::hex(&self.catalog_snapshot),
        );
        push_field(
            &mut body,
            "plan_chain_sha256",
            &sha256::hex(&self.plan_chain),
        );
        push_field(
            &mut body,
            "content_manifest_chain_sha256",
            &sha256::hex(&self.content_chain),
        );
        push_field(
            &mut body,
            "staged_database_locator",
            &["control/", self.database_file_name.as_str()].concat(),
        );
        push_field(&mut body, "staged_database_schema", &self.database_schema);
        let digest = sha256::digest(body.as_bytes());
        push_field(&mut body, "record_digest", &sha256::hex(&digest));
        body.into_bytes()
    }

    /// Strict decode. Any deviation in shape, order, spelling or digest fails
    /// closed; a torn write is corruption, never partial authority.
    pub(super) fn decode(bytes: &[u8]) -> Result<Self, String> {
        let invalid = || "DIRECT_MIGRATION_CUTOVER_CORRUPT".to_owned();
        if bytes.len() > MAX_MARKER_BYTES || bytes.is_empty() {
            return Err(invalid());
        }
        let text = core::str::from_utf8(bytes).map_err(|_| invalid())?;
        if !text.ends_with('\n') {
            return Err(invalid());
        }
        let lines: Vec<&str> = text.lines().collect();
        if lines.len() != 12 || lines[0] != MARKER_MAGIC || lines[1] != MARKER_VERSION_LINE {
            return Err(invalid());
        }
        let field = |index: usize, key: &str| -> Result<&str, String> {
            lines[index].strip_prefix(key).ok_or_else(invalid)
        };
        let target =
            SourceNamespaceId::parse(field(2, "target_namespace_id=")?).map_err(|_| invalid())?;
        if target.as_bytes() == &[0; 16] || target.to_string() != field(2, "target_namespace_id=")?
        {
            return Err(invalid());
        }
        let incarnation =
            InstallationIncarnationId::parse(field(3, "installation_incarnation_id=")?)
                .map_err(|_| invalid())?;
        if incarnation.to_string() != field(3, "installation_incarnation_id=")? {
            return Err(invalid());
        }
        let root = DataRootId::parse(field(4, "data_root_id=")?).map_err(|_| invalid())?;
        if root.to_string() != field(4, "data_root_id=")? {
            return Err(invalid());
        }
        let epoch_text = field(5, "owner_epoch=")?;
        let epoch: u64 = epoch_text.parse().map_err(|_| invalid())?;
        if epoch == 0 || epoch.to_string() != epoch_text {
            return Err(invalid());
        }
        let snapshot = canonical_digest(field(6, "catalog_snapshot_sha256=")?)?;
        let plan_chain = canonical_digest(field(7, "plan_chain_sha256=")?)?;
        let content_chain = canonical_digest(field(8, "content_manifest_chain_sha256=")?)?;
        let locator = field(9, "staged_database_locator=")?;
        let name = locator.strip_prefix("control/").ok_or_else(invalid)?;
        if name.contains('/') || name.contains('\\') {
            return Err(invalid());
        }
        let stem = name.strip_suffix(DATABASE_SUFFIX).ok_or_else(invalid)?;
        canonical_digest(stem)?;
        let schema = field(10, "staged_database_schema=")?;
        if schema != STAGED_DATABASE_SCHEMA {
            return Err(invalid());
        }
        let digest_text = field(11, "record_digest=")?;
        let body_len = bytes
            .len()
            .checked_sub(lines[11].len() + 1)
            .ok_or_else(invalid)?;
        if sha256::hex(&sha256::digest(&bytes[..body_len])) != digest_text
            || canonical_digest(digest_text).is_err()
        {
            return Err(invalid());
        }
        Ok(Self {
            target,
            incarnation,
            root,
            epoch,
            catalog_snapshot: snapshot,
            plan_chain,
            content_chain,
            database_file_name: name.to_owned(),
            database_schema: schema.to_owned(),
        })
    }

    /// Digest of the exact canonical body, the marker's stable identity.
    pub(super) fn record_digest(&self) -> [u8; 32] {
        let bytes = self.encode();
        let body_len = bytes.len() - ("record_digest=".len() + 64 + 1);
        sha256::digest(&bytes[..body_len])
    }
}

/// Read-only marker resolution. Never writes, never repairs: inspection
/// failures are fail-closed as corrupt and the caller decides whether the
/// path may arm quarantine (mutating paths) or must stay read-only (status).
#[derive(Debug, Eq, PartialEq)]
pub(super) enum MarkerState {
    Absent,
    Valid(Box<ValidMarker>),
    Corrupt,
}

/// Exact committed bytes next to their decoded authority.
#[derive(Debug, Eq, PartialEq)]
pub(super) struct ValidMarker {
    pub(super) marker: CutoverMarker,
    pub(super) bytes: Vec<u8>,
}

/// Read-only resolution of the single authority file.
pub(super) fn resolve_marker(data_root: &Path) -> MarkerState {
    let path = data_root.join("control").join(CUTOVER_MARKER_FILE);
    let metadata = match fs::symlink_metadata(&path) {
        Ok(metadata) => metadata,
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => {
            return MarkerState::Absent;
        }
        Err(_) => return MarkerState::Corrupt,
    };
    if !regular(&metadata) || metadata.len() > MAX_MARKER_BYTES as u64 {
        return MarkerState::Corrupt;
    }
    fs::read(&path).map_or(MarkerState::Corrupt, |bytes| {
        CutoverMarker::decode(&bytes).map_or(MarkerState::Corrupt, |marker| {
            MarkerState::Valid(Box::new(ValidMarker { marker, bytes }))
        })
    })
}

/// How a proposed cutover relates to an already committed marker.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(super) enum ReplayDecision {
    /// Same owner, target, snapshot and chains. A successor epoch replays the
    /// exact operation after a crash; only the committing epoch is historical.
    Identical,
    /// Same installation and root but different migrated history.
    Diverged,
    /// Different installation incarnation or data root: a second owner.
    ForeignOwner,
}

/// Classify a replay without touching any byte.
pub(super) fn check_replay(existing: &CutoverMarker, proposed: &CutoverMarker) -> ReplayDecision {
    if existing.incarnation != proposed.incarnation || existing.root != proposed.root {
        return ReplayDecision::ForeignOwner;
    }
    if existing.target == proposed.target
        && existing.catalog_snapshot == proposed.catalog_snapshot
        && existing.plan_chain == proposed.plan_chain
        && existing.content_chain == proposed.content_chain
        && existing.database_file_name == proposed.database_file_name
        && existing.database_schema == proposed.database_schema
    {
        ReplayDecision::Identical
    } else {
        ReplayDecision::Diverged
    }
}

/// Staging gate: a committed marker freezes the migrated history. Restaging
/// the identical target/snapshot reproduces evidence; anything else is
/// superseded, and a torn marker quarantines instead of staging over it.
pub(super) fn gate_staging_against_marker(
    data_root: &Path,
    target: SourceNamespaceId,
    snapshot: [u8; 32],
) -> Result<(), String> {
    match resolve_marker(data_root) {
        MarkerState::Absent => Ok(()),
        MarkerState::Valid(committed) => {
            if committed.marker.target == target && committed.marker.catalog_snapshot == snapshot {
                Ok(())
            } else {
                Err(CUTOVER_SUPERSEDED.to_owned())
            }
        }
        MarkerState::Corrupt => Err(quarantined(data_root, CUTOVER_CORRUPT)),
    }
}

/// Outcome of one marker publication attempt.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(super) enum PublishOutcome {
    /// This call published the marker.
    Committed,
    /// The exact bytes were already committed; the operation ran once.
    ReplayIdentical,
}

/// Publish exactly one marker via the proven temp/sync/rename discipline.
/// A lost acknowledgement replays byte-identical bytes into [`PublishOutcome::ReplayIdentical`].
/// Any ambiguous native outcome reports `OUTCOME_UNKNOWN` (invariant 18):
/// identical bytes observed after a failed rename still cannot prove which
/// attempt landed, so success is never claimed from the recovery read.
pub(super) fn publish_marker(data_root: &Path, expected: &[u8]) -> Result<PublishOutcome, String> {
    if expected.is_empty() || expected.len() > MAX_MARKER_BYTES {
        return Err(CUTOVER_CREATE_FAILED.to_owned());
    }
    let control = data_root.join("control");
    match fs::symlink_metadata(&control) {
        Ok(metadata) if metadata.is_dir() && !is_link(&metadata) => {}
        _ => return Err(CUTOVER_CREATE_FAILED.to_owned()),
    }
    let tmp = control.join(CUTOVER_MARKER_TMP);
    let marker = control.join(CUTOVER_MARKER_FILE);
    // Stale tmp residue from a crashed attempt is inert: it is never read as
    // authority and every attempt removes it before and after its own write.
    let _ = fs::remove_file(&tmp);
    // A present marker of any shape takes the classify path below; rename must
    // never clobber a committed cutover where the platform replaces silently.
    if fs::symlink_metadata(&marker).is_ok() {
        let _ = fs::remove_file(&tmp);
        return classify_existing(&marker, expected, data_root);
    }
    let mut file = OpenOptions::new()
        .read(true)
        .write(true)
        .create_new(true)
        .open(&tmp)
        .map_err(|_| CUTOVER_CREATE_FAILED.to_owned())?;
    if file.metadata().map_or(true, |metadata| !regular(&metadata)) {
        let _ = fs::remove_file(&tmp);
        return Err(CUTOVER_CREATE_FAILED.to_owned());
    }
    if file
        .write_all(expected)
        .and_then(|()| file.sync_all())
        .is_err()
    {
        let _ = fs::remove_file(&tmp);
        return Err(CUTOVER_CREATE_FAILED.to_owned());
    }
    drop(file);
    sync_directory(&control);
    match fs::rename(&tmp, &marker) {
        Ok(()) => {
            sync_directory(&control);
            match fs::read(&marker) {
                Ok(bytes) if bytes == expected => Ok(PublishOutcome::Committed),
                _ => Err(quarantined(data_root, CUTOVER_READBACK_MISMATCH)),
            }
        }
        Err(error) if error.kind() == std::io::ErrorKind::AlreadyExists => {
            let _ = fs::remove_file(&tmp);
            classify_existing(&marker, expected, data_root)
        }
        Err(_) => {
            let _ = fs::remove_file(&tmp);
            // A marker that appeared after the pre-check takes the classify
            // path, but identical bytes still cannot prove which attempt
            // landed, so the outcome stays unknown (invariant 18).
            if fs::symlink_metadata(&marker).is_ok() {
                let outcome = classify_existing(&marker, expected, data_root);
                if outcome == Ok(PublishOutcome::ReplayIdentical) {
                    return Err(CUTOVER_OUTCOME_UNKNOWN.to_owned());
                }
                return outcome;
            }
            Err(CUTOVER_OUTCOME_UNKNOWN.to_owned())
        }
    }
}

/// Classify a present marker during publication: identical bytes replay the
/// exact operation, a valid different marker is already committed, a torn one
/// quarantines, and an unreadable one reports the unknown outcome.
fn classify_existing(
    marker: &Path,
    expected: &[u8],
    data_root: &Path,
) -> Result<PublishOutcome, String> {
    match fs::read(marker) {
        Ok(bytes) if bytes == expected => Ok(PublishOutcome::ReplayIdentical),
        Ok(bytes) => match CutoverMarker::decode(&bytes) {
            Ok(_) => Err(CUTOVER_ALREADY_COMMITTED.to_owned()),
            Err(_) => Err(quarantined(data_root, CUTOVER_CORRUPT)),
        },
        Err(_) => Err(CUTOVER_OUTCOME_UNKNOWN.to_owned()),
    }
}

/// Explicit pre-cutover rollback. With no marker this re-verifies nothing by
/// itself; the orchestration binds the live snapshot into the receipt. A
/// committed marker is never deleted here, and a torn one quarantines.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(super) enum RollbackAction {
    Noop,
}

/// Decide rollback without touching any byte except a quarantine arm on corruption.
pub(super) fn check_rollback(data_root: &Path) -> Result<RollbackAction, String> {
    match resolve_marker(data_root) {
        MarkerState::Absent => Ok(RollbackAction::Noop),
        MarkerState::Valid(_) => Err(CUTOVER_ALREADY_COMMITTED.to_owned()),
        MarkerState::Corrupt => Err(quarantined(data_root, CUTOVER_CORRUPT)),
    }
}

/// Read-only authority status. One marker read plus at most one file stat;
/// never a journal replay, never a write. Repeated reads are byte-identical.
pub(super) fn cutover_status_json(data_root: &Path) -> String {
    let quarantined = crate::catalog_quarantine::is_quarantined(data_root);
    let (
        authority,
        marker_present,
        marker_digest,
        snapshot,
        plan,
        content,
        locator,
        db_present,
        db_bytes,
        complete,
    ) = match resolve_marker(data_root) {
        MarkerState::Absent => (
            "file-journal",
            false,
            "null".to_owned(),
            "null".to_owned(),
            "null".to_owned(),
            "null".to_owned(),
            "null".to_owned(),
            false,
            0,
            !quarantined,
        ),
        MarkerState::Corrupt => (
            "marker-corrupt",
            true,
            "null".to_owned(),
            "null".to_owned(),
            "null".to_owned(),
            "null".to_owned(),
            "null".to_owned(),
            false,
            0,
            false,
        ),
        MarkerState::Valid(committed) => {
            let db_path = data_root
                .join("control")
                .join(&committed.marker.database_file_name);
            let (present, bytes) = match fs::symlink_metadata(&db_path) {
                Ok(metadata) if regular(&metadata) && metadata.len() > 0 => (true, metadata.len()),
                _ => (false, 0),
            };
            let locator = ["control/", committed.marker.database_file_name.as_str()].concat();
            (
                "redb-control-marker-v1",
                true,
                json_string(&sha256::hex(&committed.marker.record_digest())),
                json_string(&sha256::hex(&committed.marker.catalog_snapshot)),
                json_string(&sha256::hex(&committed.marker.plan_chain)),
                json_string(&sha256::hex(&committed.marker.content_chain)),
                json_string(&locator),
                present,
                bytes,
                present && !quarantined,
            )
        }
    };
    format!(
        concat!(
            "{{\"event\":\"control_cutover_status\",\"schema\":\"control-cutover-status-v1\",",
            "\"authority\":{},\"marker_present\":{},\"marker_digest\":{},",
            "\"catalog_snapshot_sha256\":{},\"plan_chain_sha256\":{},",
            "\"content_manifest_chain_sha256\":{},\"staged_database_locator\":{},",
            "\"staged_database_present\":{},\"staged_database_bytes\":{},",
            "\"quarantined\":{},\"complete\":{},\"read_only\":true}}"
        ),
        json_string(authority),
        marker_present,
        marker_digest,
        snapshot,
        plan,
        content,
        locator,
        db_present,
        db_bytes,
        quarantined,
        complete,
    )
}

impl DirectStore {
    /// Atomically cut the primary catalog over to the verified redb mapping.
    ///
    /// Under the live owner guard: exact readback of the file journal before
    /// staging, deterministic staging into `control/`, exact readback after,
    /// then one marker publication. Any readback mismatch quarantines instead
    /// of proceeding silently; an ambiguous publish reports `OUTCOME_UNKNOWN`.
    /// The legacy file journal is preserved untouched as evidence.
    ///
    /// Serve-path query/mutation rerouting consumes the committed marker in a
    /// follow-up wiring step; the receipt states that explicitly.
    ///
    /// The serve/CLI wiring lands with the integration owner; until then this
    /// entrypoint is exercised through its unit-tested commit path.
    #[allow(dead_code)]
    pub(crate) fn cutover_control_to_redb(
        &self,
        owner: &DataRootGuard,
        target: SourceNamespaceId,
    ) -> Result<String, String> {
        let deadline = Instant::now()
            .checked_add(CUTOVER_DEADLINE)
            .ok_or_else(|| "DIRECT_MIGRATION_DEADLINE_EXCEEDED".to_owned())?;
        let root = owner.canonical_root();
        if root != self.root.as_path() {
            return Err(CUTOVER_OWNER_MISMATCH.to_owned());
        }
        crate::catalog_quarantine::check(root)?;
        crate::catalog_presence::require_existing(root)?;
        if target.as_bytes() == &[0; 16] {
            return Err("DIRECT_MIGRATION_TARGET_NAMESPACE_INVALID".to_owned());
        }
        // Exact readback before: the admitted snapshot this cutover migrates.
        let snapshot = self.inner.verify_migration_snapshot(deadline)?;
        let header = self.inner.source_mapping_header(target)?;
        if header.catalog_snapshot != snapshot {
            return Err("DIRECT_CONTROL_READBACK_MISMATCH".to_owned());
        }
        let (incarnation, data_root_id, epoch) = owner.journal_owner_inputs();
        // Fast-fail replay conflicts before the expensive staging pass.
        if let MarkerState::Valid(committed) = resolve_marker(root) {
            let existing = &committed.marker;
            if existing.incarnation != incarnation || existing.root != data_root_id {
                return Err(CUTOVER_OWNER_MISMATCH.to_owned());
            }
            if existing.target != target || existing.catalog_snapshot != snapshot {
                return Err(CUTOVER_ALREADY_COMMITTED.to_owned());
            }
        }
        check_deadline(Some(deadline))?;
        let control = root.join("control");
        let staged = Self::stage_mapping_artifact_typed(
            &self.inner,
            &self.root,
            target,
            &control,
            "control/",
            deadline,
        )?;
        if staged.catalog_snapshot != snapshot {
            return Err(quarantined(root, "DIRECT_CONTROL_READBACK_MISMATCH"));
        }
        // Exact readback after staging: the source must not have moved under us.
        if self.inner.verify_migration_snapshot(deadline)? != snapshot {
            return Err(quarantined(root, "DIRECT_CONTROL_READBACK_MISMATCH"));
        }
        let proposed = CutoverMarker {
            target,
            incarnation,
            root: data_root_id,
            epoch: epoch.get(),
            catalog_snapshot: snapshot,
            plan_chain: staged.plan_chain,
            content_chain: staged.content_chain,
            database_file_name: staged.database_name.clone(),
            database_schema: STAGED_DATABASE_SCHEMA.to_owned(),
        };
        // Full replay check with exact chains before publishing.
        if let MarkerState::Valid(committed) = resolve_marker(root) {
            match check_replay(&committed.marker, &proposed) {
                ReplayDecision::Identical => {
                    return Ok(cutover_receipt(&committed.marker, &staged, true));
                }
                ReplayDecision::Diverged => {
                    return Err(CUTOVER_ALREADY_COMMITTED.to_owned());
                }
                ReplayDecision::ForeignOwner => {
                    return Err(CUTOVER_OWNER_MISMATCH.to_owned());
                }
            }
        }
        if resolve_marker(root) == MarkerState::Corrupt {
            return Err(quarantined(root, CUTOVER_CORRUPT));
        }
        let bytes = proposed.encode();
        let replayed = match publish_marker(root, &bytes)? {
            PublishOutcome::Committed => false,
            PublishOutcome::ReplayIdentical => true,
        };
        // Exact readback of the committed marker plus a final source check.
        match resolve_marker(root) {
            MarkerState::Valid(read) if read.bytes == bytes && read.marker == proposed => {}
            _ => return Err(quarantined(root, CUTOVER_READBACK_MISMATCH)),
        }
        if self.inner.verify_migration_snapshot(deadline)? != snapshot {
            return Err(quarantined(root, "DIRECT_CONTROL_READBACK_MISMATCH"));
        }
        check_deadline(Some(deadline))?;
        Ok(cutover_receipt(&proposed, &staged, replayed))
    }

    /// Explicit rollback before cutover. Succeeds only with no marker: it
    /// binds the live file snapshot into a no-op receipt. A committed marker
    /// is refused and a torn one quarantines; neither is ever deleted here.
    ///
    /// The serve/CLI wiring lands with the integration owner; until then this
    /// entrypoint is exercised through its unit-tested decision path.
    #[allow(dead_code)]
    pub(crate) fn rollback_control_cutover(&self, owner: &DataRootGuard) -> Result<String, String> {
        let deadline = Instant::now()
            .checked_add(CUTOVER_DEADLINE)
            .ok_or_else(|| "DIRECT_MIGRATION_DEADLINE_EXCEEDED".to_owned())?;
        let root = owner.canonical_root();
        if root != self.root.as_path() {
            return Err(CUTOVER_OWNER_MISMATCH.to_owned());
        }
        crate::catalog_presence::require_existing(root)?;
        check_rollback(root)?;
        let snapshot = self.inner.verify_migration_snapshot(deadline)?;
        check_deadline(Some(deadline))?;
        Ok(format!(
            concat!(
                "{{\"event\":\"control_cutover_rollback\",\"schema\":\"eliot.control-cutover.v1\",",
                "\"marker_present\":false,\"action\":\"noop-verified-file-snapshot\",",
                "\"catalog_snapshot_sha256\":\"{}\",\"cutover_authorized\":false,",
                "\"primary_authority\":\"file-journal\"}}"
            ),
            sha256::hex(&snapshot),
        ))
    }

    /// Read-only cutover status for polling. Zero writes: safe to call at any
    /// rate, including the 10,000-read durability probe.
    ///
    /// The serve/CLI wiring lands with the integration owner; until then this
    /// entrypoint is exercised through its unit-tested status path.
    #[allow(dead_code)]
    pub(crate) fn inspect_control_cutover_status(&self) -> String {
        cutover_status_json(&self.root)
    }
}

/// Deterministic cutover receipt. No timestamps: an identical replay renders
/// the identical marker section and differs only in the `replayed` flag.
fn cutover_receipt(marker: &CutoverMarker, staged: &StagedPlan, replayed: bool) -> String {
    let locator = ["control/", marker.database_file_name.as_str()].concat();
    format!(
        concat!(
            "{{\"event\":\"control_cutover_committed\",\"schema\":\"eliot.control-cutover.v1\",",
            "\"target_namespace_id\":\"{}\",\"catalog_snapshot_sha256\":\"{}\",",
            "\"plan_chain_sha256\":\"{}\",\"content_manifest_chain_sha256\":\"{}\",",
            "\"marker_locator\":\"control/{}\",\"marker_digest\":\"{}\",",
            "\"staged_database_locator\":{},\"staged_database_schema\":{},",
            "\"staged_database_reused\":{},\"installation_incarnation_id\":\"{}\",",
            "\"data_root_id\":\"{}\",\"owner_epoch\":{},\"replayed\":{},",
            "\"cutover_authorized\":true,\"file_journal_preserved\":true,",
            "\"primary_authority\":\"redb-control-marker-v1\",",
            "\"serve_path_reroute\":\"pending-integration-wiring\"}}"
        ),
        marker.target,
        sha256::hex(&marker.catalog_snapshot),
        sha256::hex(&marker.plan_chain),
        sha256::hex(&marker.content_chain),
        CUTOVER_MARKER_FILE,
        sha256::hex(&marker.record_digest()),
        json_string(&locator),
        json_string(&marker.database_schema),
        staged.database_reused,
        marker.incarnation,
        marker.root,
        marker.epoch,
        replayed,
    )
}

/// Arm quarantine and report `code`. An arming failure is reported instead so
/// a failed fail-closed never masquerades as the original error.
fn quarantined(data_root: &Path, code: &str) -> String {
    if crate::catalog_quarantine::arm(data_root).is_err() {
        return crate::catalog_quarantine::QUARANTINE_ARM_FAILED.to_owned();
    }
    code.to_owned()
}

fn canonical_digest(text: &str) -> Result<[u8; 32], String> {
    let invalid = || "DIRECT_MIGRATION_CUTOVER_CORRUPT".to_owned();
    let digest = sha256::decode_digest(text).ok_or_else(invalid)?;
    if sha256::hex(&digest) != text {
        return Err(invalid());
    }
    Ok(digest)
}

fn push_line(output: &mut String, line: &str) {
    output.push_str(line);
    output.push('\n');
}

fn push_field(output: &mut String, key: &str, value: &str) {
    output.push_str(key);
    output.push('=');
    output.push_str(value);
    output.push('\n');
}

fn regular(metadata: &fs::Metadata) -> bool {
    metadata.is_file() && !is_link(metadata)
}

fn is_link(metadata: &fs::Metadata) -> bool {
    #[cfg(windows)]
    {
        use std::os::windows::fs::MetadataExt;
        metadata.file_type().is_symlink() || metadata.file_attributes() & 0x400 != 0
    }
    #[cfg(not(windows))]
    {
        metadata.file_type().is_symlink()
    }
}

#[cfg(unix)]
fn sync_directory(path: &Path) {
    let _ = fs::File::open(path).and_then(|file| file.sync_all());
}

#[cfg(not(unix))]
const fn sync_directory(_path: &Path) {}

#[cfg(test)]
mod tests {
    use super::*;

    fn owner_triple() -> (InstallationIncarnationId, DataRootId, u64) {
        (
            InstallationIncarnationId::from_bytes([0x11; 16]),
            DataRootId::from_bytes([0x22; 16]),
            3,
        )
    }

    fn sample_marker() -> CutoverMarker {
        let (incarnation, root, epoch) = owner_triple();
        CutoverMarker {
            target: SourceNamespaceId::parse("123e4567-e89b-12d3-a456-426614174000").unwrap(),
            incarnation,
            root,
            epoch,
            catalog_snapshot: [0x33; 32],
            plan_chain: [0x44; 32],
            content_chain: [0x55; 32],
            database_file_name: format!("{}.source-map.v2.redb", "66".repeat(32)),
            database_schema: STAGED_DATABASE_SCHEMA.to_owned(),
        }
    }

    struct Scratch {
        root: std::path::PathBuf,
    }

    impl Scratch {
        fn new() -> Self {
            use std::sync::atomic::{AtomicU64, Ordering};
            use std::time::{SystemTime, UNIX_EPOCH};
            static NEXT: AtomicU64 = AtomicU64::new(0);
            let stamp = SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .unwrap()
                .as_nanos();
            let root = std::env::temp_dir().join(format!(
                "eliot-cutover-{}-{stamp}-{}",
                std::process::id(),
                NEXT.fetch_add(1, Ordering::Relaxed),
            ));
            std::fs::create_dir_all(root.join("control")).unwrap();
            Self { root }
        }

        fn control(&self) -> std::path::PathBuf {
            self.root.join("control")
        }
    }

    impl Drop for Scratch {
        fn drop(&mut self) {
            let _ = std::fs::remove_dir_all(&self.root);
        }
    }

    #[test]
    fn marker_encode_is_deterministic_and_decodes_exactly() {
        let marker = sample_marker();
        let first = marker.encode();
        let second = sample_marker().encode();
        assert_eq!(first, second, "no timestamps or randomness in marker");
        let decoded = CutoverMarker::decode(&first).unwrap();
        assert_eq!(decoded, marker);
        assert_eq!(decoded.record_digest(), marker.record_digest());
    }

    #[test]
    fn marker_decode_rejects_non_canonical_bytes() {
        let canonical = sample_marker().encode();
        let text = String::from_utf8(canonical.clone()).unwrap();
        assert_eq!(text.lines().count(), 12, "exact marker shape");
        let mut cases: Vec<Vec<u8>> = Vec::new();
        // Truncated body.
        cases.push(canonical[..canonical.len() - 1].to_vec());
        // Truncated magic.
        cases.push(text.replacen("CUTOVER", "CUT", 1).into_bytes());
        // Duplicate field.
        cases.push(format!("{text}owner_epoch=3\n").into_bytes());
        // Unknown field.
        cases.push(
            text.replacen("owner_epoch=", "owner_epoch_x=", 1)
                .into_bytes(),
        );
        // Uppercase digest is never canonical.
        let digest_line = text
            .lines()
            .find(|line| line.starts_with("record_digest="))
            .unwrap();
        cases.push(
            text.replace(digest_line, &digest_line.to_uppercase())
                .into_bytes(),
        );
        // Leading-zero epoch is not canonical.
        cases.push(
            text.replacen("owner_epoch=3\n", "owner_epoch=03\n", 1)
                .into_bytes(),
        );
        // Zero epoch is refused.
        cases.push(
            text.replacen("owner_epoch=3\n", "owner_epoch=0\n", 1)
                .into_bytes(),
        );
        // Wrong schema relabel is refused.
        cases.push(
            text.replacen("source-map-content-v2", "source-map-content-v9", 1)
                .into_bytes(),
        );
        // Flipped body byte breaks the record digest.
        let mut tampered = canonical;
        let position = tampered.iter().position(|byte| *byte == b'=').unwrap() + 1;
        tampered[position] = if tampered[position] == b'0' {
            b'1'
        } else {
            b'0'
        };
        cases.push(tampered);
        // Foreign locator escapes the admitted control directory.
        cases.push(text.replacen("control/", "control/../", 1).into_bytes());
        for (index, case) in cases.iter().enumerate() {
            assert!(
                CutoverMarker::decode(case).is_err(),
                "case {index} must be rejected"
            );
        }
    }

    #[test]
    fn replay_decisions_separate_identical_diverged_and_foreign_owner() {
        let committed = sample_marker();
        assert_eq!(
            check_replay(&committed, &sample_marker()),
            ReplayDecision::Identical
        );
        let mut diverged = sample_marker();
        diverged.catalog_snapshot = [0x77; 32];
        assert_eq!(
            check_replay(&committed, &diverged),
            ReplayDecision::Diverged
        );
        let mut foreign = sample_marker();
        foreign.root = DataRootId::from_bytes([0x99; 16]);
        assert_eq!(
            check_replay(&committed, &foreign),
            ReplayDecision::ForeignOwner
        );
        let mut foreign_epoch = sample_marker();
        foreign_epoch.epoch = 4;
        // A successor epoch under the same installation/root replays the exact
        // operation after a crash; only the committing epoch is historical.
        assert_eq!(
            check_replay(&committed, &foreign_epoch),
            ReplayDecision::Identical
        );
    }

    #[test]
    fn publish_then_lost_ack_retry_is_identical() {
        let scratch = Scratch::new();
        let marker = sample_marker();
        let bytes = marker.encode();
        assert_eq!(
            publish_marker(&scratch.root, &bytes).unwrap(),
            PublishOutcome::Committed
        );
        // A lost acknowledgement retries the exact operation once only.
        assert_eq!(
            publish_marker(&scratch.root, &bytes).unwrap(),
            PublishOutcome::ReplayIdentical
        );
        assert_eq!(
            std::fs::read(scratch.control().join(CUTOVER_MARKER_FILE)).unwrap(),
            bytes
        );
        assert!(
            !scratch.control().join(CUTOVER_MARKER_TMP).exists(),
            "no tmp residue"
        );
    }

    #[test]
    fn truncated_marker_is_corrupt_never_partial_authority() {
        let scratch = Scratch::new();
        let bytes = sample_marker().encode();
        std::fs::write(
            scratch.control().join(CUTOVER_MARKER_FILE),
            &bytes[..bytes.len() / 2],
        )
        .unwrap();
        assert_eq!(resolve_marker(&scratch.root), MarkerState::Corrupt);
        let status = cutover_status_json(&scratch.root);
        assert!(
            status.contains("\"authority\":\"marker-corrupt\""),
            "{status}"
        );
        assert!(status.contains("\"complete\":false"), "{status}");
    }

    #[test]
    fn gate_absent_marker_allows_staging() {
        let scratch = Scratch::new();
        let target = SourceNamespaceId::parse("123e4567-e89b-12d3-a456-426614174000").unwrap();
        assert!(gate_staging_against_marker(&scratch.root, target, [0x33; 32]).is_ok());
        assert!(!scratch.control().join("catalog-quarantine.marker").exists());
    }

    #[test]
    fn gate_matching_marker_allows_identical_replan() {
        let scratch = Scratch::new();
        let marker = sample_marker();
        std::fs::write(scratch.control().join(CUTOVER_MARKER_FILE), marker.encode()).unwrap();
        assert!(
            gate_staging_against_marker(&scratch.root, marker.target, marker.catalog_snapshot)
                .is_ok()
        );
    }

    #[test]
    fn gate_diverged_marker_supersedes_without_quarantine() {
        let scratch = Scratch::new();
        let marker = sample_marker();
        std::fs::write(scratch.control().join(CUTOVER_MARKER_FILE), marker.encode()).unwrap();
        let target = SourceNamespaceId::parse("123e4567-e89b-12d3-a456-426614174000").unwrap();
        assert_eq!(
            gate_staging_against_marker(&scratch.root, target, [0x78; 32]),
            Err("DIRECT_MIGRATION_CUTOVER_SUPERSEDED".to_owned())
        );
        assert!(!scratch.control().join("catalog-quarantine.marker").exists());
    }

    #[test]
    fn gate_corrupt_marker_quarantines_and_preserves_bytes() {
        let scratch = Scratch::new();
        std::fs::write(
            scratch.control().join(CUTOVER_MARKER_FILE),
            b"corrupt-and-preserved",
        )
        .unwrap();
        let target = SourceNamespaceId::parse("123e4567-e89b-12d3-a456-426614174000").unwrap();
        assert_eq!(
            gate_staging_against_marker(&scratch.root, target, [0x33; 32]),
            Err("DIRECT_MIGRATION_CUTOVER_CORRUPT".to_owned())
        );
        assert!(scratch.control().join("catalog-quarantine.marker").exists());
        assert_eq!(
            std::fs::read(scratch.control().join(CUTOVER_MARKER_FILE)).unwrap(),
            b"corrupt-and-preserved"
        );
    }

    #[test]
    fn status_without_marker_reports_file_journal_authority() {
        let scratch = Scratch::new();
        let status = cutover_status_json(&scratch.root);
        assert!(
            status.contains("\"authority\":\"file-journal\""),
            "{status}"
        );
        assert!(status.contains("\"marker_present\":false"), "{status}");
        assert!(status.contains("\"read_only\":true"), "{status}");
    }

    #[test]
    fn status_is_read_only_across_ten_thousand_reads() {
        let scratch = Scratch::new();
        let marker = sample_marker();
        std::fs::write(scratch.control().join(CUTOVER_MARKER_FILE), marker.encode()).unwrap();
        std::fs::write(
            scratch.control().join(&marker.database_file_name),
            b"staged-redb-stand-in",
        )
        .unwrap();
        let mut before: Vec<(String, std::time::SystemTime)> = std::fs::read_dir(scratch.control())
            .unwrap()
            .filter_map(Result::ok)
            .map(|entry| {
                let name = entry.file_name().into_string().unwrap();
                let mtime = std::fs::symlink_metadata(scratch.control().join(&name))
                    .unwrap()
                    .modified()
                    .unwrap();
                (name, mtime)
            })
            .collect();
        before.sort();
        let first = cutover_status_json(&scratch.root);
        assert!(
            first.contains("\"authority\":\"redb-control-marker-v1\""),
            "{first}"
        );
        assert!(first.contains("\"complete\":true"), "{first}");
        for _ in 0..10_000 {
            assert_eq!(cutover_status_json(&scratch.root), first);
        }
        let mut after: Vec<(String, std::time::SystemTime)> = std::fs::read_dir(scratch.control())
            .unwrap()
            .filter_map(Result::ok)
            .map(|entry| {
                let name = entry.file_name().into_string().unwrap();
                let mtime = std::fs::symlink_metadata(scratch.control().join(&name))
                    .unwrap()
                    .modified()
                    .unwrap();
                (name, mtime)
            })
            .collect();
        after.sort();
        assert_eq!(before, after, "status reads write nothing durable");
    }

    #[test]
    fn rollback_without_marker_is_a_verified_noop() {
        let scratch = Scratch::new();
        assert_eq!(check_rollback(&scratch.root).unwrap(), RollbackAction::Noop);
        assert!(!scratch.control().join("catalog-quarantine.marker").exists());
    }

    #[test]
    fn rollback_refuses_a_committed_marker() {
        let scratch = Scratch::new();
        std::fs::write(
            scratch.control().join(CUTOVER_MARKER_FILE),
            sample_marker().encode(),
        )
        .unwrap();
        assert_eq!(
            check_rollback(&scratch.root),
            Err("DIRECT_MIGRATION_CUTOVER_ALREADY_COMMITTED".to_owned())
        );
    }

    #[test]
    fn rollback_on_corrupt_marker_quarantines() {
        let scratch = Scratch::new();
        std::fs::write(scratch.control().join(CUTOVER_MARKER_FILE), b"torn").unwrap();
        assert_eq!(
            check_rollback(&scratch.root),
            Err("DIRECT_MIGRATION_CUTOVER_CORRUPT".to_owned())
        );
        assert!(scratch.control().join("catalog-quarantine.marker").exists());
    }
}

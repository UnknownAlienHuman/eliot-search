//! Live-owner orchestration for source-control cutover.

use std::time::{Duration, Instant};

use search_contracts::SourceNamespaceId;
use search_control_redb::migration::{
    CONTROL_CUTOVER_MARKER_FILE as CUTOVER_MARKER_FILE,
    CONTROL_CUTOVER_STAGED_DATABASE_SCHEMA as STAGED_DATABASE_SCHEMA,
    ControlCutoverMarker as CutoverMarker,
    ControlCutoverReplayDecision as ReplayDecision,
    classify_control_cutover_replay as check_replay,
};

use super::marker_io::{
    CUTOVER_ALREADY_COMMITTED, CUTOVER_CORRUPT,
    CUTOVER_READBACK_MISMATCH, MarkerState, PublishOutcome, check_rollback,
    publish_marker, quarantined, resolve_marker,
};
use super::status::cutover_status_json;
use super::super::{DirectStore, StagedPlan, check_deadline, json_string, sha256};
use crate::development::DataRootGuard;

const CUTOVER_DEADLINE: Duration = Duration::from_secs(120);
const CUTOVER_OWNER_MISMATCH: &str = "DIRECT_MIGRATION_CUTOVER_OWNER_MISMATCH";

impl DirectStore {
    /// Atomically cuts the primary catalog over to the verified redb mapping.
    ///
    /// Under the live owner guard: exact readback of the file journal before
    /// staging, deterministic staging into `control/`, exact readback after,
    /// then one marker publication. Any mismatch quarantines instead of
    /// proceeding silently; an ambiguous publish reports `OUTCOME_UNKNOWN`.
    /// The legacy file journal is preserved untouched as evidence.
    ///
    /// Serve-path query/mutation rerouting consumes the committed marker in a
    /// follow-up wiring step; the receipt states that limitation explicitly.
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

        let snapshot = self.inner.verify_migration_snapshot(deadline)?;
        let header = self.inner.source_mapping_header(target)?;
        if header.catalog_snapshot != snapshot {
            return Err("DIRECT_CONTROL_READBACK_MISMATCH".to_owned());
        }
        let (incarnation, data_root_id, epoch) = owner.journal_owner_inputs();

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

    /// Performs explicit rollback before cutover.
    ///
    /// The operation succeeds only when no marker exists and binds the live
    /// file snapshot into a no-op receipt. A committed marker is refused and a
    /// torn marker quarantines; neither is deleted here.
    #[allow(dead_code)]
    pub(crate) fn rollback_control_cutover(
        &self,
        owner: &DataRootGuard,
    ) -> Result<String, String> {
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

    /// Returns read-only cutover status suitable for high-frequency polling.
    #[allow(dead_code)]
    pub(crate) fn inspect_control_cutover_status(&self) -> String {
        cutover_status_json(&self.root)
    }
}

fn cutover_receipt(
    marker: &CutoverMarker,
    staged: &StagedPlan,
    replayed: bool,
) -> String {
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

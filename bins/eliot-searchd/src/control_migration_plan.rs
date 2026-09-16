//! Stage a reusable imported-source mapping artifact, not a replacement authority.
//! Canonical IDs/occurrences are compiled once per pass through the existing replay.
//! No original file, policy, namespace owner or visible source state is modified.

use std::path::Path;
use std::time::{Duration, Instant, SystemTime, UNIX_EPOCH};

use search_contracts::SourceNamespaceId;
use search_control_redb::migration::{
    SourceImportRecordArtifact, SourceImportRecordArtifactError,
    inspect_source_import_record_artifact,
};

use super::super::storage_io::{
    ensure_child_directory, ensure_directory, sync_directory,
};
use super::{DirectStore, check_deadline, json_string, sha256};
use crate::development::DataRootGuard;

#[path = "control_migration_redb.rs"]
mod redb_import;
#[path = "control_migration_content.rs"]
mod content_readback;
#[path = "control_migration_cutover.rs"]
mod cutover;

const PLAN_DEADLINE: Duration = Duration::from_secs(120);

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
        &self,
        owner: &DataRootGuard,
        target: SourceNamespaceId,
    ) -> Result<String, String> {
        let deadline = Instant::now()
            .checked_add(PLAN_DEADLINE)
            .ok_or_else(|| "DIRECT_MIGRATION_DEADLINE_EXCEEDED".to_owned())?;
        if owner.canonical_root() != self.root.as_path() {
            return Err("DIRECT_MIGRATION_ROOT_OWNER_MISMATCH".to_owned());
        }
        crate::catalog_presence::require_existing(&self.root)?;
        let header = self.inner.source_mapping_header(target)?;
        if self.inner.verify_migration_snapshot(deadline)?
            != header.catalog_snapshot
        {
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
        Self::stage_mapping_artifact(
            &self.inner,
            &self.root,
            target,
            &directory,
            "control/migration-plans/",
            deadline,
        )
    }

    /// Shared artifact writer for the live owner and offline entrypoint. The caller
    /// holds `source_root`'s ordinary lock and opened source from that exact root.
    /// No source store is initialized. Payload readback resolves existing Windows
    /// credentials only; no credential or source object is created or converted.
    /// A returned locator is relative to the explicitly named location scope.
    pub(crate) fn stage_mapping_artifact(
        source: &crate::plaintext_direct_store::DirectStore,
        source_root: &Path,
        target: SourceNamespaceId,
        directory: &Path,
        locator_prefix: &'static str,
        deadline: Instant,
    ) -> Result<String, String> {
        let plan = Self::stage_mapping_artifact_typed(
            source,
            source_root,
            target,
            directory,
            locator_prefix,
            deadline,
        )?;
        Ok(staged_plan_json(&plan))
    }

    /// Typed staging pass shared by the verify-only report and the atomic
    /// cutover. Artifact I/O and exact second-pass comparison are owned by
    /// `search-control-redb`; this composition root supplies source replay.
    pub(super) fn stage_mapping_artifact_typed(
        source: &crate::plaintext_direct_store::DirectStore,
        source_root: &Path,
        target: SourceNamespaceId,
        directory: &Path,
        locator_prefix: &'static str,
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
        if source.verify_migration_snapshot(deadline)?
            != header.catalog_snapshot
        {
            return Err("DIRECT_CONTROL_READBACK_MISMATCH".to_owned());
        }
        cutover::gate_staging_against_marker(
            source_root,
            target,
            header.catalog_snapshot,
        )?;

        let temporary_name = temporary_staging_name()?;
        let mut artifact = SourceImportRecordArtifact::create(
            directory,
            &temporary_name,
            redb_import::DaemonImportOutputPlatform,
            deadline,
        )
        .map_err(plan_artifact_reason)?;
        let summary = source.compile_source_mapping(target, deadline, |row| {
            artifact
                .push(row, deadline)
                .map_err(plan_artifact_reason)
        })?;
        let frozen = artifact.freeze(deadline).map_err(plan_artifact_reason)?;
        let digest = *frozen.chain();
        let bytes = frozen.encoded_bytes();

        let mut readback = frozen
            .begin_readback(deadline)
            .map_err(plan_artifact_reason)?;
        let replayed = source.compile_source_mapping(target, deadline, |row| {
            readback
                .compare(row, deadline)
                .map_err(plan_artifact_reason)
        })?;
        if replayed != summary {
            return Err("DIRECT_MIGRATION_PLAN_READBACK_MISMATCH".to_owned());
        }
        let verified = readback.finish(deadline).map_err(plan_artifact_reason)?;
        let name = format!("{}.source-map.v1", sha256::hex(&digest));
        let published = verified
            .publish(&name, deadline)
            .map_err(plan_artifact_reason)?;
        if published.chain() != &digest
            || published.encoded_bytes() != bytes
            || published.name() != name
        {
            return Err("DIRECT_MIGRATION_PLAN_READBACK_MISMATCH".to_owned());
        }
        let reused = published.reused();

        let content = content_readback::stage(
            source,
            source_root,
            target,
            digest,
            directory,
            deadline,
        )?;
        let (database_name, database_reused) = redb_import::store(
            source,
            target,
            directory,
            digest,
            summary.import_counts(),
            &content,
            deadline,
        )?;
        if source.verify_migration_snapshot(deadline)?
            != header.catalog_snapshot
        {
            return Err("DIRECT_CONTROL_READBACK_MISMATCH".to_owned());
        }
        if verify_record_artifact(
            &directory.join(&name),
            bytes,
            deadline,
        )? != digest
            || verify_record_artifact(
                &directory.join(&content.name),
                content.encoded_bytes,
                deadline,
            )? != content.chain
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
    format!(
        concat!(
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
        ),
        plan.target,
        sha256::hex(&plan.catalog_snapshot),
        json_string(&format!(
            "{}{}",
            plan.locator_prefix, plan.plan_name
        )),
        sha256::hex(&plan.plan_chain),
        plan.plan_bytes,
        plan.plan_reused,
        plan.events,
        plan.sources,
        plan.occurrences,
        plan.path_only_events,
        plan.retirements,
        plan.location,
        json_string(&format!(
            "{}{}",
            plan.locator_prefix, plan.database_name
        )),
        plan.database_reused,
        json_string(&format!(
            "{}{}",
            plan.locator_prefix, plan.content_name
        )),
        sha256::hex(&plan.content_chain),
        plan.content_records,
        plan.content_source_bytes
    )
}

pub(super) fn temporary_staging_name() -> Result<String, String> {
    let stamp = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map_err(|_| "DIRECT_MIGRATION_CLOCK_INVALID".to_owned())?
        .as_nanos();
    Ok(format!(
        ".source-map.{}.{stamp}.tmp",
        std::process::id()
    ))
}

pub(super) fn verify_record_artifact(
    path: &Path,
    encoded_bytes: u64,
    deadline: Instant,
) -> Result<[u8; 32], String> {
    let observed = inspect_source_import_record_artifact(
        &redb_import::DaemonImportOutputPlatform,
        path,
        encoded_bytes,
        deadline,
    )
    .map_err(plan_artifact_reason)?;
    Ok(*observed.chain())
}

fn plan_artifact_reason(
    error: SourceImportRecordArtifactError<String>,
) -> String {
    match error {
        SourceImportRecordArtifactError::Platform(reason) => reason,
        SourceImportRecordArtifactError::DeadlineExceeded => {
            "DIRECT_MIGRATION_DEADLINE_EXCEEDED".to_owned()
        }
        SourceImportRecordArtifactError::TemporaryNameInvalid
        | SourceImportRecordArtifactError::CreateFailed => {
            "DIRECT_MIGRATION_PLAN_CREATE_FAILED".to_owned()
        }
        SourceImportRecordArtifactError::FinalNameInvalid
        | SourceImportRecordArtifactError::ObjectInvalid => {
            "DIRECT_MIGRATION_PLAN_OBJECT_INVALID".to_owned()
        }
        SourceImportRecordArtifactError::Closed => {
            "DIRECT_MIGRATION_PLAN_CLOSED".to_owned()
        }
        SourceImportRecordArtifactError::WriteFailed => {
            "DIRECT_MIGRATION_PLAN_WRITE_FAILED".to_owned()
        }
        SourceImportRecordArtifactError::SyncFailed => {
            "DIRECT_MIGRATION_PLAN_SYNC_FAILED".to_owned()
        }
        SourceImportRecordArtifactError::ReadFailed => {
            "DIRECT_MIGRATION_PLAN_READ_FAILED".to_owned()
        }
        SourceImportRecordArtifactError::ComparisonReadFailed => {
            "DIRECT_MIGRATION_PLAN_READBACK_FAILED".to_owned()
        }
        SourceImportRecordArtifactError::ReadbackMismatch => {
            "DIRECT_MIGRATION_PLAN_READBACK_MISMATCH".to_owned()
        }
        SourceImportRecordArtifactError::PublishOutcomeUnknown => {
            "DIRECT_MIGRATION_PLAN_PUBLISH_OUTCOME_UNKNOWN".to_owned()
        }
        SourceImportRecordArtifactError::ImmutableConflict => {
            "DIRECT_MIGRATION_PLAN_IMMUTABLE_CONFLICT".to_owned()
        }
        SourceImportRecordArtifactError::IdentityChanged => {
            "DIRECT_MIGRATION_PLAN_CLEANUP_IDENTITY_CHANGED".to_owned()
        }
        SourceImportRecordArtifactError::CleanupFailed => {
            "DIRECT_MIGRATION_PLAN_CLEANUP_FAILED".to_owned()
        }
        SourceImportRecordArtifactError::RecordChain(error) => {
            error.code().to_owned()
        }
    }
}

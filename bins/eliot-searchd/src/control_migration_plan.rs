//! Stage a reusable imported-source mapping artifact, not a replacement authority.
//! Canonical IDs/occurrences are compiled once per pass through the existing replay.
//! No original file, policy, namespace owner or visible source state is modified.

use std::path::Path;
use std::time::{Instant, SystemTime, UNIX_EPOCH};
#[cfg(test)]
use std::time::Duration;

use search_contracts::SourceNamespaceId;
use search_control_redb::migration::{
    SourceImportRecordArtifact, SourceImportRecordArtifactError,
    SourceMigrationPlanLocation, SourceMigrationStagedPlan,
    inspect_source_import_record_artifact,
};

use super::super::storage_io::{
    ensure_directory, sync_directory,
};
#[cfg(test)]
use super::super::storage_io::ensure_child_directory;
use super::{DirectStore, check_deadline, sha256};
use super::super::migration_context::{MigrationSource, NativeMigrationInput};
use search_revision_store::LegacyRevisionObjectWork as Work;
#[cfg(test)]
use crate::development::DataRootGuard;

#[path = "control_migration_redb.rs"]
mod redb_import;
#[path = "control_migration_content.rs"]
mod content_readback;
#[path = "control_migration_inventory.rs"]
mod inventory;
#[path = "control_migration_cutover.rs"]
mod cutover;

#[cfg(test)]
const PLAN_DEADLINE: Duration = Duration::from_secs(120);

impl DirectStore {
    /// Store a deterministic, content-free source mapping draft. The explicit UUID
    /// chooses the *import target*, not the original namespace or live NTFS identity.
    /// Same source history/target/profile gives the same file and IDs. No timestamps
    /// or random data enter its contents, so a lost acknowledgement can be retried.
    #[cfg(test)]
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
    #[cfg(test)]
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
        Ok(plan.render_json())
    }

    /// Typed staging pass shared by the verify-only report and the atomic
    /// cutover. Artifact I/O, exact second-pass comparison and the canonical
    /// receipt schema are owned by `search-control-redb`; this composition root
    /// supplies source replay and qualified filesystem observations.
    pub(super) fn stage_mapping_artifact_typed(
        source: &crate::plaintext_direct_store::DirectStore,
        source_root: &Path,
        target: SourceNamespaceId,
        directory: &Path,
        locator_prefix: &'static str,
        deadline: Instant,
    ) -> Result<SourceMigrationStagedPlan, String> {
        Self::stage_mapping_artifact_input(source, source_root, target, directory, locator_prefix, deadline, None)
    }

    pub(crate) fn stage_mapping_artifact_existing(
        source: &crate::plaintext_direct_store::DirectStore,
        source_root: &Path,
        target: SourceNamespaceId,
        directory: &Path,
        native: &NativeMigrationInput<'_, '_, '_>,
    ) -> Result<String, String> {
        Self::stage_mapping_artifact_input(
            source, source_root, target, directory, "", native.context.deadline(), Some(native),
        ).map(|plan| plan.render_json())
    }

    fn stage_mapping_artifact_input(
        source: &crate::plaintext_direct_store::DirectStore,
        source_root: &Path,
        target: SourceNamespaceId,
        directory: &Path,
        locator_prefix: &'static str,
        deadline: Instant,
        native: Option<&NativeMigrationInput<'_, '_, '_>>,
    ) -> Result<SourceMigrationStagedPlan, String> {
        let source = MigrationSource::new(source, deadline, native);
        let location = match locator_prefix {
            "" => SourceMigrationPlanLocation::ExplicitOutputDirectory,
            "control/migration-plans/" => {
                SourceMigrationPlanLocation::DataRootMigrationPlans
            }
            "control/" => SourceMigrationPlanLocation::DataRootControl,
            _ => return Err("DIRECT_MIGRATION_OUTPUT_INVALID".to_owned()),
        };
        source.checkpoint()?;
        source.charge(Work::Observe)?;
        ensure_directory(directory)?;
        source.checkpoint()?;
        let header = source.source_mapping_header(target)?;
        if source.verify_migration_snapshot(deadline)?
            != header.catalog_snapshot
        {
            return Err("DIRECT_CONTROL_READBACK_MISMATCH".to_owned());
        }
        source.charge(Work::Observe)?;
        if source.native().is_some() {
            gate_native_staging(source_root, target, header.catalog_snapshot)?;
        } else {
            cutover::gate_staging_against_marker(source_root, target, header.catalog_snapshot)?;
        }
        source.checkpoint()?;

        if let Some(input) = native {
            inventory::stage(&source, source_root, target, directory, input)?;
        }

        let temporary_name = temporary_staging_name()?;
        source.checkpoint()?;
        source.charge(Work::Open)?;
        let mut artifact = SourceImportRecordArtifact::create(
            directory,
            &temporary_name,
            redb_import::DaemonImportOutputPlatform,
            deadline,
        )
        .map_err(plan_artifact_reason)?;
        source.checkpoint()?;
        let summary = source.compile_source_mapping(target, deadline, |row| {
            source.charge(Work::Write(row.len()))?;
            artifact
                .push(row, deadline)
                .map_err(plan_artifact_reason)?;
            source.checkpoint()
        })?;
        source.checkpoint()?;
        let frozen = artifact.freeze(deadline).map_err(plan_artifact_reason)?;
        source.checkpoint()?;
        let digest = *frozen.chain();
        let bytes = frozen.encoded_bytes();

        let mut readback = frozen
            .begin_readback(deadline)
            .map_err(plan_artifact_reason)?;
        source.checkpoint()?;
        let replayed = source.compile_source_mapping(target, deadline, |row| {
            source.charge(Work::Compare(row.len()))?;
            readback
                .compare(row, deadline)
                .map_err(plan_artifact_reason)?;
            source.checkpoint()
        })?;
        if replayed != summary {
            return Err("DIRECT_MIGRATION_PLAN_READBACK_MISMATCH".to_owned());
        }
        source.checkpoint()?;
        let verified = readback.finish(deadline).map_err(plan_artifact_reason)?;
        source.checkpoint()?;
        let name = format!("{}.source-map.v1", sha256::hex(&digest));
        source.checkpoint()?;
        let published = verified
            .publish(&name, deadline)
            .map_err(plan_artifact_reason)?;
        source.checkpoint()?;
        if published.chain() != &digest
            || published.encoded_bytes() != bytes
            || published.name() != name
        {
            return Err("DIRECT_MIGRATION_PLAN_READBACK_MISMATCH".to_owned());
        }
        let reused = published.reused();

        let content = content_readback::stage(
            &source,
            source_root,
            target,
            digest,
            directory,
            deadline,
        )?;
        let (database_name, database_reused) = redb_import::store(
            &source,
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
        source.checkpoint()?;
        let plan_readback = verify_record_artifact(
            &directory.join(&name),
            bytes,
            deadline,
        )?;
        source.checkpoint()?;
        let content_readback = verify_record_artifact(
            &directory.join(&content.name),
            content.encoded_bytes,
            deadline,
        )?;
        source.checkpoint()?;
        if plan_readback != digest || content_readback != content.chain {
            return Err("DIRECT_MIGRATION_PLAN_READBACK_MISMATCH".to_owned());
        }
        source.checkpoint()?;
        Ok(SourceMigrationStagedPlan {
            target,
            catalog_snapshot: header.catalog_snapshot,
            plan_name: name,
            plan_chain: digest,
            plan_bytes: bytes,
            plan_reused: reused,
            summary,
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

fn gate_native_staging(root: &Path, target: SourceNamespaceId, snapshot: [u8; 32]) -> Result<(), String> {
    use search_control_redb::migration::{ControlCutoverMarkerFileState as State, resolve_control_cutover_marker};
    match resolve_control_cutover_marker(&redb_import::DaemonImportOutputPlatform, root) {
        State::Absent => Ok(()),
        State::Valid(committed) if committed.marker.target == target && committed.marker.catalog_snapshot == snapshot => Ok(()),
        State::Valid(_) => Err("DIRECT_MIGRATION_CUTOVER_SUPERSEDED".to_owned()),
        State::Corrupt => Err("DIRECT_MIGRATION_CUTOVER_CORRUPT".to_owned()),
    }
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

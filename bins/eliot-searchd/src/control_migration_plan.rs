//! Stage a reusable imported-source mapping artifact, not a replacement authority.
//! Canonical IDs/occurrences are compiled once per pass through the existing replay.
//! No original file, policy, namespace owner or visible source state is modified.

use std::path::Path;
use std::time::{Instant, SystemTime, UNIX_EPOCH};

use search_contracts::SourceNamespaceId;
use search_control_redb::migration::{
    SourceImportRecordArtifact, SourceImportRecordArtifactError, SourceMigrationPlanLocation,
    SourceMigrationStagedPlan, inspect_source_import_record_artifact,
};

use super::super::storage_io::{ensure_child_directory, ensure_directory, sync_directory};
use super::{DirectStore, check_deadline, sha256};
use crate::development::{DataRootGuard, InspectedDataRoot};

#[path = "control_migration_content.rs"]
mod content_readback;
#[path = "control_migration_cutover.rs"]
mod cutover;
#[path = "control_migration_redb.rs"]
mod redb_import;

/// Borrows an already-admitted root; paths cannot construct migration authority.
pub(super) enum MigrationRoot<'a> {
    /// Source-preserving inspection; failures must not arm source quarantine.
    Inspected(&'a InspectedDataRoot),
    /// Existing mutation under the single live native owner.
    Owned(&'a DataRootGuard),
}

impl MigrationRoot<'_> {
    fn canonical_root(&self) -> &Path {
        match self {
            Self::Inspected(cap) => cap.canonical_root(),
            Self::Owned(owner) => owner.canonical_root(),
        }
    }

    fn gate_staging(&self, target: SourceNamespaceId, snapshot: [u8; 32]) -> Result<(), String> {
        match self {
            Self::Inspected(cap) => cutover::inspect_staging_against_marker(cap, target, snapshot),
            Self::Owned(owner) => cutover::gate_staging_owned(owner, target, snapshot),
        }
    }
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
        deadline: Instant,
    ) -> Result<String, String> {
        self.check_operation()?;
        let deadline = self
            .operation_deadline()?
            .map_or(deadline, |original| original.min(deadline));
        let check = || {
            self.check_operation()?;
            check_deadline(Some(deadline))
        };
        let owner_verified = owner.verify_existing();
        check()?;
        owner_verified?;
        if owner.canonical_root() != self.root.as_path() {
            return Err("DIRECT_MIGRATION_ROOT_OWNER_MISMATCH".to_owned());
        }
        let existing = crate::catalog_presence::require_existing(&self.root);
        check()?;
        existing?;
        let header = self.inner.source_mapping_header(target)?;
        let snapshot = self.inner.verify_migration_snapshot(deadline);
        check()?;
        if snapshot? != header.catalog_snapshot {
            return Err("DIRECT_CONTROL_READBACK_MISMATCH".to_owned());
        }
        let control = self.root.join("control");
        let control_directory = ensure_directory(&control);
        check()?;
        control_directory?;
        let directory = control.join("migration-plans");
        let plans_directory = ensure_child_directory(&directory);
        check()?;
        plans_directory?;
        #[cfg(unix)]
        let synced = sync_directory(&control);
        #[cfg(not(unix))]
        sync_directory(&control);
        check()?;
        #[cfg(unix)]
        synced?;
        let staged = Self::stage_mapping_artifact(
            &self.inner,
            MigrationRoot::Owned(owner),
            target,
            &directory,
            "control/migration-plans/",
            deadline,
        );
        check()?;
        staged
    }

    /// Stages into the caller's separate existing output directory.
    ///
    /// Source admission stays borrowed for migration-grade replay and readback.
    /// Target UUID, source snapshot, record/content chains and output-artifact
    /// verification remain the existing input proof; none is recovery authority.
    pub(crate) fn stage_mapping_artifact_inspected(
        cap: &InspectedDataRoot,
        target: SourceNamespaceId,
        directory: &Path,
        deadline: Instant,
    ) -> Result<String, String> {
        let deadline = deadline.min(cap.operation_request()?.deadline());
        crate::plaintext_direct_store::DirectStore::with_existing_mapping_source(
            cap.canonical_root(),
            deadline,
            cap.operation_request()?,
            |source| {
                Self::stage_mapping_artifact(
                    source,
                    MigrationRoot::Inspected(cap),
                    target,
                    directory,
                    "",
                    deadline,
                )
            },
        )
    }

    /// Shared artifact writer for the live owner and offline entrypoint. The caller
    /// borrows typed source admission and opened source from that exact root.
    /// No source store is initialized. Payload readback resolves existing Windows
    /// credentials only; no credential or source object is created or converted.
    /// A returned locator is relative to the explicitly named location scope.
    fn stage_mapping_artifact(
        source: &crate::plaintext_direct_store::DirectStore,
        root: MigrationRoot<'_>,
        target: SourceNamespaceId,
        directory: &Path,
        locator_prefix: &'static str,
        deadline: Instant,
    ) -> Result<String, String> {
        source.check_operation()?;
        let plan = Self::stage_mapping_artifact_typed(
            source,
            root,
            target,
            directory,
            locator_prefix,
            deadline,
        );
        source.check_operation()?;
        let output = plan?.render_json();
        source.check_operation()?;
        check_deadline(Some(deadline))?;
        Ok(output)
    }

    /// Typed staging pass shared by the verify-only report and the atomic
    /// cutover. Artifact I/O, exact second-pass comparison and the canonical
    /// receipt schema are owned by `search-control-redb`; this composition root
    /// supplies source replay and qualified filesystem observations.
    pub(super) fn stage_mapping_artifact_typed(
        source: &crate::plaintext_direct_store::DirectStore,
        root: MigrationRoot<'_>,
        target: SourceNamespaceId,
        directory: &Path,
        locator_prefix: &'static str,
        deadline: Instant,
    ) -> Result<SourceMigrationStagedPlan, String> {
        source.check_operation()?;
        let deadline = match source.operation_request() {
            Ok(request) => deadline.min(request.deadline()),
            #[cfg(test)]
            Err(_) => deadline,
            #[cfg(not(test))]
            Err(error) => return Err(error),
        };
        let check = || {
            source.check_operation()?;
            check_deadline(Some(deadline))
        };
        check()?;
        let source_root = root.canonical_root();
        let location = match locator_prefix {
            "" => SourceMigrationPlanLocation::ExplicitOutputDirectory,
            "control/migration-plans/" => SourceMigrationPlanLocation::DataRootMigrationPlans,
            "control/" => SourceMigrationPlanLocation::DataRootControl,
            _ => return Err("DIRECT_MIGRATION_OUTPUT_INVALID".to_owned()),
        };
        let output_directory = ensure_directory(directory);
        check()?;
        output_directory?;
        let header = source.source_mapping_header(target)?;
        let snapshot = source.verify_migration_snapshot(deadline);
        check()?;
        if snapshot? != header.catalog_snapshot {
            return Err("DIRECT_CONTROL_READBACK_MISMATCH".to_owned());
        }
        let staging_gate = root.gate_staging(target, header.catalog_snapshot);
        check()?;
        staging_gate?;

        let temporary_name = temporary_staging_name();
        check()?;
        let temporary_name = temporary_name?;
        let created = SourceImportRecordArtifact::create(
            directory,
            &temporary_name,
            redb_import::DaemonImportOutputPlatform,
            deadline,
        );
        check()?;
        let mut artifact = created.map_err(plan_artifact_reason)?;
        let summary = source.compile_source_mapping(target, deadline, |row| {
            check()?;
            let pushed = artifact.push(row, deadline);
            check()?;
            pushed.map_err(plan_artifact_reason)
        });
        check()?;
        let summary = summary?;
        let frozen = artifact.freeze(deadline);
        check()?;
        let frozen = frozen.map_err(plan_artifact_reason)?;
        let digest = *frozen.chain();
        let bytes = frozen.encoded_bytes();

        let readback = frozen.begin_readback(deadline);
        check()?;
        let mut readback = readback.map_err(plan_artifact_reason)?;
        let replayed = source.compile_source_mapping(target, deadline, |row| {
            check()?;
            let compared = readback.compare(row, deadline);
            check()?;
            compared.map_err(plan_artifact_reason)
        });
        check()?;
        let replayed = replayed?;
        if replayed != summary {
            return Err("DIRECT_MIGRATION_PLAN_READBACK_MISMATCH".to_owned());
        }
        let verified = readback.finish(deadline);
        check()?;
        let verified = verified.map_err(plan_artifact_reason)?;
        let name = format!("{}.source-map.v1", sha256::hex(&digest));
        check()?;
        let published = verified.publish(&name, deadline);
        check()?;
        let published = published.map_err(plan_artifact_reason)?;
        if published.chain() != &digest
            || published.encoded_bytes() != bytes
            || published.name() != name
        {
            return Err("DIRECT_MIGRATION_PLAN_READBACK_MISMATCH".to_owned());
        }
        let reused = published.reused();

        let content =
            content_readback::stage(source, source_root, target, digest, directory, deadline);
        check()?;
        let content = content?;
        let database = redb_import::store(
            source,
            target,
            directory,
            digest,
            summary.import_counts(),
            &content,
            deadline,
        );
        check()?;
        let (database_name, database_reused) = database?;
        let snapshot = source.verify_migration_snapshot(deadline);
        check()?;
        if snapshot? != header.catalog_snapshot {
            return Err("DIRECT_CONTROL_READBACK_MISMATCH".to_owned());
        }
        if verify_record_artifact(&directory.join(&name), bytes, deadline, &check)? != digest
            || verify_record_artifact(
                &directory.join(&content.name),
                content.encoded_bytes,
                deadline,
                &check,
            )? != content.chain
        {
            return Err("DIRECT_MIGRATION_PLAN_READBACK_MISMATCH".to_owned());
        }
        check()?;
        let output = SourceMigrationStagedPlan {
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
        };
        check()?;
        Ok(output)
    }
}

pub(super) fn temporary_staging_name() -> Result<String, String> {
    let stamp = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map_err(|_| "DIRECT_MIGRATION_CLOCK_INVALID".to_owned())?
        .as_nanos();
    Ok(format!(".source-map.{}.{stamp}.tmp", std::process::id()))
}

pub(super) fn verify_record_artifact(
    path: &Path,
    encoded_bytes: u64,
    deadline: Instant,
    check: &dyn Fn() -> Result<(), String>,
) -> Result<[u8; 32], String> {
    check()?;
    let observed = inspect_source_import_record_artifact(
        &redb_import::DaemonImportOutputPlatform,
        path,
        encoded_bytes,
        deadline,
    );
    check()?;
    let observed = observed.map_err(plan_artifact_reason)?;
    Ok(*observed.chain())
}

fn plan_artifact_reason(error: SourceImportRecordArtifactError<String>) -> String {
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
        SourceImportRecordArtifactError::Closed => "DIRECT_MIGRATION_PLAN_CLOSED".to_owned(),
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
        SourceImportRecordArtifactError::RecordChain(error) => error.code().to_owned(),
    }
}

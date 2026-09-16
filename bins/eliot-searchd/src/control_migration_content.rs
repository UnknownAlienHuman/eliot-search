//! Persist canonical content fingerprints for the existing source-mapping plan.
//! Read retained objects, never live paths. No source bodies enter the artifact.
//!
//! `search-control-redb::migration` owns the frozen manifest line schema,
//! canonical record chain, immutable artifact lifecycle and accounting state
//! machine. This adapter owns only retained-byte readback and BLAKE3 computation.

use std::path::Path;
use std::time::Instant;

use search_contracts::{
    Blake3Digest32, Sha256Digest32, SourceNamespaceId,
};
use search_control_redb::migration::{
    SourceContentManifestEncoder, SourceContentManifestEncodingError,
    SourceContentManifestHeader, SourceContentManifestSummary,
    SourceContentObjectReadback, SourceImportRecordArtifact,
    SourceImportRecordArtifactError, source_content_profile_digest,
};
use zeroize::Zeroizing;

use crate::plaintext_direct_store::DirectStore;
use crate::revision_protection::RevisionProtector;

use super::super::read_import_revision;
use super::{
    check_deadline, ensure_directory, sha256, temporary_staging_name,
};

pub(super) struct ContentArtifact {
    pub(super) name: String,
    pub(super) chain: [u8; 32],
    pub(super) records: u64,
    pub(super) source_bytes: u64,
    pub(super) encoded_bytes: u64,
    pub(super) profile: [u8; 32],
}

/// Publication is inert until the complete importer consumes its exact bindings.
/// Each verification pass rereads every retained object with a fresh protector;
/// no digest cache can hide a missing or changed source during the second pass.
pub(super) fn stage(
    source: &DirectStore,
    root: &Path,
    target: SourceNamespaceId,
    source_plan: [u8; 32],
    directory: &Path,
    deadline: Instant,
) -> Result<ContentArtifact, String> {
    check_deadline(Some(deadline))?;
    ensure_directory(directory)?;
    let temporary_name = temporary_staging_name()?;
    let mut artifact = SourceImportRecordArtifact::create(
        directory,
        &temporary_name,
        super::redb_import::DaemonImportOutputPlatform,
        deadline,
    )
    .map_err(content_artifact_reason)?;
    let counts = compile(
        source,
        root,
        target,
        source_plan,
        deadline,
        |row| {
            artifact
                .push(row, deadline)
                .map_err(content_artifact_reason)
        },
    )?;
    let frozen = artifact
        .freeze(deadline)
        .map_err(content_artifact_reason)?;
    let digest = *frozen.chain();
    let length = frozen.encoded_bytes();

    // Recompute every digest and compare the exact generated rows with the
    // package-owned temporary artifact before publishing it.
    let mut readback = frozen
        .begin_readback(deadline)
        .map_err(content_artifact_reason)?;
    let observed = compile(
        source,
        root,
        target,
        source_plan,
        deadline,
        |row| {
            readback
                .compare(row, deadline)
                .map_err(content_artifact_reason)
        },
    )?;
    if observed != counts {
        return Err("DIRECT_MIGRATION_CONTENT_READBACK_MISMATCH".to_owned());
    }
    let verified = readback
        .finish(deadline)
        .map_err(content_artifact_reason)?;
    let name = format!("{}.source-content.v1", sha256::hex(&digest));
    let published = verified
        .publish(&name, deadline)
        .map_err(content_artifact_reason)?;
    if published.chain() != &digest
        || published.encoded_bytes() != length
        || published.name() != name.as_str()
    {
        return Err("DIRECT_MIGRATION_CONTENT_READBACK_MISMATCH".to_owned());
    }
    check_deadline(Some(deadline))?;
    Ok(ContentArtifact {
        name,
        chain: digest,
        records: counts.objects,
        source_bytes: counts.source_bytes,
        encoded_bytes: length,
        profile: *source_content_profile_digest().as_bytes(),
    })
}

fn compile(
    source: &DirectStore,
    root: &Path,
    target: SourceNamespaceId,
    source_plan: [u8; 32],
    deadline: Instant,
    mut emit: impl FnMut(&[u8]) -> Result<(), String>,
) -> Result<SourceContentManifestSummary, String> {
    check_deadline(Some(deadline))?;
    let source_header = source.source_mapping_header(target)?;
    if source.verify_migration_snapshot(deadline)?
        != source_header.catalog_snapshot
    {
        return Err("DIRECT_CONTROL_READBACK_MISMATCH".to_owned());
    }

    #[cfg(windows)]
    let protector = RevisionProtector::open_existing(source_header.legacy_namespace)?;
    #[cfg(not(windows))]
    let protector = Some(RevisionProtector::open(
        source_header.legacy_namespace,
        &root.join("revisions"),
    )?);

    let expected = source.retained_revisions().len() as u64;
    let mut encoder = SourceContentManifestEncoder::new(
        SourceContentManifestHeader {
            target_namespace: target,
            legacy_namespace: Sha256Digest32::from_bytes(
                source_header.legacy_namespace,
            ),
            catalog_snapshot: Sha256Digest32::from_bytes(
                source_header.catalog_snapshot,
            ),
            source_plan: Sha256Digest32::from_bytes(source_plan),
            expected_objects: expected,
        },
    )
    .map_err(encoding_reason)?;
    let header_row = encoder.header_row().map_err(encoding_reason)?;
    emit(&header_row)?;

    for metadata in source.retained_revisions() {
        check_deadline(Some(deadline))?;
        let bytes =
            read_import_revision(root, protector.as_ref(), &metadata, deadline)?;
        let mut hasher = Zeroizing::new(blake3::Hasher::new());
        for chunk in bytes.chunks(256 * 1024) {
            check_deadline(Some(deadline))?;
            hasher.update(chunk);
        }
        let digest =
            Blake3Digest32::from_bytes(*hasher.finalize().as_bytes());
        let row = encoder
            .object_row(SourceContentObjectReadback {
                legacy_source_id: legacy_digest(&metadata.source_id)?,
                legacy_revision_id: legacy_digest(&metadata.revision_id)?,
                content_sha256: legacy_digest(&metadata.content_digest)?,
                byte_length: metadata.byte_length,
                content_blake3: digest,
            })
            .map_err(encoding_reason)?;
        emit(&row)?;
    }

    if source.verify_migration_snapshot(deadline)?
        != source_header.catalog_snapshot
    {
        return Err("DIRECT_CONTROL_READBACK_MISMATCH".to_owned());
    }
    let (end_row, summary) = encoder.finish().map_err(encoding_reason)?;
    emit(&end_row)?;
    check_deadline(Some(deadline))?;
    Ok(summary)
}

fn legacy_digest(value: &str) -> Result<Sha256Digest32, String> {
    Sha256Digest32::parse_hex(value)
        .map_err(|_| "DIRECT_CONTROL_READBACK_MISMATCH".to_owned())
}

fn encoding_reason(error: SourceContentManifestEncodingError) -> String {
    error.code().to_owned()
}

fn content_artifact_reason(
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
            "DIRECT_MIGRATION_CONTENT_WRITE_FAILED".to_owned()
        }
        SourceImportRecordArtifactError::SyncFailed => {
            "DIRECT_MIGRATION_CONTENT_SYNC_FAILED".to_owned()
        }
        SourceImportRecordArtifactError::ReadFailed => {
            "DIRECT_MIGRATION_PLAN_READ_FAILED".to_owned()
        }
        SourceImportRecordArtifactError::ComparisonReadFailed => {
            "DIRECT_MIGRATION_CONTENT_READBACK_FAILED".to_owned()
        }
        SourceImportRecordArtifactError::ReadbackMismatch => {
            "DIRECT_MIGRATION_CONTENT_READBACK_MISMATCH".to_owned()
        }
        SourceImportRecordArtifactError::PublishOutcomeUnknown => {
            "DIRECT_MIGRATION_CONTENT_PUBLISH_OUTCOME_UNKNOWN".to_owned()
        }
        SourceImportRecordArtifactError::ImmutableConflict => {
            "DIRECT_MIGRATION_CONTENT_IMMUTABLE_CONFLICT".to_owned()
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

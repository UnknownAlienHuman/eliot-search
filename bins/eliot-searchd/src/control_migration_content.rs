//! Persist canonical content fingerprints for the existing source-mapping plan.
//! Read retained objects, never live paths. No source bodies enter the artifact.
//!
//! `search-control-redb::migration` owns the frozen manifest line schema,
//! canonical record chain, immutable artifact lifecycle and accounting state
//! machine. This adapter owns only retained-byte readback and BLAKE3 computation.

use std::path::Path;
use std::time::Instant;

use search_contracts::{Blake3Digest32, Sha256Digest32, SourceNamespaceId};
use search_control_redb::migration::{
    SourceContentManifestEncoder, SourceContentManifestEncodingError, SourceContentManifestHeader,
    SourceContentManifestSummary, SourceContentObjectReadback, SourceImportRecordArtifact,
    SourceImportRecordArtifactError, source_content_profile_digest,
};
use zeroize::Zeroizing;

use crate::plaintext_direct_store::DirectStore;
use crate::revision_protection::RevisionProtector;

use super::super::read_import_revision;
use super::{check_deadline, ensure_directory, sha256, temporary_staging_name};

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
    check(source, deadline)?;
    let output_directory = ensure_directory(directory);
    check(source, deadline)?;
    output_directory?;
    let temporary_name = temporary_staging_name()?;
    let created = SourceImportRecordArtifact::create(
        directory,
        &temporary_name,
        super::redb_import::DaemonImportOutputPlatform,
        deadline,
    )
    .map_err(content_artifact_reason);
    check(source, deadline)?;
    let mut artifact = created?;
    let counts = compile(source, root, target, source_plan, deadline, |row| {
        check(source, deadline)?;
        let pushed = artifact.push(row, deadline);
        check(source, deadline)?;
        pushed.map_err(content_artifact_reason)
    });
    check(source, deadline)?;
    let counts = counts?;
    let frozen_result = artifact.freeze(deadline).map_err(content_artifact_reason);
    check(source, deadline)?;
    let frozen = frozen_result?;
    let digest = *frozen.chain();
    let length = frozen.encoded_bytes();

    // Recompute every digest and compare the exact generated rows with the
    // package-owned temporary artifact before publishing it.
    let readback_result = frozen
        .begin_readback(deadline)
        .map_err(content_artifact_reason);
    check(source, deadline)?;
    let mut readback = readback_result?;
    let observed = compile(source, root, target, source_plan, deadline, |row| {
        check(source, deadline)?;
        let compared = readback.compare(row, deadline);
        check(source, deadline)?;
        compared.map_err(content_artifact_reason)
    });
    check(source, deadline)?;
    let observed = observed?;
    if observed != counts {
        return Err("DIRECT_MIGRATION_CONTENT_READBACK_MISMATCH".to_owned());
    }
    let verified_result = readback.finish(deadline).map_err(content_artifact_reason);
    check(source, deadline)?;
    let verified = verified_result?;
    let name = format!("{}.source-content.v1", sha256::hex(&digest));
    let published_result = verified
        .publish(&name, deadline)
        .map_err(content_artifact_reason);
    check(source, deadline)?;
    let published = published_result?;
    check(source, deadline)?;
    if published.chain() != &digest
        || published.encoded_bytes() != length
        || published.name() != name.as_str()
    {
        return Err("DIRECT_MIGRATION_CONTENT_READBACK_MISMATCH".to_owned());
    }
    check(source, deadline)?;
    let output = ContentArtifact {
        name,
        chain: digest,
        records: counts.objects,
        source_bytes: counts.source_bytes,
        encoded_bytes: length,
        profile: *source_content_profile_digest().as_bytes(),
    };
    check(source, deadline)?;
    Ok(output)
}

fn compile(
    source: &DirectStore,
    root: &Path,
    target: SourceNamespaceId,
    source_plan: [u8; 32],
    deadline: Instant,
    mut emit: impl FnMut(&[u8]) -> Result<(), String>,
) -> Result<SourceContentManifestSummary, String> {
    check(source, deadline)?;
    let source_header = source.source_mapping_header(target)?;
    let opening_snapshot = source.verify_migration_snapshot(deadline);
    check(source, deadline)?;
    if opening_snapshot? != source_header.catalog_snapshot {
        return Err("DIRECT_CONTROL_READBACK_MISMATCH".to_owned());
    }

    #[cfg(windows)]
    let protector_result = RevisionProtector::open_existing(source_header.legacy_namespace);
    #[cfg(not(windows))]
    let protector_result =
        RevisionProtector::open(source_header.legacy_namespace, &root.join("revisions"));
    check(source, deadline)?;
    #[cfg(windows)]
    let protector = protector_result?;
    #[cfg(not(windows))]
    let protector = Some(protector_result?);

    let expected = source.retained_revisions().len() as u64;
    let encoder_result = SourceContentManifestEncoder::new(SourceContentManifestHeader {
        target_namespace: target,
        legacy_namespace: Sha256Digest32::from_bytes(source_header.legacy_namespace),
        catalog_snapshot: Sha256Digest32::from_bytes(source_header.catalog_snapshot),
        source_plan: Sha256Digest32::from_bytes(source_plan),
        expected_objects: expected,
    })
    .map_err(encoding_reason);
    check(source, deadline)?;
    let mut encoder = encoder_result?;
    let header_row = encoder.header_row().map_err(encoding_reason);
    check(source, deadline)?;
    let header_row = header_row?;
    let emitted = emit(&header_row);
    check(source, deadline)?;
    emitted?;

    for metadata in source.retained_revisions() {
        check(source, deadline)?;
        let bytes_result = read_import_revision(root, protector.as_ref(), &metadata, deadline);
        check(source, deadline)?;
        let bytes = bytes_result?;
        let mut hasher = Zeroizing::new(blake3::Hasher::new());
        for chunk in bytes.chunks(256 * 1024) {
            check(source, deadline)?;
            hasher.update(chunk);
        }
        let digest = Blake3Digest32::from_bytes(*hasher.finalize().as_bytes());
        check(source, deadline)?;
        let row = encoder
            .object_row(SourceContentObjectReadback {
                legacy_source_id: legacy_digest(&metadata.source_id)?,
                legacy_revision_id: legacy_digest(&metadata.revision_id)?,
                content_sha256: legacy_digest(&metadata.content_digest)?,
                byte_length: metadata.byte_length,
                content_blake3: digest,
            })
            .map_err(encoding_reason);
        check(source, deadline)?;
        let row = row?;
        let emitted = emit(&row);
        check(source, deadline)?;
        emitted?;
    }

    let closing_snapshot = source.verify_migration_snapshot(deadline);
    check(source, deadline)?;
    if closing_snapshot? != source_header.catalog_snapshot {
        return Err("DIRECT_CONTROL_READBACK_MISMATCH".to_owned());
    }
    let finish_result = encoder.finish().map_err(encoding_reason);
    check(source, deadline)?;
    let (end_row, summary) = finish_result?;
    let emitted = emit(&end_row);
    check(source, deadline)?;
    emitted?;
    Ok(summary)
}

/// Both clocks must hold: the original bound request and the caller deadline.
/// Neither is replaced; the tighter of the two still refuses the work.
fn check(source: &DirectStore, deadline: Instant) -> Result<(), String> {
    source.check_operation()?;
    check_deadline(Some(deadline))
}

fn legacy_digest(value: &str) -> Result<Sha256Digest32, String> {
    Sha256Digest32::parse_hex(value).map_err(|_| "DIRECT_CONTROL_READBACK_MISMATCH".to_owned())
}

fn encoding_reason(error: SourceContentManifestEncodingError) -> String {
    error.code().to_owned()
}

fn content_artifact_reason(error: SourceImportRecordArtifactError<String>) -> String {
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
        SourceImportRecordArtifactError::RecordChain(error) => error.code().to_owned(),
    }
}

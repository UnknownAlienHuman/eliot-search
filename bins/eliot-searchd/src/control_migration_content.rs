//! Persist canonical content fingerprints for the existing source-mapping plan.
//! Read retained objects, never live paths. No source bodies enter the artifact.
//!
//! `search-control-redb::migration` owns the frozen manifest line schema,
//! canonical record chain and accounting state machine. This adapter owns only
//! retained-byte readback, BLAKE3 computation and native I/O composition.

use std::io::{BufReader, BufWriter, Read, Write};
use std::path::Path;
use std::time::Instant;

use search_contracts::{
    Blake3Digest32, Sha256Digest32, SourceNamespaceId,
};
use search_control_redb::migration::{
    SourceContentManifestEncoder, SourceContentManifestEncodingError,
    SourceContentManifestHeader, SourceContentManifestSummary,
    SourceContentObjectReadback, SourceImportRecordChain,
    SourceImportRecordChainError, source_content_profile_digest,
};
use zeroize::Zeroizing;

use crate::plaintext_direct_store::DirectStore;
use crate::revision_protection::RevisionProtector;

use super::super::read_import_revision;
use super::{
    MAX_ROW_BYTES, StagingFile, check_deadline, ensure_directory, fingerprint,
    open_plan, reserve, sha256, sync_directory,
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
    let mut staging = StagingFile::create(directory)?;
    let mut length = 0;
    let mut chain = SourceImportRecordChain::new();
    let counts = {
        let mut output = BufWriter::new(staging.file_mut()?);
        let counts = compile(
            source,
            root,
            target,
            source_plan,
            deadline,
            |row| {
                length = reserve(length, row.len())?;
                output
                    .write_all(row)
                    .map_err(|_| "DIRECT_MIGRATION_CONTENT_WRITE_FAILED".to_owned())?;
                chain.push(row).map_err(record_chain_reason)
            },
        )?;
        output
            .flush()
            .map_err(|_| "DIRECT_MIGRATION_CONTENT_WRITE_FAILED".to_owned())?;
        counts
    };
    staging
        .file_mut()?
        .sync_all()
        .map_err(|_| "DIRECT_MIGRATION_CONTENT_SYNC_FAILED".to_owned())?;
    drop(staging.file.take());
    let digest = chain.finish();

    // Compare newly computed BLAKE3 and legacy bindings, not merely a fingerprint
    // copied from the provisional artifact. No source/credential write is allowed.
    let mut input = BufReader::new(open_plan(&staging.path, length)?);
    let mut compared = 0;
    let mut buffer = [0_u8; MAX_ROW_BYTES];
    let observed = compile(
        source,
        root,
        target,
        source_plan,
        deadline,
        |row| {
            compared = reserve(compared, row.len())?;
            input
                .read_exact(&mut buffer[..row.len()])
                .map_err(|_| "DIRECT_MIGRATION_CONTENT_READBACK_FAILED".to_owned())?;
            if buffer[..row.len()] != *row {
                return Err("DIRECT_MIGRATION_CONTENT_READBACK_MISMATCH".to_owned());
            }
            Ok(())
        },
    )?;
    let mut extra = [0_u8; 1];
    if observed != counts
        || compared != length
        || input
            .read(&mut extra)
            .map_err(|_| "DIRECT_MIGRATION_CONTENT_READBACK_FAILED".to_owned())?
            != 0
    {
        return Err("DIRECT_MIGRATION_CONTENT_READBACK_MISMATCH".to_owned());
    }
    drop(input);
    check_deadline(Some(deadline))?;
    let name = format!("{}.source-content.v1", sha256::hex(&digest));
    let destination = directory.join(&name);
    match std::fs::hard_link(&staging.path, &destination) {
        Ok(()) => {}
        Err(error) if error.kind() == std::io::ErrorKind::AlreadyExists => {}
        Err(_) => {
            return Err(
                "DIRECT_MIGRATION_CONTENT_PUBLISH_OUTCOME_UNKNOWN".to_owned(),
            );
        }
    }
    #[cfg(unix)]
    sync_directory(directory)?;
    #[cfg(not(unix))]
    sync_directory(directory);
    if fingerprint(&destination, length, deadline)? != digest {
        return Err("DIRECT_MIGRATION_CONTENT_IMMUTABLE_CONFLICT".to_owned());
    }
    staging.remove()?;
    #[cfg(unix)]
    sync_directory(directory)?;
    #[cfg(not(unix))]
    sync_directory(directory);
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

    // Offline planning can consume a legacy plaintext object without creating a
    // key. A protected object always requires the original available credential.
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
        // Shared with ordinary DIRECT reads: both encodings, when present, must
        // agree. A damaged protected object never falls back to plaintext.
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

fn record_chain_reason(error: SourceImportRecordChainError) -> String {
    error.code().to_owned()
}

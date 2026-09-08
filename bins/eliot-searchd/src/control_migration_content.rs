//! Persist canonical content fingerprints for the existing source-mapping plan.
//! Read retained objects, never live paths. No source bodies enter the artifact.

use std::io::{BufReader, BufWriter, Read, Write};
use std::path::Path;
use std::time::Instant;
use search_contracts::{Blake3Digest32, SourceNamespaceId};
use zeroize::Zeroizing;

use crate::plaintext_direct_store::DirectStore;
use crate::revision_protection::RevisionProtector;
use super::{PlanDigest, StagingFile, MAX_ROW_BYTES, check_deadline, ensure_directory,
    fingerprint, open_plan, reserve, sha256, sync_directory};
use super::super::read_import_revision;

const PROFILE: &[u8] = b"eliot/source-content/v1;retained-plaintext;sha256-verified;blake3-256;no-normalization";

pub(super) struct ContentArtifact {
    pub(super) name: String,
    pub(super) chain: [u8; 32],
    pub(super) records: u64,
    pub(super) source_bytes: u64,
}

#[derive(Clone, Copy, Eq, PartialEq)]
struct Counts { records: u64, source_bytes: u64 }

/// Publication is inert until the complete importer consumes its exact bindings.
/// Each verification pass rereads every retained object with a fresh protector;
/// no digest cache can hide a missing or changed source during the second pass.
pub(super) fn stage(
    source: &DirectStore, root: &Path, target: SourceNamespaceId,
    source_plan: [u8; 32], directory: &Path, deadline: Instant,
) -> Result<ContentArtifact, String> {
    check_deadline(Some(deadline))?;
    ensure_directory(directory)?;
    let mut staging = StagingFile::create(directory)?;
    let mut length = 0;
    let mut chain = PlanDigest::new();
    let counts = {
        let mut output = BufWriter::new(staging.file_mut()?);
        let counts = compile(source, root, target, source_plan, deadline, |row| {
            length = reserve(length, row.len())?;
            output.write_all(row).map_err(|_| "DIRECT_MIGRATION_CONTENT_WRITE_FAILED".to_owned())?;
            chain.push(row)
        })?;
        output.flush().map_err(|_| "DIRECT_MIGRATION_CONTENT_WRITE_FAILED".to_owned())?;
        counts
    };
    staging.file_mut()?.sync_all().map_err(|_| "DIRECT_MIGRATION_CONTENT_SYNC_FAILED".to_owned())?;
    drop(staging.file.take());
    let digest = chain.finish();

    // Compare newly computed BLAKE3 and legacy bindings, not merely a fingerprint
    // copied from the provisional artifact. No source/credential write is allowed.
    let mut input = BufReader::new(open_plan(&staging.path, length)?);
    let mut compared = 0;
    let mut buffer = [0_u8; MAX_ROW_BYTES];
    let observed = compile(source, root, target, source_plan, deadline, |row| {
        compared = reserve(compared, row.len())?;
        input.read_exact(&mut buffer[..row.len()])
            .map_err(|_| "DIRECT_MIGRATION_CONTENT_READBACK_FAILED".to_owned())?;
        if buffer[..row.len()] != *row {
            return Err("DIRECT_MIGRATION_CONTENT_READBACK_MISMATCH".to_owned());
        }
        Ok(())
    })?;
    let mut extra = [0_u8; 1];
    if observed != counts || compared != length
        || input.read(&mut extra).map_err(|_| "DIRECT_MIGRATION_CONTENT_READBACK_FAILED".to_owned())? != 0
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
        Err(_) => return Err("DIRECT_MIGRATION_CONTENT_PUBLISH_OUTCOME_UNKNOWN".to_owned()),
    }
    sync_directory(directory)?;
    if fingerprint(&destination, length, deadline)? != digest {
        return Err("DIRECT_MIGRATION_CONTENT_IMMUTABLE_CONFLICT".to_owned());
    }
    staging.remove()?;
    sync_directory(directory)?;
    check_deadline(Some(deadline))?;
    Ok(ContentArtifact { name, chain: digest, records: counts.records, source_bytes: counts.source_bytes })
}

fn compile(
    source: &DirectStore, root: &Path, target: SourceNamespaceId,
    source_plan: [u8; 32], deadline: Instant,
    mut emit: impl FnMut(&[u8]) -> Result<(), String>,
) -> Result<Counts, String> {
    check_deadline(Some(deadline))?;
    let header = source.source_mapping_header(target)?;
    if source.verify_migration_snapshot(deadline)? != header.catalog_snapshot {
        return Err("DIRECT_CONTROL_READBACK_MISMATCH".to_owned());
    }
    // Offline planning can consume a legacy plaintext object without creating a
    // key. A protected object always requires the original available credential.
    #[cfg(windows)]
    let protector = RevisionProtector::open_existing(header.legacy_namespace)?;
    #[cfg(not(windows))]
    let protector = Some(RevisionProtector::open(header.legacy_namespace, &root.join("revisions"))?);
    let expected = source.retained_revisions().len() as u64;
    emit(format!(concat!(
        "{{\"kind\":\"source_content_header\",\"schema\":\"eliot.source-content.v1\",",
        "\"target_namespace_id\":\"{}\",\"legacy_namespace_sha256\":\"{}\",",
        "\"catalog_snapshot_sha256\":\"{}\",\"source_plan_chain_sha256\":\"{}\",",
        "\"content_profile_sha256\":\"{}\",\"expected_objects\":{},",
        "\"content_digest_algorithm\":\"blake3_256\",\"cutover_authorized\":false}}\n"
    ), target, sha256::hex(&header.legacy_namespace), sha256::hex(&header.catalog_snapshot),
        sha256::hex(&source_plan), sha256::hex(&sha256::digest(PROFILE)), expected).as_bytes())?;
    let mut counts = Counts { records: 0, source_bytes: 0 };
    for metadata in source.retained_revisions() {
        check_deadline(Some(deadline))?;
        // Shared with ordinary DIRECT reads: both encodings, when present, must
        // agree. A damaged protected object never falls back to plaintext.
        let bytes = read_import_revision(root, protector.as_ref(), &metadata, deadline)?;
        let mut hasher = Zeroizing::new(blake3::Hasher::new());
        for chunk in bytes.chunks(256 * 1024) {
            check_deadline(Some(deadline))?;
            hasher.update(chunk);
        }
        let digest = Blake3Digest32::from_bytes(*hasher.finalize().as_bytes());
        counts.records += 1; // bounded by the validated retained-revision inventory
        counts.source_bytes = counts.source_bytes.checked_add(metadata.byte_length)
            .ok_or_else(|| "DIRECT_MIGRATION_BYTES_EXCEEDED".to_owned())?;
        emit(format!(concat!(
            "{{\"kind\":\"source_content_readback\",\"ordinal\":{},",
            "\"legacy_source_id\":\"{}\",\"legacy_revision_id\":\"{}\",",
            "\"content_sha256\":\"{}\",\"byte_length\":{},\"content_blake3\":\"{}\"}}\n"
        ), counts.records, metadata.source_id, metadata.revision_id, metadata.content_digest,
            metadata.byte_length, sha256::hex(digest.as_bytes())).as_bytes())?;
    }
    if counts.records != expected || source.verify_migration_snapshot(deadline)? != header.catalog_snapshot {
        return Err("DIRECT_CONTROL_READBACK_MISMATCH".to_owned());
    }
    emit(format!(concat!(
        "{{\"kind\":\"source_content_end\",\"objects\":{},\"source_bytes\":{},",
        "\"legacy_sha256_verified\":true,\"blake3_computed_from_bytes\":true,",
        "\"stability_receipt_issued\":false,\"residency_authorized\":false}}\n"
    ), counts.records, counts.source_bytes).as_bytes())?;
    check_deadline(Some(deadline))?;
    Ok(counts)
}

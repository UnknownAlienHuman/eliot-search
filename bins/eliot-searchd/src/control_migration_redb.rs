//! Import the typed source mapping into an inactive redb artifact. The source
//! mapper is replayed for exact verification; no JSON parser or live catalog is added.

use std::fs::{self, File, OpenOptions};
use std::path::Path;
use std::time::Instant;

use search_contracts::{Sha256Digest32, SourceNamespaceId};
use search_control_redb::migration::{SourceImportBinding, SourceImportCounts,
    SourceContentManifest, SourceMappingImport, SourceMappingReadback};

use super::{PlanDigest, check_deadline, ensure_directory, fingerprint, regular, sha256, sync_directory};
use super::content_readback::ContentArtifact;
use crate::plaintext_direct_store::DirectStore;

/// The caller retains source exclusion and has already verified the text plan.
/// A complete existing database is rechecked, not overwritten or treated as live control.
pub(super) fn store(
    source: &DirectStore, target: SourceNamespaceId, directory: &Path,
    plan_chain: [u8; 32], expected: SourceImportCounts, content: &ContentArtifact, deadline: Instant,
) -> Result<(String, bool), String> {
    check_deadline(Some(deadline))?;
    ensure_directory(directory)?;
    let header = source.source_mapping_header(target)?;
    let binding = header.import_binding(plan_chain);
    if content.records != source.retained_revisions().len() as u64 {
        return Err("DIRECT_MIGRATION_CONTENT_COUNT_MISMATCH".to_owned());
    }
    let content_binding = SourceContentManifest {
        target_namespace: target,
        legacy_namespace: Sha256Digest32::from_bytes(header.legacy_namespace),
        catalog_snapshot: Sha256Digest32::from_bytes(header.catalog_snapshot),
        source_plan: Sha256Digest32::from_bytes(plan_chain),
        profile: Sha256Digest32::from_bytes(content.profile),
        manifest_chain: Sha256Digest32::from_bytes(content.chain),
        manifest_bytes: content.encoded_bytes,
        objects: content.records,
        source_bytes: content.source_bytes,
    };
    verify_content(directory, content, deadline)?;
    // Different content profiles/manifests cannot reuse an unbound v1 target or
    // each other's pending prefix. The complete reference is checked inside redb.
    let target_digest = sha256::digest_parts(b"eliot-search/source-content-import/v2",
        &[&plan_chain, &content.chain]);
    let name = format!("{}.source-map.v2.redb", sha256::hex(&target_digest));
    let final_path = directory.join(&name);
    match fs::symlink_metadata(&final_path) {
        Ok(metadata) => {
            if !regular(&metadata) { return Err("DIRECT_MIGRATION_IMPORT_OBJECT_INVALID".to_owned()); }
            verify(source, &final_path, binding, content_binding, expected, deadline)?;
            verify_content(directory, content, deadline)?;
            return Ok((name, true));
        }
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => {}
        Err(_) => return Err("DIRECT_MIGRATION_IMPORT_OPEN_FAILED".to_owned()),
    }
    // A deterministic pending locator lets an interrupted invocation find the
    // same transactionally committed prefix. It is not a usable/final artifact.
    // Keep it on every error, including unknown native commit outcomes. Never
    // truncate, replace, or guess completion from the filename or file length.
    let pending_path = directory.join(format!(".{name}.pending"));
    let mut writer = match OpenOptions::new().read(true).write(true).create_new(true).open(&pending_path) {
        Ok(file) => {
            let writer = SourceMappingImport::create_with_content(file, binding, content_binding, deadline)
                .map_err(|e| e.code().to_owned())?;
            sync_directory(directory)?;
            writer
        }
        Err(error) if error.kind() == std::io::ErrorKind::AlreadyExists => {
            SourceMappingImport::resume_with_content(open_existing(&pending_path)?, binding, content_binding, deadline)
                .map_err(|e| e.code().to_owned())?
        }
        Err(_) => return Err("DIRECT_MIGRATION_IMPORT_CREATE_FAILED".to_owned()),
    };
    let mut hash = PlanDigest::new();
    // Replay from event one even on resume: the adapter compares every already
    // committed row and index before dispatching the first new batch. Neither
    // the caller nor a cursor can skip verification of the persisted prefix.
    let summary = source.compile_source_mapping_with_rows(target, deadline,
        |encoded| hash.push(encoded),
        |row| writer.push(row, deadline).map_err(|e| e.code().to_owned()),
    )?;
    if summary.import_counts() != expected || hash.finish() != plan_chain {
        return Err("DIRECT_MIGRATION_IMPORT_SOURCE_CHANGED".to_owned());
    }
    // Consume/drop the native writer before reopening or publishing its file.
    writer.finish(expected, deadline).map_err(|e| e.code().to_owned())?;
    verify(source, &pending_path, binding, content_binding, expected, deadline)?;
    check_deadline(Some(deadline))?;
    let reused = match fs::hard_link(&pending_path, &final_path) {
        Ok(()) => false,
        Err(error) if error.kind() == std::io::ErrorKind::AlreadyExists => true,
        Err(_) => return Err("DIRECT_MIGRATION_IMPORT_PUBLISH_OUTCOME_UNKNOWN".to_owned()),
    };
    sync_directory(directory)?;
    // A competing existing target must match too. The final name never grants
    // integrity, complete accounting, or permission to activate the imported namespace.
    verify(source, &final_path, binding, content_binding, expected, deadline)?;
    verify_content(directory, content, deadline)?;
    // All native handles are closed and the final target has passed exact readback.
    // A failure before here deliberately retains pending progress for the next call.
    fs::remove_file(&pending_path).map_err(|_| "DIRECT_MIGRATION_IMPORT_CLEANUP_FAILED".to_owned())?;
    sync_directory(directory)?;
    check_deadline(Some(deadline))?;
    Ok((name, reused))
}

fn verify(
    source: &DirectStore, path: &Path, binding: SourceImportBinding,
    content: SourceContentManifest, expected: SourceImportCounts, deadline: Instant,
) -> Result<(), String> {
    let file = open_existing(path)?;
    let mut reader = SourceMappingReadback::open_with_content(file, binding, content, expected, deadline)
        .map_err(|e| e.code().to_owned())?;
    let mut hash = PlanDigest::new();
    let summary = source.compile_source_mapping_with_rows(binding.target_namespace, deadline,
        |encoded| hash.push(encoded),
        |row| reader.compare(&row, deadline).map_err(|e| e.code().to_owned()),
    )?;
    if summary.import_counts() != expected || hash.finish() != *binding.plan_chain.as_bytes() {
        return Err("DIRECT_MIGRATION_IMPORT_SOURCE_CHANGED".to_owned());
    }
    reader.finish(deadline).map_err(|e| e.code().to_owned())
}


fn verify_content(directory: &Path, content: &ContentArtifact, deadline: Instant) -> Result<(), String> {
    // A verified producer result is still re-read at the import boundary and after
    // final publication. Never accept a client path or parse JSON into authority.
    let expected = format!("{}.source-content.v1", sha256::hex(&content.chain));
    if content.name != expected
        || fingerprint(&directory.join(&expected), content.encoded_bytes, deadline)? != content.chain
    {
        return Err("DIRECT_MIGRATION_CONTENT_READBACK_MISMATCH".to_owned());
    }
    Ok(())
}

fn open_existing(path: &Path) -> Result<File, String> {
    let invalid = || "DIRECT_MIGRATION_IMPORT_OBJECT_INVALID".to_owned();
    ensure_directory(path.parent().ok_or_else(invalid)?)?;
    let before = fs::symlink_metadata(path).map_err(|_| invalid())?;
    if !regular(&before) || before.len() == 0 { return Err(invalid()); }
    // redb may recover native metadata of this *target*, never the source journal.
    let file = OpenOptions::new().read(true).write(true).open(path).map_err(|_| invalid())?;
    let opened = file.metadata().map_err(|_| invalid())?;
    if !regular(&opened) || opened.len() != before.len() || opened.modified().ok() != before.modified().ok() {
        return Err(invalid());
    }
    Ok(file)
}

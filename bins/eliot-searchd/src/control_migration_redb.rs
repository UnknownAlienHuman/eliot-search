//! Import the typed source mapping into an inactive redb artifact. The source
//! mapper is replayed for exact verification; no JSON parser or live catalog is added.

use std::fs::{self, File, OpenOptions, TryLockError};
use std::path::{Path, PathBuf};
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
    let pending_path = directory.join(format!(".{name}.pending"));
    // The source-root lock does not serialize two restored copies that publish
    // this same plan into a shared output directory. Keep one per-target OS lock
    // across native close/reopen, readback, publication, and pending cleanup.
    let output = ImportOutputGuard::acquire(directory, &name, deadline)?;
    match fs::symlink_metadata(&final_path) {
        Ok(metadata) => {
            if !regular(&metadata) { return Err("DIRECT_MIGRATION_IMPORT_OBJECT_INVALID".to_owned()); }
            let final_file = verify(source, &final_path, binding, content_binding, expected, deadline)?;
            verify_content(directory, content, deadline)?;
            // Resolve a crash after hard-link publication but before unlinking
            // pending. Only the same native object is disposable here; a distinct
            // pending database, even byte-identical, remains untouched.
            cleanup_published_alias(&pending_path, &final_path, final_file, &output, deadline)?;
            output.verify(deadline)?;
            return Ok((name, true));
        }
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => {}
        Err(_) => return Err("DIRECT_MIGRATION_IMPORT_OPEN_FAILED".to_owned()),
    }
    // A deterministic pending locator lets an interrupted invocation find the
    // same transactionally committed prefix. It is not a usable/final artifact.
    // Keep it on every error, including unknown native commit outcomes. Never
    // truncate, replace, or guess completion from the filename or file length.
    let (file, created) = match OpenOptions::new().read(true).write(true).create_new(true).open(&pending_path) {
        Ok(file) => (file, true),
        Err(error) if error.kind() == std::io::ErrorKind::AlreadyExists =>
            (open_existing(&pending_path)?, false),
        Err(_) => return Err("DIRECT_MIGRATION_IMPORT_CREATE_FAILED".to_owned()),
    };
    let pending_identity = native_identity(&file)?;
    verify_locator(&file, &pending_path)?;
    output.verify(deadline)?;
    let mut writer = if created {
        SourceMappingImport::create_with_content(file, binding, content_binding, deadline)
    } else {
        SourceMappingImport::resume_with_content(file, binding, content_binding, deadline)
    }.map_err(|e| e.code().to_owned())?;
    if created {
        #[cfg(unix)]
        sync_directory(directory)?;
        #[cfg(not(unix))]
        sync_directory(directory);
    }
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
    let pending_file = verify(source, &pending_path, binding, content_binding, expected, deadline)?;
    if native_identity(&pending_file)? != pending_identity {
        return Err("DIRECT_MIGRATION_IMPORT_IDENTITY_CHANGED".to_owned());
    }
    output.verify(deadline)?;
    verify_locator(&pending_file, &pending_path)?;
    let reused = match fs::hard_link(&pending_path, &final_path) {
        Ok(()) => false,
        Err(error) if error.kind() == std::io::ErrorKind::AlreadyExists => true,
        Err(_) => return Err("DIRECT_MIGRATION_IMPORT_PUBLISH_OUTCOME_UNKNOWN".to_owned()),
    };
    #[cfg(unix)]
    sync_directory(directory)?;
    #[cfg(not(unix))]
    sync_directory(directory);
    // A competing existing target must match too. The final name never grants
    // integrity, complete accounting, or permission to activate the imported namespace.
    let final_file = verify(source, &final_path, binding, content_binding, expected, deadline)?;
    verify_content(directory, content, deadline)?;
    // A newly created hard link must identify the same object whose rows were
    // verified. For an existing final object, exact readback above is mandatory.
    if !reused && native_identity(&final_file)? != pending_identity {
        return Err("DIRECT_MIGRATION_IMPORT_IDENTITY_CHANGED".to_owned());
    }
    output.verify(deadline)?;
    verify_locator(&pending_file, &pending_path)?;
    verify_locator(&final_file, &final_path)?;
    // The candidate may differ from an already-published target. Preserve
    // independent pending state even when both databases have equivalent rows.
    drop(pending_file);
    cleanup_published_alias(&pending_path, &final_path, final_file, &output, deadline)?;
    output.verify(deadline)?;
    Ok((name, reused))
}

fn verify(
    source: &DirectStore, path: &Path, binding: SourceImportBinding,
    content: SourceContentManifest, expected: SourceImportCounts, deadline: Instant,
) -> Result<File, String> {
    let file = open_existing(path)?;
    let identity = native_identity(&file)?;
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
    reader.finish(deadline).map_err(|e| e.code().to_owned())?;
    // Native redb is closed before this descriptor is opened. Holding this file
    // pins the observed object through publication without retaining a database.
    let pinned = open_existing(path)?;
    if native_identity(&pinned)? != identity {
        return Err("DIRECT_MIGRATION_IMPORT_IDENTITY_CHANGED".to_owned());
    }
    check_deadline(Some(deadline))?;
    Ok(pinned)
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
    verify_locator(&file, path)?;
    Ok(file)
}


/// Serialize the output artifact, not the source namespace or live control state.
/// The empty lock file is retained: unlinking it would let a second inode acquire
/// the same logical lock while another process still owns the first inode.
struct ImportOutputGuard {
    directory: PathBuf,
    path: PathBuf,
    file: File,
}

impl ImportOutputGuard {
    fn acquire(directory: &Path, name: &str, deadline: Instant) -> Result<Self, String> {
        check_deadline(Some(deadline))?;
        ensure_directory(directory)?;
        let path = directory.join(format!(".{name}.lock"));
        let (file, created) = match OpenOptions::new().read(true).write(true).create_new(true).open(&path) {
            Ok(file) => (file, true),
            Err(error) if error.kind() == std::io::ErrorKind::AlreadyExists => {
                let metadata = fs::symlink_metadata(&path)
                    .map_err(|_| "DIRECT_MIGRATION_OUTPUT_LOCK_INVALID".to_owned())?;
                if !regular(&metadata) || metadata.len() != 0 {
                    return Err("DIRECT_MIGRATION_OUTPUT_LOCK_INVALID".to_owned());
                }
                (OpenOptions::new().read(true).write(true).open(&path)
                    .map_err(|_| "DIRECT_MIGRATION_OUTPUT_LOCK_OPEN_FAILED".to_owned())?, false)
            }
            Err(_) => return Err("DIRECT_MIGRATION_OUTPUT_LOCK_OPEN_FAILED".to_owned()),
        };
        match file.try_lock() {
            Ok(()) => {}
            Err(TryLockError::WouldBlock) => return Err("DIRECT_MIGRATION_OUTPUT_ALREADY_OWNED".to_owned()),
            Err(TryLockError::Error(_)) => return Err("DIRECT_MIGRATION_OUTPUT_LOCK_FAILED".to_owned()),
        }
        let guard = Self { directory: directory.to_owned(), path, file };
        guard.verify(deadline)?;
        if created {
            guard.file.sync_all().map_err(|_| "DIRECT_MIGRATION_OUTPUT_LOCK_SYNC_FAILED".to_owned())?;
            #[cfg(unix)]
            sync_directory(directory)?;
            #[cfg(not(unix))]
            sync_directory(directory);
        }
        guard.verify(deadline)?;
        Ok(guard)
    }

    fn verify(&self, deadline: Instant) -> Result<(), String> {
        check_deadline(Some(deadline))?;
        ensure_directory(&self.directory)?;
        verify_locator(&self.file, &self.path)?;
        if self.file.metadata().map_err(|_| "DIRECT_MIGRATION_OUTPUT_LOCK_INVALID".to_owned())?.len() != 0 {
            return Err("DIRECT_MIGRATION_OUTPUT_LOCK_INVALID".to_owned());
        }
        check_deadline(Some(deadline))
    }
}

/// Native identity, never an mtime/length substitute. The platform observer is the
/// existing package-owned Windows boundary; no unsafe or new dependency is added.
pub(super) fn native_identity(file: &File) -> Result<(u64, u64), String> {
    let invalid = || "DIRECT_MIGRATION_IMPORT_IDENTITY_CHANGED".to_owned();
    let metadata = file.metadata().map_err(|_| invalid())?;
    if !regular(&metadata) { return Err(invalid()); }
    #[cfg(unix)]
    {
        use std::os::unix::fs::MetadataExt;
        Ok((metadata.dev(), metadata.ino()))
    }
    #[cfg(windows)]
    {
        let observed = eliot_searchd::native_file::observe(file).map_err(|_| invalid())?;
        Ok((u64::from(observed.volume_serial), observed.file_index))
    }
    #[cfg(not(any(unix, windows)))]
    { Err("DIRECT_MIGRATION_LOCK_PLATFORM_UNSUPPORTED".to_owned()) }
}

pub(super) fn verify_locator(expected: &File, path: &Path) -> Result<(), String> {
    let invalid = || "DIRECT_MIGRATION_IMPORT_IDENTITY_CHANGED".to_owned();
    ensure_directory(path.parent().ok_or_else(invalid)?)?;
    if !regular(&fs::symlink_metadata(path).map_err(|_| invalid())?) { return Err(invalid()); }
    let current = File::open(path).map_err(|_| invalid())?;
    if native_identity(&current)? != native_identity(expected)? { return Err(invalid()); }
    Ok(())
}

fn cleanup_published_alias(
    pending: &Path, final_path: &Path, published: File, guard: &ImportOutputGuard, deadline: Instant,
) -> Result<(), String> {
    guard.verify(deadline)?;
    // Revalidate the final locator even if there is no pending name to clean.
    verify_locator(&published, final_path)?;
    match fs::symlink_metadata(pending) {
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => return Ok(()),
        Err(_) => return Err("DIRECT_MIGRATION_IMPORT_OPEN_FAILED".to_owned()),
        Ok(metadata) if !regular(&metadata) =>
            return Err("DIRECT_MIGRATION_IMPORT_OBJECT_INVALID".to_owned()),
        Ok(_) => {}
    }
    let pending_file = open_existing(pending)?;
    if native_identity(&pending_file)? == native_identity(&published)? {
        verify_locator(&pending_file, pending)?;
        verify_locator(&published, final_path)?;
        guard.verify(deadline)?;
        drop(pending_file);
        drop(published);
        fs::remove_file(pending).map_err(|_| "DIRECT_MIGRATION_IMPORT_CLEANUP_FAILED".to_owned())?;
        #[cfg(unix)]
        sync_directory(&guard.directory)?;
        #[cfg(not(unix))]
        sync_directory(&guard.directory);
    }
    // Independent pending state is not an alias and is never discarded by this path.
    guard.verify(deadline)
}

//! Import the typed source mapping into an inactive redb artifact. The source
//! mapper is replayed for exact verification; no JSON parser or live catalog is added.

use std::fs::{self, File, OpenOptions};
use std::path::Path;
use std::time::Instant;

use search_contracts::SourceNamespaceId;
use search_control_redb::migration::{SourceImportBinding, SourceImportCounts,
    SourceMappingImport, SourceMappingReadback};

use super::{PlanDigest, StagingFile, check_deadline, ensure_directory, regular, sha256, sync_directory};
use crate::plaintext_direct_store::DirectStore;

/// The caller retains source exclusion and has already verified the text plan.
/// A complete existing database is rechecked, not overwritten or treated as live control.
pub(super) fn store(
    source: &DirectStore, target: SourceNamespaceId, directory: &Path,
    plan_chain: [u8; 32], expected: SourceImportCounts, deadline: Instant,
) -> Result<(String, bool), String> {
    check_deadline(Some(deadline))?;
    ensure_directory(directory)?;
    let binding = source.source_mapping_header(target)?.import_binding(plan_chain);
    let name = format!("{}.source-map.v1.redb", sha256::hex(&plan_chain));
    let final_path = directory.join(&name);
    match fs::symlink_metadata(&final_path) {
        Ok(metadata) => {
            if !regular(&metadata) { return Err("DIRECT_MIGRATION_IMPORT_OBJECT_INVALID".to_owned()); }
            verify(source, &final_path, binding, expected, deadline)?;
            return Ok((name, true));
        }
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => {}
        Err(_) => return Err("DIRECT_MIGRATION_IMPORT_OPEN_FAILED".to_owned()),
    }
    let mut staging = StagingFile::create(directory)?;
    let file = staging.file.take().ok_or_else(|| "DIRECT_MIGRATION_PLAN_CLOSED".to_owned())?;
    let mut writer = SourceMappingImport::create(file, binding, deadline).map_err(|e| e.code().to_owned())?;
    let mut hash = PlanDigest::new();
    let summary = source.compile_source_mapping_with_rows(target, deadline,
        |encoded| hash.push(encoded),
        |row| writer.push(row, deadline).map_err(|e| e.code().to_owned()),
    )?;
    if summary.import_counts() != expected || hash.finish() != plan_chain {
        return Err("DIRECT_MIGRATION_IMPORT_SOURCE_CHANGED".to_owned());
    }
    // Consume/drop the native writer before reopening or publishing its file.
    writer.finish(expected, deadline).map_err(|e| e.code().to_owned())?;
    verify(source, &staging.path, binding, expected, deadline)?;
    check_deadline(Some(deadline))?;
    let reused = match fs::hard_link(&staging.path, &final_path) {
        Ok(()) => false,
        Err(error) if error.kind() == std::io::ErrorKind::AlreadyExists => true,
        Err(_) => return Err("DIRECT_MIGRATION_IMPORT_PUBLISH_OUTCOME_UNKNOWN".to_owned()),
    };
    sync_directory(directory)?;
    // A competing existing target must match too. The final name never grants
    // integrity, complete accounting, or permission to activate the imported namespace.
    verify(source, &final_path, binding, expected, deadline)?;
    staging.remove()?;
    sync_directory(directory)?;
    check_deadline(Some(deadline))?;
    Ok((name, reused))
}

fn verify(
    source: &DirectStore, path: &Path, binding: SourceImportBinding,
    expected: SourceImportCounts, deadline: Instant,
) -> Result<(), String> {
    let file = open_existing(path)?;
    let mut reader = SourceMappingReadback::open(file, binding, expected, deadline)
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

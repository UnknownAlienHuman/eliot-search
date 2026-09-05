//! Immutable directory inventory generations and explicit reconciliation.
//!
//! No source is retired merely because an ad-hoc scan did not mention it. A
//! sync first completes the same fail-closed directory inventory used for
//! indexing, compares it with the latest verified immutable manifest, and
//! retires only a missing source whose current path digest still equals the old
//! directory binding. Moved/rebound sources are preserved.

use std::collections::BTreeMap;
use std::fs::{self, File, Metadata, OpenOptions};
use std::io::{Read, Write};
use std::path::{Path, PathBuf};
use std::time::{SystemTime, UNIX_EPOCH};

use crate::direct_store::{DirectStore, IndexedSource};
use crate::sha256;

const CONTROL_DIRECTORY: &str = "control";
const MANIFEST_DIRECTORY: &str = "directory-manifests";
const MANIFEST_HEADER: &str = "ELIOT_SEARCH_DIRECTORY_MANIFEST_V1";
const MAX_MANIFEST_BYTES: usize = 128 * 1024 * 1024;
const MAX_MANIFEST_ENTRIES: usize = 100_000;
const MAX_MANIFEST_FILES: usize = 1_000_000;
const MAX_MANIFEST_LINE_BYTES: usize = 1_024;

/// One exact source binding in a complete directory inventory.
#[derive(Clone, Debug, Eq, Ord, PartialEq, PartialOrd)]
pub(crate) struct DirectoryEntry {
    pub(crate) source_id: String,
    pub(crate) path_digest: String,
    pub(crate) revision_id: String,
}

/// One immutable verified directory inventory generation.
#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) struct DirectoryManifest {
    pub(crate) namespace_id: String,
    pub(crate) directory_digest: String,
    pub(crate) generation: u64,
    pub(crate) entries: BTreeMap<String, DirectoryEntry>,
    pub(crate) manifest_digest: String,
}

/// Result of explicit directory reconciliation.
#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) struct DirectorySyncResult {
    pub(crate) namespace_id: String,
    pub(crate) directory_digest: String,
    pub(crate) previous_generation: Option<u64>,
    pub(crate) generation: u64,
    pub(crate) previous_sources: usize,
    pub(crate) indexed_sources: usize,
    pub(crate) changed_sources: usize,
    pub(crate) missing_sources: usize,
    pub(crate) retired_sources: usize,
    pub(crate) moved_or_rebound_sources: usize,
    pub(crate) manifest_digest: String,
}

/// Verification summary for all immutable directory manifests.
#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) struct DirectoryManifestVerification {
    pub(crate) manifest_files: usize,
    pub(crate) directories: usize,
    pub(crate) current_entries: usize,
    pub(crate) highest_generation: u64,
}

/// Completes one inventory, indexes current files, retires proven missing
/// bindings, and publishes the next immutable manifest generation.
pub(crate) fn sync_directory(
    store: &mut DirectStore,
    data_root: &Path,
    directory: &Path,
) -> Result<DirectorySyncResult, String> {
    let canonical_root = fs::canonicalize(data_root)
        .map_err(|error| format!("DIRECT_SYNC_ROOT_ERROR:{error}"))?;
    let canonical_directory = fs::canonicalize(directory)
        .map_err(|error| format!("DIRECT_SYNC_DIRECTORY_ERROR:{error}"))?;
    ensure_directory(&canonical_root)?;
    ensure_directory(&canonical_directory)?;
    if canonical_directory == canonical_root || canonical_directory.starts_with(&canonical_root) {
        return Err("DIRECT_SYNC_DIRECTORY_INSIDE_DATA_ROOT".to_owned());
    }

    let namespace_id = store.namespace_id();
    let directory_digest = sha256::hex(&sha256::digest_parts(
        b"eliot-search/direct-directory/v1",
        &[&path_identity_bytes(&canonical_directory)],
    ));
    let manifest_root = manifest_root(&canonical_root)?;
    let previous = load_latest_manifest(
        &manifest_root,
        &namespace_id,
        &directory_digest,
    )?;

    let indexed = store.index_directory(&canonical_directory)?;
    let next_entries = entries_from_indexed(&indexed)?;
    let current_sources = store
        .list_sources()
        .into_iter()
        .map(|source| (source.source_id.clone(), source))
        .collect::<BTreeMap<_, _>>();

    let mut missing_sources = 0_usize;
    let mut retired_sources = 0_usize;
    let mut moved_or_rebound_sources = 0_usize;
    if let Some(previous) = &previous {
        for (source_id, old_entry) in &previous.entries {
            if next_entries.contains_key(source_id) {
                continue;
            }
            missing_sources = missing_sources.saturating_add(1);
            let current = current_sources
                .get(source_id)
                .ok_or_else(|| "DIRECT_SYNC_SOURCE_STATE_MISSING".to_owned())?;
            if !current.active {
                continue;
            }
            if current.path_digest != old_entry.path_digest {
                moved_or_rebound_sources = moved_or_rebound_sources.saturating_add(1);
                continue;
            }
            store.retire_source(source_id)?;
            retired_sources = retired_sources.saturating_add(1);
        }
    }

    let generation = previous
        .as_ref()
        .map_or(Ok(1_u64), |manifest| {
            manifest
                .generation
                .checked_add(1)
                .ok_or_else(|| "DIRECT_SYNC_GENERATION_EXHAUSTED".to_owned())
        })?;
    let manifest = build_manifest(
        namespace_id.clone(),
        directory_digest.clone(),
        generation,
        next_entries,
    )?;
    persist_manifest(&manifest_root, &manifest)?;
    let readback = load_manifest_file(&manifest_path(&manifest_root, &manifest))?;
    if readback != manifest {
        return Err("DIRECT_SYNC_MANIFEST_READBACK_MISMATCH".to_owned());
    }

    Ok(DirectorySyncResult {
        namespace_id,
        directory_digest,
        previous_generation: previous.as_ref().map(|manifest| manifest.generation),
        generation,
        previous_sources: previous.as_ref().map_or(0, |manifest| manifest.entries.len()),
        indexed_sources: indexed.len(),
        changed_sources: indexed.iter().filter(|source| source.changed).count(),
        missing_sources,
        retired_sources,
        moved_or_rebound_sources,
        manifest_digest: manifest.manifest_digest,
    })
}

/// Verifies every immutable directory manifest and selects one unambiguous
/// highest generation per directory.
pub(crate) fn verify_directory_manifests(
    data_root: &Path,
    namespace_id: &str,
) -> Result<DirectoryManifestVerification, String> {
    validate_digest(namespace_id, "DIRECT_MANIFEST_NAMESPACE_INVALID")?;
    let canonical_root = fs::canonicalize(data_root)
        .map_err(|error| format!("DIRECT_MANIFEST_ROOT_ERROR:{error}"))?;
    let root = manifest_root(&canonical_root)?;
    let files = manifest_files(&root)?;
    let mut current = BTreeMap::<String, DirectoryManifest>::new();
    let mut generations = BTreeMap::<(String, u64), String>::new();
    let mut highest_generation = 0_u64;

    for path in &files {
        let manifest = load_manifest_file(path)?;
        if manifest.namespace_id != namespace_id {
            return Err("DIRECT_MANIFEST_NAMESPACE_MISMATCH".to_owned());
        }
        let key = (manifest.directory_digest.clone(), manifest.generation);
        if let Some(existing) = generations.insert(key, manifest.manifest_digest.clone()) {
            if existing != manifest.manifest_digest {
                return Err("DIRECT_MANIFEST_GENERATION_AMBIGUOUS".to_owned());
            }
        }
        highest_generation = highest_generation.max(manifest.generation);
        match current.get(&manifest.directory_digest) {
            Some(existing) if existing.generation > manifest.generation => {}
            Some(existing)
                if existing.generation == manifest.generation
                    && existing.manifest_digest != manifest.manifest_digest =>
            {
                return Err("DIRECT_MANIFEST_GENERATION_AMBIGUOUS".to_owned());
            }
            _ => {
                current.insert(manifest.directory_digest.clone(), manifest);
            }
        }
    }

    Ok(DirectoryManifestVerification {
        manifest_files: files.len(),
        directories: current.len(),
        current_entries: current.values().map(|manifest| manifest.entries.len()).sum(),
        highest_generation,
    })
}

fn entries_from_indexed(
    indexed: &[IndexedSource],
) -> Result<BTreeMap<String, DirectoryEntry>, String> {
    if indexed.len() > MAX_MANIFEST_ENTRIES {
        return Err("DIRECT_MANIFEST_ENTRY_LIMIT_EXCEEDED".to_owned());
    }
    let mut entries = BTreeMap::new();
    for source in indexed {
        let entry = DirectoryEntry {
            source_id: source.source_id.clone(),
            path_digest: source.path_digest.clone(),
            revision_id: source.revision_id.clone(),
        };
        validate_entry(&entry)?;
        if entries.insert(entry.source_id.clone(), entry).is_some() {
            return Err("DIRECT_MANIFEST_SOURCE_DUPLICATE".to_owned());
        }
    }
    Ok(entries)
}

fn build_manifest(
    namespace_id: String,
    directory_digest: String,
    generation: u64,
    entries: BTreeMap<String, DirectoryEntry>,
) -> Result<DirectoryManifest, String> {
    validate_digest(&namespace_id, "DIRECT_MANIFEST_NAMESPACE_INVALID")?;
    validate_digest(&directory_digest, "DIRECT_MANIFEST_DIRECTORY_INVALID")?;
    if generation == 0 || entries.len() > MAX_MANIFEST_ENTRIES {
        return Err("DIRECT_MANIFEST_GENERATION_INVALID".to_owned());
    }
    for (source_id, entry) in &entries {
        if source_id != &entry.source_id {
            return Err("DIRECT_MANIFEST_SOURCE_MISMATCH".to_owned());
        }
        validate_entry(entry)?;
    }
    let body = encode_body(&entries)?;
    let manifest_digest = sha256::hex(&sha256::digest_parts(
        b"eliot-search/direct-directory-manifest/v1",
        &[
            namespace_id.as_bytes(),
            directory_digest.as_bytes(),
            &generation.to_be_bytes(),
            body.as_bytes(),
        ],
    ));
    Ok(DirectoryManifest {
        namespace_id,
        directory_digest,
        generation,
        entries,
        manifest_digest,
    })
}

fn encode_manifest(manifest: &DirectoryManifest) -> Result<String, String> {
    let body = encode_body(&manifest.entries)?;
    let encoded = format!(
        "{MANIFEST_HEADER}\t{}\t{}\t{}\t{}\n{body}",
        manifest.namespace_id,
        manifest.directory_digest,
        manifest.generation,
        manifest.manifest_digest,
    );
    if encoded.len() > MAX_MANIFEST_BYTES {
        return Err("DIRECT_MANIFEST_TOO_LARGE".to_owned());
    }
    Ok(encoded)
}

fn encode_body(entries: &BTreeMap<String, DirectoryEntry>) -> Result<String, String> {
    let mut body = String::new();
    for entry in entries.values() {
        validate_entry(entry)?;
        let line = format!(
            "V1\t{}\t{}\t{}\n",
            entry.source_id, entry.path_digest, entry.revision_id,
        );
        if line.len() > MAX_MANIFEST_LINE_BYTES {
            return Err("DIRECT_MANIFEST_LINE_TOO_LARGE".to_owned());
        }
        body.push_str(&line);
        if body.len() > MAX_MANIFEST_BYTES {
            return Err("DIRECT_MANIFEST_TOO_LARGE".to_owned());
        }
    }
    Ok(body)
}

fn persist_manifest(root: &Path, manifest: &DirectoryManifest) -> Result<(), String> {
    let final_path = manifest_path(root, manifest);
    if final_path.exists() {
        let existing = load_manifest_file(&final_path)?;
        return if existing == *manifest {
            Ok(())
        } else {
            Err("DIRECT_MANIFEST_IMMUTABLE_CONFLICT".to_owned())
        };
    }
    let encoded = encode_manifest(manifest)?;
    let timestamp = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map_err(|_| "DIRECT_MANIFEST_CLOCK_INVALID".to_owned())?
        .as_nanos();
    let temporary = root.join(format!(
        ".{}.{}.{}.tmp",
        manifest.directory_digest,
        std::process::id(),
        timestamp,
    ));
    let mut file = OpenOptions::new()
        .create_new(true)
        .write(true)
        .open(&temporary)
        .map_err(|error| format!("DIRECT_MANIFEST_CREATE_ERROR:{error}"))?;
    if let Err(error) = file
        .write_all(encoded.as_bytes())
        .and_then(|()| file.sync_all())
    {
        let _ = fs::remove_file(&temporary);
        return Err(format!("DIRECT_MANIFEST_WRITE_ERROR:{error}"));
    }
    drop(file);
    if let Err(error) = fs::rename(&temporary, &final_path) {
        let _ = fs::remove_file(&temporary);
        if final_path.exists() && load_manifest_file(&final_path)? == *manifest {
            return Ok(());
        }
        return Err(format!("DIRECT_MANIFEST_RENAME_ERROR:{error}"));
    }
    sync_manifest_directory(root)?;
    Ok(())
}

fn load_latest_manifest(
    root: &Path,
    namespace_id: &str,
    directory_digest: &str,
) -> Result<Option<DirectoryManifest>, String> {
    validate_digest(namespace_id, "DIRECT_MANIFEST_NAMESPACE_INVALID")?;
    validate_digest(directory_digest, "DIRECT_MANIFEST_DIRECTORY_INVALID")?;
    let mut latest: Option<DirectoryManifest> = None;
    let mut seen_generation = BTreeMap::<u64, String>::new();
    for path in manifest_files(root)? {
        let Some(file_name) = path.file_name().and_then(|value| value.to_str()) else {
            return Err("DIRECT_MANIFEST_FILENAME_INVALID".to_owned());
        };
        if !file_name.starts_with(directory_digest) {
            continue;
        }
        let manifest = load_manifest_file(&path)?;
        if manifest.namespace_id != namespace_id
            || manifest.directory_digest != directory_digest
        {
            return Err("DIRECT_MANIFEST_BINDING_MISMATCH".to_owned());
        }
        if let Some(existing) = seen_generation.insert(
            manifest.generation,
            manifest.manifest_digest.clone(),
        ) {
            if existing != manifest.manifest_digest {
                return Err("DIRECT_MANIFEST_GENERATION_AMBIGUOUS".to_owned());
            }
        }
        match &latest {
            Some(existing) if existing.generation > manifest.generation => {}
            Some(existing)
                if existing.generation == manifest.generation
                    && existing.manifest_digest != manifest.manifest_digest =>
            {
                return Err("DIRECT_MANIFEST_GENERATION_AMBIGUOUS".to_owned());
            }
            _ => latest = Some(manifest),
        }
    }
    Ok(latest)
}

fn load_manifest_file(path: &Path) -> Result<DirectoryManifest, String> {
    ensure_regular_file(path)?;
    let metadata = fs::metadata(path)
        .map_err(|error| format!("DIRECT_MANIFEST_METADATA_ERROR:{error}"))?;
    if metadata.len() > u64::try_from(MAX_MANIFEST_BYTES).unwrap_or(u64::MAX) {
        return Err("DIRECT_MANIFEST_TOO_LARGE".to_owned());
    }
    let mut text = String::new();
    File::open(path)
        .and_then(|file| {
            file.take(u64::try_from(MAX_MANIFEST_BYTES + 1).unwrap_or(u64::MAX))
                .read_to_string(&mut text)
        })
        .map_err(|error| format!("DIRECT_MANIFEST_READ_ERROR:{error}"))?;
    if text.len() > MAX_MANIFEST_BYTES || !text.ends_with('\n') {
        return Err("DIRECT_MANIFEST_TRUNCATED".to_owned());
    }
    let mut lines = text.split_terminator('\n');
    let header = lines
        .next()
        .ok_or_else(|| "DIRECT_MANIFEST_HEADER_MISSING".to_owned())?;
    let header_fields = header.split('\t').collect::<Vec<_>>();
    if header_fields.len() != 5 || header_fields[0] != MANIFEST_HEADER {
        return Err("DIRECT_MANIFEST_HEADER_INVALID".to_owned());
    }
    let namespace_id = header_fields[1].to_owned();
    let directory_digest = header_fields[2].to_owned();
    let generation = header_fields[3]
        .parse::<u64>()
        .map_err(|_| "DIRECT_MANIFEST_GENERATION_INVALID".to_owned())?;
    let expected_digest = header_fields[4].to_owned();
    validate_digest(&namespace_id, "DIRECT_MANIFEST_NAMESPACE_INVALID")?;
    validate_digest(&directory_digest, "DIRECT_MANIFEST_DIRECTORY_INVALID")?;
    validate_digest(&expected_digest, "DIRECT_MANIFEST_DIGEST_INVALID")?;
    if generation == 0 {
        return Err("DIRECT_MANIFEST_GENERATION_INVALID".to_owned());
    }

    let mut entries = BTreeMap::new();
    for line in lines {
        if line.is_empty() || line.len() > MAX_MANIFEST_LINE_BYTES {
            return Err("DIRECT_MANIFEST_LINE_INVALID".to_owned());
        }
        let fields = line.split('\t').collect::<Vec<_>>();
        if fields.len() != 4 || fields[0] != "V1" {
            return Err("DIRECT_MANIFEST_LINE_INVALID".to_owned());
        }
        let entry = DirectoryEntry {
            source_id: fields[1].to_owned(),
            path_digest: fields[2].to_owned(),
            revision_id: fields[3].to_owned(),
        };
        validate_entry(&entry)?;
        if entries.insert(entry.source_id.clone(), entry).is_some() {
            return Err("DIRECT_MANIFEST_SOURCE_DUPLICATE".to_owned());
        }
        if entries.len() > MAX_MANIFEST_ENTRIES {
            return Err("DIRECT_MANIFEST_ENTRY_LIMIT_EXCEEDED".to_owned());
        }
    }
    let manifest = build_manifest(
        namespace_id,
        directory_digest,
        generation,
        entries,
    )?;
    if manifest.manifest_digest != expected_digest {
        return Err("DIRECT_MANIFEST_DIGEST_MISMATCH".to_owned());
    }
    validate_filename(path, &manifest)?;
    Ok(manifest)
}

fn manifest_root(data_root: &Path) -> Result<PathBuf, String> {
    let control = data_root.join(CONTROL_DIRECTORY);
    ensure_directory(&control)?;
    let root = control.join(MANIFEST_DIRECTORY);
    if !root.exists() {
        fs::create_dir(&root)
            .map_err(|error| format!("DIRECT_MANIFEST_DIRECTORY_CREATE_ERROR:{error}"))?;
        sync_manifest_directory(&control)?;
    }
    ensure_directory(&root)?;
    Ok(root)
}

fn manifest_files(root: &Path) -> Result<Vec<PathBuf>, String> {
    let mut files = Vec::new();
    let mut entries = fs::read_dir(root)
        .map_err(|error| format!("DIRECT_MANIFEST_DIRECTORY_READ_ERROR:{error}"))?
        .collect::<Result<Vec<_>, _>>()
        .map_err(|error| format!("DIRECT_MANIFEST_DIRECTORY_READ_ERROR:{error}"))?;
    entries.sort_by_key(|entry| entry.file_name());
    for entry in entries {
        if files.len() >= MAX_MANIFEST_FILES {
            return Err("DIRECT_MANIFEST_FILE_LIMIT_EXCEEDED".to_owned());
        }
        let path = entry.path();
        let metadata = fs::symlink_metadata(&path)
            .map_err(|error| format!("DIRECT_MANIFEST_METADATA_ERROR:{error}"))?;
        if metadata.file_type().is_symlink() || is_reparse(&metadata) {
            return Err("DIRECT_MANIFEST_LINK_DENIED".to_owned());
        }
        let name = entry.file_name().to_string_lossy().into_owned();
        if name.starts_with('.') && name.ends_with(".tmp") {
            continue;
        }
        if !metadata.is_file() || !name.ends_with(".manifest") {
            return Err("DIRECT_MANIFEST_UNEXPECTED_OBJECT".to_owned());
        }
        files.push(path);
    }
    Ok(files)
}

fn manifest_path(root: &Path, manifest: &DirectoryManifest) -> PathBuf {
    root.join(format!(
        "{}.{}.{}.manifest",
        manifest.directory_digest,
        manifest.generation,
        manifest.manifest_digest,
    ))
}

fn validate_filename(path: &Path, manifest: &DirectoryManifest) -> Result<(), String> {
    let expected = manifest_path(
        path.parent()
            .ok_or_else(|| "DIRECT_MANIFEST_PARENT_MISSING".to_owned())?,
        manifest,
    );
    if expected.file_name() == path.file_name() {
        Ok(())
    } else {
        Err("DIRECT_MANIFEST_FILENAME_MISMATCH".to_owned())
    }
}

fn validate_entry(entry: &DirectoryEntry) -> Result<(), String> {
    validate_digest(&entry.source_id, "DIRECT_MANIFEST_SOURCE_ID_INVALID")?;
    validate_digest(&entry.path_digest, "DIRECT_MANIFEST_PATH_DIGEST_INVALID")?;
    validate_digest(&entry.revision_id, "DIRECT_MANIFEST_REVISION_ID_INVALID")
}

fn validate_digest(value: &str, error: &'static str) -> Result<(), String> {
    if sha256::decode_digest(value).is_some() {
        Ok(())
    } else {
        Err(error.to_owned())
    }
}

fn ensure_directory(path: &Path) -> Result<(), String> {
    let metadata = fs::symlink_metadata(path)
        .map_err(|error| format!("DIRECT_MANIFEST_METADATA_ERROR:{error}"))?;
    if metadata.file_type().is_symlink() || is_reparse(&metadata) || !metadata.is_dir() {
        return Err("DIRECT_MANIFEST_DIRECTORY_INVALID".to_owned());
    }
    Ok(())
}

fn ensure_regular_file(path: &Path) -> Result<(), String> {
    let metadata = fs::symlink_metadata(path)
        .map_err(|error| format!("DIRECT_MANIFEST_METADATA_ERROR:{error}"))?;
    if metadata.file_type().is_symlink() || is_reparse(&metadata) || !metadata.is_file() {
        return Err("DIRECT_MANIFEST_FILE_INVALID".to_owned());
    }
    Ok(())
}

#[cfg(unix)]
fn path_identity_bytes(path: &Path) -> Vec<u8> {
    use std::os::unix::ffi::OsStrExt;
    path.as_os_str().as_bytes().to_vec()
}

#[cfg(windows)]
fn path_identity_bytes(path: &Path) -> Vec<u8> {
    use std::os::windows::ffi::OsStrExt;
    path.as_os_str()
        .encode_wide()
        .flat_map(u16::to_le_bytes)
        .collect()
}

#[cfg(not(any(unix, windows)))]
fn path_identity_bytes(path: &Path) -> Vec<u8> {
    path.to_string_lossy().as_bytes().to_vec()
}

#[cfg(windows)]
fn is_reparse(metadata: &Metadata) -> bool {
    use std::os::windows::fs::MetadataExt;
    const FILE_ATTRIBUTE_REPARSE_POINT: u32 = 0x0000_0400;
    metadata.file_attributes() & FILE_ATTRIBUTE_REPARSE_POINT != 0
}

#[cfg(not(windows))]
fn is_reparse(_metadata: &Metadata) -> bool {
    false
}

#[cfg(unix)]
fn sync_manifest_directory(path: &Path) -> Result<(), String> {
    File::open(path)
        .and_then(|file| file.sync_all())
        .map_err(|error| format!("DIRECT_MANIFEST_DIRECTORY_SYNC_ERROR:{error}"))
}

#[cfg(not(unix))]
fn sync_manifest_directory(_path: &Path) -> Result<(), String> {
    Ok(())
}

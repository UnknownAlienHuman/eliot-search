//! Low-level immutable revision-object and control-log I/O.

#![allow(
    clippy::missing_errors_doc,
    clippy::module_name_repetitions,
    clippy::similar_names,
    clippy::too_many_lines
)]

use std::collections::{BTreeMap, BTreeSet};
use std::fs::{self, File, Metadata, OpenOptions};
use std::io::{Read, Write};
use std::path::{Path, PathBuf};
use std::time::{SystemTime, UNIX_EPOCH};

use crate::development::MAX_SCAN_INPUT_BYTES;
use crate::revision_protection::PROTECTED_OBJECT_EXTENSION;
use crate::sha256;

use super::{RevisionMetadata, verify_plaintext, verify_revision_identity};

const CONTROL_DIRECTORY: &str = "control";
const REVISION_DIRECTORY: &str = "revisions";
const SOURCE_LOG_FILE: &str = "source-events.log";
const SOURCE_LOG_HEADER: &str = "ELIOT_SEARCH_SOURCE_EVENTS_V1";
const ZERO_DIGEST: &str = "0000000000000000000000000000000000000000000000000000000000000000";
const MAX_LOG_BYTES: usize = 512 * 1024 * 1024;
const MAX_LOG_LINE_BYTES: usize = 256 * 1024;
const MAX_REVISION_OBJECT_BYTES: usize = 65 * 1024 * 1024;
const MAX_REVISION_OBJECTS: usize = 2_000_000;

pub(super) fn load_inventory(root: &Path) -> Result<BTreeMap<String, RevisionMetadata>, String> {
    let path = root.join(CONTROL_DIRECTORY).join(SOURCE_LOG_FILE);
    let text = read_text_file(&path, MAX_LOG_BYTES, "DIRECT_CONTROL_LOG_READ_ERROR")?;
    let mut lines = text.split_terminator('\n');
    let header = lines
        .next()
        .ok_or_else(|| "DIRECT_CONTROL_LOG_HEADER_MISSING".to_owned())?
        .trim_end_matches('\r');
    if header != SOURCE_LOG_HEADER {
        return Err("DIRECT_CONTROL_LOG_HEADER_INVALID".to_owned());
    }

    let mut inventory = BTreeMap::new();
    let mut operations = BTreeSet::new();
    let mut sequence = 0_u64;
    let mut previous_digest = ZERO_DIGEST.to_owned();
    let mut events = 0_usize;
    for raw in lines {
        let line = raw.trim_end_matches('\r');
        if line.is_empty() || line.len() > MAX_LOG_LINE_BYTES {
            return Err("DIRECT_CONTROL_LOG_EVENT_INVALID".to_owned());
        }
        let fields = line.split('\t').collect::<Vec<_>>();
        if fields.len() != 13 || fields[0] != "V1" {
            return Err("DIRECT_CONTROL_LOG_EVENT_INVALID".to_owned());
        }
        let observed_sequence = fields[1]
            .parse::<u64>()
            .map_err(|_| "DIRECT_CONTROL_LOG_SEQUENCE_INVALID".to_owned())?;
        sequence = sequence
            .checked_add(1)
            .ok_or_else(|| "DIRECT_CONTROL_LOG_SEQUENCE_EXHAUSTED".to_owned())?;
        if observed_sequence != sequence || fields[2] != previous_digest {
            return Err("DIRECT_CONTROL_LOG_CHAIN_INVALID".to_owned());
        }
        for index in [2_usize, 3, 5, 6, 7, 9, 10, 12] {
            if sha256::decode_digest(fields[index]).is_none() {
                return Err("DIRECT_CONTROL_LOG_DIGEST_INVALID".to_owned());
            }
        }
        if !matches!(fields[4], "A" | "R")
            || !matches!(fields[11], "native" | "path-bound")
            || !operations.insert(fields[3].to_owned())
        {
            return Err("DIRECT_CONTROL_LOG_EVENT_INVALID".to_owned());
        }
        let calculated = sha256::hex(&sha256::digest(
            fields[..12].join("\t").as_bytes(),
        ));
        if calculated != fields[12] {
            return Err("DIRECT_CONTROL_LOG_RECORD_DIGEST_INVALID".to_owned());
        }
        let byte_length = fields[8]
            .parse::<u64>()
            .map_err(|_| "DIRECT_CONTROL_LOG_LENGTH_INVALID".to_owned())?;
        if byte_length > u64::try_from(MAX_SCAN_INPUT_BYTES).unwrap_or(u64::MAX) {
            return Err("DIRECT_CONTROL_LOG_LENGTH_INVALID".to_owned());
        }
        let metadata = RevisionMetadata {
            source_id: fields[5].to_owned(),
            revision_id: fields[6].to_owned(),
            content_digest: fields[7].to_owned(),
            byte_length,
        };
        verify_revision_identity(&metadata)?;
        if let Some(existing) = inventory.insert(metadata.revision_id.clone(), metadata.clone()) {
            if existing != metadata {
                return Err("DIRECT_CONTROL_LOG_REVISION_COLLISION".to_owned());
            }
        }
        previous_digest = fields[12].to_owned();
        events = events.saturating_add(1);
        if events > MAX_REVISION_OBJECTS {
            return Err("DIRECT_CONTROL_LOG_EVENT_LIMIT_EXCEEDED".to_owned());
        }
    }
    if !text.ends_with('\n') {
        return Err("DIRECT_CONTROL_LOG_UNTERMINATED".to_owned());
    }
    Ok(inventory)
}

pub(super) fn load_event_count(root: &Path) -> Result<usize, String> {
    let path = root.join(CONTROL_DIRECTORY).join(SOURCE_LOG_FILE);
    let text = read_text_file(&path, MAX_LOG_BYTES, "DIRECT_CONTROL_LOG_READ_ERROR")?;
    Ok(text.split_terminator('\n').count().saturating_sub(1))
}

pub(super) fn read_plaintext_path(
    path: &Path,
    metadata: &RevisionMetadata,
) -> Result<Vec<u8>, String> {
    let bytes = read_regular_file(
        path,
        MAX_SCAN_INPUT_BYTES,
        "DIRECT_REVISION_READ_ERROR",
    )?;
    verify_plaintext(metadata, &bytes)?;
    Ok(bytes)
}

pub(super) fn read_regular_file(
    path: &Path,
    max_bytes: usize,
    error_prefix: &'static str,
) -> Result<Vec<u8>, String> {
    ensure_regular_file(path)?;
    let metadata = fs::metadata(path)
        .map_err(|error| format!("{error_prefix}:{error}"))?;
    if metadata.len() > u64::try_from(max_bytes).unwrap_or(u64::MAX) {
        return Err(format!("{error_prefix}:TOO_LARGE"));
    }
    let mut bytes = Vec::with_capacity(
        usize::try_from(metadata.len())
            .map_err(|_| format!("{error_prefix}:TOO_LARGE"))?,
    );
    File::open(path)
        .and_then(|mut file| {
            file.take(u64::try_from(max_bytes + 1).unwrap_or(u64::MAX))
                .read_to_end(&mut bytes)
        })
        .map_err(|error| format!("{error_prefix}:{error}"))?;
    if bytes.len() > max_bytes
        || bytes.len() != usize::try_from(metadata.len()).unwrap_or(usize::MAX)
    {
        return Err(format!("{error_prefix}:LENGTH_MISMATCH"));
    }
    Ok(bytes)
}

fn read_text_file(
    path: &Path,
    max_bytes: usize,
    error_prefix: &'static str,
) -> Result<String, String> {
    let bytes = read_regular_file(path, max_bytes, error_prefix)?;
    String::from_utf8(bytes).map_err(|_| format!("{error_prefix}:NOT_UTF8"))
}

pub(super) fn persist_immutable_object(path: &Path, bytes: &[u8]) -> Result<(), String> {
    if bytes.is_empty() || bytes.len() > MAX_REVISION_OBJECT_BYTES {
        return Err("DIRECT_REVISION_PROTECTED_SIZE_INVALID".to_owned());
    }
    let parent = path
        .parent()
        .ok_or_else(|| "DIRECT_REVISION_PARENT_MISSING".to_owned())?;
    ensure_child_directory(parent)?;
    match fs::symlink_metadata(path) {
        Ok(_) => return verify_encoded_object(path, bytes),
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => {}
        Err(_) => return Err("DIRECT_REVISION_OBJECT_INSPECTION_FAILED".to_owned()),
    }
    let timestamp = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map_err(|_| "DIRECT_REVISION_CLOCK_INVALID".to_owned())?
        .as_nanos();
    let file_name = path
        .file_stem()
        .and_then(|value| value.to_str())
        .ok_or_else(|| "DIRECT_REVISION_FILENAME_INVALID".to_owned())?;
    let temporary = parent.join(format!(
        ".{file_name}.{}.{}.dpapi.tmp",
        std::process::id(),
        timestamp,
    ));
    let mut file = OpenOptions::new()
        .create_new(true)
        .write(true)
        .open(&temporary)
        .map_err(|error| format!("DIRECT_REVISION_PROTECTED_CREATE_ERROR:{error}"))?;
    if let Err(error) = file.write_all(bytes).and_then(|()| file.sync_all()) {
        // Windows cannot unlink the still-open staging file.
        drop(file);
        let _ = fs::remove_file(&temporary);
        return Err(format!("DIRECT_REVISION_PROTECTED_WRITE_ERROR:{error}"));
    }
    drop(file);
    // rename may overwrite an existing object on Unix. A hard-link publication
    // is no-clobber on both target platforms, even if another object appeared
    // after the initial absence check. No unsafe replacement fallback exists.
    match fs::hard_link(&temporary, path) {
        Ok(()) => {
            fs::remove_file(&temporary)
                .map_err(|error| format!("DIRECT_REVISION_TEMP_CLEANUP_ERROR:{error}"))?;
            sync_directory(parent)?;
            verify_encoded_object(path, bytes)
        }
        Err(error) => {
            let _ = fs::remove_file(&temporary);
            if error.kind() == std::io::ErrorKind::AlreadyExists {
                verify_encoded_object(path, bytes)
            } else {
                Err(format!("DIRECT_REVISION_PROTECTED_PUBLISH_ERROR:{error}"))
            }
        }
    }
}

fn verify_encoded_object(path: &Path, expected: &[u8]) -> Result<(), String> {
    let existing = read_regular_file(
        path,
        MAX_REVISION_OBJECT_BYTES,
        "DIRECT_REVISION_PROTECTED_READ_ERROR",
    )?;
    if existing == expected {
        Ok(())
    } else {
        Err("DIRECT_REVISION_IMMUTABLE_CONFLICT".to_owned())
    }
}

pub(super) fn remove_plaintext_after_readback(path: &Path) -> Result<(), String> {
    if !path.exists() {
        return Ok(());
    }
    ensure_regular_file(path)?;
    fs::remove_file(path)
        .map_err(|error| format!("DIRECT_REVISION_PLAINTEXT_DELETE_ERROR:{error}"))?;
    let parent = path
        .parent()
        .ok_or_else(|| "DIRECT_REVISION_PARENT_MISSING".to_owned())?;
    sync_directory(parent)
}

pub(super) fn legacy_path(root: &Path, revision_id: &str) -> Result<PathBuf, String> {
    revision_object_path(root, revision_id, "bin")
}

pub(super) fn protected_path(root: &Path, revision_id: &str) -> Result<PathBuf, String> {
    revision_object_path(root, revision_id, PROTECTED_OBJECT_EXTENSION)
}

fn revision_object_path(
    root: &Path,
    revision_id: &str,
    extension: &str,
) -> Result<PathBuf, String> {
    if sha256::decode_digest(revision_id).is_none() {
        return Err("DIRECT_REVISION_ID_INVALID".to_owned());
    }
    Ok(root
        .join(REVISION_DIRECTORY)
        .join(&revision_id[..2])
        .join(format!("{revision_id}.{extension}")))
}

fn ensure_directory(path: &Path) -> Result<(), String> {
    let metadata = fs::symlink_metadata(path)
        .map_err(|error| format!("DIRECT_DIRECTORY_METADATA_ERROR:{error}"))?;
    if metadata.file_type().is_symlink() || is_reparse(&metadata) || !metadata.is_dir() {
        return Err("DIRECT_DIRECTORY_INVALID".to_owned());
    }
    Ok(())
}

fn ensure_child_directory(path: &Path) -> Result<(), String> {
    if !path.exists() {
        fs::create_dir(path)
            .map_err(|error| format!("DIRECT_DIRECTORY_CREATE_ERROR:{error}"))?;
    }
    ensure_directory(path)
}

fn ensure_regular_file(path: &Path) -> Result<(), String> {
    let metadata = fs::symlink_metadata(path)
        .map_err(|error| format!("DIRECT_FILE_METADATA_ERROR:{error}"))?;
    if metadata.file_type().is_symlink() || is_reparse(&metadata) || !metadata.is_file() {
        return Err("DIRECT_FILE_INVALID".to_owned());
    }
    Ok(())
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
fn sync_directory(path: &Path) -> Result<(), String> {
    File::open(path)
        .and_then(|file| file.sync_all())
        .map_err(|error| format!("DIRECT_DIRECTORY_SYNC_ERROR:{error}"))
}

#[cfg(not(unix))]
fn sync_directory(_path: &Path) -> Result<(), String> {
    Ok(())
}

#[cfg(test)]
#[path = "immutable_object_tests.rs"]
mod tests;

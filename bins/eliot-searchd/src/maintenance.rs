//! Explicit maintenance for the development DIRECT corpus.
//!
//! Repair removes only an unterminated final event after the complete preceding
//! SHA-256 chain verifies. GC removes only generated immutable plaintext or
//! DPAPI revision objects that are absent from the verified control-log
//! reference set. Both operations require the data-root owner lock.

use std::collections::BTreeSet;
#[cfg(test)]
use std::fs::OpenOptions;
use std::fs::{self, File, Metadata};
use std::io::Read;
use std::path::{Path, PathBuf};

use crate::sha256;

const CONTROL_DIRECTORY: &str = "control";
const REVISION_DIRECTORY: &str = "revisions";
const SOURCE_LOG_FILE: &str = "source-events.log";
const SOURCE_LOG_HEADER: &str = "ELIOT_SEARCH_SOURCE_EVENTS_V1";
const ZERO_DIGEST: &str = "0000000000000000000000000000000000000000000000000000000000000000";
const MAX_LOG_BYTES: usize = 512 * 1024 * 1024;
const MAX_LOG_LINE_BYTES: usize = 256 * 1024;
const MAX_REVISION_OBJECTS: usize = 2_000_000;

/// Result of explicit torn-tail repair.
#[derive(Clone, Debug, Eq, PartialEq)]
#[cfg(test)]
pub struct LogRepairResult {
    pub(crate) repaired: bool,
    pub(crate) removed_bytes: usize,
    pub(crate) retained_events: usize,
    pub(crate) last_sequence: u64,
    pub(crate) last_digest: String,
}

/// Result of exact unreferenced revision collection.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct GarbageCollectionResult {
    pub(crate) referenced_revisions: usize,
    pub(crate) scanned_objects: usize,
    pub(crate) plaintext_objects: usize,
    pub(crate) protected_objects: usize,
    pub(crate) temporary_objects: usize,
    pub(crate) referenced_plaintext_objects: usize,
    pub(crate) referenced_protected_objects: usize,
    pub(crate) orphan_objects: usize,
    pub(crate) orphan_bytes: u64,
    pub(crate) deleted_objects: usize,
    pub(crate) deleted_bytes: u64,
    pub(crate) unexpected_objects: usize,
    pub(crate) applied: bool,
}

#[derive(Clone, Debug, Eq, PartialEq)]
struct LogInventory {
    referenced_revisions: BTreeSet<String>,
    events: usize,
    last_sequence: u64,
    last_digest: String,
}

/// Removes only an uncommitted unterminated final event in historical fixtures.
#[cfg(test)]
pub fn repair_control_log(root: &Path) -> Result<LogRepairResult, String> {
    let control = root.join(CONTROL_DIRECTORY);
    ensure_directory(&control)?;
    let log_path = control.join(SOURCE_LOG_FILE);
    ensure_regular_file(&log_path)?;
    let mut bytes = Vec::new();
    File::open(&log_path)
        .and_then(|file| {
            file.take(u64::try_from(MAX_LOG_BYTES + 1).unwrap_or(u64::MAX))
                .read_to_end(&mut bytes)
        })
        .map_err(|error| format!("DIRECT_REPAIR_READ_ERROR:{error}"))?;
    if bytes.len() > MAX_LOG_BYTES {
        return Err("DIRECT_REPAIR_LOG_TOO_LARGE".to_owned());
    }
    if bytes.ends_with(b"\n") {
        let inventory = verify_complete_log(&bytes)?;
        return Ok(LogRepairResult {
            repaired: false,
            removed_bytes: 0,
            retained_events: inventory.events,
            last_sequence: inventory.last_sequence,
            last_digest: inventory.last_digest,
        });
    }

    let last_newline = bytes
        .iter()
        .rposition(|byte| *byte == b'\n')
        .ok_or_else(|| "DIRECT_REPAIR_HEADER_NOT_COMMITTED".to_owned())?;
    let prefix_len = last_newline
        .checked_add(1)
        .ok_or_else(|| "DIRECT_REPAIR_OFFSET_OVERFLOW".to_owned())?;
    let tail = &bytes[prefix_len..];
    if tail.is_empty() || tail.len() > MAX_LOG_LINE_BYTES {
        return Err("DIRECT_REPAIR_TAIL_INVALID".to_owned());
    }
    let inventory = verify_complete_log(&bytes[..prefix_len])?;

    let file = OpenOptions::new()
        .write(true)
        .open(&log_path)
        .map_err(|error| format!("DIRECT_REPAIR_OPEN_ERROR:{error}"))?;
    file.set_len(
        u64::try_from(prefix_len).map_err(|_| "DIRECT_REPAIR_OFFSET_OVERFLOW".to_owned())?,
    )
    .and_then(|()| file.sync_all())
    .map_err(|error| format!("DIRECT_REPAIR_TRUNCATE_ERROR:{error}"))?;
    drop(file);
    #[cfg(unix)]
    sync_directory(&control)?;
    #[cfg(not(unix))]
    sync_directory(&control);

    let mut readback = Vec::new();
    File::open(&log_path)
        .and_then(|mut file| file.read_to_end(&mut readback))
        .map_err(|error| format!("DIRECT_REPAIR_READBACK_ERROR:{error}"))?;
    if readback != bytes[..prefix_len] || verify_complete_log(&readback)? != inventory {
        return Err("DIRECT_REPAIR_READBACK_MISMATCH".to_owned());
    }
    Ok(LogRepairResult {
        repaired: true,
        removed_bytes: tail.len(),
        retained_events: inventory.events,
        last_sequence: inventory.last_sequence,
        last_digest: inventory.last_digest,
    })
}

/// Context-free historical fixture wrapper; unavailable to production callers.
#[cfg(test)]
pub fn collect_orphan_revisions(
    root: &Path,
    apply: bool,
) -> Result<GarbageCollectionResult, String> {
    collect_orphan_revisions_with_check(root, apply, &|| Ok(()))
}

/// Finds and optionally deletes generated unreferenced objects with checkpoints.
///
/// The caller supplies its original admitted operation check. Any refusal is
/// returned unchanged, including after possible deletion; this helper performs
/// no rollback or recovery-marker cleanup.
pub fn collect_orphan_revisions_with_check(
    root: &Path,
    apply: bool,
    check: &dyn Fn() -> Result<(), String>,
) -> Result<GarbageCollectionResult, String> {
    check()?;
    let control = root.join(CONTROL_DIRECTORY);
    let revisions = root.join(REVISION_DIRECTORY);
    ensure_directory(&control)?;
    ensure_directory(&revisions)?;
    let log_path = control.join(SOURCE_LOG_FILE);
    ensure_regular_file(&log_path)?;
    let mut log_bytes = Vec::new();
    check()?;
    File::open(&log_path)
        .and_then(|file| {
            file.take(u64::try_from(MAX_LOG_BYTES + 1).unwrap_or(u64::MAX))
                .read_to_end(&mut log_bytes)
        })
        .map_err(|error| format!("DIRECT_GC_LOG_READ_ERROR:{error}"))?;
    check()?;
    if log_bytes.len() > MAX_LOG_BYTES {
        return Err("DIRECT_GC_LOG_TOO_LARGE".to_owned());
    }
    let inventory = verify_complete_log_with_check(&log_bytes, check)?;
    check()?;

    let tally = collect_candidates(&revisions, &inventory, check)?;
    check()?;
    let objects = tally.objects;

    let orphan_objects = objects.iter().filter(|object| !object.referenced).count();
    let orphan_bytes = objects
        .iter()
        .filter(|object| !object.referenced)
        .try_fold(0_u64, |total, object| {
            check()?;
            total
                .checked_add(object.byte_length)
                .ok_or_else(|| "DIRECT_GC_BYTES_OVERFLOW".to_owned())
        })?;
    let mut deleted_objects = 0_usize;
    let mut deleted_bytes = 0_u64;
    check()?;
    if apply {
        for object in objects.iter().filter(|object| !object.referenced) {
            check()?;
            fs::remove_file(&object.path)
                .map_err(|error| format!("DIRECT_GC_DELETE_ERROR:{error}"))?;
            check()?;
            deleted_objects = deleted_objects.saturating_add(1);
            deleted_bytes = deleted_bytes
                .checked_add(object.byte_length)
                .ok_or_else(|| "DIRECT_GC_BYTES_OVERFLOW".to_owned())?;
        }
        for entry in
            fs::read_dir(&revisions).map_err(|error| format!("DIRECT_GC_READ_ERROR:{error}"))?
        {
            check()?;
            let path = entry
                .map_err(|error| format!("DIRECT_GC_READ_ERROR:{error}"))?
                .path();
            check()?;
            if path.is_dir()
                && fs::read_dir(&path)
                    .map_err(|error| format!("DIRECT_GC_READ_ERROR:{error}"))?
                    .next()
                    .is_none()
            {
                check()?;
                let _ = fs::remove_dir(&path);
                check()?;
            }
            check()?;
        }
        check()?;
        #[cfg(unix)]
        sync_directory(&revisions)?;
        #[cfg(not(unix))]
        sync_directory(&revisions);
        check()?;
    }

    check()?;
    Ok(GarbageCollectionResult {
        referenced_revisions: inventory.referenced_revisions.len(),
        scanned_objects: objects.len(),
        plaintext_objects: tally.plaintext_objects,
        protected_objects: tally.protected_objects,
        temporary_objects: tally.temporary_objects,
        referenced_plaintext_objects: tally.referenced_plaintext_objects,
        referenced_protected_objects: tally.referenced_protected_objects,
        orphan_objects,
        orphan_bytes,
        deleted_objects,
        deleted_bytes,
        unexpected_objects: tally.unexpected_objects,
        applied: apply,
    })
}

#[derive(Clone, Debug, Default)]
struct ObjectInventory {
    objects: Vec<ObjectCandidate>,
    unexpected_objects: usize,
    plaintext_objects: usize,
    protected_objects: usize,
    temporary_objects: usize,
    referenced_plaintext_objects: usize,
    referenced_protected_objects: usize,
}

/// Walks every revision shard and classifies each generated object without
/// deleting anything.
fn collect_candidates(
    revisions: &Path,
    inventory: &LogInventory,
    check: &dyn Fn() -> Result<(), String>,
) -> Result<ObjectInventory, String> {
    check()?;
    let mut tally = ObjectInventory::default();
    let shard_entries = sorted_entries(revisions, check)?;
    for shard_entry in shard_entries {
        check()?;
        let shard_path = shard_entry.path();
        let shard_metadata = fs::symlink_metadata(&shard_path)
            .map_err(|error| format!("DIRECT_GC_METADATA_ERROR:{error}"))?;
        check()?;
        let shard_name = shard_entry.file_name();
        let shard_name = shard_name.to_string_lossy();
        if shard_metadata.file_type().is_symlink()
            || is_reparse(&shard_metadata)
            || !shard_metadata.is_dir()
            || !valid_shard_name(&shard_name)
        {
            tally.unexpected_objects = tally.unexpected_objects.saturating_add(1);
            continue;
        }
        check()?;
        let entries = sorted_entries(&shard_path, check)?;
        for entry in entries {
            check()?;
            if tally.objects.len() >= MAX_REVISION_OBJECTS {
                return Err("DIRECT_GC_OBJECT_LIMIT_EXCEEDED".to_owned());
            }
            let path = entry.path();
            let metadata = fs::symlink_metadata(&path)
                .map_err(|error| format!("DIRECT_GC_METADATA_ERROR:{error}"))?;
            check()?;
            if metadata.file_type().is_symlink() || is_reparse(&metadata) || !metadata.is_file() {
                tally.unexpected_objects = tally.unexpected_objects.saturating_add(1);
                continue;
            }
            let name = entry.file_name().to_string_lossy().into_owned();
            match classify_generated_object(&name) {
                GeneratedObject::Revision {
                    revision_id,
                    format,
                } => {
                    if !revision_id.starts_with(shard_name.as_ref()) {
                        tally.unexpected_objects = tally.unexpected_objects.saturating_add(1);
                        continue;
                    }
                    let referenced = inventory.referenced_revisions.contains(&revision_id);
                    match format {
                        RevisionObjectFormat::Plaintext => {
                            tally.plaintext_objects = tally.plaintext_objects.saturating_add(1);
                            if referenced {
                                tally.referenced_plaintext_objects =
                                    tally.referenced_plaintext_objects.saturating_add(1);
                            }
                        }
                        RevisionObjectFormat::Protected => {
                            tally.protected_objects = tally.protected_objects.saturating_add(1);
                            if referenced {
                                tally.referenced_protected_objects =
                                    tally.referenced_protected_objects.saturating_add(1);
                            }
                        }
                    }
                    tally.objects.push(ObjectCandidate {
                        path,
                        byte_length: metadata.len(),
                        referenced,
                    });
                }
                GeneratedObject::Temporary => {
                    tally.temporary_objects = tally.temporary_objects.saturating_add(1);
                    tally.objects.push(ObjectCandidate {
                        path,
                        byte_length: metadata.len(),
                        referenced: false,
                    });
                }
                GeneratedObject::Unexpected => {
                    tally.unexpected_objects = tally.unexpected_objects.saturating_add(1);
                }
            }
            check()?;
        }
        check()?;
    }
    check()?;
    Ok(tally)
}

fn sorted_entries(
    directory: &Path,
    check: &dyn Fn() -> Result<(), String>,
) -> Result<Vec<fs::DirEntry>, String> {
    check()?;
    let mut entries = fs::read_dir(directory)
        .map_err(|error| format!("DIRECT_GC_READ_ERROR:{error}"))?
        .map(|entry| {
            check()?;
            let entry = entry.map_err(|error| format!("DIRECT_GC_READ_ERROR:{error}"))?;
            check()?;
            Ok(entry)
        })
        .collect::<Result<Vec<_>, String>>()?;
    check()?;
    entries.sort_by_key(fs::DirEntry::file_name);
    check()?;
    Ok(entries)
}

#[derive(Clone, Debug)]
struct ObjectCandidate {
    path: PathBuf,
    byte_length: u64,
    referenced: bool,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum RevisionObjectFormat {
    Plaintext,
    Protected,
}

#[derive(Clone, Debug, Eq, PartialEq)]
enum GeneratedObject {
    Revision {
        revision_id: String,
        format: RevisionObjectFormat,
    },
    Temporary,
    Unexpected,
}

fn classify_generated_object(name: &str) -> GeneratedObject {
    for (suffix, format) in [
        (".bin", RevisionObjectFormat::Plaintext),
        (".dpapi", RevisionObjectFormat::Protected),
    ] {
        if let Some(revision_id) = name.strip_suffix(suffix)
            && sha256::decode_digest(revision_id).is_some()
        {
            return GeneratedObject::Revision {
                revision_id: revision_id.to_owned(),
                format,
            };
        }
    }
    if name.starts_with('.')
        && name.len() >= ".tmp".len()
        && name.as_bytes()[name.len() - ".tmp".len()..].eq_ignore_ascii_case(b".tmp")
    {
        let body = &name[1..name.len().saturating_sub(4)];
        if body
            .split('.')
            .next()
            .and_then(sha256::decode_digest)
            .is_some()
        {
            return GeneratedObject::Temporary;
        }
    }
    GeneratedObject::Unexpected
}

#[cfg(test)]
fn verify_complete_log(bytes: &[u8]) -> Result<LogInventory, String> {
    verify_complete_log_with_check(bytes, &|| Ok(()))
}

fn verify_complete_log_with_check(
    bytes: &[u8],
    check: &dyn Fn() -> Result<(), String>,
) -> Result<LogInventory, String> {
    check()?;
    if !bytes.ends_with(b"\n") {
        return Err("DIRECT_CONTROL_LOG_UNTERMINATED".to_owned());
    }
    let text = core::str::from_utf8(bytes).map_err(|_| "DIRECT_CONTROL_LOG_NOT_UTF8".to_owned())?;
    check()?;
    let mut lines = text.split_terminator('\n');
    let header = lines
        .next()
        .ok_or_else(|| "DIRECT_CONTROL_LOG_HEADER_MISSING".to_owned())?
        .trim_end_matches('\r');
    if header != SOURCE_LOG_HEADER {
        return Err("DIRECT_CONTROL_LOG_HEADER_INVALID".to_owned());
    }

    let mut referenced_revisions = BTreeSet::new();
    let mut operations = BTreeSet::new();
    let mut last_sequence = 0_u64;
    let mut last_digest = ZERO_DIGEST.to_owned();
    let mut events = 0_usize;
    for raw_line in lines {
        check()?;
        let line = raw_line.trim_end_matches('\r');
        if line.is_empty() || line.len() > MAX_LOG_LINE_BYTES {
            return Err("DIRECT_CONTROL_LOG_EVENT_INVALID".to_owned());
        }
        let fields = line.split('\t').collect::<Vec<_>>();
        if fields.len() != 13 || fields[0] != "V1" {
            return Err("DIRECT_CONTROL_LOG_EVENT_INVALID".to_owned());
        }
        let sequence = fields[1]
            .parse::<u64>()
            .map_err(|_| "DIRECT_CONTROL_LOG_SEQUENCE_INVALID".to_owned())?;
        if sequence
            != last_sequence
                .checked_add(1)
                .ok_or_else(|| "DIRECT_CONTROL_LOG_SEQUENCE_EXHAUSTED".to_owned())?
            || fields[2] != last_digest
        {
            return Err("DIRECT_CONTROL_LOG_CHAIN_INVALID".to_owned());
        }
        if !matches!(fields[4], "A" | "R") || !matches!(fields[11], "native" | "path-bound") {
            return Err("DIRECT_CONTROL_LOG_ENUM_INVALID".to_owned());
        }
        fields[8]
            .parse::<u64>()
            .map_err(|_| "DIRECT_CONTROL_LOG_LENGTH_INVALID".to_owned())?;
        for index in [2_usize, 3, 5, 6, 7, 9, 10, 12] {
            if sha256::decode_digest(fields[index]).is_none() {
                return Err("DIRECT_CONTROL_LOG_DIGEST_INVALID".to_owned());
            }
        }
        if !operations.insert(fields[3].to_owned()) {
            return Err("DIRECT_CONTROL_LOG_OPERATION_DUPLICATE".to_owned());
        }
        let calculated = sha256::hex(&sha256::digest(fields[..12].join("\t").as_bytes()));
        if calculated != fields[12] {
            return Err("DIRECT_CONTROL_LOG_RECORD_DIGEST_INVALID".to_owned());
        }
        referenced_revisions.insert(fields[6].to_owned());
        last_sequence = sequence;
        fields[12].clone_into(&mut last_digest);
        events = events.saturating_add(1);
        if events > MAX_REVISION_OBJECTS {
            return Err("DIRECT_CONTROL_LOG_EVENT_LIMIT_EXCEEDED".to_owned());
        }
        check()?;
    }
    check()?;
    Ok(LogInventory {
        referenced_revisions,
        events,
        last_sequence,
        last_digest,
    })
}

fn valid_shard_name(value: &str) -> bool {
    value.len() == 2 && value.as_bytes().iter().all(u8::is_ascii_hexdigit)
}

fn ensure_directory(path: &Path) -> Result<(), String> {
    let metadata = fs::symlink_metadata(path)
        .map_err(|error| format!("DIRECT_MAINTENANCE_METADATA_ERROR:{error}"))?;
    if metadata.file_type().is_symlink() || is_reparse(&metadata) || !metadata.is_dir() {
        return Err("DIRECT_MAINTENANCE_DIRECTORY_INVALID".to_owned());
    }
    Ok(())
}

fn ensure_regular_file(path: &Path) -> Result<(), String> {
    let metadata = fs::symlink_metadata(path)
        .map_err(|error| format!("DIRECT_MAINTENANCE_METADATA_ERROR:{error}"))?;
    if metadata.file_type().is_symlink() || is_reparse(&metadata) || !metadata.is_file() {
        return Err("DIRECT_MAINTENANCE_FILE_INVALID".to_owned());
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
        .map_err(|error| format!("DIRECT_MAINTENANCE_SYNC_ERROR:{error}"))
}

#[cfg(not(unix))]
const fn sync_directory(_path: &Path) {}

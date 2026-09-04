//! Snapshot generation administration.
//!
//! Every operation runs while the caller holds the data-root owner lock. Reclaim
//! is two-phase: an immutable exact plan is persisted first, then execution
//! revalidates every manifest and candidate object before deletion. A failed
//! execution can resume from the same plan; an invalid manifest blocks deletion.

use std::collections::BTreeSet;
use std::fs::{self, File, OpenOptions};
use std::io::{Read, Write};
use std::path::{Path, PathBuf};

use crate::sha256::{Sha256Digest, digest_bytes};
use crate::snapshot::{
    SnapshotGap, SnapshotManifest, activate_snapshot, load_manifest_by_id,
    read_verified_object,
};

const MAX_ADMIN_FILE_BYTES: usize = 64 * 1024 * 1024;
const MAX_ADMIN_MANIFEST_BYTES: usize = 64 * 1024 * 1024;
const MAX_SNAPSHOTS: usize = 100_000;
const MAX_OBJECTS: usize = 1_000_000;
const MAX_RECLAIM_BYTES: u64 = 1024 * 1024 * 1024;
const RECLAIM_FORMAT: &str = "ELIOT_SEARCH_RECLAIM_V1";

#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) struct SnapshotGenerationSummary {
    pub(crate) snapshot_id: Sha256Digest,
    pub(crate) current: bool,
    pub(crate) valid: bool,
    pub(crate) files: usize,
    pub(crate) gaps: usize,
    pub(crate) total_bytes: u64,
    pub(crate) error: Option<String>,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) struct SnapshotCatalogReport {
    pub(crate) current_snapshot_id: Option<Sha256Digest>,
    pub(crate) generations: Vec<SnapshotGenerationSummary>,
    pub(crate) complete: bool,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) struct SnapshotVerificationReport {
    pub(crate) snapshot_id: Sha256Digest,
    pub(crate) source_root_digest: Sha256Digest,
    pub(crate) expected_files: usize,
    pub(crate) verified_files: usize,
    pub(crate) expected_bytes: u64,
    pub(crate) verified_bytes: u64,
    pub(crate) manifest_gaps: Vec<SnapshotGap>,
    pub(crate) runtime_gaps: Vec<SnapshotGap>,
    pub(crate) complete: bool,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) struct SnapshotActivationReceipt {
    pub(crate) snapshot_id: Sha256Digest,
    pub(crate) source_root_digest: Sha256Digest,
    pub(crate) files: usize,
    pub(crate) gaps: usize,
    pub(crate) total_bytes: u64,
}

#[derive(Clone, Debug, Eq, PartialEq)]
struct ReclaimObject {
    digest: Sha256Digest,
    byte_length: u64,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) struct SnapshotReclaimPlan {
    pub(crate) plan_id: Sha256Digest,
    pub(crate) snapshot_count: usize,
    pub(crate) referenced_object_count: usize,
    pub(crate) candidate_count: usize,
    pub(crate) candidate_bytes: u64,
    pub(crate) plan_path: PathBuf,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) struct SnapshotReclaimReceipt {
    pub(crate) plan_id: Sha256Digest,
    pub(crate) removed_objects: usize,
    pub(crate) already_absent_objects: usize,
    pub(crate) reclaimed_bytes: u64,
    pub(crate) complete: bool,
    pub(crate) completion_path: PathBuf,
}

pub(crate) fn list_snapshots(data_root: &Path) -> Result<SnapshotCatalogReport, String> {
    let data_root = validate_data_root(data_root)?;
    let current_snapshot_id = read_current(&data_root)?;
    let snapshot_ids = enumerate_snapshot_ids(&data_root)?;
    let mut generations = Vec::with_capacity(snapshot_ids.len());
    for snapshot_id in snapshot_ids {
        match load_manifest_by_id(&data_root, snapshot_id) {
            Ok(manifest) => generations.push(SnapshotGenerationSummary {
                snapshot_id,
                current: current_snapshot_id == Some(snapshot_id),
                valid: true,
                files: manifest.entries.len(),
                gaps: manifest.gaps.len(),
                total_bytes: manifest.total_bytes,
                error: None,
            }),
            Err(error) => generations.push(SnapshotGenerationSummary {
                snapshot_id,
                current: current_snapshot_id == Some(snapshot_id),
                valid: false,
                files: 0,
                gaps: 0,
                total_bytes: 0,
                error: Some(sanitize_reason(&error)),
            }),
        }
    }
    let current_present = current_snapshot_id.is_none_or(|current| {
        generations
            .iter()
            .any(|generation| generation.snapshot_id == current && generation.valid)
    });
    let complete = current_present && generations.iter().all(|generation| generation.valid);
    Ok(SnapshotCatalogReport {
        current_snapshot_id,
        generations,
        complete,
    })
}

pub(crate) fn verify_snapshot(
    data_root: &Path,
    snapshot_id: Sha256Digest,
) -> Result<SnapshotVerificationReport, String> {
    let data_root = validate_data_root(data_root)?;
    let manifest = load_manifest_by_id(&data_root, snapshot_id)?;
    let mut verified_files = 0_usize;
    let mut verified_bytes = 0_u64;
    let mut runtime_gaps = Vec::new();
    for entry in &manifest.entries {
        match read_verified_object(&data_root, entry) {
            Ok(text) => {
                verified_files = verified_files.saturating_add(1);
                verified_bytes = verified_bytes
                    .checked_add(
                        u64::try_from(text.len())
                            .map_err(|_| "SNAPSHOT_VERIFY_BYTE_OVERFLOW".to_owned())?,
                    )
                    .ok_or_else(|| "SNAPSHOT_VERIFY_BYTE_OVERFLOW".to_owned())?;
            }
            Err(error) => runtime_gaps.push(SnapshotGap {
                relative_path: entry.relative_path.clone(),
                reason: sanitize_reason(&error),
            }),
        }
    }
    let complete = runtime_gaps.is_empty()
        && manifest.gaps.is_empty()
        && verified_files == manifest.entries.len()
        && verified_bytes == manifest.total_bytes;
    Ok(SnapshotVerificationReport {
        snapshot_id,
        source_root_digest: manifest.source_root_digest,
        expected_files: manifest.entries.len(),
        verified_files,
        expected_bytes: manifest.total_bytes,
        verified_bytes,
        manifest_gaps: manifest.gaps,
        runtime_gaps,
        complete,
    })
}

pub(crate) fn activate_generation(
    data_root: &Path,
    snapshot_id: Sha256Digest,
) -> Result<SnapshotActivationReceipt, String> {
    let data_root = validate_data_root(data_root)?;
    let verification = verify_snapshot(&data_root, snapshot_id)?;
    if !verification.complete {
        return Err("SNAPSHOT_ACTIVATION_VERIFICATION_INCOMPLETE".to_owned());
    }
    let manifest = activate_snapshot(&data_root, snapshot_id)?;
    Ok(SnapshotActivationReceipt {
        snapshot_id,
        source_root_digest: manifest.source_root_digest,
        files: manifest.entries.len(),
        gaps: manifest.gaps.len(),
        total_bytes: manifest.total_bytes,
    })
}

pub(crate) fn prepare_reclaim(
    data_root: &Path,
) -> Result<SnapshotReclaimPlan, String> {
    let data_root = validate_data_root(data_root)?;
    let (manifests, referenced) = load_all_valid_manifests(&data_root)?;
    let objects = enumerate_objects(&data_root)?;
    let mut candidates = Vec::new();
    let mut candidate_bytes = 0_u64;
    for object in objects {
        if referenced.contains(&object.digest) {
            continue;
        }
        candidate_bytes = candidate_bytes
            .checked_add(object.byte_length)
            .ok_or_else(|| "SNAPSHOT_RECLAIM_BYTE_OVERFLOW".to_owned())?;
        if candidate_bytes > MAX_RECLAIM_BYTES {
            return Err("SNAPSHOT_RECLAIM_BYTE_LIMIT_EXCEEDED".to_owned());
        }
        candidates.push(object);
    }
    candidates.sort_by_key(|candidate| candidate.digest);
    let body = encode_reclaim_body(
        manifests.len(),
        referenced.len(),
        &candidates,
        candidate_bytes,
    )?;
    let plan_id = digest_bytes(&body);
    let reclaim_root = data_root.join("reclaim");
    fs::create_dir_all(&reclaim_root)
        .map_err(|error| format!("SNAPSHOT_RECLAIM_DIRECTORY_ERROR:{error}"))?;
    let plan_path = reclaim_root.join(format!("{}.tsv", plan_id.hex()));
    persist_exact_file(&plan_path, &body)?;
    Ok(SnapshotReclaimPlan {
        plan_id,
        snapshot_count: manifests.len(),
        referenced_object_count: referenced.len(),
        candidate_count: candidates.len(),
        candidate_bytes,
        plan_path,
    })
}

pub(crate) fn execute_reclaim(
    data_root: &Path,
    plan_id: Sha256Digest,
) -> Result<SnapshotReclaimReceipt, String> {
    let data_root = validate_data_root(data_root)?;
    let plan_path = data_root
        .join("reclaim")
        .join(format!("{}.tsv", plan_id.hex()));
    let body = read_bounded(&plan_path, MAX_ADMIN_MANIFEST_BYTES)?;
    if digest_bytes(&body) != plan_id {
        return Err("SNAPSHOT_RECLAIM_PLAN_DIGEST_MISMATCH".to_owned());
    }
    let plan = decode_reclaim_body(&body)?;
    let (_, current_referenced) = load_all_valid_manifests(&data_root)?;
    if plan
        .objects
        .iter()
        .any(|object| current_referenced.contains(&object.digest))
    {
        return Err("SNAPSHOT_RECLAIM_CANDIDATE_NOW_REFERENCED".to_owned());
    }

    for object in &plan.objects {
        let path = object_path(&data_root, object.digest);
        if !path.exists() {
            continue;
        }
        verify_reclaim_object(&path, object)?;
    }

    let mut removed_objects = 0_usize;
    let mut already_absent_objects = 0_usize;
    let mut reclaimed_bytes = 0_u64;
    for object in &plan.objects {
        let path = object_path(&data_root, object.digest);
        if !path.exists() {
            already_absent_objects = already_absent_objects.saturating_add(1);
            continue;
        }
        verify_reclaim_object(&path, object)?;
        fs::remove_file(&path)
            .map_err(|error| format!("SNAPSHOT_RECLAIM_DELETE_ERROR:{error}"))?;
        if path.exists() {
            return Err("SNAPSHOT_RECLAIM_DELETE_READBACK_FAILED".to_owned());
        }
        removed_objects = removed_objects.saturating_add(1);
        reclaimed_bytes = reclaimed_bytes
            .checked_add(object.byte_length)
            .ok_or_else(|| "SNAPSHOT_RECLAIM_BYTE_OVERFLOW".to_owned())?;
    }

    let completion_path = data_root
        .join("reclaim")
        .join(format!("{}.done", plan_id.hex()));
    let completion = format!(
        "ELIOT_SEARCH_RECLAIM_COMPLETE_V1\t{}\t{}\t{}\t{}\n",
        plan_id.hex(),
        removed_objects,
        already_absent_objects,
        reclaimed_bytes,
    );
    persist_exact_file(&completion_path, completion.as_bytes())?;
    Ok(SnapshotReclaimReceipt {
        plan_id,
        removed_objects,
        already_absent_objects,
        reclaimed_bytes,
        complete: true,
        completion_path,
    })
}

struct DecodedReclaimPlan {
    objects: Vec<ReclaimObject>,
}

fn load_all_valid_manifests(
    data_root: &Path,
) -> Result<(Vec<SnapshotManifest>, BTreeSet<Sha256Digest>), String> {
    let snapshot_ids = enumerate_snapshot_ids(data_root)?;
    let mut manifests = Vec::with_capacity(snapshot_ids.len());
    let mut referenced = BTreeSet::new();
    for snapshot_id in snapshot_ids {
        let manifest = load_manifest_by_id(data_root, snapshot_id)
            .map_err(|error| format!("SNAPSHOT_RECLAIM_BLOCKED_INVALID_MANIFEST:{error}"))?;
        for entry in &manifest.entries {
            referenced.insert(entry.object_digest);
        }
        manifests.push(manifest);
    }
    Ok((manifests, referenced))
}

fn enumerate_snapshot_ids(data_root: &Path) -> Result<Vec<Sha256Digest>, String> {
    let snapshots_root = data_root.join("snapshots");
    if !snapshots_root.exists() {
        return Ok(Vec::new());
    }
    let metadata = fs::symlink_metadata(&snapshots_root)
        .map_err(|error| format!("SNAPSHOT_DIRECTORY_METADATA_ERROR:{error}"))?;
    if metadata.file_type().is_symlink() || !metadata.is_dir() {
        return Err("SNAPSHOT_DIRECTORY_INVALID".to_owned());
    }
    let mut ids = Vec::new();
    for entry in fs::read_dir(&snapshots_root)
        .map_err(|error| format!("SNAPSHOT_DIRECTORY_READ_ERROR:{error}"))?
    {
        let entry = entry
            .map_err(|error| format!("SNAPSHOT_DIRECTORY_ENTRY_ERROR:{error}"))?;
        let metadata = entry
            .file_type()
            .map_err(|error| format!("SNAPSHOT_DIRECTORY_ENTRY_ERROR:{error}"))?;
        if metadata.is_symlink() || !metadata.is_dir() {
            return Err("SNAPSHOT_GENERATION_ENTRY_INVALID".to_owned());
        }
        let name = entry
            .file_name()
            .into_string()
            .map_err(|_| "SNAPSHOT_GENERATION_ID_INVALID".to_owned())?;
        ids.push(Sha256Digest::from_hex(&name)?);
        if ids.len() > MAX_SNAPSHOTS {
            return Err("SNAPSHOT_GENERATION_LIMIT_EXCEEDED".to_owned());
        }
    }
    ids.sort();
    if ids.windows(2).any(|pair| pair[0] == pair[1]) {
        return Err("SNAPSHOT_GENERATION_DUPLICATE".to_owned());
    }
    Ok(ids)
}

fn enumerate_objects(data_root: &Path) -> Result<Vec<ReclaimObject>, String> {
    let objects_root = data_root.join("objects");
    if !objects_root.exists() {
        return Ok(Vec::new());
    }
    let metadata = fs::symlink_metadata(&objects_root)
        .map_err(|error| format!("SNAPSHOT_OBJECT_ROOT_ERROR:{error}"))?;
    if metadata.file_type().is_symlink() || !metadata.is_dir() {
        return Err("SNAPSHOT_OBJECT_ROOT_INVALID".to_owned());
    }
    let mut objects = Vec::new();
    for prefix in fs::read_dir(&objects_root)
        .map_err(|error| format!("SNAPSHOT_OBJECT_ROOT_ERROR:{error}"))?
    {
        let prefix = prefix
            .map_err(|error| format!("SNAPSHOT_OBJECT_PREFIX_ERROR:{error}"))?;
        let prefix_type = prefix
            .file_type()
            .map_err(|error| format!("SNAPSHOT_OBJECT_PREFIX_ERROR:{error}"))?;
        if prefix_type.is_symlink() || !prefix_type.is_dir() {
            return Err("SNAPSHOT_OBJECT_PREFIX_INVALID".to_owned());
        }
        let prefix_name = prefix
            .file_name()
            .into_string()
            .map_err(|_| "SNAPSHOT_OBJECT_PREFIX_INVALID".to_owned())?;
        if prefix_name.len() != 2 || !prefix_name.bytes().all(|byte| byte.is_ascii_hexdigit()) {
            return Err("SNAPSHOT_OBJECT_PREFIX_INVALID".to_owned());
        }
        for file in fs::read_dir(prefix.path())
            .map_err(|error| format!("SNAPSHOT_OBJECT_DIRECTORY_ERROR:{error}"))?
        {
            let file = file
                .map_err(|error| format!("SNAPSHOT_OBJECT_ENTRY_ERROR:{error}"))?;
            let file_type = file
                .file_type()
                .map_err(|error| format!("SNAPSHOT_OBJECT_ENTRY_ERROR:{error}"))?;
            if file_type.is_symlink() || !file_type.is_file() {
                return Err("SNAPSHOT_OBJECT_ENTRY_INVALID".to_owned());
            }
            let name = file
                .file_name()
                .into_string()
                .map_err(|_| "SNAPSHOT_OBJECT_NAME_INVALID".to_owned())?;
            let hex = name
                .strip_suffix(".utf8")
                .ok_or_else(|| "SNAPSHOT_OBJECT_NAME_INVALID".to_owned())?;
            let digest = Sha256Digest::from_hex(hex)?;
            if &hex[..2] != prefix_name.to_ascii_lowercase() {
                return Err("SNAPSHOT_OBJECT_PREFIX_MISMATCH".to_owned());
            }
            let file_metadata = file
                .metadata()
                .map_err(|error| format!("SNAPSHOT_OBJECT_METADATA_ERROR:{error}"))?;
            let object = ReclaimObject {
                digest,
                byte_length: file_metadata.len(),
            };
            verify_reclaim_object(&file.path(), &object)?;
            objects.push(object);
            if objects.len() > MAX_OBJECTS {
                return Err("SNAPSHOT_OBJECT_LIMIT_EXCEEDED".to_owned());
            }
        }
    }
    objects.sort_by_key(|object| object.digest);
    if objects.windows(2).any(|pair| pair[0].digest == pair[1].digest) {
        return Err("SNAPSHOT_OBJECT_DUPLICATE".to_owned());
    }
    Ok(objects)
}

fn read_current(data_root: &Path) -> Result<Option<Sha256Digest>, String> {
    let current_path = data_root.join("CURRENT");
    if !current_path.exists() {
        return Ok(None);
    }
    let bytes = read_bounded(&current_path, 256)?;
    let text = std::str::from_utf8(&bytes)
        .map_err(|_| "SNAPSHOT_CURRENT_INVALID_UTF8".to_owned())?;
    Ok(Some(Sha256Digest::from_hex(text.trim())?))
}

fn encode_reclaim_body(
    snapshot_count: usize,
    referenced_count: usize,
    candidates: &[ReclaimObject],
    candidate_bytes: u64,
) -> Result<Vec<u8>, String> {
    let mut output = Vec::new();
    writeln!(
        output,
        "{RECLAIM_FORMAT}\t{}\t{}\t{}\t{}",
        snapshot_count,
        referenced_count,
        candidates.len(),
        candidate_bytes,
    )
    .map_err(|error| format!("SNAPSHOT_RECLAIM_PLAN_ENCODE_ERROR:{error}"))?;
    for candidate in candidates {
        writeln!(
            output,
            "O\t{}\t{}",
            candidate.digest.hex(),
            candidate.byte_length,
        )
        .map_err(|error| format!("SNAPSHOT_RECLAIM_PLAN_ENCODE_ERROR:{error}"))?;
    }
    if output.len() > MAX_ADMIN_MANIFEST_BYTES {
        return Err("SNAPSHOT_RECLAIM_PLAN_TOO_LARGE".to_owned());
    }
    Ok(output)
}

fn decode_reclaim_body(bytes: &[u8]) -> Result<DecodedReclaimPlan, String> {
    let text = std::str::from_utf8(bytes)
        .map_err(|_| "SNAPSHOT_RECLAIM_PLAN_INVALID_UTF8".to_owned())?;
    let mut lines = text.lines();
    let header = lines
        .next()
        .ok_or_else(|| "SNAPSHOT_RECLAIM_PLAN_HEADER_MISSING".to_owned())?
        .split('\t')
        .collect::<Vec<_>>();
    if header.len() != 5 || header[0] != RECLAIM_FORMAT {
        return Err("SNAPSHOT_RECLAIM_PLAN_HEADER_INVALID".to_owned());
    }
    let expected_objects = header[3]
        .parse::<usize>()
        .map_err(|_| "SNAPSHOT_RECLAIM_PLAN_COUNT_INVALID".to_owned())?;
    let expected_bytes = header[4]
        .parse::<u64>()
        .map_err(|_| "SNAPSHOT_RECLAIM_PLAN_BYTES_INVALID".to_owned())?;
    let mut objects = Vec::with_capacity(expected_objects);
    let mut total_bytes = 0_u64;
    for line in lines {
        let fields = line.split('\t').collect::<Vec<_>>();
        if fields.len() != 3 || fields[0] != "O" {
            return Err("SNAPSHOT_RECLAIM_PLAN_ENTRY_INVALID".to_owned());
        }
        let object = ReclaimObject {
            digest: Sha256Digest::from_hex(fields[1])?,
            byte_length: fields[2]
                .parse::<u64>()
                .map_err(|_| "SNAPSHOT_RECLAIM_PLAN_LENGTH_INVALID".to_owned())?,
        };
        total_bytes = total_bytes
            .checked_add(object.byte_length)
            .ok_or_else(|| "SNAPSHOT_RECLAIM_BYTE_OVERFLOW".to_owned())?;
        objects.push(object);
        if objects.len() > MAX_OBJECTS {
            return Err("SNAPSHOT_RECLAIM_OBJECT_LIMIT_EXCEEDED".to_owned());
        }
    }
    if objects.len() != expected_objects
        || total_bytes != expected_bytes
        || objects.windows(2).any(|pair| pair[0].digest >= pair[1].digest)
    {
        return Err("SNAPSHOT_RECLAIM_PLAN_ACCOUNTING_MISMATCH".to_owned());
    }
    Ok(DecodedReclaimPlan { objects })
}

fn verify_reclaim_object(path: &Path, object: &ReclaimObject) -> Result<(), String> {
    let metadata = fs::symlink_metadata(path)
        .map_err(|error| format!("SNAPSHOT_RECLAIM_OBJECT_METADATA_ERROR:{error}"))?;
    if metadata.file_type().is_symlink() || !metadata.is_file() {
        return Err("SNAPSHOT_RECLAIM_OBJECT_TYPE_INVALID".to_owned());
    }
    if metadata.len() != object.byte_length
        || object.byte_length > u64::try_from(MAX_ADMIN_FILE_BYTES).unwrap_or(u64::MAX)
    {
        return Err("SNAPSHOT_RECLAIM_OBJECT_LENGTH_MISMATCH".to_owned());
    }
    let bytes = read_bounded(path, MAX_ADMIN_FILE_BYTES)?;
    if digest_bytes(&bytes) != object.digest {
        return Err("SNAPSHOT_RECLAIM_OBJECT_DIGEST_MISMATCH".to_owned());
    }
    Ok(())
}

fn object_path(data_root: &Path, digest: Sha256Digest) -> PathBuf {
    let hex = digest.hex();
    data_root
        .join("objects")
        .join(&hex[..2])
        .join(format!("{hex}.utf8"))
}

fn validate_data_root(path: &Path) -> Result<PathBuf, String> {
    let metadata = fs::symlink_metadata(path)
        .map_err(|error| format!("DATA_ROOT_OPEN_ERROR:{error}"))?;
    if metadata.file_type().is_symlink() || !metadata.is_dir() {
        return Err("DATA_ROOT_INVALID".to_owned());
    }
    let canonical = fs::canonicalize(path)
        .map_err(|error| format!("DATA_ROOT_CANONICALIZE_ERROR:{error}"))?;
    let canonical_metadata = fs::symlink_metadata(&canonical)
        .map_err(|error| format!("DATA_ROOT_OPEN_ERROR:{error}"))?;
    if canonical_metadata.file_type().is_symlink() || !canonical_metadata.is_dir() {
        return Err("DATA_ROOT_IDENTITY_AMBIGUOUS".to_owned());
    }
    Ok(canonical)
}

fn read_bounded(path: &Path, max_bytes: usize) -> Result<Vec<u8>, String> {
    let metadata = fs::symlink_metadata(path)
        .map_err(|error| format!("BOUNDED_FILE_METADATA_ERROR:{error}"))?;
    if metadata.file_type().is_symlink() || !metadata.is_file() {
        return Err("BOUNDED_FILE_TYPE_INVALID".to_owned());
    }
    if metadata.len() > u64::try_from(max_bytes).unwrap_or(u64::MAX) {
        return Err("BOUNDED_FILE_TOO_LARGE".to_owned());
    }
    let mut file = File::open(path)
        .map_err(|error| format!("BOUNDED_FILE_OPEN_ERROR:{error}"))?;
    let mut bytes = Vec::with_capacity(
        usize::try_from(metadata.len()).map_err(|_| "BOUNDED_FILE_TOO_LARGE".to_owned())?,
    );
    (&mut file)
        .take(u64::try_from(max_bytes.saturating_add(1)).unwrap_or(u64::MAX))
        .read_to_end(&mut bytes)
        .map_err(|error| format!("BOUNDED_FILE_READ_ERROR:{error}"))?;
    if bytes.len() > max_bytes {
        return Err("BOUNDED_FILE_TOO_LARGE".to_owned());
    }
    Ok(bytes)
}

fn persist_exact_file(path: &Path, bytes: &[u8]) -> Result<(), String> {
    if path.exists() {
        let existing = read_bounded(path, MAX_ADMIN_MANIFEST_BYTES)?;
        return if existing == bytes {
            Ok(())
        } else {
            Err("IMMUTABLE_ADMIN_RECORD_CONFLICT".to_owned())
        };
    }
    let parent = path
        .parent()
        .ok_or_else(|| "ADMIN_RECORD_PARENT_MISSING".to_owned())?;
    fs::create_dir_all(parent)
        .map_err(|error| format!("ADMIN_RECORD_DIRECTORY_ERROR:{error}"))?;
    let temporary = parent.join(format!(
        ".{}.{}.tmp",
        path.file_name()
            .and_then(|name| name.to_str())
            .unwrap_or("admin-record"),
        std::process::id(),
    ));
    if temporary.exists() {
        fs::remove_file(&temporary)
            .map_err(|error| format!("ADMIN_RECORD_TEMP_REMOVE_ERROR:{error}"))?;
    }
    let mut file = OpenOptions::new()
        .create_new(true)
        .write(true)
        .open(&temporary)
        .map_err(|error| format!("ADMIN_RECORD_CREATE_ERROR:{error}"))?;
    file.write_all(bytes)
        .and_then(|()| file.sync_all())
        .map_err(|error| format!("ADMIN_RECORD_WRITE_ERROR:{error}"))?;
    match fs::rename(&temporary, path) {
        Ok(()) => Ok(()),
        Err(error) if path.exists() => {
            let _ = fs::remove_file(&temporary);
            let existing = read_bounded(path, MAX_ADMIN_MANIFEST_BYTES)?;
            if existing == bytes {
                Ok(())
            } else {
                Err("IMMUTABLE_ADMIN_RECORD_CONFLICT".to_owned())
            }
        }
        Err(error) => {
            let _ = fs::remove_file(&temporary);
            Err(format!("ADMIN_RECORD_COMMIT_ERROR:{error}"))
        }
    }
}

fn sanitize_reason(reason: &str) -> String {
    let mut output = String::with_capacity(reason.len().min(256));
    for character in reason.chars().take(256) {
        if character.is_ascii_alphanumeric() || matches!(character, '_' | '-' | '.') {
            output.push(character);
        } else {
            output.push('_');
        }
    }
    if output.is_empty() {
        "UNKNOWN".to_owned()
    } else {
        output
    }
}

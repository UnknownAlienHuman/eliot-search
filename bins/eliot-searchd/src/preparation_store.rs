//! Immutable DIRECT preparation objects plus content-free lookup references.
//! The existing `DirectStore` owner and revision protector own this adapter too.
//! No query path creates files, repairs missing objects or changes source metadata.

#[path = "control_migration_preparation.rs"]
mod migration_inventory;

use std::fs;
use std::io::ErrorKind;
use std::path::{Path, PathBuf};
use std::time::{Duration, Instant};
use zeroize::Zeroizing;

use super::{IndexedSource, RevisionMetadata, RevisionProtector, verify_plaintext};
use super::storage_io::{ensure_child_directory, ensure_directory, persist_immutable_object,
    read_regular_file, sync_directory};
use crate::direct_preparation::{MAX_LAYOUT_BYTES, encode_preparation, preparation_gap, profile_digest};
use crate::sha256;

const MAGIC: &[u8; 8] = b"ELSPRP01";
const REF_MAGIC: &[u8; 8] = b"ELSPRF01";
const BINDING_BYTES: usize = 176;
const REF_BYTES: usize = 81;
const MAX_MANIFEST_BYTES: usize = BINDING_BYTES + MAX_LAYOUT_BYTES + 1;
const MAX_OBJECT_BYTES: usize = 65 * 1024 * 1024;

pub(super) fn persist_source(
    root: &Path, protector: &RevisionProtector, namespace: &str,
    source: &IndexedSource, bytes: &[u8],
) -> Result<(), String> {
    super::revision_writer::persist_before_publication(root, protector, source, bytes)?;
    let metadata = RevisionMetadata {
        source_id: source.source_id.clone(), revision_id: source.revision_id.clone(),
        content_digest: source.content_digest.clone(), byte_length: source.byte_length,
    };
    persist(root, protector, namespace, &metadata, bytes).map(|_| ())
}

/// Object publication and exact readback precede its reference, which in turn
/// precedes the source-catalog event. Existing conflicting bytes are not replaced.
pub(super) fn persist(
    root: &Path, protector: &RevisionProtector, namespace: &str,
    metadata: &RevisionMetadata, source: &[u8],
) -> Result<Option<&'static str>, String> {
    verify_plaintext(metadata, source)?;
    let binding = binding(namespace, metadata)?;
    let mut manifest = Zeroizing::new(binding.to_vec());
    manifest.extend_from_slice(&encode_preparation(source).map_err(str::to_owned)?);
    let gap = preparation_gap(&manifest[BINDING_BYTES..]).map_err(str::to_owned)?;
    if manifest.len() > MAX_MANIFEST_BYTES { return Err("DIRECT_PREPARATION_TOO_LARGE".to_owned()); }
    let digest = sha256::digest(&manifest);
    let key = lookup_key(&binding, protector);
    let (reference_path, objects) = directories(root, &key, true)?;
    let object_id = object_id(&binding, protector, &digest);
    let object_shard = objects.join(&object_id[..2]);
    ensure_child_directory(&object_shard)?;
    #[cfg(unix)]
    sync_directory(&objects)?;
    #[cfg(not(unix))]
    sync_directory(&objects);
    let object_path = object_shard.join(format!("{object_id}.{}", extension(protector)));
    let expected_ref = reference(&key, digest, manifest.len() as u64, protector);
    // A corrupt/conflicting lookup never authorizes overwriting immutable state.
    match fs::symlink_metadata(&reference_path) {
        Ok(_) => {
            let observed = read_regular_file(&reference_path, REF_BYTES, "DIRECT_PREPARATION_REFERENCE_READ_FAILED")?;
            if observed != expected_ref { return Err("DIRECT_PREPARATION_REFERENCE_CONFLICT".to_owned()); }
        }
        Err(error) if error.kind() == ErrorKind::NotFound => {}
        Err(_) => return Err("DIRECT_PREPARATION_REFERENCE_READ_FAILED".to_owned()),
    }
    match fs::symlink_metadata(&object_path) {
        Ok(_) => {}
        Err(error) if error.kind() == ErrorKind::NotFound => {
            let encoded = Zeroizing::new(if protector.encrypts_new_objects() {
                protector.protect(&object_id, &sha256::hex(&digest), &manifest)?
            } else { manifest.to_vec() });
            persist_immutable_object(&object_path, &encoded)?;
        }
        Err(_) => return Err("DIRECT_PREPARATION_OBJECT_READ_FAILED".to_owned()),
    }
    let observed = read_object(&object_path, protector, &object_id, digest, manifest.len() as u64)?;
    if observed.as_slice() != manifest.as_slice() {
        return Err("DIRECT_PREPARATION_OBJECT_CONFLICT".to_owned());
    }
    persist_immutable_object(&reference_path, &expected_ref)?;
    // Reference readback is performed by persist_immutable_object itself.
    Ok(gap)
}

/// Exact, bounded, read-only lookup. The returned body is tied to the caller's
/// source revision, preparation profile and current storage protection profile.
pub(super) fn load(
    root: &Path, protector: &RevisionProtector, namespace: &str, metadata: &RevisionMetadata,
) -> Result<Zeroizing<Vec<u8>>, &'static str> {
    let binding = binding(namespace, metadata).map_err(|_| "DIRECT_PREPARATION_BINDING_INVALID")?;
    let key = lookup_key(&binding, protector);
    let (reference_path, objects) = directories(root, &key, false)
        .map_err(|_| "DIRECT_PREPARATION_UNAVAILABLE")?;
    let saved = read_regular_file(&reference_path, REF_BYTES, "DIRECT_PREPARATION_REFERENCE_READ_FAILED")
        .map_err(|_| "DIRECT_PREPARATION_UNAVAILABLE")?;
    let (digest, length) = decode_reference(&saved, &key, protector)?;
    let id = object_id(&binding, protector, &digest);
    let object_shard = objects.join(&id[..2]);
    ensure_directory(&object_shard).map_err(|_| "DIRECT_PREPARATION_OBJECT_INVALID")?;
    let path = object_shard.join(format!("{id}.{}", extension(protector)));
    let manifest = read_object(&path, protector, &id, digest, length)
        .map_err(|_| "DIRECT_PREPARATION_OBJECT_INVALID")?;
    if manifest.get(..BINDING_BYTES) != Some(binding.as_slice()) {
        return Err("DIRECT_PREPARATION_BINDING_MISMATCH");
    }
    Ok(Zeroizing::new(manifest[BINDING_BYTES..].to_vec()))
}

/// One reference/object observation; payloads never escape into migration output.
pub(super) struct PreparationEvidence {
    pub(super) present: bool,
    pub(super) stored_bytes: u64,
    pub(super) json: String,
}

/// Recompute the exact profile layout only during explicit migration inspection.
/// Shared reference/envelope decoders are also used by normal query readback.
/// Absence is an explicit missing derivative; malformed or contradictory state is an error.
pub(super) fn inspect(
    root: &Path, protector: &RevisionProtector, namespace: &str,
    metadata: &RevisionMetadata, source: &[u8], deadline: Instant,
) -> Result<PreparationEvidence, String> {
    let check = || if Instant::now() >= deadline {
        Err("DIRECT_MIGRATION_DEADLINE_EXCEEDED".to_owned())
    } else { Ok(()) };
    check()?;
    verify_plaintext(metadata, source)?;
    let binding = binding(namespace, metadata)?;
    let key = lookup_key(&binding, protector);
    let hex = sha256::hex(&key);
    let missing = || PreparationEvidence {
        present: false, stored_bytes: 0,
        json: format!(concat!(
            "{{\"status\":\"missing_reference\",\"reference_key_sha256\":\"{}\",",
            "\"record_verified\":false,\"layout_available\":false}}"
        ), hex),
    };
    // Do not translate permission/type errors into absence, follow a dangling
    // symlink, create a directory, or reconstruct a missing object here.
    ensure_directory(root)?;
    let base = root.join("preparation");
    let refs = base.join("refs");
    let shard = refs.join(&hex[..2]);
    for path in [&base, &refs, &shard] {
        check()?;
        match fs::symlink_metadata(path) {
            Ok(_) => ensure_directory(path)?,
            Err(error) if error.kind() == ErrorKind::NotFound => return Ok(missing()),
            Err(_) => return Err("DIRECT_PREPARATION_REFERENCE_READ_FAILED".to_owned()),
        }
    }
    let ref_path = shard.join(format!("{hex}.ref"));
    match fs::symlink_metadata(&ref_path) {
        Ok(_) => {}
        Err(error) if error.kind() == ErrorKind::NotFound => return Ok(missing()),
        Err(_) => return Err("DIRECT_PREPARATION_REFERENCE_READ_FAILED".to_owned()),
    }
    let saved = read_regular_file(&ref_path, REF_BYTES, "DIRECT_PREPARATION_REFERENCE_READ_FAILED")?;
    let (digest, length) = decode_reference(&saved, &key, protector).map_err(str::to_owned)?;
    let id = object_id(&binding, protector, &digest);
    let objects = base.join("objects");
    ensure_directory(&objects)?;
    let object_shard = objects.join(&id[..2]);
    ensure_directory(&object_shard)?;
    let object_path = object_shard.join(format!("{id}.{}", extension(protector)));
    check()?;
    let encoded = Zeroizing::new(read_regular_file(
        &object_path, MAX_OBJECT_BYTES, "DIRECT_PREPARATION_OBJECT_READ_FAILED",
    )?);
    let raw_digest = sha256::digest(&encoded);
    let raw_length = encoded.len() as u64;
    let manifest = decode_object(&encoded, protector, &id, digest, length)?;
    drop(encoded);
    if manifest.get(..BINDING_BYTES) != Some(binding.as_slice()) {
        return Err("DIRECT_PREPARATION_BINDING_MISMATCH".to_owned());
    }
    check()?;
    let expected = Zeroizing::new(encode_preparation(source).map_err(str::to_owned)?);
    if manifest.get(BINDING_BYTES..) != Some(expected.as_slice()) {
        return Err("DIRECT_PREPARATION_DERIVATION_MISMATCH".to_owned());
    }
    let gap = preparation_gap(&expected).map_err(str::to_owned)?;
    drop(expected);
    drop(manifest);
    // The fingerprints describe the same decoded object/reference, not another
    // path read substituted after validation. The importer must still revalidate
    // at cutover; this is not an atomic snapshot of the entire filesystem.
    check()?;
    let reread = Zeroizing::new(read_regular_file(
        &object_path, MAX_OBJECT_BYTES, "DIRECT_PREPARATION_OBJECT_READ_FAILED",
    )?);
    if reread.len() as u64 != raw_length || sha256::digest(&reread) != raw_digest
        || read_regular_file(&ref_path, REF_BYTES, "DIRECT_PREPARATION_REFERENCE_READ_FAILED")? != saved
    {
        return Err("DIRECT_MIGRATION_PREPARATION_CHANGED".to_owned());
    }
    check()?;
    Ok(PreparationEvidence {
        present: true, stored_bytes: raw_length + REF_BYTES as u64,
        json: format!(concat!(
            "{{\"status\":\"verified_record\",\"reference_key_sha256\":\"{}\",",
            "\"reference_file_sha256\":\"{}\",\"reference_bytes\":{},",
            "\"object_id\":\"{}\",\"object_format\":\"{}\",",
            "\"manifest_sha256\":\"{}\",\"manifest_bytes\":{},",
            "\"encoded_sha256\":\"{}\",\"encoded_bytes\":{},",
            "\"record_verified\":true,\"layout_available\":{},\"preparation_gap\":{}}}"
        ), hex, sha256::hex(&sha256::digest(&saved)), REF_BYTES, id, extension(protector),
            sha256::hex(&digest), length, sha256::hex(&raw_digest), raw_length,
            gap.is_none(), gap.map_or_else(|| "null".to_owned(), crate::service_output::json_string)),
    })
}

fn decode_reference(
    saved: &[u8], key: &[u8; 32], protector: &RevisionProtector,
) -> Result<([u8; 32], u64), &'static str> {
    let (digest, length, protected) = decode_reference_fields(saved, key)?;
    if protected != protector.encrypts_new_objects() {
        return Err("DIRECT_PREPARATION_REFERENCE_INVALID");
    }
    Ok((digest, length))
}

/// Shared wire validation, independent of the currently active protection backend.
/// Used to account for old-profile references without treating them as admitted data.
fn decode_reference_fields(
    saved: &[u8], key: &[u8; 32],
) -> Result<([u8; 32], u64, bool), &'static str> {
    if saved.len() != REF_BYTES || &saved[..8] != REF_MAGIC || saved[8..40] != key[..]
        || saved[80] > 1
    {
        return Err("DIRECT_PREPARATION_REFERENCE_INVALID");
    }
    let digest = saved[40..72].try_into().map_err(|_| "DIRECT_PREPARATION_REFERENCE_INVALID")?;
    let length = u64::from_be_bytes(saved[72..80].try_into().map_err(|_| "DIRECT_PREPARATION_REFERENCE_INVALID")?);
    if length <= BINDING_BYTES as u64 || length > MAX_MANIFEST_BYTES as u64 {
        return Err("DIRECT_PREPARATION_REFERENCE_INVALID");
    }
    Ok((digest, length, saved[80] == 1))
}

fn binding(namespace: &str, metadata: &RevisionMetadata) -> Result<[u8; BINDING_BYTES], String> {
    let mut out = [0; BINDING_BYTES];
    out[..8].copy_from_slice(MAGIC);
    for (index, value) in [namespace, &metadata.source_id, &metadata.revision_id, &metadata.content_digest]
        .into_iter().enumerate()
    {
        let digest = sha256::decode_digest(value).ok_or_else(|| "DIRECT_PREPARATION_BINDING_INVALID".to_owned())?;
        out[8 + index * 32..40 + index * 32].copy_from_slice(&digest);
    }
    out[136..144].copy_from_slice(&metadata.byte_length.to_be_bytes());
    out[144..176].copy_from_slice(&profile_digest());
    Ok(out)
}
fn lookup_key(binding: &[u8], protector: &RevisionProtector) -> [u8; 32] {
    sha256::digest_parts(b"eliot-search/direct-preparation-ref/v1", &[binding, protector.backend_name().as_bytes()])
}
fn object_id(binding: &[u8], protector: &RevisionProtector, digest: &[u8; 32]) -> String {
    sha256::hex(&sha256::digest_parts(b"eliot-search/direct-preparation-object/v1",
        &[binding, protector.backend_name().as_bytes(), digest]))
}
const fn extension(protector: &RevisionProtector) -> &'static str {
    if protector.encrypts_new_objects() { "dpapi" } else { "bin" }
}
fn reference(key: &[u8; 32], digest: [u8; 32], length: u64, protector: &RevisionProtector) -> Vec<u8> {
    let mut out = Vec::with_capacity(REF_BYTES);
    out.extend_from_slice(REF_MAGIC); out.extend_from_slice(key); out.extend_from_slice(&digest);
    out.extend_from_slice(&length.to_be_bytes()); out.push(u8::from(protector.encrypts_new_objects()));
    out
}
fn read_object(
    path: &Path, protector: &RevisionProtector, id: &str, digest: [u8; 32], length: u64,
) -> Result<Zeroizing<Vec<u8>>, String> {
    let encoded = Zeroizing::new(read_regular_file(path, MAX_OBJECT_BYTES, "DIRECT_PREPARATION_OBJECT_READ_FAILED")?);
    decode_object(&encoded, protector, id, digest, length)
}

fn decode_object(
    encoded: &[u8], protector: &RevisionProtector, id: &str, digest: [u8; 32], length: u64,
) -> Result<Zeroizing<Vec<u8>>, String> {
    let decoded = Zeroizing::new(if protector.encrypts_new_objects() {
        protector.unprotect(encoded, id, &sha256::hex(&digest), length)?
    } else { encoded.to_vec() });
    if decoded.len() as u64 != length || sha256::digest(&decoded) != digest {
        return Err("DIRECT_PREPARATION_CONTENT_MISMATCH".to_owned());
    }
    Ok(decoded)
}
fn directories(root: &Path, key: &[u8; 32], create: bool) -> Result<(PathBuf, PathBuf), String> {
    ensure_directory(root)?;
    let base = root.join("preparation");
    let refs = base.join("refs");
    let objects = base.join("objects");
    let hex = sha256::hex(key);
    let shard = refs.join(&hex[..2]);
    for path in [&base, &refs, &objects, &shard] {
        if create {
            ensure_child_directory(path)?;
            #[cfg(unix)]
            sync_directory(path.parent().ok_or_else(|| "DIRECT_PREPARATION_PARENT_INVALID".to_owned())?)?;
            #[cfg(not(unix))]
            sync_directory(path.parent().ok_or_else(|| "DIRECT_PREPARATION_PARENT_INVALID".to_owned())?);
        } else { ensure_directory(path)?; }
    }
    Ok((shard.join(format!("{hex}.ref")), objects))
}


const MAX_BATCH_REVISIONS: usize = 64;
const MAX_BATCH_SOURCE_BYTES: u64 = 256 * 1024 * 1024;
const BATCH_SLICE: Duration = Duration::from_secs(10);

/// Stateless admin bookmark, never a bearer token or proof of the processed prefix.
/// Bound to the complete source-event history, preparation profile and storage backend.
pub struct PreparationCursor {
    checkpoint: [u8; 32],
    after: String,
}

impl PreparationCursor {
    pub(crate) fn parse(value: &str) -> Result<Self, String> {
        // v1.<64 lowercase hex>.<64 lowercase hex>; ASCII validated before slicing.
        if value.len() != 132 || !value.is_ascii() || !value.starts_with("v1.")
            || value.as_bytes()[67] != b'.'
        {
            return Err("DIRECT_PREPARATION_CURSOR_INVALID".to_owned());
        }
        let checkpoint = sha256::decode_digest(&value[3..67])
            .ok_or_else(|| "DIRECT_PREPARATION_CURSOR_INVALID".to_owned())?;
        let after = &value[68..];
        if sha256::hex(&checkpoint) != value[3..67]
            || sha256::decode_digest(after).is_none()
            || after.bytes().any(|byte| byte.is_ascii_uppercase())
        {
            return Err("DIRECT_PREPARATION_CURSOR_INVALID".to_owned());
        }
        Ok(Self { checkpoint, after: after.to_owned() })
    }
}

/// One bounded suffix batch. Exhaustion is not a complete-corpus search proof.
pub struct PreparationBatch {
    pub(crate) stored: usize,
    pub(crate) layouts: usize,
    pub(crate) source_bytes: u64,
    pub(crate) gaps: Vec<(String, &'static str)>,
    pub(crate) next_cursor: Option<String>,
}

impl super::DirectStore {
    fn preparation_checkpoint(&self) -> [u8; 32] {
        sha256::digest_parts(b"eliot-search/direct-preparation-cursor/v1", &[
            &self.inner.preparation_catalog_digest(), &profile_digest(),
            self.protector.backend_name().as_bytes(),
        ])
    }

    /// Cheap pre-dispatch validation against the owner's admitted catalog snapshot.
    /// The batch also rereads control state before its first possible object write.
    pub(crate) fn validate_preparation_cursor(
        &self, cursor: Option<&PreparationCursor>,
    ) -> Result<(), String> {
        if let Some(cursor) = cursor
            && (cursor.checkpoint != self.preparation_checkpoint()
                || self.inner.retained_revision(&cursor.after).is_none())
        {
            return Err("DIRECT_PREPARATION_CURSOR_STALE".to_owned());
        }
        Ok(())
    }

    /// Explicit restart-safe backfill of retained revisions, including retired history.
    /// Each object keeps the existing immutable commit/readback boundary. This is
    /// not one atomic corpus transaction: on failure, retry the last accepted cursor.
    /// No file enumeration, source-path read, source event or query-time repair occurs.
    pub(crate) fn prepare_root(
        &self, cursor: Option<&PreparationCursor>,
    ) -> Result<PreparationBatch, String> {
        crate::catalog_presence::require_existing(&self.root)?;
        self.inner.verify_control()?;
        self.validate_preparation_cursor(cursor)?;
        let checkpoint = self.preparation_checkpoint();
        let namespace = self.inner.namespace_id();
        let mut pending = self.inner.retained_revisions_after(
            cursor.map(|value| value.after.as_str()),
        ).peekable();
        let mut batch = PreparationBatch {
            stored: 0, layouts: 0, source_bytes: 0, gaps: Vec::new(), next_cursor: None,
        };
        let started = Instant::now();
        let mut last = None;
        while let Some(metadata) = pending.peek() {
            if batch.stored >= MAX_BATCH_REVISIONS
                || batch.source_bytes.checked_add(metadata.byte_length)
                    .is_none_or(|bytes| bytes > MAX_BATCH_SOURCE_BYTES)
                || (batch.stored > 0 && started.elapsed() >= BATCH_SLICE)
            {
                break;
            }
            let metadata = pending.next().ok_or_else(|| "DIRECT_PREPARATION_NO_PROGRESS".to_owned())?;
            // Only one revision's bytes/layouts are retained at a time. The time
            // slice is cooperative between objects; it cannot interrupt OS I/O.
            let bytes = Zeroizing::new(self.read_revision_detailed(&metadata)?);
            let gap = persist(&self.root, &self.protector, &namespace, &metadata, &bytes)?;
            batch.source_bytes += metadata.byte_length; // checked against the ceiling above
            batch.stored += 1;
            if let Some(reason) = gap {
                batch.gaps.push((metadata.revision_id.clone(), reason));
            } else {
                batch.layouts += 1;
            }
            last = Some(metadata.revision_id);
        }
        if pending.peek().is_some() {
            let last = last.ok_or_else(|| "DIRECT_PREPARATION_NO_PROGRESS".to_owned())?;
            batch.next_cursor = Some(format!("v1.{}.{last}", sha256::hex(&checkpoint)));
        }
        // A torn/stale catalog cannot produce an acknowledged continuation after
        // successful object writes. They remain immutable and safe to re-inspect.
        self.inner.verify_control()?;
        Ok(batch)
    }
}

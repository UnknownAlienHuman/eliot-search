//! Immutable DIRECT preparation objects plus content-free lookup references.
//! The existing DirectStore owner and revision protector own this adapter too.
//! No query path creates files, repairs missing objects or changes source metadata.

use std::fs;
use std::io::ErrorKind;
use std::path::{Path, PathBuf};
use zeroize::Zeroizing;

use super::{IndexedSource, RevisionMetadata, RevisionProtector, verify_plaintext};
use super::storage_io::{ensure_child_directory, ensure_directory, persist_immutable_object,
    read_regular_file, sync_directory};
use crate::direct_preparation::{MAX_LAYOUT_BYTES, encode_preparation, profile_digest};
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
    persist(root, protector, namespace, &metadata, bytes)
}

/// Object publication and exact readback precede its reference, which in turn
/// precedes the source-catalog event. Existing conflicting bytes are not replaced.
pub(super) fn persist(
    root: &Path, protector: &RevisionProtector, namespace: &str,
    metadata: &RevisionMetadata, source: &[u8],
) -> Result<(), String> {
    verify_plaintext(metadata, source)?;
    let binding = binding(namespace, metadata)?;
    let mut manifest = Zeroizing::new(binding.to_vec());
    manifest.extend_from_slice(&encode_preparation(source).map_err(str::to_owned)?);
    if manifest.len() > MAX_MANIFEST_BYTES { return Err("DIRECT_PREPARATION_TOO_LARGE".to_owned()); }
    let digest = sha256::digest(&manifest);
    let key = lookup_key(&binding, protector);
    let (reference_path, objects) = directories(root, &key, true)?;
    let object_id = object_id(&binding, protector, &digest);
    let object_shard = objects.join(&object_id[..2]);
    ensure_child_directory(&object_shard)?;
    sync_directory(&objects)?;
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
    Ok(())
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
    if saved.len() != REF_BYTES || &saved[..8] != REF_MAGIC || saved[8..40] != key
        || saved[80] != u8::from(protector.encrypts_new_objects())
    {
        return Err("DIRECT_PREPARATION_REFERENCE_INVALID");
    }
    let digest: [u8; 32] = saved[40..72].try_into().map_err(|_| "DIRECT_PREPARATION_REFERENCE_INVALID")?;
    let length = u64::from_be_bytes(saved[72..80].try_into().map_err(|_| "DIRECT_PREPARATION_REFERENCE_INVALID")?);
    if length <= BINDING_BYTES as u64 || length > MAX_MANIFEST_BYTES as u64 {
        return Err("DIRECT_PREPARATION_REFERENCE_INVALID");
    }
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
fn extension(protector: &RevisionProtector) -> &'static str {
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
    let decoded = Zeroizing::new(if protector.encrypts_new_objects() {
        protector.unprotect(&encoded, id, &sha256::hex(&digest), length)?
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
            sync_directory(path.parent().ok_or_else(|| "DIRECT_PREPARATION_PARENT_INVALID".to_owned())?)?;
        } else { ensure_directory(path)?; }
    }
    Ok((shard.join(format!("{hex}.ref")), objects))
}

//! Exact preparation binding, reference and object codecs.

use std::path::Path;

use zeroize::Zeroizing;

use super::super::super::storage_io::read_regular_file;
use super::super::super::{RevisionMetadata, RevisionProtector};
use super::spec::{
    BINDING_BYTES, HEADER_BYTES, MAGIC, MAX_MANIFEST_BYTES,
    MAX_OBJECT_BYTES, OLD_BINDING_BYTES, REF_BYTES, REF_MAGIC,
};
use crate::direct_preparation::{
    CANONICAL_MATERIALIZER_REVISION, CANONICAL_UNITIZER_REVISION,
    CONTENT_DIGEST_ALGORITHM, MANIFEST_DIGEST_ALGORITHM,
    REPRESENTATION_DIGEST_ALGORITHM, canonical_materializer_digest,
    canonical_unitizer_digest,
};
use crate::sha256;

pub(crate) fn binding(
    namespace: &str,
    metadata: &RevisionMetadata,
) -> Result<[u8; BINDING_BYTES], String> {
    let mut out = [0; BINDING_BYTES];
    out[..8].copy_from_slice(MAGIC);
    for (index, value) in [
        namespace,
        &metadata.source_id,
        &metadata.revision_id,
        &metadata.content_digest,
    ]
    .into_iter()
    .enumerate()
    {
        let digest = sha256::decode_digest(value)
            .ok_or_else(|| "DIRECT_PREPARATION_BINDING_INVALID".to_owned())?;
        out[8 + index * 32..40 + index * 32].copy_from_slice(&digest);
    }
    out[136..144].copy_from_slice(&metadata.byte_length.to_be_bytes());
    let materializer = canonical_materializer_digest()
        .map_err(|_| "DIRECT_PREPARATION_PROFILE_INVALID".to_owned())?;
    let unitizer = canonical_unitizer_digest()
        .map_err(|_| "DIRECT_PREPARATION_PROFILE_INVALID".to_owned())?;
    out[144..176].copy_from_slice(&materializer);
    out[176..208].copy_from_slice(&unitizer);
    Ok(out)
}

#[must_use]
pub(crate) fn lookup_key(
    binding: &[u8],
    protector: &RevisionProtector,
) -> [u8; 32] {
    sha256::digest_parts(
        b"eliot-search/direct-preparation-ref/v2",
        &[binding, protector.backend_name().as_bytes()],
    )
}

#[must_use]
pub(crate) fn object_id(
    binding: &[u8],
    protector: &RevisionProtector,
    digest: &[u8; 32],
) -> String {
    sha256::hex(&sha256::digest_parts(
        b"eliot-search/direct-preparation-object/v2",
        &[binding, protector.backend_name().as_bytes(), digest],
    ))
}

#[must_use]
pub(crate) const fn extension(
    protector: &RevisionProtector,
) -> &'static str {
    if protector.encrypts_new_objects() {
        "dpapi"
    } else {
        "bin"
    }
}

#[must_use]
pub(crate) fn reference(
    key: &[u8; 32],
    digest: [u8; 32],
    length: u64,
    protector: &RevisionProtector,
) -> Vec<u8> {
    let mut out = Vec::with_capacity(REF_BYTES);
    out.extend_from_slice(REF_MAGIC);
    out.extend_from_slice(key);
    out.extend_from_slice(&digest);
    out.extend_from_slice(&length.to_be_bytes());
    out.push(u8::from(protector.encrypts_new_objects()));
    out
}

pub(crate) fn decode_reference(
    saved: &[u8],
    key: &[u8; 32],
    protector: &RevisionProtector,
) -> Result<([u8; 32], u64), &'static str> {
    let (digest, length, protected) = decode_reference_fields(saved, key)?;
    if protected != protector.encrypts_new_objects() {
        return Err("DIRECT_PREPARATION_REFERENCE_INVALID");
    }
    Ok((digest, length))
}

/// Shared wire validation independent of the active protection backend.
/// Both v1 and v2 binding lengths remain inventory-visible during migration.
pub(crate) fn decode_reference_fields(
    saved: &[u8],
    key: &[u8; 32],
) -> Result<([u8; 32], u64, bool), &'static str> {
    if saved.len() != REF_BYTES
        || &saved[..8] != REF_MAGIC
        || saved[8..40] != key[..]
        || saved[80] > 1
    {
        return Err("DIRECT_PREPARATION_REFERENCE_INVALID");
    }
    let digest = saved[40..72]
        .try_into()
        .map_err(|_| "DIRECT_PREPARATION_REFERENCE_INVALID")?;
    let length = u64::from_be_bytes(
        saved[72..80]
            .try_into()
            .map_err(|_| "DIRECT_PREPARATION_REFERENCE_INVALID")?,
    );
    if length <= OLD_BINDING_BYTES as u64
        || length > MAX_MANIFEST_BYTES as u64
    {
        return Err("DIRECT_PREPARATION_REFERENCE_INVALID");
    }
    Ok((digest, length, saved[80] == 1))
}

pub(crate) fn read_object(
    path: &Path,
    protector: &RevisionProtector,
    id: &str,
    digest: [u8; 32],
    length: u64,
) -> Result<Zeroizing<Vec<u8>>, String> {
    let encoded = Zeroizing::new(read_regular_file(
        path,
        MAX_OBJECT_BYTES,
        "DIRECT_PREPARATION_OBJECT_READ_FAILED",
    )?);
    decode_object(&encoded, protector, id, digest, length)
}

pub(crate) fn decode_object(
    encoded: &[u8],
    protector: &RevisionProtector,
    id: &str,
    digest: [u8; 32],
    length: u64,
) -> Result<Zeroizing<Vec<u8>>, String> {
    let decoded = Zeroizing::new(if protector.encrypts_new_objects() {
        protector.unprotect(encoded, id, &sha256::hex(&digest), length)?
    } else {
        encoded.to_vec()
    });
    if decoded.len() as u64 != length || sha256::digest(&decoded) != digest {
        return Err("DIRECT_PREPARATION_CONTENT_MISMATCH".to_owned());
    }
    Ok(decoded)
}

/// Verifies the exact manifest binding, algorithms, profiles and
/// representation. Unknown tags are never reinterpreted.
pub(crate) fn verify_manifest(
    manifest: &[u8],
    binding: &[u8; BINDING_BYTES],
    metadata: &RevisionMetadata,
    namespace: &str,
) -> Result<(), &'static str> {
    use crate::direct_preparation::{
        preparation_gap as gap_of, representation_id as repr_of,
    };

    if manifest.len() < HEADER_BYTES + 1
        || manifest.len() > MAX_MANIFEST_BYTES
    {
        return Err("DIRECT_PREPARATION_OBJECT_INVALID");
    }
    if manifest.get(..BINDING_BYTES) != Some(binding.as_slice()) {
        return Err("DIRECT_PREPARATION_BINDING_MISMATCH");
    }
    let stored_representation: [u8; 32] = manifest
        [BINDING_BYTES..BINDING_BYTES + 32]
        .try_into()
        .map_err(|_| "DIRECT_PREPARATION_OBJECT_INVALID")?;
    let suffix = &manifest[BINDING_BYTES + 32..HEADER_BYTES];
    if suffix.len() != 19
        || suffix[0] != CONTENT_DIGEST_ALGORITHM
        || suffix[1] != REPRESENTATION_DIGEST_ALGORITHM
        || suffix[18] != MANIFEST_DIGEST_ALGORITHM
    {
        return Err("DIRECT_PREPARATION_DIGEST_ALGORITHM_MISMATCH");
    }
    if u64::from_be_bytes(
        suffix[2..10]
            .try_into()
            .map_err(|_| "DIRECT_PREPARATION_OBJECT_INVALID")?,
    ) != CANONICAL_MATERIALIZER_REVISION
        || u64::from_be_bytes(
            suffix[10..18]
                .try_into()
                .map_err(|_| "DIRECT_PREPARATION_OBJECT_INVALID")?,
        ) != CANONICAL_UNITIZER_REVISION
    {
        return Err("DIRECT_PREPARATION_PROFILE_MISMATCH");
    }

    let body = &manifest[HEADER_BYTES..];
    gap_of(body).map_err(|_| "DIRECT_PREPARATION_OBJECT_INVALID")?;
    let namespace_bytes = sha256::decode_digest(namespace)
        .ok_or("DIRECT_PREPARATION_BINDING_INVALID")?;
    let source_bytes = sha256::decode_digest(&metadata.source_id)
        .ok_or("DIRECT_PREPARATION_BINDING_INVALID")?;
    let revision_bytes = sha256::decode_digest(&metadata.revision_id)
        .ok_or("DIRECT_PREPARATION_BINDING_INVALID")?;
    let content_bytes = sha256::decode_digest(&metadata.content_digest)
        .ok_or("DIRECT_PREPARATION_BINDING_INVALID")?;
    let materializer_digest = canonical_materializer_digest()
        .map_err(|_| "DIRECT_PREPARATION_PROFILE_MISMATCH")?;
    let unitizer_digest = canonical_unitizer_digest()
        .map_err(|_| "DIRECT_PREPARATION_PROFILE_MISMATCH")?;
    if binding[144..176] != materializer_digest
        || binding[176..208] != unitizer_digest
    {
        return Err("DIRECT_PREPARATION_PROFILE_MISMATCH");
    }
    let marker: &[u8] = match body {
        [0, layout @ ..] => layout,
        [1] => b"DIRECT_REVISION_NOT_UTF8",
        [2] => b"MATERIALIZATION_BINARY_CONTENT",
        [3] => b"MATERIALIZATION_TOO_MANY_LINES",
        [4] => b"UNITIZATION_TOO_MANY_UNITS",
        [5] => b"DIRECT_PREPARATION_LAYOUT_TOO_LARGE",
        [6] => b"DIRECT_REVISION_HAS_BOM",
        _ => return Err("DIRECT_PREPARATION_OBJECT_INVALID"),
    };
    let expected = repr_of(
        &namespace_bytes,
        &source_bytes,
        &revision_bytes,
        &content_bytes,
        metadata.byte_length,
        &materializer_digest,
        &unitizer_digest,
        marker,
    );
    if expected != stored_representation {
        return Err("DIRECT_PREPARATION_BINDING_MISMATCH");
    }
    Ok(())
}

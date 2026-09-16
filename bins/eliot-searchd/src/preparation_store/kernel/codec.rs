//! Exact preparation binding, reference and object codecs.
//!
//! Persisted layout and validation live in `search-materializer`; this module
//! supplies legacy digest implementations and active protection composition.

use std::path::Path;

use search_materializer::api::{
    LEGACY_PREPARATION_BINDING_BYTES as BINDING_BYTES,
    LegacyDirectPreparationBinding, LegacyDirectRepresentationDigest,
    LegacyPreparationManifestView, LegacyPreparationProtection,
    LegacyPreparationStoreDigest, decode_legacy_preparation_reference,
    derive_legacy_preparation_lookup_key, derive_legacy_preparation_object_id,
    encode_legacy_preparation_binding, encode_legacy_preparation_reference,
    legacy_preparation_object_file_name, legacy_preparation_shard,
    verify_legacy_preparation_manifest, verify_legacy_preparation_payload,
};
use zeroize::Zeroizing;

use super::super::super::storage_io::read_regular_file;
use super::super::super::{RevisionMetadata, RevisionProtector};
use super::spec::MAX_OBJECT_BYTES;
use crate::direct_preparation::{
    CANONICAL_MATERIALIZER_REVISION, CANONICAL_UNITIZER_REVISION,
    canonical_materializer_digest, canonical_unitizer_digest,
};
use crate::sha256;

struct DirectPreparationStoreDigest;

impl LegacyPreparationStoreDigest for DirectPreparationStoreDigest {
    fn digest(bytes: &[u8]) -> [u8; 32] {
        sha256::digest(bytes)
    }

    fn digest_parts(domain: &[u8], parts: &[&[u8]]) -> [u8; 32] {
        sha256::digest_parts(domain, parts)
    }
}

struct DirectPreparationRepresentationDigest;

impl LegacyDirectRepresentationDigest for DirectPreparationRepresentationDigest {
    fn digest_parts(domain: &[u8], parts: &[&[u8]]) -> [u8; 32] {
        let mut hasher = blake3::Hasher::new();
        hasher.update(domain);
        for part in parts {
            hasher.update(part);
        }
        *hasher.finalize().as_bytes()
    }
}

pub(crate) fn binding(
    namespace: &str,
    metadata: &RevisionMetadata,
) -> Result<[u8; BINDING_BYTES], String> {
    let binding = LegacyDirectPreparationBinding {
        namespace: sha256::decode_digest(namespace)
            .ok_or_else(|| "DIRECT_PREPARATION_BINDING_INVALID".to_owned())?,
        source_id: sha256::decode_digest(&metadata.source_id)
            .ok_or_else(|| "DIRECT_PREPARATION_BINDING_INVALID".to_owned())?,
        revision_id: sha256::decode_digest(&metadata.revision_id)
            .ok_or_else(|| "DIRECT_PREPARATION_BINDING_INVALID".to_owned())?,
        content_digest: sha256::decode_digest(&metadata.content_digest)
            .ok_or_else(|| "DIRECT_PREPARATION_BINDING_INVALID".to_owned())?,
        byte_length: metadata.byte_length,
        materializer_digest: canonical_materializer_digest()
            .map_err(str::to_owned)?,
        unitizer_digest: canonical_unitizer_digest().map_err(str::to_owned)?,
    };
    Ok(encode_legacy_preparation_binding(&binding))
}

#[must_use]
pub(crate) fn lookup_key(
    binding: &[u8; BINDING_BYTES],
    protector: &RevisionProtector,
) -> [u8; 32] {
    derive_legacy_preparation_lookup_key::<DirectPreparationStoreDigest>(
        binding,
        protector.backend_name(),
    )
}

#[must_use]
pub(crate) fn object_id(
    binding: &[u8; BINDING_BYTES],
    protector: &RevisionProtector,
    digest: &[u8; 32],
) -> String {
    sha256::hex(
        &derive_legacy_preparation_object_id::<DirectPreparationStoreDigest>(
            binding,
            protector.backend_name(),
            digest,
        ),
    )
}

pub(crate) fn object_shard(id: &str) -> Result<String, String> {
    let bytes = sha256::decode_digest(id)
        .ok_or_else(|| "DIRECT_PREPARATION_OBJECT_INVALID".to_owned())?;
    Ok(legacy_preparation_shard(&bytes))
}

pub(crate) fn object_file_name(
    id: &str,
    protector: &RevisionProtector,
) -> Result<String, String> {
    let bytes = sha256::decode_digest(id)
        .ok_or_else(|| "DIRECT_PREPARATION_OBJECT_INVALID".to_owned())?;
    Ok(legacy_preparation_object_file_name(
        &bytes,
        protection(protector),
    ))
}

#[must_use]
pub(crate) fn protection(
    protector: &RevisionProtector,
) -> LegacyPreparationProtection {
    if protector.encrypts_new_objects() {
        LegacyPreparationProtection::Protected
    } else {
        LegacyPreparationProtection::Plaintext
    }
}

#[must_use]
pub(crate) fn extension(
    protector: &RevisionProtector,
) -> &'static str {
    protection(protector).extension()
}

pub(crate) fn reference(
    key: &[u8; 32],
    digest: [u8; 32],
    length: u64,
    protector: &RevisionProtector,
) -> Result<Vec<u8>, String> {
    encode_legacy_preparation_reference(
        key,
        &digest,
        length,
        protection(protector),
    )
    .map(|encoded| encoded.to_vec())
    .map_err(|error| error.code().to_owned())
}

pub(crate) fn decode_reference(
    saved: &[u8],
    key: &[u8; 32],
    protector: &RevisionProtector,
) -> Result<([u8; 32], u64), &'static str> {
    let reference = decode_legacy_preparation_reference(saved, key)
        .map_err(|error| error.code())?;
    if reference.protection() != protection(protector) {
        return Err("DIRECT_PREPARATION_REFERENCE_INVALID");
    }
    Ok((*reference.manifest_digest(), reference.manifest_bytes()))
}

/// Shared wire validation independent of the active protection backend.
pub(crate) fn decode_reference_fields(
    saved: &[u8],
    key: &[u8; 32],
) -> Result<([u8; 32], u64, bool), &'static str> {
    let reference = decode_legacy_preparation_reference(saved, key)
        .map_err(|error| error.code())?;
    Ok((
        *reference.manifest_digest(),
        reference.manifest_bytes(),
        reference.protection() == LegacyPreparationProtection::Protected,
    ))
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
    verify_legacy_preparation_payload::<DirectPreparationStoreDigest>(
        &decoded,
        &digest,
        length,
    )
    .map_err(|error| error.code().to_owned())?;
    Ok(decoded)
}

/// Verifies the exact manifest binding, algorithms, profiles and representation.
pub(crate) fn verify_manifest<'a>(
    manifest: &'a [u8],
    binding: &[u8; BINDING_BYTES],
) -> Result<LegacyPreparationManifestView<'a>, &'static str> {
    verify_legacy_preparation_manifest::<DirectPreparationRepresentationDigest>(
        manifest,
        binding,
        CANONICAL_MATERIALIZER_REVISION,
        CANONICAL_UNITIZER_REVISION,
    )
    .map_err(|error| error.code())
}

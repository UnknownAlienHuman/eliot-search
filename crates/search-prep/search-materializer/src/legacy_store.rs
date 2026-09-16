//! Canonical legacy DIRECT preparation-store schema and pure validation.
//!
//! The materializer package owns the persisted binding, manifest, reference,
//! digest preimages and locator-name layout. Filesystem I/O, source reads,
//! secret protection and concrete digest implementations remain injected by
//! composition.

use core::fmt;

use crate::legacy_direct::{
    CONTENT_DIGEST_ALGORITHM, LegacyDirectPreparationBinding,
    LegacyDirectPreparationFrame, LegacyDirectRepresentationDigest,
    MANIFEST_DIGEST_ALGORITHM, REPRESENTATION_DIGEST_ALGORITHM,
    decode_legacy_direct_preparation, verify_legacy_direct_representation,
};

/// Version-two preparation binding magic.
pub const LEGACY_PREPARATION_MAGIC: &[u8; 8] = b"ELSPRP02";
/// Version-one lookup-reference magic.
pub const LEGACY_PREPARATION_REFERENCE_MAGIC: &[u8; 8] = b"ELSPRF01";
/// Exact encoded preparation binding bytes.
pub const LEGACY_PREPARATION_BINDING_BYTES: usize = 208;
/// Historical version-one binding bytes retained for reference validation.
pub const LEGACY_PREPARATION_OLD_BINDING_BYTES: usize = 176;
/// Exact representation identifier bytes.
pub const LEGACY_PREPARATION_REPRESENTATION_BYTES: usize = 32;
/// Exact algorithm/revision suffix bytes after the representation identifier.
pub const LEGACY_PREPARATION_BINDING_SUFFIX_BYTES: usize = 19;
/// Exact manifest header bytes before the persisted preparation frame.
pub const LEGACY_PREPARATION_HEADER_BYTES: usize =
    LEGACY_PREPARATION_BINDING_BYTES
        + LEGACY_PREPARATION_REPRESENTATION_BYTES
        + LEGACY_PREPARATION_BINDING_SUFFIX_BYTES;
/// Exact lookup-reference record bytes.
pub const LEGACY_PREPARATION_REFERENCE_BYTES: usize = 81;
/// Maximum encoded exact layout bytes retained by legacy DIRECT.
pub const LEGACY_DIRECT_MAX_LAYOUT_BYTES: usize = 64 * 1024 * 1024 - 512;
/// Maximum decoded manifest bytes.
pub const LEGACY_PREPARATION_MAX_MANIFEST_BYTES: usize =
    LEGACY_PREPARATION_HEADER_BYTES + LEGACY_DIRECT_MAX_LAYOUT_BYTES + 1;
/// Maximum encoded plaintext or protected artifact bytes.
pub const LEGACY_PREPARATION_MAX_OBJECT_BYTES: usize = 65 * 1024 * 1024;
/// Preparation root directory name.
pub const LEGACY_PREPARATION_DIRECTORY: &str = "preparation";
/// Preparation lookup-reference directory name.
pub const LEGACY_PREPARATION_REFERENCES_DIRECTORY: &str = "refs";
/// Preparation object directory name.
pub const LEGACY_PREPARATION_OBJECTS_DIRECTORY: &str = "objects";

const LOOKUP_KEY_DOMAIN: &[u8] = b"eliot-search/direct-preparation-ref/v2";
const OBJECT_ID_DOMAIN: &[u8] = b"eliot-search/direct-preparation-object/v2";

/// Closed storage protection tag persisted in lookup references.
#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub enum LegacyPreparationProtection {
    /// Plain unencrypted artifact bytes.
    Plaintext,
    /// Artifact bytes protected by the active secret adapter.
    Protected,
}

impl LegacyPreparationProtection {
    /// Persisted one-byte protection tag.
    #[must_use]
    pub const fn tag(self) -> u8 {
        match self {
            Self::Plaintext => 0,
            Self::Protected => 1,
        }
    }

    /// Canonical legacy artifact extension.
    #[must_use]
    pub const fn extension(self) -> &'static str {
        match self {
            Self::Plaintext => "bin",
            Self::Protected => "dpapi",
        }
    }

    fn from_tag(tag: u8) -> Result<Self, LegacyPreparationStoreError> {
        match tag {
            0 => Ok(Self::Plaintext),
            1 => Ok(Self::Protected),
            _ => Err(LegacyPreparationStoreError::ReferenceInvalid),
        }
    }
}

/// Pure legacy preparation-store failure with the historical daemon reason.
#[derive(Clone, Copy, Debug, Eq, Ord, PartialEq, PartialOrd)]
pub enum LegacyPreparationStoreError {
    /// Binding bytes or supplied binding identity are invalid.
    BindingInvalid,
    /// Lookup-reference bytes are malformed or bind another key.
    ReferenceInvalid,
    /// Manifest framing or preparation frame is invalid.
    ObjectInvalid,
    /// Persisted digest-algorithm tags differ from the fixed schema.
    DigestAlgorithmMismatch,
    /// Persisted materializer or unitizer revision differs from expectation.
    ProfileMismatch,
    /// Manifest binding or representation identity differs from expectation.
    BindingMismatch,
    /// Decoded payload length or SHA-256 differs from its reference.
    ContentMismatch,
    /// Manifest body exceeds the frozen preparation ceiling.
    ManifestTooLarge,
}

impl LegacyPreparationStoreError {
    /// Stable daemon-compatible reason code.
    #[must_use]
    pub const fn code(self) -> &'static str {
        match self {
            Self::BindingInvalid => "DIRECT_PREPARATION_BINDING_INVALID",
            Self::ReferenceInvalid => "DIRECT_PREPARATION_REFERENCE_INVALID",
            Self::ObjectInvalid => "DIRECT_PREPARATION_OBJECT_INVALID",
            Self::DigestAlgorithmMismatch => {
                "DIRECT_PREPARATION_DIGEST_ALGORITHM_MISMATCH"
            }
            Self::ProfileMismatch => "DIRECT_PREPARATION_PROFILE_MISMATCH",
            Self::BindingMismatch => "DIRECT_PREPARATION_BINDING_MISMATCH",
            Self::ContentMismatch => "DIRECT_PREPARATION_CONTENT_MISMATCH",
            Self::ManifestTooLarge => "DIRECT_PREPARATION_TOO_LARGE",
        }
    }
}

impl fmt::Display for LegacyPreparationStoreError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(self.code())
    }
}

impl std::error::Error for LegacyPreparationStoreError {}

/// Composition-supplied SHA-256 primitive for the frozen legacy store schema.
pub trait LegacyPreparationStoreDigest {
    /// Computes SHA-256 over one exact byte slice.
    fn digest(bytes: &[u8]) -> [u8; 32];

    /// Computes the existing domain-separated ordered-parts preimage.
    fn digest_parts(domain: &[u8], parts: &[&[u8]]) -> [u8; 32];
}

/// Decoded lookup-reference state.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct LegacyPreparationReference {
    manifest_digest: [u8; 32],
    manifest_bytes: u64,
    protection: LegacyPreparationProtection,
}

impl LegacyPreparationReference {
    /// Referenced decoded manifest SHA-256.
    #[must_use]
    pub const fn manifest_digest(&self) -> &[u8; 32] {
        &self.manifest_digest
    }

    /// Referenced decoded manifest byte length.
    #[must_use]
    pub const fn manifest_bytes(&self) -> u64 {
        self.manifest_bytes
    }

    /// Persisted storage-protection class.
    #[must_use]
    pub const fn protection(&self) -> LegacyPreparationProtection {
        self.protection
    }
}

/// Verified borrowed manifest projection.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct LegacyPreparationManifestView<'a> {
    binding: LegacyDirectPreparationBinding,
    representation_id: [u8; 32],
    materializer_revision: u64,
    unitizer_revision: u64,
    body: &'a [u8],
}

impl<'a> LegacyPreparationManifestView<'a> {
    /// Decoded immutable source/profile binding.
    #[must_use]
    pub const fn binding(&self) -> &LegacyDirectPreparationBinding {
        &self.binding
    }

    /// Persisted representation identifier.
    #[must_use]
    pub const fn representation_id(&self) -> &[u8; 32] {
        &self.representation_id
    }

    /// Persisted materializer profile revision.
    #[must_use]
    pub const fn materializer_revision(&self) -> u64 {
        self.materializer_revision
    }

    /// Persisted unitizer profile revision.
    #[must_use]
    pub const fn unitizer_revision(&self) -> u64 {
        self.unitizer_revision
    }

    /// Exact persisted preparation frame bytes.
    #[must_use]
    pub const fn body(&self) -> &'a [u8] {
        self.body
    }
}

/// Encodes the exact version-two preparation binding.
#[must_use]
pub fn encode_legacy_preparation_binding(
    binding: &LegacyDirectPreparationBinding,
) -> [u8; LEGACY_PREPARATION_BINDING_BYTES] {
    let mut output = [0_u8; LEGACY_PREPARATION_BINDING_BYTES];
    output[..8].copy_from_slice(LEGACY_PREPARATION_MAGIC);
    output[8..40].copy_from_slice(&binding.namespace);
    output[40..72].copy_from_slice(&binding.source_id);
    output[72..104].copy_from_slice(&binding.revision_id);
    output[104..136].copy_from_slice(&binding.content_digest);
    output[136..144].copy_from_slice(&binding.byte_length.to_be_bytes());
    output[144..176].copy_from_slice(&binding.materializer_digest);
    output[176..208].copy_from_slice(&binding.unitizer_digest);
    output
}

/// Decodes one exact version-two preparation binding.
///
/// # Errors
///
/// Returns [`LegacyPreparationStoreError::BindingInvalid`] for any wrong
/// length or magic.
pub fn decode_legacy_preparation_binding(
    encoded: &[u8],
) -> Result<LegacyDirectPreparationBinding, LegacyPreparationStoreError> {
    if encoded.len() != LEGACY_PREPARATION_BINDING_BYTES
        || &encoded[..8] != LEGACY_PREPARATION_MAGIC
    {
        return Err(LegacyPreparationStoreError::BindingInvalid);
    }
    Ok(LegacyDirectPreparationBinding {
        namespace: copy_32(&encoded[8..40])?,
        source_id: copy_32(&encoded[40..72])?,
        revision_id: copy_32(&encoded[72..104])?,
        content_digest: copy_32(&encoded[104..136])?,
        byte_length: u64::from_be_bytes(
            encoded[136..144]
                .try_into()
                .map_err(|_| LegacyPreparationStoreError::BindingInvalid)?,
        ),
        materializer_digest: copy_32(&encoded[144..176])?,
        unitizer_digest: copy_32(&encoded[176..208])?,
    })
}

/// Derives the exact lookup-reference key.
#[must_use]
pub fn derive_legacy_preparation_lookup_key<D: LegacyPreparationStoreDigest>(
    binding: &[u8; LEGACY_PREPARATION_BINDING_BYTES],
    backend_name: &str,
) -> [u8; 32] {
    D::digest_parts(LOOKUP_KEY_DOMAIN, &[binding, backend_name.as_bytes()])
}

/// Derives the exact preparation object identifier bytes.
#[must_use]
pub fn derive_legacy_preparation_object_id<D: LegacyPreparationStoreDigest>(
    binding: &[u8; LEGACY_PREPARATION_BINDING_BYTES],
    backend_name: &str,
    manifest_digest: &[u8; 32],
) -> [u8; 32] {
    D::digest_parts(
        OBJECT_ID_DOMAIN,
        &[binding, backend_name.as_bytes(), manifest_digest],
    )
}

/// Canonical two-character lower-case shard for a digest-derived locator.
#[must_use]
pub fn legacy_preparation_shard(id: &[u8; 32]) -> String {
    lower_hex(&id[..1])
}

/// Canonical lookup-reference basename.
#[must_use]
pub fn legacy_preparation_reference_file_name(key: &[u8; 32]) -> String {
    format!("{}.ref", lower_hex(key))
}

/// Canonical preparation-object basename.
#[must_use]
pub fn legacy_preparation_object_file_name(
    id: &[u8; 32],
    protection: LegacyPreparationProtection,
) -> String {
    format!("{}.{}", lower_hex(id), protection.extension())
}

/// Encodes one exact lookup-reference record.
///
/// # Errors
///
/// Returns [`LegacyPreparationStoreError::ReferenceInvalid`] when the
/// referenced decoded manifest length is outside the frozen schema bounds.
pub fn encode_legacy_preparation_reference(
    key: &[u8; 32],
    manifest_digest: &[u8; 32],
    manifest_bytes: u64,
    protection: LegacyPreparationProtection,
) -> Result<[u8; LEGACY_PREPARATION_REFERENCE_BYTES], LegacyPreparationStoreError> {
    if manifest_bytes <= LEGACY_PREPARATION_OLD_BINDING_BYTES as u64
        || manifest_bytes > LEGACY_PREPARATION_MAX_MANIFEST_BYTES as u64
    {
        return Err(LegacyPreparationStoreError::ReferenceInvalid);
    }
    let mut output = [0_u8; LEGACY_PREPARATION_REFERENCE_BYTES];
    output[..8].copy_from_slice(LEGACY_PREPARATION_REFERENCE_MAGIC);
    output[8..40].copy_from_slice(key);
    output[40..72].copy_from_slice(manifest_digest);
    output[72..80].copy_from_slice(&manifest_bytes.to_be_bytes());
    output[80] = protection.tag();
    Ok(output)
}

/// Decodes one exact lookup-reference record bound to `key`.
///
/// # Errors
///
/// Returns [`LegacyPreparationStoreError::ReferenceInvalid`] for any wrong
/// framing, key, protection tag or length.
pub fn decode_legacy_preparation_reference(
    encoded: &[u8],
    key: &[u8; 32],
) -> Result<LegacyPreparationReference, LegacyPreparationStoreError> {
    if encoded.len() != LEGACY_PREPARATION_REFERENCE_BYTES
        || &encoded[..8] != LEGACY_PREPARATION_REFERENCE_MAGIC
        || encoded[8..40] != key[..]
    {
        return Err(LegacyPreparationStoreError::ReferenceInvalid);
    }
    let manifest_bytes = u64::from_be_bytes(
        encoded[72..80]
            .try_into()
            .map_err(|_| LegacyPreparationStoreError::ReferenceInvalid)?,
    );
    if manifest_bytes <= LEGACY_PREPARATION_OLD_BINDING_BYTES as u64
        || manifest_bytes > LEGACY_PREPARATION_MAX_MANIFEST_BYTES as u64
    {
        return Err(LegacyPreparationStoreError::ReferenceInvalid);
    }
    Ok(LegacyPreparationReference {
        manifest_digest: copy_32(&encoded[40..72])
            .map_err(|_| LegacyPreparationStoreError::ReferenceInvalid)?,
        manifest_bytes,
        protection: LegacyPreparationProtection::from_tag(encoded[80])?,
    })
}

/// Encodes one exact preparation manifest.
///
/// # Errors
///
/// Returns a typed failure for invalid binding/frame, zero profile revisions
/// or an oversized body.
pub fn encode_legacy_preparation_manifest(
    binding: &[u8; LEGACY_PREPARATION_BINDING_BYTES],
    representation_id: &[u8; 32],
    materializer_revision: u64,
    unitizer_revision: u64,
    body: &[u8],
) -> Result<Vec<u8>, LegacyPreparationStoreError> {
    decode_legacy_preparation_binding(binding)?;
    decode_legacy_direct_preparation(body)
        .map_err(|_| LegacyPreparationStoreError::ObjectInvalid)?;
    if materializer_revision == 0 || unitizer_revision == 0 {
        return Err(LegacyPreparationStoreError::ProfileMismatch);
    }
    let length = LEGACY_PREPARATION_HEADER_BYTES
        .checked_add(body.len())
        .ok_or(LegacyPreparationStoreError::ManifestTooLarge)?;
    if length > LEGACY_PREPARATION_MAX_MANIFEST_BYTES {
        return Err(LegacyPreparationStoreError::ManifestTooLarge);
    }

    let mut output = Vec::with_capacity(length);
    output.extend_from_slice(binding);
    output.extend_from_slice(representation_id);
    output.push(CONTENT_DIGEST_ALGORITHM);
    output.push(REPRESENTATION_DIGEST_ALGORITHM);
    output.extend_from_slice(&materializer_revision.to_be_bytes());
    output.extend_from_slice(&unitizer_revision.to_be_bytes());
    output.push(MANIFEST_DIGEST_ALGORITHM);
    output.extend_from_slice(body);
    Ok(output)
}

/// Verifies one exact preparation manifest and projects its typed fields.
///
/// # Errors
///
/// Returns a typed failure for invalid framing, algorithm/profile drift,
/// binding mismatch or representation mismatch.
pub fn verify_legacy_preparation_manifest<R: LegacyDirectRepresentationDigest>(
    manifest: &[u8],
    expected_binding: &[u8; LEGACY_PREPARATION_BINDING_BYTES],
    expected_materializer_revision: u64,
    expected_unitizer_revision: u64,
) -> Result<LegacyPreparationManifestView<'_>, LegacyPreparationStoreError> {
    if manifest.len() < LEGACY_PREPARATION_HEADER_BYTES + 1
        || manifest.len() > LEGACY_PREPARATION_MAX_MANIFEST_BYTES
    {
        return Err(LegacyPreparationStoreError::ObjectInvalid);
    }
    if manifest.get(..LEGACY_PREPARATION_BINDING_BYTES)
        != Some(expected_binding.as_slice())
    {
        return Err(LegacyPreparationStoreError::BindingMismatch);
    }
    let binding = decode_legacy_preparation_binding(expected_binding)?;
    let representation_start = LEGACY_PREPARATION_BINDING_BYTES;
    let suffix_start =
        representation_start + LEGACY_PREPARATION_REPRESENTATION_BYTES;
    let representation_id = copy_32(
        &manifest[representation_start..suffix_start],
    )
    .map_err(|_| LegacyPreparationStoreError::ObjectInvalid)?;
    let suffix = &manifest[suffix_start..LEGACY_PREPARATION_HEADER_BYTES];
    if suffix.len() != LEGACY_PREPARATION_BINDING_SUFFIX_BYTES
        || suffix[0] != CONTENT_DIGEST_ALGORITHM
        || suffix[1] != REPRESENTATION_DIGEST_ALGORITHM
        || suffix[18] != MANIFEST_DIGEST_ALGORITHM
    {
        return Err(LegacyPreparationStoreError::DigestAlgorithmMismatch);
    }
    let materializer_revision = u64::from_be_bytes(
        suffix[2..10]
            .try_into()
            .map_err(|_| LegacyPreparationStoreError::ObjectInvalid)?,
    );
    let unitizer_revision = u64::from_be_bytes(
        suffix[10..18]
            .try_into()
            .map_err(|_| LegacyPreparationStoreError::ObjectInvalid)?,
    );
    if materializer_revision != expected_materializer_revision
        || unitizer_revision != expected_unitizer_revision
    {
        return Err(LegacyPreparationStoreError::ProfileMismatch);
    }

    let body = &manifest[LEGACY_PREPARATION_HEADER_BYTES..];
    let frame: LegacyDirectPreparationFrame<'_> =
        decode_legacy_direct_preparation(body)
            .map_err(|_| LegacyPreparationStoreError::ObjectInvalid)?;
    verify_legacy_direct_representation::<R>(
        &representation_id,
        &binding,
        frame.identity_marker(),
    )
    .map_err(|_| LegacyPreparationStoreError::BindingMismatch)?;

    Ok(LegacyPreparationManifestView {
        binding,
        representation_id,
        materializer_revision,
        unitizer_revision,
        body,
    })
}

/// Verifies decoded manifest bytes against a lookup reference.
///
/// # Errors
///
/// Returns [`LegacyPreparationStoreError::ContentMismatch`] for any length or
/// SHA-256 mismatch.
pub fn verify_legacy_preparation_payload<D: LegacyPreparationStoreDigest>(
    decoded: &[u8],
    expected_digest: &[u8; 32],
    expected_bytes: u64,
) -> Result<(), LegacyPreparationStoreError> {
    if u64::try_from(decoded.len()).ok() != Some(expected_bytes)
        || D::digest(decoded) != *expected_digest
    {
        return Err(LegacyPreparationStoreError::ContentMismatch);
    }
    Ok(())
}

fn copy_32(bytes: &[u8]) -> Result<[u8; 32], LegacyPreparationStoreError> {
    bytes
        .try_into()
        .map_err(|_| LegacyPreparationStoreError::BindingInvalid)
}

fn lower_hex(bytes: &[u8]) -> String {
    const HEX: &[u8; 16] = b"0123456789abcdef";
    let mut output = String::with_capacity(bytes.len().saturating_mul(2));
    for &byte in bytes {
        output.push(char::from(HEX[usize::from(byte >> 4)]));
        output.push(char::from(HEX[usize::from(byte & 0x0f)]));
    }
    output
}

#[cfg(test)]
mod tests;

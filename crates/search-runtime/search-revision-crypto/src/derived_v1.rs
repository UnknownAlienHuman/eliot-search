//! Pure AES-256-GCM operations for C07 derived-artifact payload v1.
//!
//! C07 owns the descriptor, canonical AAD and ELSDRV01 codec. This module only
//! consumes the exact bounded AAD and selected-reference metadata supplied by
//! that owner; it creates no nonce, counter, permit, key authority or qualification.

use aes_gcm::{
    Aes256Gcm, Nonce,
    aead::{Aead, KeyInit, Payload},
};
use core::fmt;
use search_contracts::{self as c, Blake3Digest32, NonZeroRevision, OpaqueId};
use zeroize::Zeroize;

use crate::{
    MAX_REVISION_PLAINTEXT_BYTES, REVISION_KEY_BYTES, REVISION_NONCE_BYTES, REVISION_TAG_BYTES,
    SELECTED_KEY_MAX_ATTEMPTS, SELECTED_KEY_MAX_AUTHENTICATED_BLOCKS,
    SELECTED_KEY_MAX_PLAINTEXT_BYTES, selected_revision_nonce as selected_nonce,
};

/// Maximum complete C07 v1 AAD accepted by the pure crypto boundary.
pub const DERIVED_ARTIFACT_AAD_MAX_BYTES: usize = crate::SELECTED_REVISION_AAD_MAX_BYTES;
/// Maximum exact canonical selected-Current record carried as data.
pub const DERIVED_ARTIFACT_CURRENT_MAX_BYTES: usize = crate::SELECTED_OWNER_RECORD_MAX_BYTES;

/// Closed, content-free refusal of a derived-artifact v1 primitive.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum DerivedArtifactCryptoErrorV1 {
    /// AAD, selected-reference metadata, counter, nonce or size is invalid.
    InvalidBinding,
    /// Key bytes are the prohibited all-zero value.
    InvalidKey,
    /// Plaintext length or encoded-body BLAKE3 differs from its exact binding.
    ContentMismatch,
    /// Ciphertext/tag length is not exactly plaintext length plus 16 bytes.
    InvalidCiphertext,
    /// Shared canonical encoding or digest owner refused bounded input.
    Encoding,
    /// AEAD encryption failed.
    Encryption,
    /// AEAD authentication failed for the supplied exact binding.
    Authentication,
}

impl DerivedArtifactCryptoErrorV1 {
    /// Stable content-free failure code.
    #[must_use]
    pub const fn code(self) -> &'static str {
        match self {
            Self::InvalidBinding => "DERIVED_ARTIFACT_CRYPTO_BINDING_INVALID",
            Self::InvalidKey => "DERIVED_ARTIFACT_CRYPTO_KEY_INVALID",
            Self::ContentMismatch => "DERIVED_ARTIFACT_CRYPTO_CONTENT_MISMATCH",
            Self::InvalidCiphertext => "DERIVED_ARTIFACT_CRYPTO_CIPHERTEXT_INVALID",
            Self::Encoding => "DERIVED_ARTIFACT_CRYPTO_ENCODING_REFUSED",
            Self::Encryption => "DERIVED_ARTIFACT_CRYPTO_ENCRYPTION_FAILED",
            Self::Authentication => "DERIVED_ARTIFACT_CRYPTO_AUTHENTICATION_FAILED",
        }
    }
}

impl fmt::Display for DerivedArtifactCryptoErrorV1 {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.code())
    }
}

impl std::error::Error for DerivedArtifactCryptoErrorV1 {}

/// Exact selected Current/key metadata carried as data, never as a capability.
///
/// The Windows owner must compare these values with C07's complete binding and
/// the fresh C02 permit. Construction here establishes no selection or authority.
#[derive(Clone, Eq, PartialEq)]
pub struct DerivedArtifactSelectedMetadataV1 {
    current_canonical: Vec<u8>,
    key_reference: OpaqueId,
    key_version: NonZeroRevision,
}

impl DerivedArtifactSelectedMetadataV1 {
    /// Record the exact Current bytes and selected key reference/version.
    pub fn new(
        current_canonical: &[u8],
        key_reference: OpaqueId,
        key_version: NonZeroRevision,
    ) -> Result<Self, DerivedArtifactCryptoErrorV1> {
        if current_canonical.is_empty()
            || current_canonical.len() > DERIVED_ARTIFACT_CURRENT_MAX_BYTES
        {
            return Err(DerivedArtifactCryptoErrorV1::InvalidBinding);
        }
        Ok(Self {
            current_canonical: current_canonical.to_vec(),
            key_reference,
            key_version,
        })
    }

    /// Complete selected Current record, preserved byte-for-byte.
    #[must_use]
    pub fn current_canonical(&self) -> &[u8] {
        &self.current_canonical
    }

    /// Selected key reference as owner-supplied metadata.
    #[must_use]
    pub const fn key_reference(&self) -> &OpaqueId {
        &self.key_reference
    }

    /// Selected key version as owner-supplied metadata.
    #[must_use]
    pub const fn key_version(&self) -> NonZeroRevision {
        self.key_version
    }
}

impl fmt::Debug for DerivedArtifactSelectedMetadataV1 {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str("DerivedArtifactSelectedMetadataV1(<metadata redacted>)")
    }
}

/// Immutable data binding for the existing C07 derived-artifact v1 profile.
///
/// `aad` is C07's already encoded canonical binding. This module does not parse
/// or re-encode its descriptor/AAD/envelope. The digest and length identify the
/// exact encoded artifact body, not a source revision body. All fields are data;
/// the Windows owner remains responsible for comparing them with C07/C02/key
/// owners before using these stateless cryptographic functions.
#[derive(Clone, Eq, PartialEq)]
pub struct DerivedArtifactPayloadBindingV1 {
    aad: Vec<u8>,
    selected: DerivedArtifactSelectedMetadataV1,
    counter: u64,
    nonce: [u8; REVISION_NONCE_BYTES],
    encoded_body_digest: Blake3Digest32,
    encoded_body_length: u64,
}

impl DerivedArtifactPayloadBindingV1 {
    /// Bind exact C07 AAD, selected-reference metadata and externally reserved use.
    ///
    /// The caller supplies the counter and nonce already recorded by C07/C02.
    /// Only the existing zero32 plus big-endian counter64 policy is accepted.
    pub fn from_c07_aad(
        aad: &[u8],
        selected: DerivedArtifactSelectedMetadataV1,
        counter: u64,
        nonce: [u8; REVISION_NONCE_BYTES],
        encoded_body_digest: Blake3Digest32,
        encoded_body_length: u64,
    ) -> Result<Self, DerivedArtifactCryptoErrorV1> {
        if aad.is_empty() || aad.len() > DERIVED_ARTIFACT_AAD_MAX_BYTES {
            return Err(DerivedArtifactCryptoErrorV1::InvalidBinding);
        }
        let binding = Self {
            aad: aad.to_vec(),
            selected,
            counter,
            nonce,
            encoded_body_digest,
            encoded_body_length,
        };
        validate_binding(&binding)?;
        Ok(binding)
    }

    /// Exact canonical AAD bytes supplied by C07.
    #[must_use]
    pub fn aad_bytes(&self) -> &[u8] {
        &self.aad
    }

    /// Exact selected Current/key metadata supplied by C07 and its owner.
    #[must_use]
    pub const fn selected_metadata(&self) -> &DerivedArtifactSelectedMetadataV1 {
        &self.selected
    }

    /// Exact selected Current bytes supplied as metadata.
    #[must_use]
    pub fn current_canonical(&self) -> &[u8] {
        self.selected.current_canonical()
    }

    /// Selected key reference supplied as metadata.
    #[must_use]
    pub const fn key_reference(&self) -> &OpaqueId {
        self.selected.key_reference()
    }

    /// Selected key version supplied as metadata.
    #[must_use]
    pub const fn key_version(&self) -> NonZeroRevision {
        self.selected.key_version()
    }

    /// Externally reserved counter; this API never allocates or advances it.
    #[must_use]
    pub const fn counter(&self) -> u64 {
        self.counter
    }

    /// Existing zero32 plus big-endian counter64 nonce.
    #[must_use]
    pub const fn nonce(&self) -> &[u8; REVISION_NONCE_BYTES] {
        &self.nonce
    }

    /// Direct BLAKE3 of the exact encoded plaintext artifact body.
    #[must_use]
    pub const fn encoded_body_digest(&self) -> Blake3Digest32 {
        self.encoded_body_digest
    }

    /// Exact encoded plaintext artifact-body length.
    #[must_use]
    pub const fn encoded_body_length(&self) -> u64 {
        self.encoded_body_length
    }

    /// Exact per-call GHASH blocks: ceil(P/16) + ceil(AAD/16) + one length block.
    pub fn authenticated_blocks(&self) -> Result<u64, DerivedArtifactCryptoErrorV1> {
        authenticated_blocks(self.encoded_body_length, self.aad.len())
    }
}

impl fmt::Debug for DerivedArtifactPayloadBindingV1 {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str("DerivedArtifactPayloadBindingV1(Unqualified)")
    }
}

/// Authenticated encoded-artifact plaintext, privately owned and wiped on drop.
pub struct DerivedArtifactPayloadPlaintextV1 {
    bytes: Vec<u8>,
}

impl DerivedArtifactPayloadPlaintextV1 {
    /// Borrow authenticated encoded artifact bytes; this grants no source authority.
    #[must_use]
    pub fn as_bytes(&self) -> &[u8] {
        &self.bytes
    }
}

impl Drop for DerivedArtifactPayloadPlaintextV1 {
    fn drop(&mut self) {
        self.bytes.zeroize();
    }
}

impl fmt::Debug for DerivedArtifactPayloadPlaintextV1 {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str("DerivedArtifactPayloadPlaintextV1(<redacted>)")
    }
}

/// Seal exact encoded artifact bytes with the existing AES-256-GCM/tag128 primitive.
///
/// The result is only ciphertext and its 16-byte tag for C07's ELSDRV01 codec.
/// This stateless call creates no reservation, nonce, permit or qualification.
pub fn seal_derived_artifact_payload_v1(
    key: &[u8; REVISION_KEY_BYTES],
    binding: &DerivedArtifactPayloadBindingV1,
    encoded_body: &[u8],
) -> Result<Vec<u8>, DerivedArtifactCryptoErrorV1> {
    validate_key(key)?;
    validate_binding(binding)?;
    validate_encoded_body(binding, encoded_body)?;

    let cipher =
        Aes256Gcm::new_from_slice(key).map_err(|_| DerivedArtifactCryptoErrorV1::InvalidKey)?;
    let ciphertext_and_tag = cipher
        .encrypt(
            Nonce::from_slice(binding.nonce()),
            Payload {
                msg: encoded_body,
                aad: binding.aad_bytes(),
            },
        )
        .map_err(|_| DerivedArtifactCryptoErrorV1::Encryption)?;
    let expected_length = ciphertext_length(binding.encoded_body_length)?;
    if ciphertext_and_tag.len() != expected_length {
        return Err(DerivedArtifactCryptoErrorV1::InvalidCiphertext);
    }
    Ok(ciphertext_and_tag)
}

/// Open ciphertext/tag against the exact expected C07 AAD and encoded-body identity.
///
/// The returned owned plaintext is zeroizing. The caller must still perform C07's
/// independent body check; authentication here is not source readback or qualification.
pub fn open_derived_artifact_payload_v1(
    key: &[u8; REVISION_KEY_BYTES],
    expected: &DerivedArtifactPayloadBindingV1,
    ciphertext_and_tag: &[u8],
) -> Result<DerivedArtifactPayloadPlaintextV1, DerivedArtifactCryptoErrorV1> {
    validate_key(key)?;
    validate_binding(expected)?;
    if ciphertext_and_tag.len() != ciphertext_length(expected.encoded_body_length)? {
        return Err(DerivedArtifactCryptoErrorV1::InvalidCiphertext);
    }

    let cipher =
        Aes256Gcm::new_from_slice(key).map_err(|_| DerivedArtifactCryptoErrorV1::InvalidKey)?;
    let bytes = cipher
        .decrypt(
            Nonce::from_slice(expected.nonce()),
            Payload {
                msg: ciphertext_and_tag,
                aad: expected.aad_bytes(),
            },
        )
        .map_err(|_| DerivedArtifactCryptoErrorV1::Authentication)?;
    let plaintext = DerivedArtifactPayloadPlaintextV1 { bytes };
    validate_encoded_body(expected, plaintext.as_bytes())?;
    Ok(plaintext)
}

fn validate_key(key: &[u8; REVISION_KEY_BYTES]) -> Result<(), DerivedArtifactCryptoErrorV1> {
    if key.iter().all(|byte| *byte == 0) {
        Err(DerivedArtifactCryptoErrorV1::InvalidKey)
    } else {
        Ok(())
    }
}

fn validate_binding(
    binding: &DerivedArtifactPayloadBindingV1,
) -> Result<(), DerivedArtifactCryptoErrorV1> {
    if binding.counter >= SELECTED_KEY_MAX_ATTEMPTS
        || binding.nonce != selected_nonce(binding.counter)
        || binding.encoded_body_length > MAX_REVISION_PLAINTEXT_BYTES as u64
        || binding.encoded_body_length > SELECTED_KEY_MAX_PLAINTEXT_BYTES
        || authenticated_blocks(binding.encoded_body_length, binding.aad.len())?
            > SELECTED_KEY_MAX_AUTHENTICATED_BLOCKS
    {
        return Err(DerivedArtifactCryptoErrorV1::InvalidBinding);
    }
    Ok(())
}

fn validate_encoded_body(
    binding: &DerivedArtifactPayloadBindingV1,
    encoded_body: &[u8],
) -> Result<(), DerivedArtifactCryptoErrorV1> {
    let length =
        u64::try_from(encoded_body.len()).map_err(|_| DerivedArtifactCryptoErrorV1::Encoding)?;
    if length != binding.encoded_body_length {
        return Err(DerivedArtifactCryptoErrorV1::ContentMismatch);
    }
    let budget = c::Blake3ByteBudget::new(MAX_REVISION_PLAINTEXT_BYTES)
        .map_err(|_| DerivedArtifactCryptoErrorV1::Encoding)?;
    let digest = c::blake3_bounded_bytes(encoded_body, budget)
        .map_err(|_| DerivedArtifactCryptoErrorV1::Encoding)?;
    if digest != binding.encoded_body_digest {
        return Err(DerivedArtifactCryptoErrorV1::ContentMismatch);
    }
    Ok(())
}

fn authenticated_blocks(
    plaintext_length: u64,
    aad_length: usize,
) -> Result<u64, DerivedArtifactCryptoErrorV1> {
    let aad_length =
        u64::try_from(aad_length).map_err(|_| DerivedArtifactCryptoErrorV1::InvalidBinding)?;
    let plaintext_blocks = plaintext_length
        .checked_add(15)
        .ok_or(DerivedArtifactCryptoErrorV1::InvalidBinding)?
        / 16;
    let aad_blocks = aad_length
        .checked_add(15)
        .ok_or(DerivedArtifactCryptoErrorV1::InvalidBinding)?
        / 16;
    plaintext_blocks
        .checked_add(aad_blocks)
        .and_then(|blocks| blocks.checked_add(1))
        .ok_or(DerivedArtifactCryptoErrorV1::InvalidBinding)
}

fn ciphertext_length(plaintext_length: u64) -> Result<usize, DerivedArtifactCryptoErrorV1> {
    let plaintext_length = usize::try_from(plaintext_length)
        .map_err(|_| DerivedArtifactCryptoErrorV1::InvalidBinding)?;
    plaintext_length
        .checked_add(REVISION_TAG_BYTES)
        .ok_or(DerivedArtifactCryptoErrorV1::InvalidBinding)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn selected_metadata() -> DerivedArtifactSelectedMetadataV1 {
        DerivedArtifactSelectedMetadataV1::new(
            &[0x82, 0x01, 0x07],
            OpaqueId::new("test-derived-key").unwrap(),
            NonZeroRevision::new(3).unwrap(),
        )
        .unwrap()
    }

    fn aad_fixture(marker: u8) -> Vec<u8> {
        // Opaque canonical bytes keep this pure test independent of C07's codec.
        vec![0x82, 0x01, marker]
    }
    fn binding(
        aad: &[u8],
        digest: Blake3Digest32,
        length: u64,
        counter: u64,
        nonce: [u8; REVISION_NONCE_BYTES],
    ) -> DerivedArtifactPayloadBindingV1 {
        DerivedArtifactPayloadBindingV1::from_c07_aad(
            aad,
            selected_metadata(),
            counter,
            nonce,
            digest,
            length,
        )
        .unwrap()
    }

    #[test]
    fn c07_aad_roundtrip_and_tampering_fail_closed() {
        let key = [0xA5; REVISION_KEY_BYTES];
        let encoded_body = b"encoded derived representation bytes";
        let digest = c::blake3_bounded_bytes(
            encoded_body,
            c::Blake3ByteBudget::new(MAX_REVISION_PLAINTEXT_BYTES).unwrap(),
        )
        .unwrap();
        let counter = 41;
        let aad = aad_fixture(1);
        let expected = binding(
            &aad,
            digest,
            encoded_body.len() as u64,
            counter,
            selected_nonce(counter),
        );
        let ciphertext_and_tag =
            seal_derived_artifact_payload_v1(&key, &expected, encoded_body).unwrap();
        assert_eq!(
            ciphertext_and_tag.len(),
            encoded_body.len() + REVISION_TAG_BYTES
        );
        let opened =
            open_derived_artifact_payload_v1(&key, &expected, &ciphertext_and_tag).unwrap();
        assert_eq!(opened.as_bytes(), encoded_body);

        let wrong_aad = binding(
            &aad_fixture(2),
            digest,
            encoded_body.len() as u64,
            counter,
            selected_nonce(counter),
        );
        assert_eq!(
            open_derived_artifact_payload_v1(&key, &wrong_aad, &ciphertext_and_tag).unwrap_err(),
            DerivedArtifactCryptoErrorV1::Authentication,
        );

        let wrong_digest = binding(
            &aad,
            Blake3Digest32::from_bytes([0x5A; 32]),
            encoded_body.len() as u64,
            counter,
            selected_nonce(counter),
        );
        assert_eq!(
            open_derived_artifact_payload_v1(&key, &wrong_digest, &ciphertext_and_tag).unwrap_err(),
            DerivedArtifactCryptoErrorV1::ContentMismatch,
        );

        let wrong_length = binding(
            &aad,
            digest,
            encoded_body.len() as u64 + 1,
            counter,
            selected_nonce(counter),
        );
        assert_eq!(
            open_derived_artifact_payload_v1(&key, &wrong_length, &ciphertext_and_tag).unwrap_err(),
            DerivedArtifactCryptoErrorV1::InvalidCiphertext,
        );

        let wrong_counter_nonce = binding(
            &aad,
            digest,
            encoded_body.len() as u64,
            counter + 1,
            selected_nonce(counter + 1),
        );
        assert_eq!(
            open_derived_artifact_payload_v1(&key, &wrong_counter_nonce, &ciphertext_and_tag)
                .unwrap_err(),
            DerivedArtifactCryptoErrorV1::Authentication,
        );
        let mut wrong_nonce = selected_nonce(counter);
        wrong_nonce[0] = 1;
        assert!(matches!(
            DerivedArtifactPayloadBindingV1::from_c07_aad(
                &aad,
                selected_metadata(),
                counter,
                wrong_nonce,
                digest,
                encoded_body.len() as u64,
            ),
            Err(DerivedArtifactCryptoErrorV1::InvalidBinding),
        ));
    }
}

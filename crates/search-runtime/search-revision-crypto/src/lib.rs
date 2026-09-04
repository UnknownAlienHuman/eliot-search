//! Authenticated encryption for immutable source-revision bytes.
//!
//! Large revisions are encrypted with a random-nonce AES-256-GCM envelope. The
//! data-encryption key is deliberately external: on Windows it is expected to
//! be persisted only after wrapping by `search-os-secrets-windows` DPAPI.
//!
//! Every envelope authenticates its source-revision, residency, encryption
//! profile, key generation, plaintext length, and expected content digest. A
//! ciphertext cannot therefore be replayed under another revision or policy
//! binding even when its bytes are copied successfully.

#![forbid(unsafe_code)]
#![deny(missing_docs)]
#![allow(clippy::missing_errors_doc, clippy::module_name_repetitions)]

use core::fmt;
use core::num::NonZeroU64;

use aes_gcm::aead::{Aead, KeyInit, Payload};
use aes_gcm::{Aes256Gcm, Nonce};
use sha2::{Digest, Sha256};
use zeroize::Zeroize;

/// AES-256 key size.
pub const REVISION_KEY_BYTES: usize = 32;
/// GCM nonce size.
pub const REVISION_NONCE_BYTES: usize = 12;
/// GCM authentication-tag size appended by the AEAD implementation.
pub const REVISION_TAG_BYTES: usize = 16;
/// Maximum plaintext revision size accepted by this local profile.
pub const MAX_REVISION_PLAINTEXT_BYTES: usize = 64 * 1024 * 1024;
/// Binary envelope header size.
pub const REVISION_ENVELOPE_HEADER_BYTES: usize = 176;
/// Maximum complete encoded envelope size.
pub const MAX_REVISION_ENVELOPE_BYTES: usize =
    REVISION_ENVELOPE_HEADER_BYTES + MAX_REVISION_PLAINTEXT_BYTES + REVISION_TAG_BYTES;
/// Current envelope version.
pub const REVISION_ENVELOPE_VERSION: u16 = 1;

const ENVELOPE_MAGIC: [u8; 8] = *b"ELSREV01";
const ALGORITHM_AES_256_GCM: u8 = 1;
const AAD_DOMAIN: &[u8] = b"eliot-search.revision-aead.v1";

/// Closed authenticated-revision failure.
#[derive(Clone, Copy, Debug, Eq, Ord, PartialEq, PartialOrd)]
pub enum RevisionCryptoError {
    /// Key generation is zero or does not match the envelope binding.
    KeyGenerationMismatch,
    /// Key material is the all-zero value.
    InvalidKey,
    /// The operating-system random source failed.
    RandomnessUnavailable,
    /// Plaintext exceeds the finite revision limit.
    PlaintextTooLarge,
    /// Caller-supplied plaintext length differs from the exact bytes.
    PlaintextLengthMismatch,
    /// Caller-supplied content digest differs from the exact bytes.
    ContentDigestMismatch,
    /// AEAD encryption failed.
    EncryptionFailed,
    /// AEAD authentication or decryption failed.
    AuthenticationFailed,
    /// Encoded envelope is truncated, oversized, or internally inconsistent.
    InvalidEnvelope,
    /// Envelope version is not supported.
    UnsupportedVersion,
    /// Envelope algorithm is not supported.
    UnsupportedAlgorithm,
    /// A byte length cannot be represented safely.
    LengthOverflow,
}

impl RevisionCryptoError {
    /// Stable machine-readable reason code.
    #[must_use]
    pub const fn code(self) -> &'static str {
        match self {
            Self::KeyGenerationMismatch => "REVISION_CRYPTO_KEY_GENERATION_MISMATCH",
            Self::InvalidKey => "REVISION_CRYPTO_KEY_INVALID",
            Self::RandomnessUnavailable => "REVISION_CRYPTO_RANDOMNESS_UNAVAILABLE",
            Self::PlaintextTooLarge => "REVISION_CRYPTO_PLAINTEXT_TOO_LARGE",
            Self::PlaintextLengthMismatch => "REVISION_CRYPTO_PLAINTEXT_LENGTH_MISMATCH",
            Self::ContentDigestMismatch => "REVISION_CRYPTO_CONTENT_DIGEST_MISMATCH",
            Self::EncryptionFailed => "REVISION_CRYPTO_ENCRYPTION_FAILED",
            Self::AuthenticationFailed => "REVISION_CRYPTO_AUTHENTICATION_FAILED",
            Self::InvalidEnvelope => "REVISION_CRYPTO_ENVELOPE_INVALID",
            Self::UnsupportedVersion => "REVISION_CRYPTO_VERSION_UNSUPPORTED",
            Self::UnsupportedAlgorithm => "REVISION_CRYPTO_ALGORITHM_UNSUPPORTED",
            Self::LengthOverflow => "REVISION_CRYPTO_LENGTH_OVERFLOW",
        }
    }
}

impl fmt::Display for RevisionCryptoError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(self.code())
    }
}

impl std::error::Error for RevisionCryptoError {}

/// Non-cloneable 256-bit data-encryption key with monotone generation.
pub struct RevisionKey {
    generation: NonZeroU64,
    bytes: [u8; REVISION_KEY_BYTES],
}

impl RevisionKey {
    /// Generates a fresh key from the operating-system CSPRNG.
    pub fn generate(generation: NonZeroU64) -> Result<Self, RevisionCryptoError> {
        let mut bytes = [0_u8; REVISION_KEY_BYTES];
        getrandom::fill(&mut bytes).map_err(|_| RevisionCryptoError::RandomnessUnavailable)?;
        Self::from_bytes(generation, bytes)
    }

    /// Imports exact unwrapped key bytes.
    ///
    /// The caller should obtain these bytes only from an authenticated OS-secret
    /// boundary and must not persist the returned key in plaintext.
    pub fn from_bytes(
        generation: NonZeroU64,
        bytes: [u8; REVISION_KEY_BYTES],
    ) -> Result<Self, RevisionCryptoError> {
        if bytes.iter().all(|byte| *byte == 0) {
            return Err(RevisionCryptoError::InvalidKey);
        }
        Ok(Self { generation, bytes })
    }

    /// Monotone key generation authenticated into every envelope.
    #[must_use]
    pub const fn generation(&self) -> NonZeroU64 {
        self.generation
    }

    /// Exposes key bytes only for an immediate wrapping or cryptographic call.
    pub fn with_key_bytes<T>(&self, use_key: impl FnOnce(&[u8; REVISION_KEY_BYTES]) -> T) -> T {
        use_key(&self.bytes)
    }
}

impl fmt::Debug for RevisionKey {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("RevisionKey")
            .field("generation", &self.generation)
            .field("bytes", &"<redacted>")
            .finish()
    }
}

impl Drop for RevisionKey {
    fn drop(&mut self) {
        self.bytes.zeroize();
    }
}

/// Exact authority and integrity binding authenticated as AEAD associated data.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct RevisionBinding {
    /// Monotone data-encryption-key generation.
    pub key_generation: NonZeroU64,
    /// Digest of stable source plus immutable revision identity.
    pub source_revision_binding_digest: [u8; 32],
    /// Digest of the complete object-residency key.
    pub residency_binding_digest: [u8; 32],
    /// Digest of encryption/materialization profile identity.
    pub encryption_profile_digest: [u8; 32],
    /// SHA-256 digest of exact plaintext bytes.
    pub content_digest: [u8; 32],
    /// Exact plaintext byte length.
    pub plaintext_length: u64,
}

impl RevisionBinding {
    /// Validates the finite plaintext length.
    pub fn validate(self) -> Result<Self, RevisionCryptoError> {
        if self.plaintext_length
            > u64::try_from(MAX_REVISION_PLAINTEXT_BYTES)
                .map_err(|_| RevisionCryptoError::LengthOverflow)?
        {
            return Err(RevisionCryptoError::PlaintextTooLarge);
        }
        Ok(self)
    }
}

/// Versioned authenticated ciphertext for one immutable revision.
#[derive(Clone, Eq, PartialEq)]
pub struct EncryptedRevisionEnvelope {
    version: u16,
    nonce: [u8; REVISION_NONCE_BYTES],
    binding: RevisionBinding,
    ciphertext_and_tag: Vec<u8>,
}

impl EncryptedRevisionEnvelope {
    /// Envelope version.
    #[must_use]
    pub const fn version(&self) -> u16 {
        self.version
    }

    /// Random 96-bit GCM nonce.
    #[must_use]
    pub const fn nonce(&self) -> [u8; REVISION_NONCE_BYTES] {
        self.nonce
    }

    /// Authenticated authority and content binding.
    #[must_use]
    pub const fn binding(&self) -> RevisionBinding {
        self.binding
    }

    /// Exact ciphertext followed by the GCM authentication tag.
    #[must_use]
    pub fn ciphertext_and_tag(&self) -> &[u8] {
        &self.ciphertext_and_tag
    }

    /// Encodes the complete stable binary envelope.
    pub fn encode(&self) -> Result<Vec<u8>, RevisionCryptoError> {
        validate_envelope(self)?;
        let ciphertext_length = u64::try_from(self.ciphertext_and_tag.len())
            .map_err(|_| RevisionCryptoError::LengthOverflow)?;
        let mut output = Vec::with_capacity(
            REVISION_ENVELOPE_HEADER_BYTES
                .checked_add(self.ciphertext_and_tag.len())
                .ok_or(RevisionCryptoError::LengthOverflow)?,
        );
        output.extend_from_slice(&ENVELOPE_MAGIC);
        output.extend_from_slice(&self.version.to_be_bytes());
        output.push(ALGORITHM_AES_256_GCM);
        output.push(0);
        output.extend_from_slice(&self.binding.key_generation.get().to_be_bytes());
        output.extend_from_slice(&self.nonce);
        output.extend_from_slice(&self.binding.source_revision_binding_digest);
        output.extend_from_slice(&self.binding.residency_binding_digest);
        output.extend_from_slice(&self.binding.encryption_profile_digest);
        output.extend_from_slice(&self.binding.content_digest);
        output.extend_from_slice(&self.binding.plaintext_length.to_be_bytes());
        output.extend_from_slice(&ciphertext_length.to_be_bytes());
        output.extend_from_slice(&self.ciphertext_and_tag);
        if output.len() > MAX_REVISION_ENVELOPE_BYTES {
            return Err(RevisionCryptoError::InvalidEnvelope);
        }
        Ok(output)
    }

    /// Decodes and structurally validates one complete binary envelope.
    pub fn decode(bytes: &[u8]) -> Result<Self, RevisionCryptoError> {
        if bytes.len() < REVISION_ENVELOPE_HEADER_BYTES + REVISION_TAG_BYTES
            || bytes.len() > MAX_REVISION_ENVELOPE_BYTES
        {
            return Err(RevisionCryptoError::InvalidEnvelope);
        }
        let mut cursor = 0_usize;
        if take::<8>(bytes, &mut cursor)? != ENVELOPE_MAGIC {
            return Err(RevisionCryptoError::InvalidEnvelope);
        }
        let version = u16::from_be_bytes(take::<2>(bytes, &mut cursor)?);
        if version != REVISION_ENVELOPE_VERSION {
            return Err(RevisionCryptoError::UnsupportedVersion);
        }
        let algorithm = take::<1>(bytes, &mut cursor)?[0];
        if algorithm != ALGORITHM_AES_256_GCM {
            return Err(RevisionCryptoError::UnsupportedAlgorithm);
        }
        if take::<1>(bytes, &mut cursor)?[0] != 0 {
            return Err(RevisionCryptoError::InvalidEnvelope);
        }
        let key_generation = NonZeroU64::new(u64::from_be_bytes(take::<8>(bytes, &mut cursor)?))
            .ok_or(RevisionCryptoError::KeyGenerationMismatch)?;
        let nonce = take::<REVISION_NONCE_BYTES>(bytes, &mut cursor)?;
        let source_revision_binding_digest = take::<32>(bytes, &mut cursor)?;
        let residency_binding_digest = take::<32>(bytes, &mut cursor)?;
        let encryption_profile_digest = take::<32>(bytes, &mut cursor)?;
        let content_digest = take::<32>(bytes, &mut cursor)?;
        let plaintext_length = u64::from_be_bytes(take::<8>(bytes, &mut cursor)?);
        let ciphertext_length = usize::try_from(u64::from_be_bytes(take::<8>(bytes, &mut cursor)?))
            .map_err(|_| RevisionCryptoError::LengthOverflow)?;
        if cursor != REVISION_ENVELOPE_HEADER_BYTES
            || bytes.len().checked_sub(cursor) != Some(ciphertext_length)
        {
            return Err(RevisionCryptoError::InvalidEnvelope);
        }
        let envelope = Self {
            version,
            nonce,
            binding: RevisionBinding {
                key_generation,
                source_revision_binding_digest,
                residency_binding_digest,
                encryption_profile_digest,
                content_digest,
                plaintext_length,
            }
            .validate()?,
            ciphertext_and_tag: bytes[cursor..].to_vec(),
        };
        validate_envelope(&envelope)?;
        Ok(envelope)
    }
}

impl fmt::Debug for EncryptedRevisionEnvelope {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("EncryptedRevisionEnvelope")
            .field("version", &self.version)
            .field("nonce", &self.nonce)
            .field("binding", &self.binding)
            .field(
                "ciphertext_and_tag",
                &format_args!("<{} encrypted bytes>", self.ciphertext_and_tag.len()),
            )
            .finish()
    }
}

/// Owned decrypted revision bytes with overwrite-on-drop semantics.
pub struct PlaintextRevision {
    bytes: Vec<u8>,
}

impl PlaintextRevision {
    /// Exact authenticated plaintext bytes.
    #[must_use]
    pub fn as_bytes(&self) -> &[u8] {
        &self.bytes
    }

    /// Authenticated plaintext byte length.
    #[must_use]
    pub fn len(&self) -> usize {
        self.bytes.len()
    }

    /// Returns whether the authenticated revision is empty.
    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.bytes.is_empty()
    }
}

impl fmt::Debug for PlaintextRevision {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("PlaintextRevision")
            .field("bytes", &"<redacted>")
            .field("length", &self.bytes.len())
            .finish()
    }
}

impl Drop for PlaintextRevision {
    fn drop(&mut self) {
        self.bytes.zeroize();
    }
}

/// Computes the SHA-256 digest used by [`RevisionBinding::content_digest`].
#[must_use]
pub fn sha256_digest(bytes: &[u8]) -> [u8; 32] {
    Sha256::digest(bytes).into()
}

/// Encrypts exact revision bytes with a fresh operating-system random nonce.
pub fn seal_revision(
    key: &RevisionKey,
    binding: RevisionBinding,
    plaintext: &[u8],
) -> Result<EncryptedRevisionEnvelope, RevisionCryptoError> {
    let binding = binding.validate()?;
    validate_plaintext_binding(key, binding, plaintext)?;
    let mut nonce = [0_u8; REVISION_NONCE_BYTES];
    getrandom::fill(&mut nonce).map_err(|_| RevisionCryptoError::RandomnessUnavailable)?;
    seal_with_nonce(key, binding, plaintext, nonce)
}

/// Authenticates and decrypts one exact revision envelope.
pub fn open_revision(
    key: &RevisionKey,
    expected_binding: RevisionBinding,
    envelope: &EncryptedRevisionEnvelope,
) -> Result<PlaintextRevision, RevisionCryptoError> {
    validate_envelope(envelope)?;
    let expected_binding = expected_binding.validate()?;
    if key.generation() != envelope.binding.key_generation
        || key.generation() != expected_binding.key_generation
    {
        return Err(RevisionCryptoError::KeyGenerationMismatch);
    }
    if envelope.binding != expected_binding {
        return Err(RevisionCryptoError::AuthenticationFailed);
    }
    let aad = associated_data(envelope.binding, envelope.nonce);
    let cipher = key.with_key_bytes(|bytes| Aes256Gcm::new_from_slice(bytes))
        .map_err(|_| RevisionCryptoError::InvalidKey)?;
    let plaintext = cipher
        .decrypt(
            Nonce::from_slice(&envelope.nonce),
            Payload {
                msg: &envelope.ciphertext_and_tag,
                aad: &aad,
            },
        )
        .map_err(|_| RevisionCryptoError::AuthenticationFailed)?;
    if u64::try_from(plaintext.len()).map_err(|_| RevisionCryptoError::LengthOverflow)?
        != envelope.binding.plaintext_length
    {
        return Err(RevisionCryptoError::PlaintextLengthMismatch);
    }
    if sha256_digest(&plaintext) != envelope.binding.content_digest {
        return Err(RevisionCryptoError::ContentDigestMismatch);
    }
    Ok(PlaintextRevision { bytes: plaintext })
}

fn seal_with_nonce(
    key: &RevisionKey,
    binding: RevisionBinding,
    plaintext: &[u8],
    nonce: [u8; REVISION_NONCE_BYTES],
) -> Result<EncryptedRevisionEnvelope, RevisionCryptoError> {
    let aad = associated_data(binding, nonce);
    let cipher = key.with_key_bytes(|bytes| Aes256Gcm::new_from_slice(bytes))
        .map_err(|_| RevisionCryptoError::InvalidKey)?;
    let ciphertext_and_tag = cipher
        .encrypt(
            Nonce::from_slice(&nonce),
            Payload {
                msg: plaintext,
                aad: &aad,
            },
        )
        .map_err(|_| RevisionCryptoError::EncryptionFailed)?;
    let envelope = EncryptedRevisionEnvelope {
        version: REVISION_ENVELOPE_VERSION,
        nonce,
        binding,
        ciphertext_and_tag,
    };
    validate_envelope(&envelope)?;
    Ok(envelope)
}

fn validate_plaintext_binding(
    key: &RevisionKey,
    binding: RevisionBinding,
    plaintext: &[u8],
) -> Result<(), RevisionCryptoError> {
    if plaintext.len() > MAX_REVISION_PLAINTEXT_BYTES {
        return Err(RevisionCryptoError::PlaintextTooLarge);
    }
    if key.generation() != binding.key_generation {
        return Err(RevisionCryptoError::KeyGenerationMismatch);
    }
    let length = u64::try_from(plaintext.len()).map_err(|_| RevisionCryptoError::LengthOverflow)?;
    if length != binding.plaintext_length {
        return Err(RevisionCryptoError::PlaintextLengthMismatch);
    }
    if sha256_digest(plaintext) != binding.content_digest {
        return Err(RevisionCryptoError::ContentDigestMismatch);
    }
    Ok(())
}

fn validate_envelope(envelope: &EncryptedRevisionEnvelope) -> Result<(), RevisionCryptoError> {
    if envelope.version != REVISION_ENVELOPE_VERSION {
        return Err(RevisionCryptoError::UnsupportedVersion);
    }
    envelope.binding.validate()?;
    let plaintext_length = usize::try_from(envelope.binding.plaintext_length)
        .map_err(|_| RevisionCryptoError::LengthOverflow)?;
    let expected_ciphertext_length = plaintext_length
        .checked_add(REVISION_TAG_BYTES)
        .ok_or(RevisionCryptoError::LengthOverflow)?;
    if envelope.ciphertext_and_tag.len() != expected_ciphertext_length
        || envelope.ciphertext_and_tag.len()
            > MAX_REVISION_PLAINTEXT_BYTES + REVISION_TAG_BYTES
    {
        return Err(RevisionCryptoError::InvalidEnvelope);
    }
    Ok(())
}

fn associated_data(
    binding: RevisionBinding,
    nonce: [u8; REVISION_NONCE_BYTES],
) -> Vec<u8> {
    let mut aad = Vec::with_capacity(
        AAD_DOMAIN.len() + 2 + 1 + 8 + REVISION_NONCE_BYTES + 32 * 4 + 8,
    );
    aad.extend_from_slice(AAD_DOMAIN);
    aad.extend_from_slice(&REVISION_ENVELOPE_VERSION.to_be_bytes());
    aad.push(ALGORITHM_AES_256_GCM);
    aad.extend_from_slice(&binding.key_generation.get().to_be_bytes());
    aad.extend_from_slice(&nonce);
    aad.extend_from_slice(&binding.source_revision_binding_digest);
    aad.extend_from_slice(&binding.residency_binding_digest);
    aad.extend_from_slice(&binding.encryption_profile_digest);
    aad.extend_from_slice(&binding.content_digest);
    aad.extend_from_slice(&binding.plaintext_length.to_be_bytes());
    aad
}

fn take<const SIZE: usize>(
    bytes: &[u8],
    cursor: &mut usize,
) -> Result<[u8; SIZE], RevisionCryptoError> {
    let end = cursor
        .checked_add(SIZE)
        .ok_or(RevisionCryptoError::LengthOverflow)?;
    let slice = bytes
        .get(*cursor..end)
        .ok_or(RevisionCryptoError::InvalidEnvelope)?;
    let output = slice
        .try_into()
        .map_err(|_| RevisionCryptoError::InvalidEnvelope)?;
    *cursor = end;
    Ok(output)
}

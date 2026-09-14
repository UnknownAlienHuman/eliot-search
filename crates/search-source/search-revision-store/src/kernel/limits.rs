//! Finite revision-store limits and persisted format constants.

use super::error::RevisionStoreError;

/// Version of the typed [`super::ResidencyClosure`] encoding.
pub const RESIDENCY_CLOSURE_VERSION: u16 = 1;
/// Version of the [`super::EnvelopeBinding`] carried beside every intent.
///
/// This tracks the existing authenticated-encryption envelope profile version
/// (`search-revision-crypto` `REVISION_ENVELOPE_VERSION = 1`). The store
/// creates no cipher and accepts no other envelope version.
pub const ENVELOPE_BINDING_VERSION: u16 = 1;
/// Version of the [`super::CasObjectAddress`] schema.
pub const CAS_ADDRESS_VERSION: u16 = 1;
/// Minimum ciphertext bytes: the authentication-tag floor of the existing
/// envelope profile. Shorter objects are truncated envelopes, never revisions.
pub const MIN_CIPHERTEXT_BYTES: usize = 16;

/// Conservative finite retained-revision limits.
pub const DEFAULT_REVISION_STORE_LIMITS: RevisionStoreLimits = RevisionStoreLimits {
    max_plaintext_bytes: 8 * 1024 * 1024 * 1024,
    max_ciphertext_bytes: 8 * 1024 * 1024 * 1024 + 1_048_576,
    max_revisions: 4_000_000,
    max_operations: 8_000_000,
    max_nonce_bytes: 64,
};

/// Finite retained-revision limits.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct RevisionStoreLimits {
    /// Maximum exact plaintext bytes represented by one retained revision.
    pub max_plaintext_bytes: u64,
    /// Maximum encrypted object bytes.
    pub max_ciphertext_bytes: u64,
    /// Maximum retained immutable revision records.
    pub max_revisions: usize,
    /// Maximum retained operation identities.
    pub max_operations: usize,
    /// Maximum authenticated-encryption nonce bytes.
    pub max_nonce_bytes: usize,
}

impl RevisionStoreLimits {
    /// Validates all finite dimensions and ciphertext capacity.
    pub const fn validate(self) -> Result<Self, RevisionStoreError> {
        if self.max_plaintext_bytes == 0
            || self.max_ciphertext_bytes == 0
            || self.max_ciphertext_bytes < self.max_plaintext_bytes
            || self.max_revisions == 0
            || self.max_operations == 0
            || self.max_nonce_bytes == 0
        {
            Err(RevisionStoreError::InvalidLimits)
        } else {
            Ok(self)
        }
    }
}

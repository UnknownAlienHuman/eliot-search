//! Envelope authority, canonical ingest, and explicit legacy migration bindings.

use search_contracts::{NonZeroRevision, OpaqueId, ReceiptRef};

use super::error::RevisionStoreError;
use super::limits::ENVELOPE_BINDING_VERSION;

/// Exact envelope binding mirror for the authenticated-encryption profile.
///
/// This carries the same authority and integrity digests the
/// `search-revision-crypto` envelope authenticates (source-revision binding,
/// residency binding, encryption profile, SHA-256 content digest, plaintext
/// length, key generation) without importing that crate or performing any
/// cryptographic operation. The store compares these bindings for exact
/// equality on every reuse and readback; it never converts between digest
/// algorithms, so a BLAKE3 payload digest can never be substituted for the
/// envelope SHA-256 digest or vice versa.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct EnvelopeBinding {
    /// Envelope version; must equal [`ENVELOPE_BINDING_VERSION`].
    pub version: u16,
    /// Monotone data-encryption-key generation; must equal the payload
    /// encryption binding key version.
    pub key_generation: NonZeroRevision,
    /// Digest of stable source plus immutable revision identity.
    pub source_revision_binding_digest: [u8; 32],
    /// Digest of the complete object-residency key.
    pub residency_binding_digest: [u8; 32],
    /// Digest of encryption/materialization profile identity.
    pub encryption_profile_digest: [u8; 32],
    /// SHA-256 digest of exact plaintext bytes (never a BLAKE3 digest).
    pub content_digest_sha256: [u8; 32],
    /// Exact plaintext byte length; must equal the payload plaintext bytes.
    pub plaintext_length: u64,
}

/// Exact canonical T13 ingest binding for one retained revision.
///
/// Every field names the canonical composition receipt digest produced before
/// any CAS write: the admission receipt digest, the admission policy
/// fingerprint and revision, the observation digest, and the durable
/// canonical source/revision identifiers. All digest fields are lowercase
/// hexadecimal SHA-256 as emitted by the canonical composition layer; the
/// store validates that shape and binds the values into reuse identity, but
/// it never fabricates composition receipts.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct CanonicalIngestBinding {
    /// Canonical admission receipt digest authorizing retention.
    pub admission_receipt: ReceiptRef,
    /// Canonical admission policy fingerprint.
    pub policy_fingerprint: OpaqueId,
    /// Canonical admission policy revision.
    pub policy_revision: NonZeroRevision,
    /// Canonical admission observation digest.
    pub observation_digest: OpaqueId,
    /// Durable canonical source identifier.
    pub canonical_source_id: OpaqueId,
    /// Canonical revision identifier binding source, content, and size.
    pub canonical_revision_id: OpaqueId,
}

impl CanonicalIngestBinding {
    /// Creates one ingest binding after validating every digest shape.
    pub fn new(
        admission_receipt: ReceiptRef,
        policy_fingerprint: OpaqueId,
        policy_revision: NonZeroRevision,
        observation_digest: OpaqueId,
        canonical_source_id: OpaqueId,
        canonical_revision_id: OpaqueId,
    ) -> Result<Self, RevisionStoreError> {
        for digest in [
            &policy_fingerprint,
            &observation_digest,
            &canonical_source_id,
            &canonical_revision_id,
        ] {
            validate_t13_hex(digest)?;
        }
        Ok(Self {
            admission_receipt,
            policy_fingerprint,
            policy_revision,
            observation_digest,
            canonical_source_id,
            canonical_revision_id,
        })
    }
}

/// Explicit legacy residency migration witness.
///
/// Old opaque `residency_key` values predate typed closures. They are
/// accepted only beside this witness, which names the preserved legacy key
/// and the migration authority receipt. Migrated bytes keep their existing
/// envelope ciphertext (no re-encryption, no new cipher); all new
/// comparisons use the typed closure from the intent key.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct LegacyMigration {
    /// Preserved opaque legacy residency key, equal to the intent witness.
    pub legacy_residency_key: OpaqueId,
    /// Explicit migration authority receipt.
    pub migration_receipt: ReceiptRef,
}

/// Binds one preserved legacy residency key to its migration authority.
pub fn migrate_legacy_residency_key(
    legacy_residency_key: OpaqueId,
    migration_receipt: ReceiptRef,
) -> LegacyMigration {
    LegacyMigration {
        legacy_residency_key,
        migration_receipt,
    }
}

/// Validates one canonical T13 hexadecimal digest shape.
///
/// Canonical composition emits lowercase hexadecimal SHA-256; uppercase,
/// truncated, or non-hex values fail closed instead of binding the wrong
/// receipt.
fn validate_t13_hex(value: &OpaqueId) -> Result<(), RevisionStoreError> {
    let text = value.as_str();
    if text.len() != 64
        || !text
            .bytes()
            .all(|byte| byte.is_ascii_hexdigit() && !byte.is_ascii_uppercase())
    {
        return Err(RevisionStoreError::IngestBindingInvalid);
    }
    Ok(())
}

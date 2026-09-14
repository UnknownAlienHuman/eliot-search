//! Immutable revision, encrypted payload, receipt, and lifecycle data model.

use core::fmt;

use search_contracts::{
    Blake3Digest32, NonZeroRevision, OpaqueId, ReceiptRef, ScopeDomainId,
};

use super::binding::{CanonicalIngestBinding, EnvelopeBinding, LegacyMigration};
use super::error::RevisionStoreError;
use super::limits::{MIN_CIPHERTEXT_BYTES, RevisionStoreLimits};
use super::residency::ResidencyClosure;

/// Immutable domain-qualified source-revision key.
///
/// The `(source_id, revision)` pair remains the occurrence identity with
/// per-source monotone sequencing; the embedded typed [`ResidencyClosure`]
/// qualifies storage identity. One occurrence binds exactly one closure: the
/// same pair with another closure is a typed [`RevisionStoreError::ResidencyMismatch`],
/// never a silent second slot. `source_id` stays opaque so T13 legacy-domain
/// mappings survive migration; paths are locators and never identity.
#[derive(Clone, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub struct RevisionKey {
    /// Stable source identity (canonical or preserved legacy domain).
    pub source_id: OpaqueId,
    /// Monotone retained revision for that source.
    pub revision: NonZeroRevision,
    /// Complete typed residency closure qualifying this occurrence.
    pub residency: ResidencyClosure,
}

/// Closed encrypted-object suite identifier.
#[derive(Clone, Copy, Debug, Eq, Ord, PartialEq, PartialOrd)]
pub enum CipherSuite {
    /// Version-one authenticated-encryption adapter profile.
    AuthenticatedEncryptionV1,
}

/// Exact encryption-key binding without key material.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct EncryptionBinding {
    /// Opaque secret reference resolved by the secret-owning adapter.
    pub key_reference: OpaqueId,
    /// Monotone key version.
    pub key_version: NonZeroRevision,
    /// Closed cipher-suite profile.
    pub cipher_suite: CipherSuite,
}

/// Finite already-encrypted retained-revision payload.
#[derive(Clone, Eq, PartialEq)]
pub struct EncryptedRevisionPayload {
    /// Exact plaintext content digest.
    pub plaintext_digest: Blake3Digest32,
    /// Exact plaintext byte count.
    pub plaintext_bytes: u64,
    /// Exact ciphertext digest.
    pub ciphertext_digest: Blake3Digest32,
    /// Exact authenticated-encryption nonce.
    pub(super) nonce: Vec<u8>,
    /// Exact encrypted object bytes including authentication tag.
    pub(super) ciphertext: Vec<u8>,
    /// Exact encryption-key binding.
    pub encryption: EncryptionBinding,
}

impl EncryptedRevisionPayload {
    /// Creates a finite non-empty encrypted payload.
    pub fn new(
        plaintext_digest: Blake3Digest32,
        plaintext_bytes: u64,
        ciphertext_digest: Blake3Digest32,
        nonce: Vec<u8>,
        ciphertext: Vec<u8>,
        encryption: EncryptionBinding,
        limits: RevisionStoreLimits,
    ) -> Result<Self, RevisionStoreError> {
        let limits = limits.validate()?;
        if plaintext_bytes == 0 || plaintext_bytes > limits.max_plaintext_bytes {
            return Err(RevisionStoreError::PlaintextSizeInvalid);
        }
        let ciphertext_len = u64::try_from(ciphertext.len())
            .map_err(|_| RevisionStoreError::CiphertextSizeInvalid)?;
        if ciphertext.is_empty()
            || ciphertext_len > limits.max_ciphertext_bytes
            || ciphertext.len() < MIN_CIPHERTEXT_BYTES
        {
            return Err(RevisionStoreError::CiphertextSizeInvalid);
        }
        if nonce.is_empty() || nonce.len() > limits.max_nonce_bytes {
            return Err(RevisionStoreError::NonceInvalid);
        }
        Ok(Self {
            plaintext_digest,
            plaintext_bytes,
            ciphertext_digest,
            nonce,
            ciphertext,
            encryption,
        })
    }

    /// Exact nonce bytes for the encryption/storage adapter.
    pub fn nonce(&self) -> &[u8] {
        &self.nonce
    }

    /// Exact encrypted object bytes.
    pub fn ciphertext(&self) -> &[u8] {
        &self.ciphertext
    }

    /// Exact ciphertext byte length.
    pub fn ciphertext_len(&self) -> usize {
        self.ciphertext.len()
    }
}

impl fmt::Debug for EncryptedRevisionPayload {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("EncryptedRevisionPayload")
            .field("plaintext_digest", &self.plaintext_digest)
            .field("plaintext_bytes", &self.plaintext_bytes)
            .field("ciphertext_digest", &self.ciphertext_digest)
            .field("nonce", &format_args!("<{} bytes>", self.nonce.len()))
            .field(
                "ciphertext",
                &format_args!("<{} encrypted bytes>", self.ciphertext.len()),
            )
            .field("encryption", &self.encryption)
            .finish()
    }
}

/// Full-payload immutable revision-store operation.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct RevisionOperation {
    operation_id: OpaqueId,
    request_digest: Blake3Digest32,
}

impl RevisionOperation {
    /// Creates a replay-fenced operation.
    #[must_use]
    pub const fn new(operation_id: OpaqueId, request_digest: Blake3Digest32) -> Self {
        Self {
            operation_id,
            request_digest,
        }
    }

    /// Immutable operation identifier.
    pub const fn operation_id(&self) -> &OpaqueId {
        &self.operation_id
    }

    /// Digest of exact canonical operation payload.
    pub const fn request_digest(&self) -> Blake3Digest32 {
        self.request_digest
    }
}

/// Exact append intent supplied to the object-store adapter.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct RevisionWriteIntent {
    /// Immutable domain-qualified source/revision key.
    pub key: RevisionKey,
    /// Exact source-binding revision observed by the safe-reader pipeline.
    pub source_binding_revision: NonZeroRevision,
    /// Exact encrypted payload.
    pub payload: EncryptedRevisionPayload,
    /// Exact envelope authority binding (no cryptography performed here).
    pub envelope: EnvelopeBinding,
    /// Exact canonical T13 ingest binding.
    pub ingest: CanonicalIngestBinding,
    /// Opaque content-addressed storage object identity.
    pub storage_object_id: OpaqueId,
    /// Residency witness: the derived [`ResidencyClosure::scope_id`] for fresh
    /// writes, or the preserved legacy key beside [`Self::legacy_migration`].
    pub residency_key: OpaqueId,
    /// Explicit legacy migration witness, if any.
    pub legacy_migration: Option<LegacyMigration>,
    /// Authorization receipt for retaining this source revision.
    pub authorization_receipt: Option<ReceiptRef>,
    /// Full-payload immutable operation.
    pub operation: RevisionOperation,
}

/// Exact durable object readback after a possible write.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct RevisionObjectReadback {
    /// Immutable domain-qualified source/revision key.
    pub key: RevisionKey,
    /// Opaque storage object identity.
    pub storage_object_id: OpaqueId,
    /// Exact ciphertext digest.
    pub ciphertext_digest: Blake3Digest32,
    /// Exact ciphertext byte count.
    pub ciphertext_bytes: u64,
    /// Exact plaintext content digest stored in authenticated metadata.
    pub plaintext_digest: Blake3Digest32,
    /// Exact plaintext byte count stored in authenticated metadata.
    pub plaintext_bytes: u64,
    /// Exact encryption-key binding.
    pub encryption: EncryptionBinding,
    /// Exact envelope authority binding read from the stored object.
    pub envelope: EnvelopeBinding,
    /// Whether authoritative durable readback completed.
    pub readback_verified: bool,
    /// Content-free durable object receipt.
    pub object_receipt: Option<ReceiptRef>,
}

/// Durable immutable retained-revision record.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct RevisionRecord {
    /// Immutable domain-qualified source/revision key.
    pub key: RevisionKey,
    /// Source-binding revision used for the read.
    pub source_binding_revision: NonZeroRevision,
    /// Exact plaintext content digest.
    pub content_digest: Blake3Digest32,
    /// Exact plaintext byte count.
    pub plaintext_bytes: u64,
    /// Exact ciphertext digest.
    pub ciphertext_digest: Blake3Digest32,
    /// Exact ciphertext byte count.
    pub ciphertext_bytes: u64,
    /// Opaque storage object identity.
    pub storage_object_id: OpaqueId,
    /// Residency witness presented at admission.
    pub residency_key: OpaqueId,
    /// Exact encryption-key binding.
    pub encryption: EncryptionBinding,
    /// Exact envelope authority binding.
    pub envelope: EnvelopeBinding,
    /// Exact canonical T13 ingest binding.
    pub ingest: CanonicalIngestBinding,
    /// Authorization receipt.
    pub authorization_receipt: ReceiptRef,
    /// Durable object readback receipt.
    pub object_receipt: ReceiptRef,
    /// Full-payload immutable operation.
    pub operation: RevisionOperation,
}

/// Pending or terminal state of one revision key.
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum RevisionState {
    /// Durable control intent exists; object write is not confirmed.
    Pending(RevisionWriteIntent),
    /// Possible object write has unknown authoritative outcome.
    OutcomeUnknown(RevisionWriteIntent),
    /// Exact durable object readback confirmed immutable storage.
    Active(RevisionRecord),
    /// Contradictory object state requires quarantine.
    Quarantined {
        /// Immutable revision key.
        key: RevisionKey,
        /// Full-payload operation that encountered contradiction.
        operation: RevisionOperation,
    },
}

/// Content-free append receipt.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct RevisionStoreReceipt {
    /// Immutable domain-qualified revision key.
    pub key: RevisionKey,
    /// Residency closure bound by this receipt.
    pub residency: ResidencyClosure,
    /// Full-payload operation.
    pub operation: RevisionOperation,
    /// Exact plaintext content digest.
    pub content_digest: Blake3Digest32,
    /// Exact ciphertext digest.
    pub ciphertext_digest: Blake3Digest32,
    /// Durable object receipt.
    pub object_receipt: ReceiptRef,
    /// Whether the receipt was replayed from an existing active revision.
    pub replayed: bool,
}

/// Result of preparing one append.
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum PrepareAppendResult {
    /// New exact intent was installed and must be written by the backend.
    Prepared(RevisionWriteIntent),
    /// Exact immutable revision already exists.
    AlreadyStored(RevisionStoreReceipt),
}

/// Result of exact unknown-outcome recovery.
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum RecoveryResult {
    /// Exact write was durably present and activated.
    Applied(RevisionStoreReceipt),
    /// Exact write is absent and the pending state was removed for retry.
    NotApplied,
    /// Readback contradicted the prepared intent and the key was quarantined.
    Quarantined,
}

/// Scope fenced by a purge tombstone.
///
/// Tombstones are enforcement state, not decisions: the lifecycle owner
/// decides purges, and this kernel blocks every fenced write, import, and
/// re-admission at its boundary. There is no public tombstone removal.
#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub enum TombstoneScope {
    /// Exactly one residency closure is fenced.
    Residency(ResidencyClosure),
    /// Every residency under one scope domain is fenced.
    Scope(ScopeDomainId),
}

/// Durable purge tombstone installed at the admission boundary.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct PurgeTombstone {
    /// Fenced scope.
    pub scope: TombstoneScope,
    /// Tombstone generation.
    pub generation: NonZeroRevision,
    /// Non-content tombstone authority receipt.
    pub tombstone_receipt: ReceiptRef,
    /// Full-payload immutable install operation.
    pub operation: RevisionOperation,
}

/// Content-free tombstone install receipt.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct PurgeTombstoneReceipt {
    /// Fenced scope.
    pub scope: TombstoneScope,
    /// Tombstone generation.
    pub generation: NonZeroRevision,
    /// Non-content tombstone authority receipt.
    pub tombstone_receipt: ReceiptRef,
    /// Full-payload immutable install operation.
    pub operation: RevisionOperation,
    /// Whether the receipt was replayed from an existing tombstone.
    pub replayed: bool,
}

/// Lifecycle authority kind behind an exact deletion plan.
///
/// The receipt always names its authority: ordinary retired-object sweep and
/// security/legal purge remain distinct and neither claims backup deletion
/// or physical secure erase.
#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub enum DeletionAuthorityKind {
    /// Ordinary retired-object sweep.
    OrdinarySweep,
    /// Security or legal purge.
    SecurityPurge,
}

/// Exact bounded deletion plan issued by the lifecycle owner.
///
/// The store executes deletions only against this exact plan: one domain-
/// qualified revision key plus one storage object identity. Broad directory,
/// prefix, or digest-only deletion is unrepresentable. Retention and purge
/// decisions stay with their owners; this plan is enforcement input.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct LifecycleDeletionPlan {
    /// Accepted sweep or purge plan receipt from the lifecycle owner.
    pub plan_receipt: ReceiptRef,
    /// Authority kind behind the plan.
    pub authority: DeletionAuthorityKind,
    /// Exact domain-qualified revision to delete.
    pub target: RevisionKey,
    /// Exact storage object identity to delete.
    pub target_storage_object_id: OpaqueId,
    /// Full-payload immutable deletion operation.
    pub operation: RevisionOperation,
}

/// Content-free exact deletion receipt.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ObjectDeletionReceipt {
    /// Deleted domain-qualified revision.
    pub target: RevisionKey,
    /// Exact storage object identity that was deleted.
    pub target_storage_object_id: OpaqueId,
    /// Authority kind that authorized the deletion.
    pub authority: DeletionAuthorityKind,
    /// Lifecycle plan receipt that authorized the deletion.
    pub plan_receipt: ReceiptRef,
    /// Full-payload immutable deletion operation.
    pub operation: RevisionOperation,
    /// Whether the receipt was replayed from an executed deletion.
    pub replayed: bool,
}

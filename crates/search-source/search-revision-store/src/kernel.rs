//! Immutable encrypted retained-revision storage semantics.
//!
//! This package performs no filesystem, database, encryption, or secret-store
//! I/O. Callers provide already encrypted finite payloads and exact backend
//! readback. The kernel enforces source-revision monotonicity, immutability,
//! replay fencing, unknown-outcome recovery, and content-free receipts. A
//! concrete object-store adapter must separately prove atomic write/readback and
//! encryption-at-rest behavior.
//!
//! Residency model (T14): every retained occurrence is bound to one complete
//! typed [`ResidencyClosure`] covering all six canonical domains (scope,
//! access, confidentiality, encryption key, retention, erasure). Equal
//! plaintext alone never deduplicates: physical object identities,
//! ciphertext bytes, envelope binding digests, and secret references are
//! never reused across inequivalent closures. Reuse inside one equivalent
//! closure is idempotent and verifies exact bytes. Encryption itself stays
//! with the `search-revision-crypto` envelope profile and the OS secret
//! boundary; this kernel only binds, compares, and fences the resulting
//! digests and never invents a cipher.
//!
//! Ingest model (T13 binding): every intent carries a [`CanonicalIngestBinding`]
//! with the exact canonical admission/identity/registry receipt digests. Legacy
//! opaque residency keys are accepted only through an explicit
//! [`LegacyMigration`] receipt; fresh writes must present the derived
//! [`ResidencyClosure::scope_id`].
//!
//! Retention model: this kernel enforces purge tombstones and exact
//! lifecycle-plan deletions at its boundary. Retention, purge, and restore
//! decisions stay with their owning packages; the kernel never decides them.

#![forbid(unsafe_code)]
#![deny(missing_docs)]
#![allow(
    clippy::doc_markdown,
    clippy::large_enum_variant,
    clippy::manual_let_else,
    clippy::match_same_arms,
    clippy::missing_const_for_fn,
    clippy::missing_errors_doc,
    clippy::module_name_repetitions,
    clippy::must_use_candidate,
    clippy::needless_pass_by_value,
    clippy::option_if_let_else,
    clippy::similar_names,
    clippy::struct_excessive_bools,
    clippy::too_many_arguments,
    clippy::too_many_lines,
    clippy::trivially_copy_pass_by_ref
)]

use core::fmt;
use std::collections::BTreeMap;

use search_contracts::{
    AccessDomainId, Blake3Digest32, ConfidentialityDomainId, EncryptionKeyDomainId,
    ErasureDomainId, NonZeroRevision, OpaqueId, ReceiptRef, RetentionDomainId, ScopeDomainId,
    SearchObjectResidencyKey,
};

/// Version of the typed [`ResidencyClosure`] encoding.
pub const RESIDENCY_CLOSURE_VERSION: u16 = 1;
/// Version of the [`EnvelopeBinding`] carried beside every intent.
///
/// This tracks the existing authenticated-encryption envelope profile version
/// (`search-revision-crypto` `REVISION_ENVELOPE_VERSION = 1`). The store
/// creates no cipher and accepts no other envelope version.
pub const ENVELOPE_BINDING_VERSION: u16 = 1;
/// Version of the [`CasObjectAddress`] schema.
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

/// Closed content-free revision-store failure.
#[derive(Clone, Copy, Debug, Eq, Ord, PartialEq, PartialOrd)]
pub enum RevisionStoreError {
    /// Store limits are zero or internally inconsistent.
    InvalidLimits,
    /// Plaintext byte count is zero or exceeds its finite ceiling.
    PlaintextSizeInvalid,
    /// Ciphertext is empty or exceeds its finite ceiling.
    CiphertextSizeInvalid,
    /// Nonce is empty or exceeds its finite ceiling.
    NonceInvalid,
    /// Source revision is absent or cannot advance exactly once.
    RevisionSequenceInvalid,
    /// Exact source/revision key already stores different immutable metadata.
    RevisionConflict,
    /// Exact revision is absent.
    RevisionNotFound,
    /// Operation identity was reused with another complete request digest.
    OperationConflict,
    /// Finite revision or operation capacity was exhausted.
    CapacityExceeded,
    /// Write confirmation does not match the prepared exact intent.
    ReadbackMismatch,
    /// Required authorization or durable readback evidence is absent.
    EvidenceMissing,
    /// Possible external write has unknown authoritative outcome.
    OutcomeUnknown,
    /// Contradictory object state requires quarantine.
    Quarantined,
    /// Backend failed before a verified outcome existed.
    BackendFailure,
    /// Backend returned malformed or contradictory data.
    BackendContractViolation,
    /// Shared revision cannot advance.
    ContractExhausted,
    /// Residency closures differ where exact equivalence was required, or
    /// physical bytes/keys were reused across inequivalent closures.
    ResidencyMismatch,
    /// Object address or residency scope identity is malformed or versioned
    /// incorrectly.
    AddressInvalid,
    /// Envelope binding version, length, or key-generation linkage is invalid.
    EnvelopeInvalid,
    /// Canonical T13 ingest binding carries a malformed receipt digest.
    IngestBindingInvalid,
    /// A purge tombstone prohibits this residency scope from entering state.
    Tombstoned,
    /// Deletion plan does not name this exact record and storage object.
    DeletionNotAuthorized,
}

impl RevisionStoreError {
    /// Stable machine-readable reason code.
    pub const fn code(self) -> &'static str {
        match self {
            Self::InvalidLimits => "REVISION_STORE_INVALID_LIMITS",
            Self::PlaintextSizeInvalid => "REVISION_STORE_PLAINTEXT_SIZE_INVALID",
            Self::CiphertextSizeInvalid => "REVISION_STORE_CIPHERTEXT_SIZE_INVALID",
            Self::NonceInvalid => "REVISION_STORE_NONCE_INVALID",
            Self::RevisionSequenceInvalid => "REVISION_STORE_REVISION_SEQUENCE_INVALID",
            Self::RevisionConflict => "REVISION_STORE_REVISION_CONFLICT",
            Self::RevisionNotFound => "REVISION_STORE_REVISION_NOT_FOUND",
            Self::OperationConflict => "REVISION_STORE_OPERATION_CONFLICT",
            Self::CapacityExceeded => "REVISION_STORE_CAPACITY_EXCEEDED",
            Self::ReadbackMismatch => "REVISION_STORE_READBACK_MISMATCH",
            Self::EvidenceMissing => "REVISION_STORE_EVIDENCE_MISSING",
            Self::OutcomeUnknown => "REVISION_STORE_OUTCOME_UNKNOWN",
            Self::Quarantined => "REVISION_STORE_QUARANTINED",
            Self::BackendFailure => "REVISION_STORE_BACKEND_FAILURE",
            Self::BackendContractViolation => "REVISION_STORE_BACKEND_CONTRACT_VIOLATION",
            Self::ContractExhausted => "REVISION_STORE_CONTRACT_EXHAUSTED",
            Self::ResidencyMismatch => "REVISION_STORE_RESIDENCY_DOMAIN_MISMATCH",
            Self::AddressInvalid => "REVISION_STORE_OBJECT_ADDRESS_INVALID",
            Self::EnvelopeInvalid => "REVISION_STORE_ENVELOPE_INVALID",
            Self::IngestBindingInvalid => "REVISION_STORE_INGEST_BINDING_INVALID",
            Self::Tombstoned => "REVISION_STORE_PURGE_TOMBSTONE_CONFLICT",
            Self::DeletionNotAuthorized => "REVISION_STORE_OBJECT_DELETE_NOT_AUTHORIZED",
        }
    }
}

impl fmt::Display for RevisionStoreError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(self.code())
    }
}

impl std::error::Error for RevisionStoreError {}

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

/// Complete typed residency closure for one retained revision.
///
/// All six canonical domains are strongly typed. Two closures are equivalent
/// only when every domain is equal; equal content digests alone never imply
/// equivalence. The per-object content digest stays outside this closure (see
/// [`SearchObjectResidencyKey`): it identifies bytes, not the policy domains
/// that authorize their storage.
#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub struct ResidencyClosure {
    /// Scope domain owning the source namespace.
    pub scope: ScopeDomainId,
    /// Access domain authorizing reads.
    pub access: AccessDomainId,
    /// Confidentiality domain classifying the bytes.
    pub confidentiality: ConfidentialityDomainId,
    /// Encryption-key domain that wrapped the bytes.
    pub encryption_key: EncryptionKeyDomainId,
    /// Retention domain governing lifetime.
    pub retention: RetentionDomainId,
    /// Erasure domain governing purge.
    pub erasure: ErasureDomainId,
}

impl ResidencyClosure {
    /// Creates one complete residency closure from all six typed domains.
    pub const fn new(
        scope: ScopeDomainId,
        access: AccessDomainId,
        confidentiality: ConfidentialityDomainId,
        encryption_key: EncryptionKeyDomainId,
        retention: RetentionDomainId,
        erasure: ErasureDomainId,
    ) -> Self {
        Self {
            scope,
            access,
            confidentiality,
            encryption_key,
            retention,
            erasure,
        }
    }

    /// Projects the canonical object residency key onto its policy domains.
    ///
    /// The per-object versioned content digest is intentionally dropped: it
    /// identifies bytes, while the closure identifies the domains that must
    /// match before bytes may be physically shared.
    pub const fn from_search_object_key(key: &SearchObjectResidencyKey) -> Self {
        Self {
            scope: key.scope_domain_id,
            access: key.access_domain_id,
            confidentiality: key.confidentiality_domain_id,
            encryption_key: key.encryption_key_domain_id,
            retention: key.retention_domain_id,
            erasure: key.erasure_domain_id,
        }
    }

    /// Derives the deterministic opaque residency scope identity.
    ///
    /// The encoding is versioned (`rs1`) and position-binds every domain as
    /// compact domain UUID hex, so two inequivalent closures never share an
    /// identity. Fresh intents must present exactly this value as their
    /// `residency_key` witness; anything else fails closed unless it travels
    /// through an explicit [`LegacyMigration`].
    pub fn scope_id(&self) -> Result<OpaqueId, RevisionStoreError> {
        let mut text = String::with_capacity(201);
        text.push_str("rs1.");
        push_compact_hex(&mut text, self.scope.as_bytes());
        text.push('.');
        push_compact_hex(&mut text, self.access.as_bytes());
        text.push('.');
        push_compact_hex(&mut text, self.confidentiality.as_bytes());
        text.push('.');
        push_compact_hex(&mut text, self.encryption_key.as_bytes());
        text.push('.');
        push_compact_hex(&mut text, self.retention.as_bytes());
        text.push('.');
        push_compact_hex(&mut text, self.erasure.as_bytes());
        OpaqueId::new(text).map_err(|_| RevisionStoreError::AddressInvalid)
    }
}

/// Closed revision object kind carried by a [`CasObjectAddress`].
#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub enum RevisionObjectKind {
    /// Version-one authenticated-encryption revision envelope object.
    RevisionEnvelopeV1,
}

impl RevisionObjectKind {
    /// Stable path segment for this object kind.
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::RevisionEnvelopeV1 => "revision-envelope-v1",
        }
    }
}

/// Domain-separated content address for one immutable revision object.
///
/// The address binds the complete [`ResidencyClosure`], the object kind, the
/// content digest, and the schema version. It carries no source path or
/// display name and cannot collide across inequivalent residency closures.
#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub struct CasObjectAddress {
    /// Residency closure authorizing storage of this object.
    pub residency: ResidencyClosure,
    /// Closed object kind.
    pub kind: RevisionObjectKind,
    /// Exact content digest of the addressed bytes.
    pub content_digest: Blake3Digest32,
    /// Address schema version; must equal [`CAS_ADDRESS_VERSION`].
    pub schema_version: u16,
}

impl CasObjectAddress {
    /// Renders the deterministic slash-separated object path for adapters.
    ///
    /// Every domain appears as its own hyphenated-UUID segment, so filesystem
    /// or object-store layouts derived from this path inherit residency
    /// separation without further checks.
    pub fn to_path_string(&self) -> String {
        format!(
            "cas/v{}/{}/scope-{}/access-{}/confidentiality-{}/encryption-key-{}/retention-{}/erasure-{}/blake3-{}",
            self.schema_version,
            self.kind.as_str(),
            self.residency.scope,
            self.residency.access,
            self.residency.confidentiality,
            self.residency.encryption_key,
            self.residency.retention,
            self.residency.erasure,
            self.content_digest,
        )
    }
}

/// Derives the domain-separated address for one revision object.
///
/// Fails closed on an unsupported schema version or on the all-zero digest:
/// neither may address a real object.
pub fn derive_object_address(
    residency: &ResidencyClosure,
    kind: RevisionObjectKind,
    content_digest: Blake3Digest32,
    schema_version: u16,
) -> Result<CasObjectAddress, RevisionStoreError> {
    if schema_version != CAS_ADDRESS_VERSION {
        return Err(RevisionStoreError::AddressInvalid);
    }
    if content_digest.as_bytes() == &[0; 32] {
        return Err(RevisionStoreError::AddressInvalid);
    }
    Ok(CasObjectAddress {
        residency: *residency,
        kind,
        content_digest,
        schema_version,
    })
}

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
    nonce: Vec<u8>,
    /// Exact encrypted object bytes including authentication tag.
    ciphertext: Vec<u8>,
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
    /// writes, or the preserved legacy key beside [`legacy_migration`].
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

/// Finite immutable revision-store state machine.
#[derive(Clone, Debug)]
pub struct RevisionStore {
    limits: RevisionStoreLimits,
    states: BTreeMap<RevisionKey, RevisionState>,
    operations: Vec<(OpaqueId, Blake3Digest32, RevisionStoreReceipt)>,
    deletions: Vec<(OpaqueId, Blake3Digest32, ObjectDeletionReceipt)>,
    tombstones: Vec<PurgeTombstone>,
}

impl RevisionStore {
    /// Creates an empty finite store kernel.
    pub fn new(limits: RevisionStoreLimits) -> Result<Self, RevisionStoreError> {
        Ok(Self {
            limits: limits.validate()?,
            states: BTreeMap::new(),
            operations: Vec::new(),
            deletions: Vec::new(),
            tombstones: Vec::new(),
        })
    }

    /// Returns the exact state of one domain-qualified source/revision key.
    pub fn state(&self, key: &RevisionKey) -> Result<&RevisionState, RevisionStoreError> {
        self.states
            .get(key)
            .ok_or(RevisionStoreError::RevisionNotFound)
    }

    /// Returns one exact active immutable record.
    pub fn active_record(&self, key: &RevisionKey) -> Result<&RevisionRecord, RevisionStoreError> {
        match self.state(key)? {
            RevisionState::Active(record) => Ok(record),
            RevisionState::Pending(_) | RevisionState::OutcomeUnknown(_) => {
                Err(RevisionStoreError::OutcomeUnknown)
            }
            RevisionState::Quarantined { .. } => Err(RevisionStoreError::Quarantined),
        }
    }

    /// Number of retained revision states.
    pub fn len(&self) -> usize {
        self.states.len()
    }

    /// Returns whether no revision state is retained.
    pub fn is_empty(&self) -> bool {
        self.states.is_empty()
    }

    /// Counts every retained operation identity across appends, deletions,
    /// and tombstone installs against the finite operation ceiling.
    fn operation_count(&self) -> usize {
        self.operations
            .len()
            .saturating_add(self.deletions.len())
            .saturating_add(self.tombstones.len())
    }

    /// Returns whether a purge tombstone fences this domain-qualified key.
    fn is_fenced(&self, key: &RevisionKey) -> bool {
        self.tombstones
            .iter()
            .any(|tombstone| match tombstone.scope {
                TombstoneScope::Residency(residency) => residency == key.residency,
                TombstoneScope::Scope(scope) => scope == key.residency.scope,
            })
    }

    /// Prepares one append-only revision intent or replays an exact active receipt.
    ///
    /// Reuse across inequivalent residency closures is a typed
    /// [`RevisionStoreError::ResidencyMismatch`]; reuse inside one equivalent
    /// closure replays only after verifying exact bytes. A bound operation
    /// whose state was deleted falls through to honest re-admission with
    /// fresh readback instead of replaying a stale receipt.
    pub fn prepare_append(
        &mut self,
        intent: RevisionWriteIntent,
    ) -> Result<PrepareAppendResult, RevisionStoreError> {
        validate_intent(&intent, self.limits)?;
        if let Some((_, digest, receipt)) = self
            .operations
            .iter()
            .find(|(operation_id, _, _)| operation_id == intent.operation.operation_id())
        {
            let bound = *digest == intent.operation.request_digest()
                && receipt.key == intent.key
                && receipt.content_digest == intent.payload.plaintext_digest
                && receipt.ciphertext_digest == intent.payload.ciphertext_digest;
            if !bound {
                return Err(RevisionStoreError::OperationConflict);
            }
            if let Some(RevisionState::Active(record)) = self.states.get(&intent.key)
                && record.operation == intent.operation
                && exact_record_matches_intent(record, &intent)
            {
                let mut replay = receipt.clone();
                replay.replayed = true;
                return Ok(PrepareAppendResult::AlreadyStored(replay));
            }
        }
        if self
            .deletions
            .iter()
            .any(|(operation_id, _, _)| operation_id == intent.operation.operation_id())
            || self.tombstones.iter().any(|tombstone| {
                tombstone.operation.operation_id() == intent.operation.operation_id()
            })
        {
            return Err(RevisionStoreError::OperationConflict);
        }
        if self.is_fenced(&intent.key) {
            return Err(RevisionStoreError::Tombstoned);
        }
        if let Some(bound) =
            occurrence_residency(&self.states, &intent.key.source_id, intent.key.revision)
            && bound != intent.key.residency
        {
            return Err(RevisionStoreError::ResidencyMismatch);
        }
        if self.operation_count() >= self.limits.max_operations
            || self.states.len() >= self.limits.max_revisions
        {
            return Err(RevisionStoreError::CapacityExceeded);
        }
        if let Some(existing) = self.states.get(&intent.key) {
            return match existing {
                RevisionState::Active(record) if exact_record_matches_intent(record, &intent) => {
                    Ok(PrepareAppendResult::AlreadyStored(receipt_from_record(
                        record, true,
                    )))
                }
                RevisionState::Pending(existing) | RevisionState::OutcomeUnknown(existing) => {
                    if existing == &intent {
                        Ok(PrepareAppendResult::Prepared(existing.clone()))
                    } else if existing.operation == intent.operation {
                        Err(RevisionStoreError::OperationConflict)
                    } else {
                        Err(RevisionStoreError::RevisionConflict)
                    }
                }
                RevisionState::Active(_) | RevisionState::Quarantined { .. } => {
                    Err(RevisionStoreError::RevisionConflict)
                }
            };
        }
        if let Some(conflict) = reuse_conflict(&self.states, &intent) {
            return Err(conflict);
        }
        validate_next_source_revision(&self.states, &intent.key)?;
        self.states
            .insert(intent.key.clone(), RevisionState::Pending(intent.clone()));
        Ok(PrepareAppendResult::Prepared(intent))
    }

    /// Marks a possible external object write as unresolved.
    pub fn mark_outcome_unknown(
        &mut self,
        key: &RevisionKey,
        operation: &RevisionOperation,
    ) -> Result<(), RevisionStoreError> {
        let state = self
            .states
            .get_mut(key)
            .ok_or(RevisionStoreError::RevisionNotFound)?;
        match state {
            RevisionState::Pending(intent) if &intent.operation == operation => {
                *state = RevisionState::OutcomeUnknown(intent.clone());
                Ok(())
            }
            RevisionState::Pending(_) | RevisionState::OutcomeUnknown(_) => {
                Err(RevisionStoreError::OperationConflict)
            }
            RevisionState::Active(_) => Err(RevisionStoreError::RevisionConflict),
            RevisionState::Quarantined { .. } => Err(RevisionStoreError::Quarantined),
        }
    }

    /// Confirms an exact prepared write after authoritative durable readback.
    pub fn confirm_append(
        &mut self,
        key: &RevisionKey,
        operation: &RevisionOperation,
        readback: RevisionObjectReadback,
    ) -> Result<RevisionStoreReceipt, RevisionStoreError> {
        let intent = match self.state(key)? {
            RevisionState::Pending(intent) | RevisionState::OutcomeUnknown(intent)
                if &intent.operation == operation => intent.clone(),
            RevisionState::Pending(_) | RevisionState::OutcomeUnknown(_) => {
                return Err(RevisionStoreError::OperationConflict);
            }
            RevisionState::Active(record) if &record.operation == operation => {
                return Ok(receipt_from_record(record, true));
            }
            RevisionState::Active(_) => return Err(RevisionStoreError::RevisionConflict),
            RevisionState::Quarantined { .. } => return Err(RevisionStoreError::Quarantined),
        };
        if self.is_fenced(key) {
            return Err(RevisionStoreError::Tombstoned);
        }
        let record = record_from_readback(&intent, readback)?;
        let receipt = receipt_from_record(&record, false);
        self.states
            .insert(key.clone(), RevisionState::Active(record));
        push_operation(&mut self.operations, operation, receipt.clone());
        Ok(receipt)
    }

    /// Recovers a possible write by exact authoritative readback.
    pub fn recover_unknown(
        &mut self,
        key: &RevisionKey,
        operation: &RevisionOperation,
        readback: Option<RevisionObjectReadback>,
    ) -> Result<RecoveryResult, RevisionStoreError> {
        let intent = match self.state(key)? {
            RevisionState::OutcomeUnknown(intent) if &intent.operation == operation => {
                intent.clone()
            }
            RevisionState::OutcomeUnknown(_) | RevisionState::Pending(_) => {
                return Err(RevisionStoreError::OperationConflict);
            }
            RevisionState::Active(record) if &record.operation == operation => {
                return Ok(RecoveryResult::Applied(receipt_from_record(record, true)));
            }
            RevisionState::Active(_) => return Err(RevisionStoreError::RevisionConflict),
            RevisionState::Quarantined { .. } => return Err(RevisionStoreError::Quarantined),
        };
        if self.is_fenced(key) {
            return Err(RevisionStoreError::Tombstoned);
        }
        let Some(readback) = readback else {
            self.states.remove(key);
            return Ok(RecoveryResult::NotApplied);
        };
        match record_from_readback(&intent, readback) {
            Ok(record) => {
                let receipt = receipt_from_record(&record, false);
                self.states
                    .insert(key.clone(), RevisionState::Active(record));
                push_operation(&mut self.operations, operation, receipt.clone());
                Ok(RecoveryResult::Applied(receipt))
            }
            Err(
                RevisionStoreError::ReadbackMismatch
                | RevisionStoreError::EvidenceMissing
                | RevisionStoreError::BackendContractViolation,
            ) => {
                self.states.insert(
                    key.clone(),
                    RevisionState::Quarantined {
                        key: key.clone(),
                        operation: operation.clone(),
                    },
                );
                Ok(RecoveryResult::Quarantined)
            }
            Err(error) => Err(error),
        }
    }

    /// Installs a purge tombstone at the admission boundary.
    ///
    /// The same tombstone reinstalls idempotently. A conflicting receipt for
    /// the same scope and generation, or any operation-identity reuse with a
    /// different payload, fails closed. There is no removal operation.
    pub fn install_purge_tombstone(
        &mut self,
        tombstone: PurgeTombstone,
    ) -> Result<PurgeTombstoneReceipt, RevisionStoreError> {
        if self
            .operations
            .iter()
            .any(|(operation_id, _, _)| operation_id == tombstone.operation.operation_id())
            || self
                .deletions
                .iter()
                .any(|(operation_id, _, _)| operation_id == tombstone.operation.operation_id())
        {
            return Err(RevisionStoreError::OperationConflict);
        }
        if let Some(existing) = self.tombstones.iter().find(|candidate| {
            candidate.operation.operation_id() == tombstone.operation.operation_id()
        }) {
            if existing.operation.request_digest() != tombstone.operation.request_digest()
                || *existing != tombstone
            {
                return Err(RevisionStoreError::OperationConflict);
            }
            return Ok(tombstone_receipt(existing, true));
        }
        if let Some(existing) = self.tombstones.iter().find(|candidate| {
            candidate.scope == tombstone.scope && candidate.generation == tombstone.generation
        }) {
            if existing.tombstone_receipt != tombstone.tombstone_receipt {
                return Err(RevisionStoreError::RevisionConflict);
            }
            return Ok(tombstone_receipt(existing, true));
        }
        if self.operation_count() >= self.limits.max_operations {
            return Err(RevisionStoreError::CapacityExceeded);
        }
        self.tombstones.push(tombstone.clone());
        Ok(tombstone_receipt(&tombstone, false))
    }

    /// Executes one exact bounded deletion under a lifecycle-owner plan.
    ///
    /// Only an `Active` record matching the exact target key and storage
    /// object is removed, and only with the plan receipt the lifecycle owner
    /// issued. Pending or unknown writes are never reported as deleted;
    /// the same plan replays idempotently while any operation reuse with a
    /// different plan conflicts.
    pub fn apply_exact_object_deletion(
        &mut self,
        plan: LifecycleDeletionPlan,
    ) -> Result<ObjectDeletionReceipt, RevisionStoreError> {
        if self
            .operations
            .iter()
            .any(|(operation_id, _, _)| operation_id == plan.operation.operation_id())
            || self.tombstones.iter().any(|tombstone| {
                tombstone.operation.operation_id() == plan.operation.operation_id()
            })
        {
            return Err(RevisionStoreError::OperationConflict);
        }
        if let Some((_, digest, receipt)) = self
            .deletions
            .iter()
            .find(|(operation_id, _, _)| operation_id == plan.operation.operation_id())
        {
            let bound = *digest == plan.operation.request_digest()
                && receipt.target == plan.target
                && receipt.target_storage_object_id == plan.target_storage_object_id
                && receipt.authority == plan.authority
                && receipt.plan_receipt == plan.plan_receipt;
            if !bound {
                return Err(RevisionStoreError::OperationConflict);
            }
            let mut replay = receipt.clone();
            replay.replayed = true;
            return Ok(replay);
        }
        if self.operation_count() >= self.limits.max_operations {
            return Err(RevisionStoreError::CapacityExceeded);
        }
        let record = match self.states.get(&plan.target) {
            None => return Err(RevisionStoreError::RevisionNotFound),
            Some(RevisionState::Pending(_) | RevisionState::OutcomeUnknown(_)) => {
                return Err(RevisionStoreError::OutcomeUnknown);
            }
            Some(RevisionState::Quarantined { .. }) => {
                return Err(RevisionStoreError::Quarantined);
            }
            Some(RevisionState::Active(record)) => record.clone(),
        };
        if record.key != plan.target || record.storage_object_id != plan.target_storage_object_id {
            return Err(RevisionStoreError::DeletionNotAuthorized);
        }
        self.states.remove(&plan.target);
        let receipt = ObjectDeletionReceipt {
            target: plan.target.clone(),
            target_storage_object_id: plan.target_storage_object_id.clone(),
            authority: plan.authority,
            plan_receipt: plan.plan_receipt.clone(),
            operation: plan.operation.clone(),
            replayed: false,
        };
        self.deletions.push((
            plan.operation.operation_id().clone(),
            plan.operation.request_digest(),
            receipt.clone(),
        ));
        Ok(receipt)
    }
}

fn validate_intent(
    intent: &RevisionWriteIntent,
    limits: RevisionStoreLimits,
) -> Result<(), RevisionStoreError> {
    let limits = limits.validate()?;
    if intent.payload.plaintext_bytes == 0
        || intent.payload.plaintext_bytes > limits.max_plaintext_bytes
    {
        return Err(RevisionStoreError::PlaintextSizeInvalid);
    }
    let ciphertext_bytes = u64::try_from(intent.payload.ciphertext_len())
        .map_err(|_| RevisionStoreError::CiphertextSizeInvalid)?;
    if ciphertext_bytes == 0
        || ciphertext_bytes > limits.max_ciphertext_bytes
        || intent.payload.ciphertext_len() < MIN_CIPHERTEXT_BYTES
    {
        return Err(RevisionStoreError::CiphertextSizeInvalid);
    }
    if intent.payload.nonce().is_empty()
        || intent.payload.nonce().len() > limits.max_nonce_bytes
    {
        return Err(RevisionStoreError::NonceInvalid);
    }
    if intent.authorization_receipt.is_none() {
        return Err(RevisionStoreError::EvidenceMissing);
    }
    if intent.envelope.version != ENVELOPE_BINDING_VERSION {
        return Err(RevisionStoreError::EnvelopeInvalid);
    }
    if intent.envelope.plaintext_length != intent.payload.plaintext_bytes {
        return Err(RevisionStoreError::EnvelopeInvalid);
    }
    if intent.envelope.key_generation != intent.payload.encryption.key_version {
        return Err(RevisionStoreError::EnvelopeInvalid);
    }
    match &intent.legacy_migration {
        Some(migration) => {
            if intent.residency_key != migration.legacy_residency_key {
                return Err(RevisionStoreError::ResidencyMismatch);
            }
        }
        None => {
            if intent.residency_key != intent.key.residency.scope_id()? {
                return Err(RevisionStoreError::ResidencyMismatch);
            }
        }
    }
    Ok(())
}

/// Returns the residency bound to one occurrence, if any state names it.
///
/// Occurrence identity is `(source_id, revision)`; the map key additionally
/// carries the closure so this scan is the single binding check.
fn occurrence_residency(
    states: &BTreeMap<RevisionKey, RevisionState>,
    source_id: &OpaqueId,
    revision: NonZeroRevision,
) -> Option<ResidencyClosure> {
    states
        .keys()
        .find(|key| key.source_id == *source_id && key.revision == revision)
        .map(|key| key.residency)
}

/// Detects physical reuse across inequivalent residency closures.
///
/// Object identities, ciphertext digests, envelope binding digests, and
/// secret references must never cross residency boundaries. Inside one
/// equivalent closure, sharing one object identity for different bytes is an
/// immutable object conflict, never silent overwriting.
fn reuse_conflict(
    states: &BTreeMap<RevisionKey, RevisionState>,
    intent: &RevisionWriteIntent,
) -> Option<RevisionStoreError> {
    for state in states.values() {
        let (
            residency,
            storage_object_id,
            ciphertext_digest,
            key_reference,
            source_binding_digest,
            residency_binding_digest,
        ) = match state {
            RevisionState::Pending(existing) | RevisionState::OutcomeUnknown(existing) => (
                existing.key.residency,
                &existing.storage_object_id,
                existing.payload.ciphertext_digest,
                &existing.payload.encryption.key_reference,
                existing.envelope.source_revision_binding_digest,
                existing.envelope.residency_binding_digest,
            ),
            RevisionState::Active(record) => (
                record.key.residency,
                &record.storage_object_id,
                record.ciphertext_digest,
                &record.encryption.key_reference,
                record.envelope.source_revision_binding_digest,
                record.envelope.residency_binding_digest,
            ),
            RevisionState::Quarantined { .. } => continue,
        };
        if residency == intent.key.residency {
            if storage_object_id == &intent.storage_object_id
                && ciphertext_digest != intent.payload.ciphertext_digest
            {
                return Some(RevisionStoreError::RevisionConflict);
            }
            continue;
        }
        if storage_object_id == &intent.storage_object_id
            || ciphertext_digest == intent.payload.ciphertext_digest
            || key_reference == &intent.payload.encryption.key_reference
            || source_binding_digest == intent.envelope.source_revision_binding_digest
            || residency_binding_digest == intent.envelope.residency_binding_digest
        {
            return Some(RevisionStoreError::ResidencyMismatch);
        }
    }
    None
}

/// Records one append operation identity unless it is already indexed.
///
/// Re-admission of a deleted occurrence reuses its bound operation identity;
/// the index keeps the first entry so history never duplicates.
fn push_operation(
    operations: &mut Vec<(OpaqueId, Blake3Digest32, RevisionStoreReceipt)>,
    operation: &RevisionOperation,
    receipt: RevisionStoreReceipt,
) {
    if !operations
        .iter()
        .any(|(operation_id, _, _)| operation_id == operation.operation_id())
    {
        operations.push((
            operation.operation_id().clone(),
            operation.request_digest(),
            receipt,
        ));
    }
}

/// Builds a content-free tombstone install receipt.
fn tombstone_receipt(tombstone: &PurgeTombstone, replayed: bool) -> PurgeTombstoneReceipt {
    PurgeTombstoneReceipt {
        scope: tombstone.scope,
        generation: tombstone.generation,
        tombstone_receipt: tombstone.tombstone_receipt.clone(),
        operation: tombstone.operation.clone(),
        replayed,
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

/// Appends one domain UUID as compact lowercase hexadecimal.
fn push_compact_hex(text: &mut String, bytes: &[u8; 16]) {
    const HEXDIGITS: &[u8; 16] = b"0123456789abcdef";
    for byte in bytes {
        text.push(HEXDIGITS[(byte >> 4) as usize] as char);
        text.push(HEXDIGITS[(byte & 0x0F) as usize] as char);
    }
}

fn validate_next_source_revision(
    states: &BTreeMap<RevisionKey, RevisionState>,
    key: &RevisionKey,
) -> Result<(), RevisionStoreError> {
    let latest = states
        .keys()
        .filter(|existing| existing.source_id == key.source_id)
        .map(|existing| existing.revision)
        .max();
    match latest {
        None if key.revision.get() == 1 => Ok(()),
        Some(current)
            if current
                .checked_next()
                .map_err(|_| RevisionStoreError::ContractExhausted)?
                == key.revision =>
        {
            Ok(())
        }
        None | Some(_) => Err(RevisionStoreError::RevisionSequenceInvalid),
    }
}

fn exact_record_matches_intent(record: &RevisionRecord, intent: &RevisionWriteIntent) -> bool {
    record.key == intent.key
        && record.source_binding_revision == intent.source_binding_revision
        && record.content_digest == intent.payload.plaintext_digest
        && record.plaintext_bytes == intent.payload.plaintext_bytes
        && record.ciphertext_digest == intent.payload.ciphertext_digest
        && record.storage_object_id == intent.storage_object_id
        && record.residency_key == intent.residency_key
        && record.encryption == intent.payload.encryption
        && record.envelope == intent.envelope
        && record.ingest == intent.ingest
        && record.operation == intent.operation
}

fn record_from_readback(
    intent: &RevisionWriteIntent,
    readback: RevisionObjectReadback,
) -> Result<RevisionRecord, RevisionStoreError> {
    if !readback.readback_verified {
        return Err(RevisionStoreError::EvidenceMissing);
    }
    let object_receipt = readback
        .object_receipt
        .ok_or(RevisionStoreError::EvidenceMissing)?;
    let authorization_receipt = intent
        .authorization_receipt
        .clone()
        .ok_or(RevisionStoreError::EvidenceMissing)?;
    let expected_ciphertext_bytes = u64::try_from(intent.payload.ciphertext_len())
        .map_err(|_| RevisionStoreError::BackendContractViolation)?;
    if readback.key != intent.key
        || readback.storage_object_id != intent.storage_object_id
        || readback.ciphertext_digest != intent.payload.ciphertext_digest
        || readback.ciphertext_bytes != expected_ciphertext_bytes
        || readback.plaintext_digest != intent.payload.plaintext_digest
        || readback.plaintext_bytes != intent.payload.plaintext_bytes
        || readback.encryption != intent.payload.encryption
        || readback.envelope != intent.envelope
    {
        return Err(RevisionStoreError::ReadbackMismatch);
    }
    Ok(RevisionRecord {
        key: intent.key.clone(),
        source_binding_revision: intent.source_binding_revision,
        content_digest: intent.payload.plaintext_digest,
        plaintext_bytes: intent.payload.plaintext_bytes,
        ciphertext_digest: intent.payload.ciphertext_digest,
        ciphertext_bytes: expected_ciphertext_bytes,
        storage_object_id: intent.storage_object_id.clone(),
        residency_key: intent.residency_key.clone(),
        encryption: intent.payload.encryption.clone(),
        envelope: intent.envelope,
        ingest: intent.ingest.clone(),
        authorization_receipt,
        object_receipt,
        operation: intent.operation.clone(),
    })
}

fn receipt_from_record(record: &RevisionRecord, replayed: bool) -> RevisionStoreReceipt {
    RevisionStoreReceipt {
        key: record.key.clone(),
        residency: record.key.residency,
        operation: record.operation.clone(),
        content_digest: record.content_digest,
        ciphertext_digest: record.ciphertext_digest,
        object_receipt: record.object_receipt.clone(),
        replayed,
    }
}

/// Concrete encrypted-object backend contract.
pub trait RevisionObjectBackend {
    /// Concrete backend error.
    type BackendError;

    /// Attempts one atomic immutable encrypted-object write.
    fn write_immutable(
        &mut self,
        intent: &RevisionWriteIntent,
    ) -> Result<(), Self::BackendError>;

    /// Reads exact content-free object metadata after write or unknown outcome.
    fn readback(
        &mut self,
        key: &RevisionKey,
        storage_object_id: &OpaqueId,
    ) -> Result<Option<RevisionObjectReadback>, Self::BackendError>;

    /// Reads exact encrypted bytes for an active record.
    fn read_encrypted(
        &mut self,
        record: &RevisionRecord,
        max_ciphertext_bytes: u64,
    ) -> Result<EncryptedRevisionPayload, Self::BackendError>;

    /// Maps a concrete error without including source or ciphertext bytes.
    fn map_backend_error(error: &Self::BackendError) -> RevisionStoreError;
}

#[cfg(test)]
mod tests {
    use super::*;
    use search_contracts::{
        AccessDomainId, ConfidentialityDomainId, DigestAlgorithm, EncryptionKeyDomainId,
        ErasureDomainId, RetentionDomainId, ScopeDomainId, SearchObjectResidencyKey,
        VersionedContentDigest,
    };

    fn baseline_residency() -> ResidencyClosure {
        ResidencyClosure::new(
            ScopeDomainId::from_bytes([0x11; 16]),
            AccessDomainId::from_bytes([0x22; 16]),
            ConfidentialityDomainId::from_bytes([0x33; 16]),
            EncryptionKeyDomainId::from_bytes([0x44; 16]),
            RetentionDomainId::from_bytes([0x55; 16]),
            ErasureDomainId::from_bytes([0x66; 16]),
        )
    }

    fn residency_variant(which: usize) -> ResidencyClosure {
        let mut residency = baseline_residency();
        match which {
            0 => residency.scope = ScopeDomainId::from_bytes([0x99; 16]),
            1 => residency.access = AccessDomainId::from_bytes([0x99; 16]),
            2 => residency.confidentiality = ConfidentialityDomainId::from_bytes([0x99; 16]),
            3 => residency.encryption_key = EncryptionKeyDomainId::from_bytes([0x99; 16]),
            4 => residency.retention = RetentionDomainId::from_bytes([0x99; 16]),
            5 => residency.erasure = ErasureDomainId::from_bytes([0x99; 16]),
            _ => panic!("unknown residency domain index"),
        }
        residency
    }

    fn source(name: &str) -> OpaqueId {
        OpaqueId::new(format!("source:{name}")).expect("source")
    }

    fn hex_opaque(byte: u8) -> OpaqueId {
        OpaqueId::new(format!("{byte:02x}").repeat(32)).expect("hex digest")
    }

    fn operation(name: &str, digest: u8) -> RevisionOperation {
        RevisionOperation::new(
            OpaqueId::new(format!("revision-operation:{name}")).expect("operation"),
            Blake3Digest32::from_bytes([digest; 32]),
        )
    }

    fn payload(plaintext_seed: u8, ciphertext_seed: u8) -> EncryptedRevisionPayload {
        payload_with_key(plaintext_seed, ciphertext_seed, "secret:revision-key")
    }

    fn payload_with_key(
        plaintext_seed: u8,
        ciphertext_seed: u8,
        key_name: &str,
    ) -> EncryptedRevisionPayload {
        EncryptedRevisionPayload::new(
            Blake3Digest32::from_bytes([plaintext_seed; 32]),
            3,
            Blake3Digest32::from_bytes([ciphertext_seed.wrapping_add(1); 32]),
            vec![ciphertext_seed; 12],
            vec![ciphertext_seed; 32],
            EncryptionBinding {
                key_reference: OpaqueId::new(key_name).expect("key"),
                key_version: NonZeroRevision::new(1).expect("version"),
                cipher_suite: CipherSuite::AuthenticatedEncryptionV1,
            },
            DEFAULT_REVISION_STORE_LIMITS,
        )
        .expect("payload")
    }

    fn envelope(binding_seed: u8) -> EnvelopeBinding {
        EnvelopeBinding {
            version: ENVELOPE_BINDING_VERSION,
            key_generation: NonZeroRevision::new(1).expect("generation"),
            source_revision_binding_digest: [binding_seed; 32],
            residency_binding_digest: [binding_seed.wrapping_add(0x10); 32],
            encryption_profile_digest: [0xD0; 32],
            content_digest_sha256: [0xE0; 32],
            plaintext_length: 3,
        }
    }

    fn ingest(name: &str, seed: u8) -> CanonicalIngestBinding {
        CanonicalIngestBinding::new(
            ReceiptRef::new(format!("receipt:t13-admission:{name}")).expect("receipt"),
            hex_opaque(0xA0),
            NonZeroRevision::new(1).expect("policy"),
            hex_opaque(0xB0),
            hex_opaque(seed),
            hex_opaque(seed.wrapping_add(1)),
        )
        .expect("ingest")
    }

    fn key(revision: u64) -> RevisionKey {
        RevisionKey {
            source_id: source("test"),
            revision: NonZeroRevision::new(revision).expect("revision"),
            residency: baseline_residency(),
        }
    }

    fn intent(revision: u64, name: &str, content: u8) -> RevisionWriteIntent {
        intent_full(
            source("test"),
            baseline_residency(),
            revision,
            name,
            content,
            content,
            content,
            "secret:revision-key",
        )
    }

    #[allow(clippy::too_many_arguments)]
    fn intent_full(
        source_id: OpaqueId,
        residency: ResidencyClosure,
        revision: u64,
        name: &str,
        plaintext_seed: u8,
        ciphertext_seed: u8,
        binding_seed: u8,
        key_name: &str,
    ) -> RevisionWriteIntent {
        let key = RevisionKey {
            source_id,
            revision: NonZeroRevision::new(revision).expect("revision"),
            residency,
        };
        let residency_key = residency.scope_id().expect("scope id");
        RevisionWriteIntent {
            key,
            source_binding_revision: NonZeroRevision::new(1).expect("revision"),
            payload: payload_with_key(plaintext_seed, ciphertext_seed, key_name),
            envelope: envelope(binding_seed),
            ingest: ingest(name, plaintext_seed),
            storage_object_id: OpaqueId::new(format!("object:{name}")).expect("object"),
            residency_key,
            legacy_migration: None,
            authorization_receipt: Some(
                ReceiptRef::new(format!("receipt:authorization:{name}")).expect("receipt"),
            ),
            operation: operation(name, plaintext_seed),
        }
    }

    fn readback(intent: &RevisionWriteIntent) -> RevisionObjectReadback {
        RevisionObjectReadback {
            key: intent.key.clone(),
            storage_object_id: intent.storage_object_id.clone(),
            ciphertext_digest: intent.payload.ciphertext_digest,
            ciphertext_bytes: u64::try_from(intent.payload.ciphertext_len()).expect("length"),
            plaintext_digest: intent.payload.plaintext_digest,
            plaintext_bytes: intent.payload.plaintext_bytes,
            encryption: intent.payload.encryption.clone(),
            envelope: intent.envelope,
            readback_verified: true,
            object_receipt: Some(ReceiptRef::new("receipt:object").expect("receipt")),
        }
    }

    fn confirm(store: &mut RevisionStore, intent: &RevisionWriteIntent) -> RevisionStoreReceipt {
        store
            .prepare_append(intent.clone())
            .expect("prepare must succeed");
        store
            .confirm_append(&intent.key, &intent.operation, readback(intent))
            .expect("confirm must succeed")
    }

    #[test]
    fn encrypted_payload_debug_never_dumps_ciphertext() {
        let mut payload = payload(7, 7);
        // Deliberately distinct from both public digest byte patterns (7 and 8).
        payload.nonce = vec![113, 59, 211, 41];
        payload.ciphertext = vec![229, 17, 193, 83, 251, 47];
        let nonce_sentinel = format!("{:?}", payload.nonce());
        let ciphertext_sentinel = format!("{:?}", payload.ciphertext());
        for debug in [format!("{payload:?}"), format!("{payload:#?}")] {
            assert!(!debug.contains(&nonce_sentinel));
            assert!(!debug.contains(&ciphertext_sentinel));
            assert!(!debug.contains("229,"));
            assert!(!debug.contains("113,"));
            assert!(debug.contains("<4 bytes>"));
            assert!(debug.contains("<6 encrypted bytes>"));
        }
        // Positive controls: the sentinels would detect an accidental raw dump.
        assert!(format!("{:?}", payload.ciphertext()).contains(&ciphertext_sentinel));
        assert!(format!("{:?}", payload.nonce()).contains(&nonce_sentinel));
    }

    #[test]
    fn first_revision_must_be_one_and_revisions_are_sequential() {
        let mut store = RevisionStore::new(DEFAULT_REVISION_STORE_LIMITS)
            .expect("store");
        assert_eq!(
            store.prepare_append(intent(2, "two", 2)),
            Err(RevisionStoreError::RevisionSequenceInvalid)
        );
        let first = intent(1, "one", 1);
        store.prepare_append(first.clone()).expect("prepare");
        store
            .confirm_append(&first.key, &first.operation, readback(&first))
            .expect("confirm");
        assert!(matches!(
            store.prepare_append(intent(2, "two", 2)),
            Ok(PrepareAppendResult::Prepared(_))
        ));
    }

    #[test]
    fn exact_active_revision_replays_without_rewrite() {
        let mut store = RevisionStore::new(DEFAULT_REVISION_STORE_LIMITS)
            .expect("store");
        let intent = intent(1, "one", 1);
        store.prepare_append(intent.clone()).expect("prepare");
        store
            .confirm_append(&intent.key, &intent.operation, readback(&intent))
            .expect("confirm");
        let PrepareAppendResult::AlreadyStored(receipt) = store
            .prepare_append(intent)
            .expect("replay")
        else {
            panic!("exact immutable revision must replay")
        };
        assert!(receipt.replayed);
    }

    #[test]
    fn same_revision_with_other_content_is_conflict() {
        let mut store = RevisionStore::new(DEFAULT_REVISION_STORE_LIMITS)
            .expect("store");
        let first = intent(1, "one", 1);
        store.prepare_append(first.clone()).expect("prepare");
        store
            .confirm_append(&first.key, &first.operation, readback(&first))
            .expect("confirm");
        assert_eq!(
            store.prepare_append(intent(1, "other", 9)),
            Err(RevisionStoreError::RevisionConflict)
        );
    }

    #[test]
    fn unknown_write_is_not_active_success() {
        let mut store = RevisionStore::new(DEFAULT_REVISION_STORE_LIMITS)
            .expect("store");
        let intent = intent(1, "one", 1);
        store.prepare_append(intent.clone()).expect("prepare");
        store
            .mark_outcome_unknown(&intent.key, &intent.operation)
            .expect("unknown");
        assert_eq!(
            store.active_record(&intent.key),
            Err(RevisionStoreError::OutcomeUnknown)
        );
    }

    #[test]
    fn exact_readback_recovers_unknown_write() {
        let mut store = RevisionStore::new(DEFAULT_REVISION_STORE_LIMITS)
            .expect("store");
        let intent = intent(1, "one", 1);
        store.prepare_append(intent.clone()).expect("prepare");
        store
            .mark_outcome_unknown(&intent.key, &intent.operation)
            .expect("unknown");
        assert!(matches!(
            store
                .recover_unknown(
                    &intent.key,
                    &intent.operation,
                    Some(readback(&intent)),
                )
                .expect("recover"),
            RecoveryResult::Applied(_)
        ));
        assert!(store.active_record(&intent.key).is_ok());
    }

    #[test]
    fn absent_readback_removes_unknown_intent_for_retry() {
        let mut store = RevisionStore::new(DEFAULT_REVISION_STORE_LIMITS)
            .expect("store");
        let intent = intent(1, "one", 1);
        store.prepare_append(intent.clone()).expect("prepare");
        store
            .mark_outcome_unknown(&intent.key, &intent.operation)
            .expect("unknown");
        assert_eq!(
            store
                .recover_unknown(&intent.key, &intent.operation, None)
                .expect("recover"),
            RecoveryResult::NotApplied
        );
        assert_eq!(
            store.state(&intent.key),
            Err(RevisionStoreError::RevisionNotFound)
        );
    }

    #[test]
    fn contradictory_readback_quarantines_revision() {
        let mut store = RevisionStore::new(DEFAULT_REVISION_STORE_LIMITS)
            .expect("store");
        let intent = intent(1, "one", 1);
        store.prepare_append(intent.clone()).expect("prepare");
        store
            .mark_outcome_unknown(&intent.key, &intent.operation)
            .expect("unknown");
        let mut wrong = readback(&intent);
        wrong.ciphertext_digest = Blake3Digest32::from_bytes([99; 32]);
        assert_eq!(
            store
                .recover_unknown(&intent.key, &intent.operation, Some(wrong))
                .expect("recover"),
            RecoveryResult::Quarantined
        );
        assert_eq!(
            store.active_record(&intent.key),
            Err(RevisionStoreError::Quarantined)
        );
    }

    #[test]
    fn operation_id_reuse_with_other_request_digest_is_rejected() {
        let mut store = RevisionStore::new(DEFAULT_REVISION_STORE_LIMITS)
            .expect("store");
        let first = intent(1, "same", 1);
        store.prepare_append(first.clone()).expect("prepare");
        store
            .confirm_append(&first.key, &first.operation, readback(&first))
            .expect("confirm");
        let mut second = intent(2, "second", 2);
        second.operation = RevisionOperation::new(
            first.operation.operation_id().clone(),
            Blake3Digest32::from_bytes([88; 32]),
        );
        assert_eq!(
            store.prepare_append(second),
            Err(RevisionStoreError::OperationConflict)
        );
    }

    #[test]
    fn residency_closure_round_trips_all_six_typed_domains() {
        let canonical = SearchObjectResidencyKey {
            scope_domain_id: ScopeDomainId::from_bytes([0x11; 16]),
            access_domain_id: AccessDomainId::from_bytes([0x22; 16]),
            confidentiality_domain_id: ConfidentialityDomainId::from_bytes([0x33; 16]),
            encryption_key_domain_id: EncryptionKeyDomainId::from_bytes([0x44; 16]),
            retention_domain_id: RetentionDomainId::from_bytes([0x55; 16]),
            erasure_domain_id: ErasureDomainId::from_bytes([0x66; 16]),
            versioned_content_digest: VersionedContentDigest {
                algorithm: DigestAlgorithm::Blake3_256,
                bytes: [0x07; 32],
            },
        };
        assert_eq!(
            ResidencyClosure::from_search_object_key(&canonical),
            baseline_residency()
        );
        let first = baseline_residency().scope_id().expect("scope id");
        assert!(first.as_str().starts_with("rs1."));
        assert_eq!(first, baseline_residency().scope_id().expect("scope id"));
        for index in 0..6 {
            assert_ne!(
                residency_variant(index).scope_id().expect("scope id"),
                first,
                "domain {index} must change the residency scope identity"
            );
        }
    }

    #[test]
    fn same_plaintext_across_all_six_domains_gets_separate_storage_identity() {
        let mut store = RevisionStore::new(DEFAULT_REVISION_STORE_LIMITS).expect("store");
        let plaintext_digest = Blake3Digest32::from_bytes([0x07; 32]);
        let mut addresses = Vec::new();
        for index in 0..6_u8 {
            let residency = residency_variant(usize::from(index));
            // Identical plaintext, separately encrypted and keyed per residency.
            let current = intent_full(
                source(&format!("cross-{index}")),
                residency,
                1,
                &format!("cross-{index}"),
                0x07,
                0x70 + index,
                0x80 + index,
                &format!("secret:revision-key-{index}"),
            );
            let receipt = confirm(&mut store, &current);
            assert!(!receipt.replayed);
            assert_eq!(receipt.residency, residency);
            let address = derive_object_address(
                &residency,
                RevisionObjectKind::RevisionEnvelopeV1,
                plaintext_digest,
                CAS_ADDRESS_VERSION,
            )
            .expect("address");
            addresses.push(address.to_path_string());
        }
        for (left, address) in addresses.iter().enumerate() {
            for (right, other) in addresses.iter().enumerate() {
                if left != right {
                    assert_ne!(address, other, "domains {left} and {right} collide");
                }
            }
        }
    }

    #[test]
    fn cross_domain_storage_object_reuse_is_typed_conflict() {
        let mut store = RevisionStore::new(DEFAULT_REVISION_STORE_LIMITS).expect("store");
        let first = intent(1, "one", 1);
        let target = first.storage_object_id.clone();
        confirm(&mut store, &first);
        for index in 0..6_u8 {
            let mut other = intent_full(
                source(&format!("other-{index}")),
                residency_variant(usize::from(index)),
                1,
                &format!("other-{index}"),
                9,
                0x40 + index,
                0x50 + index,
                "secret:revision-key",
            );
            other.storage_object_id.clone_from(&target);
            assert_eq!(
                store.prepare_append(other),
                Err(RevisionStoreError::ResidencyMismatch),
                "domain {index} must not reuse the physical object"
            );
        }
    }

    #[test]
    fn cross_domain_ciphertext_and_envelope_reuse_is_typed_conflict() {
        let mut store = RevisionStore::new(DEFAULT_REVISION_STORE_LIMITS).expect("store");
        let first = intent(1, "one", 1);
        confirm(&mut store, &first);
        // Identical ciphertext bytes under another residency, fresh object id.
        let second = intent_full(
            source("copy"),
            residency_variant(2),
            1,
            "copy",
            1,
            1,
            0x22,
            "secret:copy-key",
        );
        assert_eq!(
            store.prepare_append(second),
            Err(RevisionStoreError::ResidencyMismatch)
        );
        // Identical envelope binding digests under another residency, fresh bytes.
        let third = intent_full(
            source("envelope-copy"),
            residency_variant(4),
            1,
            "envelope-copy",
            9,
            0x41,
            1,
            "secret:envelope-copy-key",
        );
        assert_eq!(
            store.prepare_append(third),
            Err(RevisionStoreError::ResidencyMismatch)
        );
    }

    #[test]
    fn cross_domain_key_reference_reuse_is_typed_conflict() {
        let mut store = RevisionStore::new(DEFAULT_REVISION_STORE_LIMITS).expect("store");
        let first = intent(1, "one", 1);
        confirm(&mut store, &first);
        // Only the encryption-key domain differs; fresh object id and fresh
        // ciphertext, but the same secret reference is reused.
        let second = intent_full(
            source("rekey"),
            residency_variant(3),
            1,
            "rekey",
            9,
            0x42,
            0x52,
            "secret:revision-key",
        );
        assert_eq!(
            store.prepare_append(second),
            Err(RevisionStoreError::ResidencyMismatch)
        );
    }

    #[test]
    fn equivalent_residency_reuse_verifies_exact_bytes() {
        let mut store = RevisionStore::new(DEFAULT_REVISION_STORE_LIMITS).expect("store");
        let first = intent(1, "one", 1);
        confirm(&mut store, &first);
        let PrepareAppendResult::AlreadyStored(receipt) =
            store.prepare_append(intent(1, "one", 1)).expect("replay")
        else {
            panic!("exact immutable revision must replay")
        };
        assert!(receipt.replayed);
        assert_eq!(receipt.residency, baseline_residency());
        assert_eq!(
            store.prepare_append(intent(1, "other", 9)),
            Err(RevisionStoreError::RevisionConflict)
        );
        let mut clash = intent(2, "two", 2);
        clash.storage_object_id = first.storage_object_id.clone();
        assert_eq!(
            store.prepare_append(clash),
            Err(RevisionStoreError::RevisionConflict)
        );
    }

    #[test]
    fn same_occurrence_with_different_residency_is_typed_conflict() {
        let mut store = RevisionStore::new(DEFAULT_REVISION_STORE_LIMITS).expect("store");
        let pending = intent(1, "one", 1);
        store.prepare_append(pending.clone()).expect("prepare");
        let variant = intent_full(
            source("test"),
            residency_variant(0),
            1,
            "variant",
            9,
            0x43,
            0x53,
            "secret:variant-key",
        );
        assert_eq!(
            store.prepare_append(variant.clone()),
            Err(RevisionStoreError::ResidencyMismatch)
        );
        store
            .confirm_append(&pending.key, &pending.operation, readback(&pending))
            .expect("confirm");
        assert_eq!(
            store.prepare_append(variant),
            Err(RevisionStoreError::ResidencyMismatch)
        );
    }

    #[test]
    fn wrong_envelope_binding_fails_prepare_and_confirm() {
        let mut store = RevisionStore::new(DEFAULT_REVISION_STORE_LIMITS).expect("store");
        let mut versioned = intent(1, "bad-version", 1);
        versioned.envelope.version = 2;
        assert_eq!(
            store.prepare_append(versioned),
            Err(RevisionStoreError::EnvelopeInvalid)
        );
        let mut lengthed = intent(1, "bad-length", 1);
        lengthed.envelope.plaintext_length = 99;
        assert_eq!(
            store.prepare_append(lengthed),
            Err(RevisionStoreError::EnvelopeInvalid)
        );
        let mut generated = intent(1, "bad-generation", 1);
        generated.envelope.key_generation = NonZeroRevision::new(2).expect("generation");
        assert_eq!(
            store.prepare_append(generated),
            Err(RevisionStoreError::EnvelopeInvalid)
        );
        let good = intent(1, "good", 1);
        store.prepare_append(good.clone()).expect("prepare");
        let mut rebound = readback(&good);
        rebound.envelope.residency_binding_digest = [0xFF; 32];
        assert_eq!(
            store.confirm_append(&good.key, &good.operation, rebound),
            Err(RevisionStoreError::ReadbackMismatch)
        );
        let mut redomained = readback(&good);
        redomained.key.residency = residency_variant(5);
        assert_eq!(
            store.confirm_append(&good.key, &good.operation, redomained),
            Err(RevisionStoreError::ReadbackMismatch)
        );
    }

    #[test]
    fn truncated_ciphertext_is_rejected() {
        let short = EncryptedRevisionPayload::new(
            Blake3Digest32::from_bytes([9; 32]),
            3,
            Blake3Digest32::from_bytes([10; 32]),
            vec![9; 12],
            vec![9; 5],
            EncryptionBinding {
                key_reference: OpaqueId::new("secret:revision-key").expect("key"),
                key_version: NonZeroRevision::new(1).expect("version"),
                cipher_suite: CipherSuite::AuthenticatedEncryptionV1,
            },
            DEFAULT_REVISION_STORE_LIMITS,
        );
        assert_eq!(short, Err(RevisionStoreError::CiphertextSizeInvalid));
        let tagged = EncryptedRevisionPayload::new(
            Blake3Digest32::from_bytes([9; 32]),
            3,
            Blake3Digest32::from_bytes([10; 32]),
            vec![9; 12],
            vec![9; 16],
            EncryptionBinding {
                key_reference: OpaqueId::new("secret:revision-key").expect("key"),
                key_version: NonZeroRevision::new(1).expect("version"),
                cipher_suite: CipherSuite::AuthenticatedEncryptionV1,
            },
            DEFAULT_REVISION_STORE_LIMITS,
        );
        assert!(tagged.is_ok());
    }

    #[test]
    fn legacy_migration_requires_explicit_receipt() {
        let mut store = RevisionStore::new(DEFAULT_REVISION_STORE_LIMITS).expect("store");
        let mut legacy = intent(1, "legacy", 1);
        legacy.residency_key = OpaqueId::new("residency:legacy-opaque").expect("legacy");
        assert_eq!(
            store.prepare_append(legacy.clone()),
            Err(RevisionStoreError::ResidencyMismatch)
        );
        legacy.legacy_migration = Some(migrate_legacy_residency_key(
            legacy.residency_key.clone(),
            ReceiptRef::new("receipt:migration:1").expect("receipt"),
        ));
        store
            .prepare_append(legacy.clone())
            .expect("migrated prepare");
        let receipt = store
            .confirm_append(&legacy.key, &legacy.operation, readback(&legacy))
            .expect("migrated confirm");
        assert_eq!(receipt.residency, baseline_residency());
        let mut smuggled = intent_full(
            source("smuggled"),
            baseline_residency(),
            1,
            "smuggled",
            4,
            4,
            4,
            "secret:revision-key",
        );
        smuggled.residency_key = OpaqueId::new("residency:legacy-a").expect("legacy");
        smuggled.legacy_migration = Some(migrate_legacy_residency_key(
            OpaqueId::new("residency:legacy-b").expect("legacy"),
            ReceiptRef::new("receipt:migration:2").expect("receipt"),
        ));
        assert_eq!(
            store.prepare_append(smuggled),
            Err(RevisionStoreError::ResidencyMismatch)
        );
    }

    #[test]
    fn t13_ingest_hex_fields_are_validated() {
        let receipt = ReceiptRef::new("receipt:t13-admission:probe").expect("receipt");
        let policy = NonZeroRevision::new(1).expect("policy");
        for field in ["short", "UPPERCASE-HEX-DIGEST-PLACEHOLDER-INVALID!!", "zz"] {
            let candidate = OpaqueId::new(field).expect("candidate");
            assert_eq!(
                CanonicalIngestBinding::new(
                    receipt.clone(),
                    hex_opaque(0xA0),
                    policy,
                    candidate,
                    hex_opaque(0x07),
                    hex_opaque(0x08),
                ),
                Err(RevisionStoreError::IngestBindingInvalid)
            );
        }
        let uppercase = OpaqueId::new("A".repeat(64)).expect("candidate");
        assert_eq!(
            CanonicalIngestBinding::new(
                receipt.clone(),
                uppercase,
                policy,
                hex_opaque(0xB0),
                hex_opaque(0x07),
                hex_opaque(0x08),
            ),
            Err(RevisionStoreError::IngestBindingInvalid)
        );
        assert!(
            CanonicalIngestBinding::new(
                receipt,
                hex_opaque(0xA0),
                policy,
                hex_opaque(0xB0),
                hex_opaque(0x07),
                hex_opaque(0x08),
            )
            .is_ok()
        );
    }

    #[test]
    fn purge_tombstone_fences_residency_writes() {
        let mut store = RevisionStore::new(DEFAULT_REVISION_STORE_LIMITS).expect("store");
        let tombstone = PurgeTombstone {
            scope: TombstoneScope::Residency(baseline_residency()),
            generation: NonZeroRevision::new(1).expect("generation"),
            tombstone_receipt: ReceiptRef::new("receipt:tombstone:1").expect("receipt"),
            operation: operation("tombstone", 1),
        };
        let installed = store
            .install_purge_tombstone(tombstone.clone())
            .expect("install");
        assert!(!installed.replayed);
        let replayed = store.install_purge_tombstone(tombstone).expect("reinstall");
        assert!(replayed.replayed);
        let conflicting = PurgeTombstone {
            scope: TombstoneScope::Residency(baseline_residency()),
            generation: NonZeroRevision::new(1).expect("generation"),
            tombstone_receipt: ReceiptRef::new("receipt:tombstone:other").expect("receipt"),
            operation: operation("tombstone-other", 2),
        };
        assert_eq!(
            store.install_purge_tombstone(conflicting),
            Err(RevisionStoreError::RevisionConflict)
        );
        assert_eq!(
            store.prepare_append(intent(1, "fenced", 1)),
            Err(RevisionStoreError::Tombstoned)
        );
        let free = intent_full(
            source("scope-free"),
            residency_variant(0),
            1,
            "scope-free",
            5,
            5,
            5,
            "secret:revision-key",
        );
        store.prepare_append(free).expect("exact fence");
        store
            .install_purge_tombstone(PurgeTombstone {
                scope: TombstoneScope::Scope(baseline_residency().scope),
                generation: NonZeroRevision::new(2).expect("generation"),
                tombstone_receipt: ReceiptRef::new("receipt:tombstone:scope").expect("receipt"),
                operation: operation("tombstone-scope", 3),
            })
            .expect("scope fence");
        let scoped = intent_full(
            source("scoped"),
            residency_variant(1),
            1,
            "scoped",
            6,
            6,
            6,
            "secret:revision-key",
        );
        assert_eq!(
            store.prepare_append(scoped),
            Err(RevisionStoreError::Tombstoned)
        );
        let mut pending = intent_full(
            source("pending-fenced"),
            residency_variant(0),
            1,
            "pending-fenced",
            7,
            7,
            7,
            "secret:revision-key",
        );
        pending.storage_object_id = OpaqueId::new("object:pending-fenced-own").expect("object");
        store.prepare_append(pending.clone()).expect("pending");
        store
            .install_purge_tombstone(PurgeTombstone {
                scope: TombstoneScope::Residency(residency_variant(0)),
                generation: NonZeroRevision::new(3).expect("generation"),
                tombstone_receipt: ReceiptRef::new("receipt:tombstone:pending").expect("receipt"),
                operation: operation("tombstone-pending", 4),
            })
            .expect("pending fence");
        assert_eq!(
            store.confirm_append(&pending.key, &pending.operation, readback(&pending)),
            Err(RevisionStoreError::Tombstoned)
        );
    }

    #[test]
    fn exact_deletion_requires_lifecycle_plan_and_exact_address() {
        let mut store = RevisionStore::new(DEFAULT_REVISION_STORE_LIMITS).expect("store");
        let first = intent(1, "one", 1);
        confirm(&mut store, &first);
        let wrong_address = LifecycleDeletionPlan {
            plan_receipt: ReceiptRef::new("receipt:lifecycle-plan:1").expect("receipt"),
            authority: DeletionAuthorityKind::OrdinarySweep,
            target: first.key.clone(),
            target_storage_object_id: OpaqueId::new("object:wrong").expect("object"),
            operation: operation("delete-one", 1),
        };
        assert_eq!(
            store.apply_exact_object_deletion(wrong_address),
            Err(RevisionStoreError::DeletionNotAuthorized)
        );
        let unknown = LifecycleDeletionPlan {
            plan_receipt: ReceiptRef::new("receipt:lifecycle-plan:1").expect("receipt"),
            authority: DeletionAuthorityKind::OrdinarySweep,
            target: key(2),
            target_storage_object_id: OpaqueId::new("object:unknown").expect("object"),
            operation: operation("delete-unknown", 2),
        };
        assert_eq!(
            store.apply_exact_object_deletion(unknown),
            Err(RevisionStoreError::RevisionNotFound)
        );
        let plan = LifecycleDeletionPlan {
            plan_receipt: ReceiptRef::new("receipt:lifecycle-plan:1").expect("receipt"),
            authority: DeletionAuthorityKind::OrdinarySweep,
            target: first.key.clone(),
            target_storage_object_id: first.storage_object_id.clone(),
            operation: operation("delete-one", 3),
        };
        let receipt = store
            .apply_exact_object_deletion(plan.clone())
            .expect("delete");
        assert!(!receipt.replayed);
        assert_eq!(receipt.authority, DeletionAuthorityKind::OrdinarySweep);
        let replayed = store.apply_exact_object_deletion(plan).expect("replay");
        assert!(replayed.replayed);
        let reused_operation = LifecycleDeletionPlan {
            plan_receipt: ReceiptRef::new("receipt:lifecycle-plan:other").expect("receipt"),
            authority: DeletionAuthorityKind::OrdinarySweep,
            target: first.key.clone(),
            target_storage_object_id: first.storage_object_id.clone(),
            operation: operation("delete-one", 3),
        };
        assert_eq!(
            store.apply_exact_object_deletion(reused_operation),
            Err(RevisionStoreError::OperationConflict)
        );
        assert_eq!(
            store.active_record(&first.key),
            Err(RevisionStoreError::RevisionNotFound)
        );
        confirm(&mut store, &intent(1, "one", 1));
        let second = intent(2, "two", 2);
        confirm(&mut store, &second);
        let purge = LifecycleDeletionPlan {
            plan_receipt: ReceiptRef::new("receipt:lifecycle-plan:purge").expect("receipt"),
            authority: DeletionAuthorityKind::SecurityPurge,
            target: second.key.clone(),
            target_storage_object_id: second.storage_object_id.clone(),
            operation: operation("delete-two", 4),
        };
        let purge_receipt = store.apply_exact_object_deletion(purge).expect("purge");
        assert_eq!(
            purge_receipt.authority,
            DeletionAuthorityKind::SecurityPurge
        );
        store
            .install_purge_tombstone(PurgeTombstone {
                scope: TombstoneScope::Residency(baseline_residency()),
                generation: NonZeroRevision::new(1).expect("generation"),
                tombstone_receipt: ReceiptRef::new("receipt:tombstone:purge").expect("receipt"),
                operation: operation("tombstone-purge", 5),
            })
            .expect("purge fence");
        assert_eq!(
            store.prepare_append(intent(2, "two", 2)),
            Err(RevisionStoreError::Tombstoned)
        );
        let pending = intent_full(
            source("pending-src"),
            residency_variant(0),
            1,
            "pending-del",
            5,
            5,
            5,
            "secret:pending-key",
        );
        store.prepare_append(pending.clone()).expect("pending");
        let pending_plan = LifecycleDeletionPlan {
            plan_receipt: ReceiptRef::new("receipt:lifecycle-plan:pending").expect("receipt"),
            authority: DeletionAuthorityKind::OrdinarySweep,
            target: pending.key.clone(),
            target_storage_object_id: pending.storage_object_id,
            operation: operation("delete-pending", 6),
        };
        assert_eq!(
            store.apply_exact_object_deletion(pending_plan),
            Err(RevisionStoreError::OutcomeUnknown)
        );
    }

    #[test]
    fn derive_object_address_is_domain_separated() {
        let digest = Blake3Digest32::from_bytes([0x07; 32]);
        let base = derive_object_address(
            &baseline_residency(),
            RevisionObjectKind::RevisionEnvelopeV1,
            digest,
            CAS_ADDRESS_VERSION,
        )
        .expect("address");
        let path = base.to_path_string();
        assert!(path.starts_with("cas/v1/revision-envelope-v1/"));
        assert!(!path.contains("source:test"));
        assert_eq!(
            path,
            derive_object_address(
                &baseline_residency(),
                RevisionObjectKind::RevisionEnvelopeV1,
                digest,
                CAS_ADDRESS_VERSION,
            )
            .expect("address")
            .to_path_string()
        );
        for index in 0..6 {
            let other = derive_object_address(
                &residency_variant(index),
                RevisionObjectKind::RevisionEnvelopeV1,
                digest,
                CAS_ADDRESS_VERSION,
            )
            .expect("address");
            assert_ne!(path, other.to_path_string(), "domain {index} collides");
        }
        let other_digest = derive_object_address(
            &baseline_residency(),
            RevisionObjectKind::RevisionEnvelopeV1,
            Blake3Digest32::from_bytes([0x08; 32]),
            CAS_ADDRESS_VERSION,
        )
        .expect("address");
        assert_ne!(path, other_digest.to_path_string());
        assert_eq!(
            derive_object_address(
                &baseline_residency(),
                RevisionObjectKind::RevisionEnvelopeV1,
                Blake3Digest32::from_bytes([0; 32]),
                CAS_ADDRESS_VERSION,
            ),
            Err(RevisionStoreError::AddressInvalid)
        );
        assert_eq!(
            derive_object_address(
                &baseline_residency(),
                RevisionObjectKind::RevisionEnvelopeV1,
                digest,
                99,
            ),
            Err(RevisionStoreError::AddressInvalid)
        );
    }

    #[test]
    fn restart_has_no_history_and_debug_hides_plaintext_bytes() {
        let mut store = RevisionStore::new(DEFAULT_REVISION_STORE_LIMITS).expect("store");
        let mut sentinel = intent(1, "sentinel", 1);
        sentinel.payload = payload_with_key(0x01, 0xE5, "secret:revision-key");
        sentinel.envelope = envelope(0x01);
        let nonce_sentinel = format!("{:?}", sentinel.payload.nonce());
        let ciphertext_sentinel = format!("{:?}", sentinel.payload.ciphertext());
        assert!(nonce_sentinel.contains("113") || ciphertext_sentinel.contains("229"));
        let stored = confirm(&mut store, &sentinel);
        let record = store.active_record(&sentinel.key).expect("record");
        for debug in [
            format!("{:?}", sentinel.payload),
            format!("{sentinel:?}"),
            format!("{record:?}"),
            format!("{:?}", readback(&sentinel)),
            format!("{stored:?}"),
        ] {
            assert!(!debug.contains("229"), "ciphertext residue: {debug}");
            assert!(!debug.contains("113"), "nonce residue: {debug}");
        }
        drop(store);
        let fresh = RevisionStore::new(DEFAULT_REVISION_STORE_LIMITS).expect("store");
        assert!(fresh.is_empty());
        assert_eq!(
            fresh.state(&sentinel.key),
            Err(RevisionStoreError::RevisionNotFound)
        );
    }

    #[test]
    fn operation_reuse_with_different_key_is_rejected() {
        let mut store = RevisionStore::new(DEFAULT_REVISION_STORE_LIMITS).expect("store");
        let first = intent(1, "same", 1);
        confirm(&mut store, &first);
        let mut second = intent(2, "second", 2);
        second.operation = RevisionOperation::new(
            first.operation.operation_id().clone(),
            Blake3Digest32::from_bytes([88; 32]),
        );
        assert_eq!(
            store.prepare_append(second),
            Err(RevisionStoreError::OperationConflict)
        );
        let mut third = intent(2, "third", 1);
        third.operation = first.operation.clone();
        assert_eq!(
            store.prepare_append(third),
            Err(RevisionStoreError::OperationConflict)
        );
    }
}

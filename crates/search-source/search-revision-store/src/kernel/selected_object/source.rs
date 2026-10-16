//! Typed source identities, pre-seal descriptor, and late object binding.

use search_contracts::{
    Blake3Digest32, NonZeroRevision, OpaqueId, SourceId, SourceNamespaceId, SourceRevisionId,
};

use super::super::error::RevisionStoreError;
use super::super::model::{EncryptedRevisionPayload, EncryptionBinding};
use super::domain::{DomainBinding, residency_canonical_bytes};

/// Typed source identity for one retained revision occurrence.
///
/// The namespace, source, and revision IDs are genuine typed identifiers
/// supplied by the root. They are never inferred from path, ordinal, or
/// content hash. The sequence provides per-source monotone ordering.
#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub struct TypedSourceIdentity {
    /// Source namespace (root/install/owner scope).
    pub namespace_id: SourceNamespaceId,
    /// Stable source identity within the namespace.
    pub source_id: SourceId,
    /// Immutable revision identifier for this occurrence.
    pub revision_id: SourceRevisionId,
    /// Per-source monotone occurrence sequence (starts at 1).
    pub sequence: NonZeroRevision,
}

impl TypedSourceIdentity {
    /// Creates a typed source identity after validating the sequence is nonzero.
    pub fn new(
        namespace_id: SourceNamespaceId,
        source_id: SourceId,
        revision_id: SourceRevisionId,
        sequence: NonZeroRevision,
    ) -> Self {
        Self {
            namespace_id,
            source_id,
            revision_id,
            sequence,
        }
    }
}

/// Pre-seal occurrence descriptor retained before AES.
///
/// This descriptor captures the complete original pre-seal state: typed source
/// identity, six domain bindings, and the exact source-body digest/length.
/// It contains no physical object ID, no ciphertext digest, and no encryption
/// binding. C02 durably retains this descriptor before any cryptographic
/// effect; recovery keeps the same UUID.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct PreSealDescriptor {
    /// Typed source identity for this occurrence.
    pub source: TypedSourceIdentity,
    /// Six domain bindings in canonical order.
    pub domain_bindings: [DomainBinding; 6],
    /// Exact authoritative source-body digest.
    pub content_digest: Blake3Digest32,
    /// Exact source-body byte length.
    pub content_bytes: u64,
    /// Original retention operation ID (distinct from admission and phase IDs).
    pub operation_id: OpaqueId,
    /// Complete canonical residency bytes for AAD construction.
    pub residency_canonical: Vec<u8>,
}

impl PreSealDescriptor {
    /// Creates a pre-seal descriptor from the typed identity, domain bindings,
    /// content digest/length, and operation ID.
    ///
    /// The residency canonical bytes are computed from the six domain bindings.
    /// Empty source bodies are valid for selected-v2.
    pub fn new(
        source: TypedSourceIdentity,
        domain_bindings: [DomainBinding; 6],
        content_digest: Blake3Digest32,
        content_bytes: u64,
        operation_id: OpaqueId,
    ) -> Result<Self, RevisionStoreError> {
        let residency_canonical = residency_canonical_bytes(&domain_bindings)?;
        if residency_canonical.is_empty() || residency_canonical.len() > 8192 {
            return Err(RevisionStoreError::ContractExhausted);
        }
        Ok(Self {
            source,
            domain_bindings,
            content_digest,
            content_bytes,
            operation_id,
            residency_canonical,
        })
    }

    /// Returns the typed source identity.
    pub const fn source(&self) -> &TypedSourceIdentity {
        &self.source
    }

    /// Returns the six domain bindings.
    pub const fn domain_bindings(&self) -> &[DomainBinding; 6] {
        &self.domain_bindings
    }

    /// Returns the complete canonical residency bytes for AAD.
    pub fn residency_canonical(&self) -> &[u8] {
        &self.residency_canonical
    }
}

/// Late physical-object metadata binding applied after AES.
///
/// This binds the derived physical object identity (storage object ID,
/// ciphertext digest, ciphertext length) to the pre-seal descriptor after
/// encryption is complete. The ciphertext digest and length are derived from
/// the owned `EncryptedRevisionPayload` bytes, not caller-asserted.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ObjectMetadataBinding {
    /// Pre-seal descriptor this binding extends.
    pub preseal: PreSealDescriptor,
    /// Opaque storage object identity (CAS address).
    pub storage_object_id: OpaqueId,
    /// Exact encryption-key binding (reference, version, cipher suite).
    pub encryption: EncryptionBinding,
    /// Owned encrypted payload (ciphertext digest/length derived from bytes).
    payload: EncryptedRevisionPayload,
}

impl ObjectMetadataBinding {
    /// Creates a late object metadata binding from the owned encrypted payload.
    ///
    /// The ciphertext digest and length are derived from the payload's owned
    /// bytes via `ciphertext_digest()` and `ciphertext_len()`, not from
    /// caller-asserted values.
    pub fn new(
        preseal: PreSealDescriptor,
        storage_object_id: OpaqueId,
        payload: EncryptedRevisionPayload,
        encryption: EncryptionBinding,
    ) -> Result<Self, RevisionStoreError> {
        Ok(Self {
            preseal,
            storage_object_id,
            encryption,
            payload,
        })
    }

    /// Returns the pre-seal descriptor.
    pub const fn preseal(&self) -> &PreSealDescriptor {
        &self.preseal
    }

    /// Returns the storage object ID.
    pub const fn storage_object_id(&self) -> &OpaqueId {
        &self.storage_object_id
    }

    /// Returns the ciphertext digest.
    pub const fn ciphertext_digest(&self) -> Blake3Digest32 {
        self.payload.ciphertext_digest()
    }

    /// Returns the ciphertext byte count.
    pub fn ciphertext_bytes(&self) -> u64 {
        self.payload.ciphertext_len() as u64
    }

    /// Returns the encryption binding.
    pub const fn encryption(&self) -> &EncryptionBinding {
        &self.encryption
    }

    /// Returns the owned encrypted payload.
    pub fn payload(&self) -> &EncryptedRevisionPayload {
        &self.payload
    }

    /// Returns the complete canonical residency bytes for AAD construction.
    pub fn residency_canonical(&self) -> &[u8] {
        &self.preseal.residency_canonical
    }

    /// Returns the original retention operation ID.
    pub const fn operation_id(&self) -> &OpaqueId {
        &self.preseal.operation_id
    }
}

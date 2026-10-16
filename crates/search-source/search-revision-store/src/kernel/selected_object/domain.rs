//! Closed domain kinds and genuine-UUID domain bindings with full tagged BLAKE3.

use search_contracts::{
    blake3_raw, Blake3Digest32, CanonicalDigestDomain, CanonicalValue, DigestInputLimit,
};

use super::super::error::RevisionStoreError;

/// Closed set of six semantic domain kinds for selected-reference objects.
#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub enum DomainKind {
    /// Root/install/namespace/owner scope domain.
    Scope,
    /// User/access-policy domain.
    Access,
    /// Confidentiality classification domain.
    Confidentiality,
    /// Encryption-key selection domain.
    EncryptionKey,
    /// Retention/protection-release domain.
    Retention,
    /// Erasure/purge domain.
    Erasure,
}

impl DomainKind {
    /// Stable wire label for this domain kind.
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Scope => "scope",
            Self::Access => "access",
            Self::Confidentiality => "confidentiality",
            Self::EncryptionKey => "encryption-key",
            Self::Retention => "retention",
            Self::Erasure => "erasure",
        }
    }

    /// Canonical digest domain for tagged BLAKE3 over the complete body.
    ///
    /// # Panics
    /// Panics if the static domain literal fails to parse (which is impossible
    /// for a hardcoded valid string).
    pub fn digest_domain() -> CanonicalDigestDomain {
        CanonicalDigestDomain::parse("eliot/raw/selected-domain-binding/v1")
            .expect("static domain literal is valid")
    }

    /// All six domain kinds in canonical order.
    pub const ALL: [Self; 6] = [
        Self::Scope,
        Self::Access,
        Self::Confidentiality,
        Self::EncryptionKey,
        Self::Retention,
        Self::Erasure,
    ];
}

/// Genuine metered UUID supplied by the root, never digest-truncated or derived.
pub type GenuineUuid = [u8; 16];

/// Complete domain binding: kind, genuine UUID, bounded canonical body, full tagged BLAKE3.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct DomainBinding {
    /// Closed semantic domain kind.
    pub kind: DomainKind,
    /// Genuine 16-byte UUID metered by the root.
    pub uuid: GenuineUuid,
    /// Complete bounded canonical body for this domain (CBOR bytes).
    pub canonical_body: Vec<u8>,
    /// Full tagged BLAKE3 digest over the canonical body.
    pub tagged_digest: Blake3Digest32,
}

impl DomainBinding {
    /// Maximum canonical body bytes per domain binding.
    pub const MAX_BODY_BYTES: usize = 8192;

    /// Creates a domain binding after validating the body and computing the
    /// full tagged BLAKE3 digest.
    pub fn new(
        kind: DomainKind,
        uuid: GenuineUuid,
        canonical_body: Vec<u8>,
    ) -> Result<Self, RevisionStoreError> {
        if canonical_body.is_empty() || canonical_body.len() > Self::MAX_BODY_BYTES {
            return Err(RevisionStoreError::AddressInvalid);
        }
        let domain = DomainKind::digest_domain();
        let limit = DigestInputLimit::new(Self::MAX_BODY_BYTES + domain.as_str().len() + 1)
            .map_err(|_| RevisionStoreError::ContractExhausted)?;
        let tagged_digest = blake3_raw(&domain, &canonical_body, limit)
            .map_err(|_| RevisionStoreError::ContractExhausted)?;
        Ok(Self {
            kind,
            uuid,
            canonical_body,
            tagged_digest,
        })
    }

    /// Verifies that a candidate binding matches this binding exactly.
    pub fn verify(&self, candidate: &Self) -> Result<(), RevisionStoreError> {
        if self.kind != candidate.kind {
            return Err(RevisionStoreError::ResidencyMismatch);
        }
        if self.uuid != candidate.uuid {
            return Err(RevisionStoreError::ResidencyMismatch);
        }
        if self.canonical_body != candidate.canonical_body {
            return Err(RevisionStoreError::ResidencyMismatch);
        }
        if self.tagged_digest != candidate.tagged_digest {
            return Err(RevisionStoreError::ResidencyMismatch);
        }
        Ok(())
    }

    /// Refuses a collision: same UUID with different body bytes.
    pub fn refuse_uuid_collision(&self, other: &Self) -> Result<(), RevisionStoreError> {
        if self.uuid == other.uuid && self.canonical_body != other.canonical_body {
            return Err(RevisionStoreError::ResidencyMismatch);
        }
        Ok(())
    }

    /// Refuses a collision: same tagged digest with different canonical body bytes.
    pub fn refuse_digest_collision(&self, other: &Self) -> Result<(), RevisionStoreError> {
        if self.tagged_digest == other.tagged_digest && self.canonical_body != other.canonical_body {
            return Err(RevisionStoreError::ResidencyMismatch);
        }
        Ok(())
    }
}

/// Builds the complete canonical residency bytes from six domain bindings.
pub fn residency_canonical_bytes(
    bindings: &[DomainBinding; 6],
) -> Result<Vec<u8>, RevisionStoreError> {
    for (index, binding) in bindings.iter().enumerate() {
        if binding.kind != DomainKind::ALL[index] {
            return Err(RevisionStoreError::ResidencyMismatch);
        }
    }
    let mut rows: Vec<CanonicalValue> = Vec::with_capacity(6);
    for binding in bindings {
        let mut object: search_contracts::BoundedMap<
            search_contracts::CanonicalKey,
            CanonicalValue,
            { search_contracts::MAX_MAP_ENTRIES },
        > = search_contracts::BoundedMap::empty();
        object.insert(
            search_contracts::CanonicalKey::new_non_empty("kind")
                .map_err(|_| RevisionStoreError::ContractExhausted)?,
            CanonicalValue::Text(
                search_contracts::CanonicalText::new(binding.kind.as_str())
                    .map_err(|_| RevisionStoreError::ContractExhausted)?,
            ),
        ).map_err(|_| RevisionStoreError::ContractExhausted)?;
        object.insert(
            search_contracts::CanonicalKey::new_non_empty("uuid")
                .map_err(|_| RevisionStoreError::ContractExhausted)?,
            CanonicalValue::Bytes(
                search_contracts::BoundedBytes::new(binding.uuid.to_vec())
                    .map_err(|_| RevisionStoreError::ContractExhausted)?,
            ),
        ).map_err(|_| RevisionStoreError::ContractExhausted)?;
        object.insert(
            search_contracts::CanonicalKey::new_non_empty("body")
                .map_err(|_| RevisionStoreError::ContractExhausted)?,
            CanonicalValue::Bytes(
                search_contracts::BoundedBytes::new(binding.canonical_body.clone())
                    .map_err(|_| RevisionStoreError::ContractExhausted)?,
            ),
        ).map_err(|_| RevisionStoreError::ContractExhausted)?;
        object.insert(
            search_contracts::CanonicalKey::new_non_empty("tagged_digest")
                .map_err(|_| RevisionStoreError::ContractExhausted)?,
            CanonicalValue::Bytes(
                search_contracts::BoundedBytes::new(binding.tagged_digest.as_bytes().to_vec())
                    .map_err(|_| RevisionStoreError::ContractExhausted)?,
            ),
        ).map_err(|_| RevisionStoreError::ContractExhausted)?;
        rows.push(CanonicalValue::Object(object));
    }
    let array = search_contracts::BoundedList::new(rows)
        .map_err(|_| RevisionStoreError::ContractExhausted)?;
    let value = CanonicalValue::Array(array);
    search_contracts::to_canonical_cbor(&value)
        .map_err(|_| RevisionStoreError::ContractExhausted)
        .map(|bytes| bytes.as_slice().to_vec())
}

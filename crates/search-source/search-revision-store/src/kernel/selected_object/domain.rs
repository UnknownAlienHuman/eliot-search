//! Closed domain kinds and genuine-UUID domain bindings with full tagged BLAKE3.

use search_contracts::{
    blake3_raw, parse_canonical_cbor, Blake3Digest32, CanonicalDigestDomain,
    CanonicalValue, DigestInputLimit,
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

    /// All six domain kinds in canonical order.
    pub const ALL: [Self; 6] = [
        Self::Scope,
        Self::Access,
        Self::Confidentiality,
        Self::EncryptionKey,
        Self::Retention,
        Self::Erasure,
    ];

    /// Per-kind tagged BLAKE3 domain. Each domain kind uses a distinct static
    /// domain string so identical bodies under different kinds produce
    /// different tagged digests.
    pub fn digest_domain(self) -> CanonicalDigestDomain {
        match self {
            Self::Scope => {
                parse_static_domain("eliot/raw/selected-domain-scope/v1")
            }
            Self::Access => {
                parse_static_domain("eliot/raw/selected-domain-access/v1")
            }
            Self::Confidentiality => {
                parse_static_domain("eliot/raw/selected-domain-confidentiality/v1")
            }
            Self::EncryptionKey => {
                parse_static_domain("eliot/raw/selected-domain-encryption-key/v1")
            }
            Self::Retention => {
                parse_static_domain("eliot/raw/selected-domain-retention/v1")
            }
            Self::Erasure => {
                parse_static_domain("eliot/raw/selected-domain-erasure/v1")
            }
        }
    }
}

/// Parses a static domain literal. Panics if the literal is invalid (which is
/// impossible for hardcoded valid strings).
fn parse_static_domain(name: &str) -> CanonicalDigestDomain {
    CanonicalDigestDomain::parse(name).expect("static domain literal is valid")
}

/// Genuine metered UUID supplied by the root, never digest-truncated or derived.
pub type GenuineUuid = [u8; 16];

/// Complete domain binding: kind, genuine UUID, bounded canonical body, full tagged BLAKE3.
///
/// Fields are private. Use `new` to construct and the getters to inspect.
/// `new` validates canonical CBOR body, RFC 4122 version-4 UUID, and computes
/// the full tagged BLAKE3 digest over the complete body using a per-kind
/// static domain.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct DomainBinding {
    kind: DomainKind,
    uuid: GenuineUuid,
    canonical_body: Vec<u8>,
    tagged_digest: Blake3Digest32,
}

impl DomainBinding {
    /// Maximum canonical body bytes per domain binding.
    pub const MAX_BODY_BYTES: usize = 8192;

    /// Returns the closed semantic domain kind.
    pub const fn kind(&self) -> DomainKind {
        self.kind
    }

    /// Returns the genuine 16-byte UUID.
    pub const fn uuid(&self) -> GenuineUuid {
        self.uuid
    }

    /// Returns the complete bounded canonical body bytes.
    pub fn canonical_body(&self) -> &[u8] {
        &self.canonical_body
    }

    /// Returns the full tagged BLAKE3 digest.
    pub const fn tagged_digest(&self) -> Blake3Digest32 {
        self.tagged_digest
    }

    /// Creates a domain binding after validating the body and computing the
    /// full tagged BLAKE3 digest using the per-kind static domain.
    ///
    /// The body must be valid canonical CBOR and at most
    /// `MAX_BODY_BYTES` bytes. The UUID must be RFC 4122 version 4
    /// (version nibble 4, variant bits 10).
    pub fn new(
        kind: DomainKind,
        uuid: GenuineUuid,
        canonical_body: Vec<u8>,
    ) -> Result<Self, RevisionStoreError> {
        if canonical_body.is_empty()
            || canonical_body.len() > Self::MAX_BODY_BYTES
            || !is_valid_canonical_cbor(&canonical_body)
        {
            return Err(RevisionStoreError::AddressInvalid);
        }
        if !is_rfc4122_v4(uuid) {
            return Err(RevisionStoreError::AddressInvalid);
        }
        let domain = kind.digest_domain();
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
        if self.kind() != candidate.kind() {
            return Err(RevisionStoreError::ResidencyMismatch);
        }
        if self.uuid() != candidate.uuid() {
            return Err(RevisionStoreError::ResidencyMismatch);
        }
        if self.canonical_body() != candidate.canonical_body() {
            return Err(RevisionStoreError::ResidencyMismatch);
        }
        if self.tagged_digest() != candidate.tagged_digest() {
            return Err(RevisionStoreError::ResidencyMismatch);
        }
        Ok(())
    }

    /// Refuses a collision: same UUID with different kind or body bytes.
    ///
    /// The same UUID under different domain kinds is a cross-kind collision
    /// and is refused even when the body bytes are identical.
    pub fn refuse_uuid_collision(&self, other: &Self) -> Result<(), RevisionStoreError> {
        if self.uuid() != other.uuid() {
            return Ok(());
        }
        if self.kind() != other.kind() {
            return Err(RevisionStoreError::ResidencyMismatch);
        }
        if self.canonical_body() != other.canonical_body() {
            return Err(RevisionStoreError::ResidencyMismatch);
        }
        Ok(())
    }

    /// Refuses a collision: same tagged digest with different canonical body bytes.
    pub fn refuse_digest_collision(&self, other: &Self) -> Result<(), RevisionStoreError> {
        if self.tagged_digest() != other.tagged_digest() {
            return Ok(());
        }
        if self.kind() != other.kind() {
            return Err(RevisionStoreError::ResidencyMismatch);
        }
        if self.canonical_body() != other.canonical_body() {
            return Err(RevisionStoreError::ResidencyMismatch);
        }
        Ok(())
    }
}

/// Builds the complete canonical residency bytes from six domain bindings.
///
/// Validates the full set: one binding per domain kind in canonical order,
/// cross-kind UUID uniqueness, exact digest/body agreement, and total size
/// cap of 8192 bytes.
pub fn residency_canonical_bytes(
    bindings: &[DomainBinding; 6],
) -> Result<Vec<u8>, RevisionStoreError> {
    // Validate exact domain order.
    for (index, binding) in bindings.iter().enumerate() {
        if binding.kind != DomainKind::ALL[index] {
            return Err(RevisionStoreError::ResidencyMismatch);
        }
    }
    // Enforce cross-kind UUID uniqueness.
    for i in 0..bindings.len() {
        for j in (i + 1)..bindings.len() {
            if bindings[i].uuid() == bindings[j].uuid() {
                return Err(RevisionStoreError::ResidencyMismatch);
            }
        }
    }
    // Enforce exact digest/body agreement.
    for binding in bindings {
        let expected = binding.tagged_digest;
        let candidate = DomainBinding::new(
            binding.kind,
            binding.uuid,
            binding.canonical_body.clone(),
        )?;
        if candidate.tagged_digest() != expected {
            return Err(RevisionStoreError::ResidencyMismatch);
        }
    }
    // Build the canonical CBOR array.
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
                search_contracts::BoundedBytes::new(binding.uuid().to_vec())
                    .map_err(|_| RevisionStoreError::ContractExhausted)?,
            ),
        ).map_err(|_| RevisionStoreError::ContractExhausted)?;
        object.insert(
            search_contracts::CanonicalKey::new_non_empty("body")
                .map_err(|_| RevisionStoreError::ContractExhausted)?,
            CanonicalValue::Bytes(
                search_contracts::BoundedBytes::new(binding.canonical_body().to_vec())
                    .map_err(|_| RevisionStoreError::ContractExhausted)?,
            ),
        ).map_err(|_| RevisionStoreError::ContractExhausted)?;
        object.insert(
            search_contracts::CanonicalKey::new_non_empty("tagged_digest")
                .map_err(|_| RevisionStoreError::ContractExhausted)?,
            CanonicalValue::Bytes(
                search_contracts::BoundedBytes::new(binding.tagged_digest().as_bytes().to_vec())
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
        .map(|bytes| {
            let out = bytes.as_slice().to_vec();
            if out.len() > 8192 {
                return Err(RevisionStoreError::ContractExhausted);
            }
            Ok(out)
        })?
}

/// Finite ceiling for the complete canonical residency byte block.
///
/// The encoder enforces the same ceiling on emission; this decoder enforces
/// it on retained readback so both sides agree exactly.
pub const MAX_RESIDENCY_CANONICAL_BYTES: usize = 8192;

/// Decodes retained canonical residency bytes into the six exact domain
/// bindings that produced them.
///
/// This is a readback path for metadata the root already persisted. It
/// reconstructs bindings only through the canonical owner (DomainBinding::new),
/// so an unchanged body under an unchanged UUID resolves to the same retained
/// kind, UUID and tagged digest across operations and process restarts. It
/// mints no UUID, issues no authority, and leaves the existing v1 byte layout
/// untouched.
///
/// The input must be at most MAX_RESIDENCY_CANONICAL_BYTES and must be exactly
/// the canonical CBOR array emitted by residency_canonical_bytes: six rows in
/// DomainKind::ALL order, each a map carrying exactly the keys kind, uuid, body
/// and tagged_digest. Unknown, missing or mistyped keys, malformed lengths,
/// non-v4 UUIDs, cross-row UUID or digest reuse, a declared digest that does
/// not recompute from its own kind/UUID/body, and bytes that do not re-encode
/// to themselves are all refused.
pub fn decode_residency_canonical(
    bytes: &[u8],
) -> Result<[DomainBinding; 6], RevisionStoreError> {
    if bytes.is_empty() || bytes.len() > MAX_RESIDENCY_CANONICAL_BYTES {
        return Err(RevisionStoreError::AddressInvalid);
    }
    let value = parse_canonical_cbor(bytes).map_err(|_| RevisionStoreError::AddressInvalid)?;
    let CanonicalValue::Array(rows) = value else {
        return Err(RevisionStoreError::AddressInvalid);
    };
    if rows.len() != DomainKind::ALL.len() {
        return Err(RevisionStoreError::AddressInvalid);
    }
    let mut bindings: Vec<DomainBinding> = Vec::with_capacity(DomainKind::ALL.len());
    for (kind, row) in DomainKind::ALL.iter().copied().zip(rows.iter()) {
        let binding = decode_domain_binding(row, kind)?;
        for existing in &bindings {
            existing.refuse_uuid_collision(&binding)?;
            existing.refuse_digest_collision(&binding)?;
        }
        bindings.push(binding);
    }
    let bindings: [DomainBinding; 6] = bindings
        .try_into()
        .map_err(|_| RevisionStoreError::AddressInvalid)?;
    if residency_canonical_bytes(&bindings)? != bytes {
        return Err(RevisionStoreError::AddressInvalid);
    }
    Ok(bindings)
}

/// Decodes one canonical residency row into a validated domain binding.
///
/// The row must be an object carrying exactly the four canonical keys, the
/// declared kind must be the expected positional kind, and the declared tagged
/// digest must recompute from the decoded kind, UUID and body.
fn decode_domain_binding(
    row: &CanonicalValue,
    expected_kind: DomainKind,
) -> Result<DomainBinding, RevisionStoreError> {
    let CanonicalValue::Object(fields) = row else {
        return Err(RevisionStoreError::AddressInvalid);
    };
    let mut kind: Option<DomainKind> = None;
    let mut uuid: Option<GenuineUuid> = None;
    let mut body: Option<Vec<u8>> = None;
    let mut digest: Option<Blake3Digest32> = None;
    for (key, field) in fields {
        match key.as_str() {
            "kind" => {
                if kind.is_some() {
                    return Err(RevisionStoreError::AddressInvalid);
                }
                let CanonicalValue::Text(label) = field else {
                    return Err(RevisionStoreError::AddressInvalid);
                };
                kind = Some(decode_domain_kind(label.as_str())?);
            }
            "uuid" => {
                if uuid.is_some() {
                    return Err(RevisionStoreError::AddressInvalid);
                }
                let CanonicalValue::Bytes(raw) = field else {
                    return Err(RevisionStoreError::AddressInvalid);
                };
                uuid = Some(
                    raw.as_slice()
                        .try_into()
                        .map_err(|_| RevisionStoreError::AddressInvalid)?,
                );
            }
            "body" => {
                if body.is_some() {
                    return Err(RevisionStoreError::AddressInvalid);
                }
                let CanonicalValue::Bytes(raw) = field else {
                    return Err(RevisionStoreError::AddressInvalid);
                };
                body = Some(raw.as_slice().to_vec());
            }
            "tagged_digest" => {
                if digest.is_some() {
                    return Err(RevisionStoreError::AddressInvalid);
                }
                let CanonicalValue::Bytes(raw) = field else {
                    return Err(RevisionStoreError::AddressInvalid);
                };
                let raw: [u8; 32] = raw
                    .as_slice()
                    .try_into()
                    .map_err(|_| RevisionStoreError::AddressInvalid)?;
                digest = Some(Blake3Digest32::from_bytes(raw));
            }
            _ => return Err(RevisionStoreError::AddressInvalid),
        }
    }
    let (Some(kind), Some(uuid), Some(body), Some(digest)) = (kind, uuid, body, digest) else {
        return Err(RevisionStoreError::AddressInvalid);
    };
    if kind != expected_kind {
        return Err(RevisionStoreError::ResidencyMismatch);
    }
    let binding = DomainBinding::new(kind, uuid, body)?;
    if binding.tagged_digest() != digest {
        return Err(RevisionStoreError::ResidencyMismatch);
    }
    Ok(binding)
}

/// Maps a wire domain label back to its closed domain kind.
fn decode_domain_kind(label: &str) -> Result<DomainKind, RevisionStoreError> {
    DomainKind::ALL
        .iter()
        .copied()
        .find(|kind| kind.as_str() == label)
        .ok_or(RevisionStoreError::AddressInvalid)
}

/// Validates that bytes are canonical CBOR by round-tripping through the
/// canonical CBOR parser and encoder.
fn is_valid_canonical_cbor(bytes: &[u8]) -> bool {
    match parse_canonical_cbor(bytes) {
        Ok(value) => {
            match search_contracts::to_canonical_cbor(&value) {
                Ok(reencoded) => reencoded.as_slice() == bytes,
                Err(_) => false,
            }
        }
        Err(_) => false,
    }
}

/// Validates RFC 4122 version-4 UUID: version nibble is 4 and variant bits
/// are 10 (0x80-0xBF for byte 8).
fn is_rfc4122_v4(uuid: GenuineUuid) -> bool {
    (uuid[6] >> 4) == 0x4 && (uuid[8] & 0xC0) == 0x80
}

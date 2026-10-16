//! Closed selected-object metadata; encrypted bytes remain in the C07 object.

use core::fmt;
use search_contracts::{self as c, CanonicalValue as V};
use super::{DomainBinding, ObjectMetadataBinding, PreSealDescriptor, TypedSourceIdentity,
    decode_residency_canonical, residency_canonical_bytes};
use super::super::{CipherSuite, EncryptionBinding, RevisionStoreError as Error, DEFAULT_REVISION_STORE_LIMITS, MIN_CIPHERTEXT_BYTES};

const PROFILE: &str = "eliot.selected-object-metadata.v1";
/// Complete canonical selected-object metadata ceiling, excluding the object bytes.
pub const MAX_PERSISTED_SELECTED_OBJECT_BYTES: usize = 8192;

fn bytes(value: &[u8]) -> Result<V, Error> {
    c::BoundedBytes::new(value.to_vec()).map(V::Bytes).map_err(|_| Error::ContractExhausted)
}
fn text(value: &str) -> Result<V, Error> {
    c::CanonicalText::new(value).map(V::Text).map_err(|_| Error::ContractExhausted)
}
fn array(values: Vec<V>) -> Result<V, Error> {
    c::BoundedList::new(values).map(V::Array).map_err(|_| Error::ContractExhausted)
}
fn key(domains: &[DomainBinding; 6], digest: c::Blake3Digest32) -> Result<c::SearchObjectResidencyKey, Error> {
    residency_canonical_bytes(domains)?;
    Ok(c::SearchObjectResidencyKey {
        scope_domain_id: c::ScopeDomainId::from_bytes(domains[0].uuid()),
        access_domain_id: c::AccessDomainId::from_bytes(domains[1].uuid()),
        confidentiality_domain_id: c::ConfidentialityDomainId::from_bytes(domains[2].uuid()),
        encryption_key_domain_id: c::EncryptionKeyDomainId::from_bytes(domains[3].uuid()),
        retention_domain_id: c::RetentionDomainId::from_bytes(domains[4].uuid()),
        erasure_domain_id: c::ErasureDomainId::from_bytes(domains[5].uuid()),
        versioned_content_digest: c::VersionedContentDigest::from_stored_bytes(c::DigestAlgorithm::Blake3_256, *digest.as_bytes()),
    })
}

/// Hashes every field of the complete key, including the digest algorithm tag.
/// This physical CAS identity allocates no source or domain UUID.
///
/// # Panics
/// Panics if compiled-in canonical limits cease to admit this fixed-size shape.
pub fn derive_object_residency_key_digest(key: &c::SearchObjectResidencyKey) -> c::ObjectResidencyKeyDigest {
    let algorithm = match key.versioned_content_digest.algorithm() {
        c::DigestAlgorithm::Blake3_256 => "blake3_256", c::DigestAlgorithm::Sha256 => "sha256",
    };
    // Every value below has a fixed, statically admitted bound.
    let value = array(vec![V::U64(1), bytes(key.scope_domain_id.as_bytes()).expect("UUID"),
        bytes(key.access_domain_id.as_bytes()).expect("UUID"), bytes(key.confidentiality_domain_id.as_bytes()).expect("UUID"),
        bytes(key.encryption_key_domain_id.as_bytes()).expect("UUID"), bytes(key.retention_domain_id.as_bytes()).expect("UUID"),
        bytes(key.erasure_domain_id.as_bytes()).expect("UUID"), text(algorithm).expect("static algorithm"),
        bytes(key.versioned_content_digest.as_bytes()).expect("digest")]).expect("fixed key shape");
    let digest = c::blake3_canonical(&c::CanonicalDigestDomain::parse("eliot/cbor/selected-object-residency-key/v1")
        .expect("static domain"), &value, c::DigestInputLimit::new(1024).expect("fixed bound")).expect("fixed key bound");
    c::ObjectResidencyKeyDigest::from_bytes(*digest.as_bytes())
}

/// Canonical opaque identity of one selected-v2 physical object.
pub fn selected_object_storage_id(physical: c::ObjectResidencyKeyDigest) -> Result<c::OpaqueId, Error> {
    c::OpaqueId::new(format!("selected-source-v2:{physical}")).map_err(|_| Error::AddressInvalid)
}

impl ObjectMetadataBinding {
    /// Complete physical key from the genuine domain IDs and owned object digest.
    pub fn object_residency_key(&self) -> Result<c::SearchObjectResidencyKey, Error> {
        key(self.preseal.domain_bindings(), self.payload().ciphertext_digest())
    }
}

/// Finite immutable object link. Decoding supplies metadata, never access authority.
#[derive(Clone, Eq, PartialEq)]
pub struct PersistedSelectedObjectMetadata {
    preseal: PreSealDescriptor,
    object: c::OpaqueId,
    encryption: EncryptionBinding,
    nonce: Vec<u8>,
    ciphertext_digest: c::Blake3Digest32,
    ciphertext_bytes: u64,
    physical: c::ObjectResidencyKeyDigest,
}
impl fmt::Debug for PersistedSelectedObjectMetadata {
    fn fmt(&self, out: &mut fmt::Formatter<'_>) -> fmt::Result { out.write_str("PersistedSelectedObjectMetadata([REDACTED])") }
}

impl PersistedSelectedObjectMetadata {
    /// Derives all object values from the owned encrypted payload.
    pub fn from_binding(binding: &ObjectMetadataBinding) -> Result<Self, Error> {
        let payload = binding.payload();
        if binding.preseal.content_digest != payload.plaintext_digest
            || binding.preseal.content_bytes != payload.plaintext_bytes || binding.encryption != payload.encryption {
            return Err(Error::EnvelopeInvalid);
        }
        let physical = derive_object_residency_key_digest(&binding.object_residency_key()?);
        let value = Self { preseal: binding.preseal.clone(), object: binding.storage_object_id.clone(),
            encryption: binding.encryption.clone(), nonce: payload.nonce().to_vec(), ciphertext_digest: payload.ciphertext_digest(),
            ciphertext_bytes: payload.ciphertext_len() as u64, physical };
        value.validate()?;
        value.encode()?;
        Ok(value)
    }
    fn validate(&self) -> Result<(), Error> {
        for id in [self.preseal.source.namespace_id.as_bytes(), self.preseal.source.source_id.as_bytes(),
            self.preseal.source.revision_id.as_bytes()] {
            if id[6] & 0xf0 != 0x40 || id[8] & 0xc0 != 0x80 { return Err(Error::AddressInvalid); }
        }
        if self.nonce.len() != 12 || self.preseal.content_bytes > DEFAULT_REVISION_STORE_LIMITS.max_plaintext_bytes
            || self.ciphertext_bytes > DEFAULT_REVISION_STORE_LIMITS.max_ciphertext_bytes
            || self.ciphertext_bytes < MIN_CIPHERTEXT_BYTES as u64
            || self.preseal.content_bytes.checked_add(16).is_none_or(|minimum| self.ciphertext_bytes < minimum)
            || self.object != selected_object_storage_id(self.physical)?
            || residency_canonical_bytes(&self.preseal.domain_bindings)? != self.preseal.residency_canonical
            || derive_object_residency_key_digest(&key(&self.preseal.domain_bindings, self.ciphertext_digest)?) != self.physical
        { return Err(Error::ReadbackMismatch); }
        Ok(())
    }
    /// Serializes exactly one closed, versioned shape, with no encrypted bytes.
    pub fn encode(&self) -> Result<Vec<u8>, Error> {
        self.validate()?;
        let value = array(vec![text(PROFILE)?, V::U64(1), bytes(self.source().namespace_id.as_bytes())?,
            bytes(self.source().source_id.as_bytes())?, bytes(self.source().revision_id.as_bytes())?,
            V::U64(self.source().sequence.get()), text(self.operation_id().as_str())?, text(self.object.as_str())?,
            bytes(self.content_digest().as_bytes())?, V::U64(self.content_bytes()), bytes(self.preseal.residency_canonical())?,
            text(self.encryption.key_reference.as_str())?, V::U64(self.encryption.key_version.get()), V::U64(1),
            bytes(&self.nonce)?, bytes(self.ciphertext_digest.as_bytes())?, V::U64(self.ciphertext_bytes), bytes(self.physical.as_bytes())?])?;
        let encoded = c::to_canonical_cbor(&value).map_err(|_| Error::ContractExhausted)?.into_vec();
        if encoded.len() > MAX_PERSISTED_SELECTED_OBJECT_BYTES { return Err(Error::ContractExhausted); }
        Ok(encoded)
    }
    /// Refuses foreign shapes, IDs, domains, physical identities and noncanonical data.
    pub fn decode(encoded: &[u8]) -> Result<Self, Error> {
        if encoded.is_empty() || encoded.len() > MAX_PERSISTED_SELECTED_OBJECT_BYTES { return Err(Error::AddressInvalid); }
        let V::Array(fields) = c::parse_canonical_cbor(encoded).map_err(|_| Error::AddressInvalid)? else { return Err(Error::AddressInvalid); };
        let [V::Text(profile), V::U64(1), V::Bytes(ns), V::Bytes(source), V::Bytes(revision), V::U64(sequence),
            V::Text(operation), V::Text(object), V::Bytes(content), V::U64(content_bytes), V::Bytes(residency),
            V::Text(key_reference), V::U64(key_version), V::U64(1), V::Bytes(nonce), V::Bytes(ciphertext),
            V::U64(ciphertext_bytes), V::Bytes(physical)] = fields.as_slice() else { return Err(Error::AddressInvalid); };
        if profile.as_str() != PROFILE { return Err(Error::AddressInvalid); }
        let domains = decode_residency_canonical(residency.as_slice())?;
        let preseal = PreSealDescriptor::new(TypedSourceIdentity::new(
            c::SourceNamespaceId::from_bytes(ns.as_slice().try_into().map_err(|_| Error::AddressInvalid)?),
            c::SourceId::from_bytes(source.as_slice().try_into().map_err(|_| Error::AddressInvalid)?),
            c::SourceRevisionId::from_bytes(revision.as_slice().try_into().map_err(|_| Error::AddressInvalid)?),
            c::NonZeroRevision::new(*sequence).map_err(|_| Error::AddressInvalid)?), domains,
            c::Blake3Digest32::from_bytes(content.as_slice().try_into().map_err(|_| Error::AddressInvalid)?), *content_bytes,
            c::OpaqueId::new(operation.as_str()).map_err(|_| Error::AddressInvalid)?)?;
        let metadata = Self { preseal, object: c::OpaqueId::new(object.as_str()).map_err(|_| Error::AddressInvalid)?,
            encryption: EncryptionBinding { key_reference: c::OpaqueId::new(key_reference.as_str()).map_err(|_| Error::AddressInvalid)?,
                key_version: c::NonZeroRevision::new(*key_version).map_err(|_| Error::AddressInvalid)?, cipher_suite: CipherSuite::AuthenticatedEncryptionV1 },
            nonce: nonce.as_slice().to_vec(), ciphertext_digest: c::Blake3Digest32::from_bytes(ciphertext.as_slice().try_into().map_err(|_| Error::AddressInvalid)?),
            ciphertext_bytes: *ciphertext_bytes, physical: c::ObjectResidencyKeyDigest::from_bytes(physical.as_slice().try_into().map_err(|_| Error::AddressInvalid)?) };
        metadata.validate()?;
        if metadata.encode()? != encoded { return Err(Error::AddressInvalid); }
        Ok(metadata)
    }
    /// Exact typed source occurrence.
    pub const fn source(&self) -> &TypedSourceIdentity { &self.preseal.source }
    /// Complete original domain meanings and genuine identities.
    pub const fn domain_bindings(&self) -> &[DomainBinding; 6] { &self.preseal.domain_bindings }
    /// Exact plaintext BLAKE3 from the source owner.
    pub const fn content_digest(&self) -> c::Blake3Digest32 { self.preseal.content_digest }
    /// Exact source length.
    pub const fn content_bytes(&self) -> u64 { self.preseal.content_bytes }
    /// Original retention operation, separate from admission.
    pub const fn operation_id(&self) -> &c::OpaqueId { &self.preseal.operation_id }
    /// Exact physical CAS object identity.
    pub const fn storage_object_id(&self) -> &c::OpaqueId { &self.object }
    /// Complete key reference and cipher profile, without key material.
    pub const fn encryption(&self) -> &EncryptionBinding { &self.encryption }
    /// Exact nonce retained beside the object link.
    pub fn nonce(&self) -> &[u8] { &self.nonce }
    /// BLAKE3 of the actual encrypted object bytes.
    pub const fn ciphertext_digest(&self) -> c::Blake3Digest32 { self.ciphertext_digest }
    /// Exact encrypted object size.
    pub const fn ciphertext_bytes(&self) -> u64 { self.ciphertext_bytes }
    /// Complete domain-qualified physical key digest.
    pub const fn object_residency_key_digest(&self) -> c::ObjectResidencyKeyDigest { self.physical }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn uuid(seed: u8) -> [u8; 16] {
        let mut value = [seed; 16]; value[6] = (value[6] & 0x0f) | 0x40; value[8] = (value[8] & 0x3f) | 0x80; value
    }
    fn binding() -> ObjectMetadataBinding {
        let domains = std::array::from_fn(|index| DomainBinding::new(super::super::DomainKind::ALL[index],
            uuid(index as u8 + 1), c::to_canonical_cbor(&V::U64(index as u64)).unwrap().into_vec()).unwrap());
        let encryption = EncryptionBinding { key_reference: c::OpaqueId::new("test-only-selected-key").unwrap(),
            key_version: c::NonZeroRevision::new(1).unwrap(), cipher_suite: CipherSuite::AuthenticatedEncryptionV1 };
        let payload = super::super::super::EncryptedRevisionPayload::new_selected_v2(c::Blake3Digest32::from_bytes([5; 32]),
            8, vec![0; 12], vec![7; 64], encryption.clone(), DEFAULT_REVISION_STORE_LIMITS).unwrap();
        let physical = derive_object_residency_key_digest(&key(&domains, payload.ciphertext_digest()).unwrap());
        let source = TypedSourceIdentity::new(c::SourceNamespaceId::from_bytes(uuid(10)), c::SourceId::from_bytes(uuid(11)),
            c::SourceRevisionId::from_bytes(uuid(12)), c::NonZeroRevision::new(1).unwrap());
        ObjectMetadataBinding::new(PreSealDescriptor::new(source, domains, payload.plaintext_digest, 8,
            c::OpaqueId::new("test-only-retention").unwrap()).unwrap(), selected_object_storage_id(physical).unwrap(), payload, encryption).unwrap()
    }

    #[test]
    fn selected_metadata_roundtrip_is_derived_and_does_not_embed_object_bytes() {
        let binding = binding();
        let metadata = PersistedSelectedObjectMetadata::from_binding(&binding).unwrap();
        let encoded = metadata.encode().unwrap();
        assert_eq!(PersistedSelectedObjectMetadata::decode(&encoded).unwrap(), metadata);
        assert_eq!(metadata.ciphertext_digest(), binding.payload().ciphertext_digest());
        assert_eq!(metadata.ciphertext_bytes(), binding.payload().ciphertext_len() as u64);
        assert!(!encoded.windows(binding.payload().ciphertext_len()).any(|bytes| bytes == binding.payload().ciphertext()));
        let mut other = binding.object_residency_key().unwrap();
        other.versioned_content_digest = c::VersionedContentDigest::from_stored_bytes(c::DigestAlgorithm::Sha256,
            *other.versioned_content_digest.as_bytes());
        assert_ne!(derive_object_residency_key_digest(&other), metadata.object_residency_key_digest());
    }

    #[test]
    fn selected_metadata_rejects_unknown_shape_identity_and_physical_tampering() {
        let encoded = PersistedSelectedObjectMetadata::from_binding(&binding()).unwrap().encode().unwrap();
        let V::Array(fields) = c::parse_canonical_cbor(&encoded).unwrap() else { panic!("array"); };
        for index in [2, 5, 13, 17, 18] {
            let mut changed = fields.clone().into_vec();
            match index {
                2 => changed[index] = bytes(&[0; 16]).unwrap(),
                5 => changed[index] = V::U64(0),
                13 => changed[index] = V::U64(2),
                17 => changed[index] = bytes(&[0; 32]).unwrap(),
                _ => changed.push(V::Null),
            }
            let altered = c::to_canonical_cbor(&array(changed).unwrap()).unwrap();
            assert!(PersistedSelectedObjectMetadata::decode(altered.as_slice()).is_err());
        }
    }
}

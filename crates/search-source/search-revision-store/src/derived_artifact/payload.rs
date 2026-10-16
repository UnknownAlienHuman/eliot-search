//! Explicit derived AAD/envelope and unqualified stored-byte candidates.
use super::{
    DerivedArtifactDescriptorV1 as Descriptor, DerivedArtifactErrorV1 as Error,
    DerivedArtifactKindV1 as Kind, DerivedArtifactQualificationV1, MAX_DERIVED_AAD_BYTES,
    MAX_DERIVED_CURRENT_BYTES, codec as wire,
};
use crate::{CipherSuite, EncryptionBinding};
use core::fmt;
use search_contracts::{self as c, CanonicalValue as V};
use zeroize::Zeroizing;

/// Distinct derived envelope identity. Raw ELSREV01/ELSREV02 are never decoded here.
pub const DERIVED_ARTIFACT_ENVELOPE_PROFILE_V1: &str = "eliot.derived-artifact-envelope.v1";
/// Distinct canonical derived associated-data profile.
pub const DERIVED_ARTIFACT_AAD_PROFILE_V1: &str = "eliot.derived-artifact-aad.v1";
/// Existing selected-reference algorithm identity; C07 implements no cipher.
pub const DERIVED_ARTIFACT_CIPHER_PROFILE_V1: &str = "aes-256-gcm/tag128";
/// Existing nonce/accounting policy identity; C07 never calculates a nonce or counter.
pub const DERIVED_ARTIFACT_NONCE_PROFILE_V1: &str =
    "zero32-counter64/attempts32-blocks32-bytes36/v1";
const MAGIC: &[u8; 8] = b"ELSDRV01";
const HEADER: usize = 8 + 2 + 4 + 8 + 12;

/// Exact pre-effect AAD metadata, without key/counter/permit authority.
#[derive(Clone, Eq, PartialEq)]
pub struct DerivedArtifactSealBindingV1 {
    descriptor: Descriptor,
    current: Vec<u8>,
    encryption: EncryptionBinding,
    counter: u64,
    nonce: [u8; 12],
    aad: Vec<u8>,
}
impl fmt::Debug for DerivedArtifactSealBindingV1 {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str("DerivedArtifactSealBindingV1(Unqualified)")
    }
}
impl DerivedArtifactSealBindingV1 {
    /// Record actual owner-selected Current/key/counter/nonce metadata for C02
    /// reservation. Framing is checked; only the actual Crypto/Windows owner can
    /// verify selection, nonce policy, fresh permit consumption and authentication.
    pub fn new(
        descriptor: Descriptor,
        current_canonical: &[u8],
        encryption: EncryptionBinding,
        counter: u64,
        nonce: [u8; 12],
    ) -> Result<Self, Error> {
        wire::parse(current_canonical, MAX_DERIVED_CURRENT_BYTES)?;
        if encryption.cipher_suite != CipherSuite::AuthenticatedEncryptionV1 {
            return Err(Error::ProfileInvalid);
        }
        let value = wire::array(vec![
            wire::text(DERIVED_ARTIFACT_AAD_PROFILE_V1)?,
            V::U64(1),
            wire::text(DERIVED_ARTIFACT_ENVELOPE_PROFILE_V1)?,
            wire::text(DERIVED_ARTIFACT_CIPHER_PROFILE_V1)?,
            wire::text(DERIVED_ARTIFACT_NONCE_PROFILE_V1)?,
            wire::bytes(&descriptor.encode()?)?,
            wire::bytes(current_canonical)?,
            wire::text(encryption.key_reference.as_str())?,
            V::U64(encryption.key_version.get()),
            V::U64(counter),
            wire::bytes(&nonce)?,
        ])?;
        let aad = wire::encode(&value, MAX_DERIVED_AAD_BYTES)?;
        Ok(Self {
            descriptor,
            current: current_canonical.to_vec(),
            encryption,
            counter,
            nonce,
            aad,
        })
    }
    /// Exact canonical AAD bytes to reserve and authenticate, not a digest alias.
    pub fn aad_bytes(&self) -> &[u8] {
        &self.aad
    }
    /// Exact derived subject, independent of source-history occurrences.
    pub const fn descriptor(&self) -> &Descriptor {
        &self.descriptor
    }
    /// Complete selected Current record, to compare with the actual C02/key owners.
    pub fn selected_current_canonical(&self) -> &[u8] {
        &self.current
    }
    /// Owner-supplied exact key reference/version and existing algorithm binding.
    pub const fn encryption(&self) -> &EncryptionBinding {
        &self.encryption
    }
    /// Externally reserved counter; recording it does not grant use.
    pub const fn counter(&self) -> u64 {
        self.counter
    }
    /// Externally supplied nonce; actual policy checking belongs to Crypto/Windows.
    pub const fn nonce(&self) -> &[u8; 12] {
        &self.nonce
    }
    /// Exact existing GCM charge, for comparison with the actual C02 reservation.
    pub fn authenticated_blocks(&self) -> Result<u64, Error> {
        self.descriptor
            .encoded_body_bytes()
            .div_ceil(16)
            .checked_add((self.aad.len() as u64).div_ceil(16))
            .and_then(|n| n.checked_add(1))
            .ok_or(Error::BudgetExceeded)
    }
    pub(super) fn decode(encoded: &[u8]) -> Result<Self, Error> {
        let V::Array(fields) = wire::parse(encoded, MAX_DERIVED_AAD_BYTES)? else {
            return Err(Error::ProfileInvalid);
        };
        let [
            V::Text(profile),
            V::U64(1),
            V::Text(envelope),
            V::Text(cipher),
            V::Text(nonce_profile),
            V::Bytes(descriptor),
            V::Bytes(current),
            V::Text(key),
            V::U64(version),
            V::U64(counter),
            V::Bytes(nonce),
        ] = fields.as_slice()
        else {
            return Err(Error::ProfileInvalid);
        };
        if profile.as_str() != DERIVED_ARTIFACT_AAD_PROFILE_V1
            || envelope.as_str() != DERIVED_ARTIFACT_ENVELOPE_PROFILE_V1
            || cipher.as_str() != DERIVED_ARTIFACT_CIPHER_PROFILE_V1
            || nonce_profile.as_str() != DERIVED_ARTIFACT_NONCE_PROFILE_V1
        {
            return Err(Error::ProfileInvalid);
        }
        let value = Self::new(
            Descriptor::decode(descriptor.as_slice())?,
            current.as_slice(),
            EncryptionBinding {
                key_reference: c::OpaqueId::new(key.as_str()).map_err(|_| Error::ProfileInvalid)?,
                key_version: c::NonZeroRevision::new(*version)
                    .map_err(|_| Error::ProfileInvalid)?,
                cipher_suite: CipherSuite::AuthenticatedEncryptionV1,
            },
            *counter,
            nonce
                .as_slice()
                .try_into()
                .map_err(|_| Error::ProfileInvalid)?,
        )?;
        if value.aad_bytes() != encoded {
            return Err(Error::Mismatch);
        }
        Ok(value)
    }
}

/// Stored format is explicit and independent of the semantic artifact kind.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum DerivedArtifactStorageV1 {
    /// Actual owner-verified body-free metadata; root policy must permit plaintext.
    MetadataOnly,
    /// Distinct derived envelope containing existing AES256-GCM ciphertext/tag.
    Aes256GcmEnvelope,
}

/// Residency-aware CAS address for exact stored bytes, never a raw source UUID.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct DerivedArtifactAddressV1 {
    kind: Kind,
    residency: c::ObjectResidencyKeyDigest,
    stored_digest: c::Blake3Digest32,
}
impl DerivedArtifactAddressV1 {
    fn derive(descriptor: &Descriptor, stored_digest: c::Blake3Digest32) -> Result<Self, Error> {
        let residency = crate::residency_canonical_bytes(descriptor.domains())
            .map_err(|_| Error::ResidencyMismatch)?;
        let key = wire::array(vec![
            V::U64(1),
            wire::bytes(&residency)?,
            wire::text("blake3-256")?,
            wire::bytes(stored_digest.as_bytes())?,
        ])?;
        let digest = wire::tagged(
            "eliot/cbor/derived-artifact-residency-key/v1",
            &key,
            16 * 1024,
        )?;
        Ok(Self {
            kind: descriptor.kind(),
            residency: c::ObjectResidencyKeyDigest::from_bytes(*digest.as_bytes()),
            stored_digest,
        })
    }
    /// Digest over every complete semantic domain binding and stored-byte identity.
    pub const fn residency_key_digest(&self) -> c::ObjectResidencyKeyDigest {
        self.residency
    }
    /// Direct BLAKE3 of the exact bytes on disk, including the complete envelope if sealed.
    pub const fn stored_digest(&self) -> c::Blake3Digest32 {
        self.stored_digest
    }
    /// Closed derived kind, never a raw revision envelope or selected-source-v2.
    pub const fn kind(&self) -> Kind {
        self.kind
    }
    /// Relative location beneath the caller's actual admitted CAS root.
    pub fn relative_path(&self) -> String {
        let [residency, kind, shard] = self.components();
        format!("{residency}/{kind}/{shard}/{}", self.stored_digest)
    }
    pub(super) fn components(&self) -> [String; 3] {
        [
            self.residency.to_string(),
            format!("derived-artifact-v1-{}", self.kind.as_str()),
            self.stored_digest.to_string()[..2].to_owned(),
        ]
    }
}

/// Exact stored-object reference metadata. Decode supplies no durability receipt.
#[derive(Clone, Eq, PartialEq)]
pub struct DerivedArtifactObjectRefV1 {
    pub(super) descriptor: Descriptor,
    pub(super) storage: DerivedArtifactStorageV1,
    pub(super) stored_digest: c::Blake3Digest32,
    pub(super) stored_bytes: u64,
}
impl fmt::Debug for DerivedArtifactObjectRefV1 {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str("DerivedArtifactObjectRefV1(Unqualified)")
    }
}
impl DerivedArtifactObjectRefV1 {
    /// Exact immutable descriptor with source provenance and actual encoded-body identity.
    pub const fn descriptor(&self) -> &Descriptor {
        &self.descriptor
    }
    /// Explicit metadata/encrypted storage shape.
    pub const fn storage(&self) -> DerivedArtifactStorageV1 {
        self.storage
    }
    /// Exact encoded on-disk object length, not plaintext body length.
    pub const fn stored_bytes(&self) -> u64 {
        self.stored_bytes
    }
    /// Direct digest of the exact stored metadata/encrypted envelope bytes.
    /// It is never substituted for the plaintext body or owner semantic digest.
    pub const fn stored_object_digest(&self) -> c::Blake3Digest32 {
        self.stored_digest
    }
    /// Physical address calculated from full residency and actual stored-byte digest.
    pub fn address(&self) -> Result<DerivedArtifactAddressV1, Error> {
        DerivedArtifactAddressV1::derive(&self.descriptor, self.stored_digest)
    }
    /// Canonical reference metadata; no source body or durable/serving receipt.
    pub fn encode(&self) -> Result<Vec<u8>, Error> {
        wire::encode(
            &wire::array(vec![
                wire::text("eliot.derived-artifact-reference.v1")?,
                V::U64(1),
                wire::bytes(&self.descriptor.encode()?)?,
                wire::text(match self.storage {
                    DerivedArtifactStorageV1::MetadataOnly => "metadata-only",
                    DerivedArtifactStorageV1::Aes256GcmEnvelope => "aes256-gcm-envelope-v1",
                })?,
                wire::bytes(self.stored_digest.as_bytes())?,
                V::U64(self.stored_bytes),
            ])?,
            super::MAX_DERIVED_DESCRIPTOR_BYTES + 512,
        )
    }
    /// Decode equality metadata only; actual I/O and owner verification remain mandatory.
    pub fn decode(encoded: &[u8]) -> Result<Self, Error> {
        let V::Array(fields) = wire::parse(encoded, super::MAX_DERIVED_DESCRIPTOR_BYTES + 512)?
        else {
            return Err(Error::ProfileInvalid);
        };
        let [
            V::Text(profile),
            V::U64(1),
            V::Bytes(descriptor),
            V::Text(storage),
            V::Bytes(digest),
            V::U64(length),
        ] = fields.as_slice()
        else {
            return Err(Error::ProfileInvalid);
        };
        if profile.as_str() != "eliot.derived-artifact-reference.v1" {
            return Err(Error::ProfileInvalid);
        }
        let descriptor = Descriptor::decode(descriptor.as_slice())?;
        let storage = match storage.as_str() {
            "metadata-only" => DerivedArtifactStorageV1::MetadataOnly,
            "aes256-gcm-envelope-v1" => DerivedArtifactStorageV1::Aes256GcmEnvelope,
            _ => return Err(Error::ProfileInvalid),
        };
        if storage == DerivedArtifactStorageV1::MetadataOnly
            && !descriptor.kind().is_metadata_only()
        {
            return Err(Error::PlaintextBodyForbidden);
        }
        if storage == DerivedArtifactStorageV1::MetadataOnly
            && (*length != descriptor.encoded_body_bytes()
                || wire::digest(digest.as_slice())? != descriptor.encoded_body_digest())
        {
            return Err(Error::Mismatch);
        }
        if storage == DerivedArtifactStorageV1::Aes256GcmEnvelope {
            let minimum = descriptor
                .encoded_body_bytes()
                .checked_add(HEADER as u64 + 17)
                .ok_or(Error::BudgetExceeded)?;
            let maximum = descriptor
                .encoded_body_bytes()
                .checked_add((HEADER + 16 + MAX_DERIVED_AAD_BYTES) as u64)
                .ok_or(Error::BudgetExceeded)?;
            if *length < minimum || *length > maximum {
                return Err(Error::Mismatch);
            }
        }
        let value = Self {
            descriptor,
            storage,
            stored_digest: wire::digest(digest.as_slice())?,
            stored_bytes: *length,
        };
        if value.encode()? != encoded {
            return Err(Error::Mismatch);
        }
        value.address()?;
        Ok(value)
    }
}

/// Owned candidate bytes. There is no public constructor for a verified object or receipt.
pub struct DerivedArtifactPayloadCandidateV1 {
    pub(super) reference: DerivedArtifactObjectRefV1,
    pub(super) encoded: Zeroizing<Vec<u8>>,
}
impl fmt::Debug for DerivedArtifactPayloadCandidateV1 {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str("DerivedArtifactPayloadCandidateV1(Unqualified)")
    }
}
impl DerivedArtifactPayloadCandidateV1 {
    /// Admit exact candidate technical bytes. Actual owner schema/profile and absence
    /// of source/representation bodies must be verified before any file write.
    pub fn metadata(
        descriptor: Descriptor,
        encoded: Vec<u8>,
        maximum_bytes: usize,
    ) -> Result<Self, Error> {
        let encoded = Zeroizing::new(encoded);
        if !descriptor.kind().is_metadata_only() {
            return Err(Error::PlaintextBodyForbidden);
        }
        descriptor.verify_body(&encoded, maximum_bytes)?;
        Ok(Self {
            reference: DerivedArtifactObjectRefV1 {
                stored_digest: descriptor.encoded_body_digest(),
                stored_bytes: descriptor.encoded_body_bytes(),
                descriptor,
                storage: DerivedArtifactStorageV1::MetadataOnly,
            },
            encoded,
        })
    }
    /// Frame an UNQUALIFIED ciphertext/tag from the actual crypto producer.
    /// The supplied bytes are not accepted as encrypted until the verifier opens
    /// them against real Current/C02/nonce/AAD owners and C07 checks the plaintext.
    pub fn sealed(
        binding: &DerivedArtifactSealBindingV1,
        ciphertext_and_tag: Vec<u8>,
        maximum_bytes: usize,
    ) -> Result<Self, Error> {
        let ciphertext_and_tag = Zeroizing::new(ciphertext_and_tag);
        if ciphertext_and_tag.len() as u64
            != binding
                .descriptor
                .encoded_body_bytes()
                .checked_add(16)
                .ok_or(Error::BudgetExceeded)?
        {
            return Err(Error::Mismatch);
        }
        let size = HEADER
            .checked_add(binding.aad.len())
            .and_then(|n| n.checked_add(ciphertext_and_tag.len()))
            .ok_or(Error::BudgetExceeded)?;
        if size > maximum_bytes {
            return Err(Error::BudgetExceeded);
        }
        let mut encoded = Zeroizing::new(Vec::new());
        encoded
            .try_reserve_exact(size)
            .map_err(|_| Error::BudgetExceeded)?;
        encoded.extend_from_slice(MAGIC);
        encoded.extend_from_slice(&1_u16.to_be_bytes());
        encoded.extend_from_slice(
            &u32::try_from(binding.aad.len())
                .map_err(|_| Error::BudgetExceeded)?
                .to_be_bytes(),
        );
        encoded.extend_from_slice(&(ciphertext_and_tag.len() as u64).to_be_bytes());
        encoded.extend_from_slice(&binding.nonce);
        encoded.extend_from_slice(binding.aad_bytes());
        encoded.extend_from_slice(&ciphertext_and_tag);
        Ok(Self {
            reference: DerivedArtifactObjectRefV1 {
                descriptor: binding.descriptor.clone(),
                storage: DerivedArtifactStorageV1::Aes256GcmEnvelope,
                stored_digest: wire::hash_bytes(&encoded, maximum_bytes)?,
                stored_bytes: size as u64,
            },
            encoded,
        })
    }
    /// Always unqualified until the actual owner port and exact I/O succeed.
    pub const fn qualification(&self) -> DerivedArtifactQualificationV1 {
        DerivedArtifactQualificationV1::Unqualified
    }
    /// Candidate equality reference, never a caller-minted durability receipt.
    pub const fn object_ref(&self) -> &DerivedArtifactObjectRefV1 {
        &self.reference
    }
    /// Encoded metadata or sealed envelope; this API never returns a plaintext body candidate.
    pub fn encoded_object(&self) -> &[u8] {
        &self.encoded
    }
}

pub(super) fn open_frame(encoded: &[u8]) -> Result<(DerivedArtifactSealBindingV1, &[u8]), Error> {
    if encoded.len() < HEADER || &encoded[..8] != MAGIC || encoded[8..10] != 1_u16.to_be_bytes() {
        return Err(Error::ProfileInvalid);
    }
    let aad_bytes = u32::from_be_bytes(
        encoded[10..14]
            .try_into()
            .map_err(|_| Error::ProfileInvalid)?,
    ) as usize;
    let ciphertext_bytes = usize::try_from(u64::from_be_bytes(
        encoded[14..22]
            .try_into()
            .map_err(|_| Error::ProfileInvalid)?,
    ))
    .map_err(|_| Error::BudgetExceeded)?;
    let end = HEADER.checked_add(aad_bytes).ok_or(Error::BudgetExceeded)?;
    if aad_bytes > MAX_DERIVED_AAD_BYTES || end.checked_add(ciphertext_bytes) != Some(encoded.len())
    {
        return Err(Error::Mismatch);
    }
    let binding = DerivedArtifactSealBindingV1::decode(&encoded[HEADER..end])?;
    if encoded[22..HEADER] != binding.nonce
        || ciphertext_bytes as u64
            != binding
                .descriptor
                .encoded_body_bytes()
                .checked_add(16)
                .ok_or(Error::BudgetExceeded)?
    {
        return Err(Error::Mismatch);
    }
    Ok((binding, &encoded[end..]))
}

/// Untrusted plaintext observation returned by the actual crypto opener.
/// C07 independently checks its exact bytes/digest and never writes this buffer.
pub struct DerivedArtifactPlaintextV1(Zeroizing<Vec<u8>>);
impl DerivedArtifactPlaintextV1 {
    /// Transfer a plaintext buffer into mandatory zeroizing ownership; no receipt.
    pub fn new(bytes: Vec<u8>) -> Self {
        Self(Zeroizing::new(bytes))
    }
    /// Exact observed plaintext, borrowed only for bounded C07 comparison/hash.
    pub fn as_bytes(&self) -> &[u8] {
        &self.0
    }
}
impl fmt::Debug for DerivedArtifactPlaintextV1 {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str("DerivedArtifactPlaintextV1(<redacted>)")
    }
}

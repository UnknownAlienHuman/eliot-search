//! Bounded, untrusted AEAD primitives for the selected-reference profile.
//!
//! Canonical metadata and a reserved counter do not confer source, selection,
//! nonce or lease authority. The Windows consumer must verify those owners,
//! consume its fresh C02 permit and lend its private key for this single call.
//! This module performs no selection, clock, randomness or durable accounting.

use aes_gcm::{
    Aes256Gcm, Nonce,
    aead::{Aead, KeyInit, Payload},
};
use core::fmt;
use search_contracts::{self as c, Blake3Digest32, CanonicalValue as V, OpaqueId};

use crate::{
    MAX_REVISION_PLAINTEXT_BYTES, REVISION_KEY_BYTES, REVISION_NONCE_BYTES, REVISION_TAG_BYTES,
};

/// Selected-reference envelope version; legacy ELSREV01 remains separate.
pub const SELECTED_REVISION_VERSION: u16 = 2;
/// Maximum complete canonical record from each metadata-owning package.
pub const SELECTED_OWNER_RECORD_MAX_BYTES: usize = 8192;
/// Maximum complete canonical associated-data frame.
pub const SELECTED_REVISION_AAD_MAX_BYTES: usize = 32 * 1024;
/// Conservative cryptoperiod: reserved encryption attempts per exact key.
pub const SELECTED_KEY_MAX_ATTEMPTS: u64 = 1_u64 << 32;
/// Conservative cryptoperiod: aggregate authenticated blocks per exact key.
pub const SELECTED_KEY_MAX_AUTHENTICATED_BLOCKS: u64 = 1_u64 << 32;
/// Conservative cryptoperiod: aggregate reserved plaintext bytes per exact key.
pub const SELECTED_KEY_MAX_PLAINTEXT_BYTES: u64 = 1_u64 << 36;
/// Fixed selected-reference header preceding AAD and ciphertext/tag bytes.
pub const SELECTED_REVISION_HEADER_BYTES: usize = 8 + 2 + 4 + 8 + REVISION_NONCE_BYTES;
/// Maximum complete selected-reference envelope.
pub const SELECTED_REVISION_MAX_BYTES: usize = SELECTED_REVISION_HEADER_BYTES
    + SELECTED_REVISION_AAD_MAX_BYTES
    + MAX_REVISION_PLAINTEXT_BYTES
    + REVISION_TAG_BYTES;

const MAGIC: &[u8; 8] = b"ELSREV02";
const AAD_MARKER: &str = "eliot.selected-revision.aad.v2";
const CIPHER_PROFILE: &str = "aes-256-gcm/tag128";
const NONCE_PROFILE: &str = "zero32-counter64/attempts32-blocks32-bytes36/v1";

/// Closed content-free refusal of a selected-reference primitive.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum SelectedRevisionErrorV2 {
    /// Metadata is not a bounded complete canonical nonempty record.
    InvalidMetadata,
    /// The fixed profile or counter is outside the accepted bounds.
    InvalidBinding,
    /// Canonical encoding or the shared digest owner refused.
    Encoding,
    /// Key bytes are the prohibited all-zero value.
    InvalidKey,
    /// Plaintext length or BLAKE3 differs from its exact binding.
    ContentMismatch,
    /// Encoded framing, version, length or nonce is invalid.
    InvalidEnvelope,
    /// AEAD encryption failed.
    Encryption,
    /// Expected binding or AEAD authentication differs.
    Authentication,
}
impl SelectedRevisionErrorV2 {
    /// Stable content-free failure code.
    #[must_use]
    pub const fn code(self) -> &'static str {
        match self {
            Self::InvalidMetadata => "SELECTED_REVISION_METADATA_INVALID",
            Self::InvalidBinding => "SELECTED_REVISION_BINDING_INVALID",
            Self::Encoding => "SELECTED_REVISION_ENCODING_REFUSED",
            Self::InvalidKey => "SELECTED_REVISION_KEY_INVALID",
            Self::ContentMismatch => "SELECTED_REVISION_CONTENT_MISMATCH",
            Self::InvalidEnvelope => "SELECTED_REVISION_ENVELOPE_INVALID",
            Self::Encryption => "SELECTED_REVISION_ENCRYPTION_FAILED",
            Self::Authentication => "SELECTED_REVISION_AUTHENTICATION_FAILED",
        }
    }
}
impl fmt::Display for SelectedRevisionErrorV2 {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.code())
    }
}
impl std::error::Error for SelectedRevisionErrorV2 {}

/// Immutable equality binding, never an accepted source/key/nonce capability.
///
/// Complete canonical owner records remain in AAD, rather than being replaced
/// by caller digest aliases. Physical ciphertext-derived identifiers belong
/// in the post-seal durable link and must not be introduced into these records.
#[derive(Clone, Eq, PartialEq)]
pub struct SelectedRevisionBindingV2 {
    aad: Vec<u8>,
    counter: u64,
    content_digest: Blake3Digest32,
    plaintext_length: u64,
}
impl SelectedRevisionBindingV2 {
    /// Binds complete owner-produced records and the exact reserved use.
    ///
    /// Framing/equality validation is not owner validation. The actual consumer
    /// must independently join Current/backend, prepared occurrence, full domain
    /// bodies, original operation and the fresh C02 reservation before seal.
    pub fn from_owner_records(
        current: &[u8],
        occurrence: &[u8],
        residency: &[u8],
        operation: &OpaqueId,
        counter: u64,
        content_digest: Blake3Digest32,
        plaintext_length: u64,
    ) -> Result<Self, SelectedRevisionErrorV2> {
        for record in [current, occurrence, residency] {
            validate_record(record)?;
        }
        if counter >= SELECTED_KEY_MAX_ATTEMPTS
            || plaintext_length > MAX_REVISION_PLAINTEXT_BYTES as u64
        {
            return Err(SelectedRevisionErrorV2::InvalidBinding);
        }
        let nonce = nonce(counter);
        let value = array(vec![
            text(AAD_MARKER)?,
            V::U64(u64::from(SELECTED_REVISION_VERSION)),
            text(CIPHER_PROFILE)?,
            text(NONCE_PROFILE)?,
            bytes(current)?,
            bytes(occurrence)?,
            bytes(residency)?,
            text(operation.as_str())?,
            V::U64(counter),
            bytes(&nonce)?,
            text("blake3-256")?,
            bytes(content_digest.as_bytes())?,
            V::U64(plaintext_length),
        ])?;
        let encoded =
            c::to_canonical_cbor(&value).map_err(|_| SelectedRevisionErrorV2::Encoding)?;
        if encoded.as_slice().len() > SELECTED_REVISION_AAD_MAX_BYTES {
            return Err(SelectedRevisionErrorV2::InvalidBinding);
        }
        Ok(Self {
            aad: encoded.as_slice().to_vec(),
            counter,
            content_digest,
            plaintext_length,
        })
    }
    /// Exact final canonical AAD; C02 must charge this length, not an estimate.
    #[must_use]
    pub fn aad_bytes(&self) -> &[u8] {
        &self.aad
    }
    /// Reserved counter; selection/restart must not reset its owning ledger.
    #[must_use]
    pub const fn counter(&self) -> u64 {
        self.counter
    }
    /// Deterministic96 nonce; counter0 legitimately gives the all-zero nonce.
    #[must_use]
    pub fn nonce(&self) -> [u8; REVISION_NONCE_BYTES] {
        nonce(self.counter)
    }
    /// Exact expected plaintext BLAKE3, distinct from ciphertext object identity.
    #[must_use]
    pub const fn content_digest(&self) -> Blake3Digest32 {
        self.content_digest
    }
    /// Exact expected plaintext byte length.
    #[must_use]
    pub const fn plaintext_length(&self) -> u64 {
        self.plaintext_length
    }
    /// Exact GHASH charge: ceil(P/16) + ceil(A/16) + the GCM length block.
    pub fn authenticated_blocks(&self) -> Result<u64, SelectedRevisionErrorV2> {
        let aad = u64::try_from(self.aad.len()).map_err(|_| SelectedRevisionErrorV2::Encoding)?;
        let p = self
            .plaintext_length
            .checked_add(15)
            .ok_or(SelectedRevisionErrorV2::Encoding)?
            / 16;
        let a = aad
            .checked_add(15)
            .ok_or(SelectedRevisionErrorV2::Encoding)?
            / 16;
        p.checked_add(a)
            .and_then(|n| n.checked_add(1))
            .ok_or(SelectedRevisionErrorV2::Encoding)
    }
    fn decode(aad: &[u8]) -> Result<Self, SelectedRevisionErrorV2> {
        if aad.len() > SELECTED_REVISION_AAD_MAX_BYTES {
            return Err(SelectedRevisionErrorV2::InvalidEnvelope);
        }
        let V::Array(fields) =
            c::parse_canonical_cbor(aad).map_err(|_| SelectedRevisionErrorV2::InvalidEnvelope)?
        else {
            return Err(SelectedRevisionErrorV2::InvalidEnvelope);
        };
        let [
            V::Text(marker),
            V::U64(2),
            V::Text(cipher),
            V::Text(profile),
            V::Bytes(current),
            V::Bytes(occurrence),
            V::Bytes(residency),
            V::Text(operation),
            V::U64(counter),
            V::Bytes(observed_nonce),
            V::Text(algorithm),
            V::Bytes(digest),
            V::U64(length),
        ] = fields.as_slice()
        else {
            return Err(SelectedRevisionErrorV2::InvalidEnvelope);
        };
        if marker.as_str() != AAD_MARKER
            || cipher.as_str() != CIPHER_PROFILE
            || profile.as_str() != NONCE_PROFILE
            || algorithm.as_str() != "blake3-256"
        {
            return Err(SelectedRevisionErrorV2::InvalidEnvelope);
        }
        let digest: [u8; 32] = digest
            .as_slice()
            .try_into()
            .map_err(|_| SelectedRevisionErrorV2::InvalidEnvelope)?;
        let operation = OpaqueId::new(operation.as_str())
            .map_err(|_| SelectedRevisionErrorV2::InvalidEnvelope)?;
        let binding = Self::from_owner_records(
            current.as_slice(),
            occurrence.as_slice(),
            residency.as_slice(),
            &operation,
            *counter,
            Blake3Digest32::from_bytes(digest),
            *length,
        )?;
        if observed_nonce.as_slice() != binding.nonce() || binding.aad_bytes() != aad {
            return Err(SelectedRevisionErrorV2::InvalidEnvelope);
        }
        Ok(binding)
    }
}
impl fmt::Debug for SelectedRevisionBindingV2 {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str("SelectedRevisionBindingV2(<metadata redacted>)")
    }
}

/// Bounded encoded selected-reference ciphertext; decoding confers no authority.
#[derive(Clone, Eq, PartialEq)]
pub struct SelectedRevisionEnvelopeV2 {
    binding: SelectedRevisionBindingV2,
    ciphertext: Vec<u8>,
}
impl SelectedRevisionEnvelopeV2 {
    /// Authenticated equality metadata; current owners must still be revalidated.
    #[must_use]
    pub const fn binding(&self) -> &SelectedRevisionBindingV2 {
        &self.binding
    }
    /// Ciphertext and128-bit tag, for the opaque immutable-object adapter.
    #[must_use]
    pub fn ciphertext_and_tag(&self) -> &[u8] {
        &self.ciphertext
    }
    /// Encodes the distinct ELSREV02 frame with bounded exact lengths.
    pub fn encode(&self) -> Result<Vec<u8>, SelectedRevisionErrorV2> {
        let aad_len =
            u32::try_from(self.binding.aad.len()).map_err(|_| SelectedRevisionErrorV2::Encoding)?;
        let ciphertext_len =
            u64::try_from(self.ciphertext.len()).map_err(|_| SelectedRevisionErrorV2::Encoding)?;
        if ciphertext_len
            != self
                .binding
                .plaintext_length
                .checked_add(REVISION_TAG_BYTES as u64)
                .ok_or(SelectedRevisionErrorV2::Encoding)?
        {
            return Err(SelectedRevisionErrorV2::InvalidEnvelope);
        }
        let mut output = Vec::with_capacity(
            SELECTED_REVISION_HEADER_BYTES + self.binding.aad.len() + self.ciphertext.len(),
        );
        output.extend_from_slice(MAGIC);
        output.extend_from_slice(&SELECTED_REVISION_VERSION.to_be_bytes());
        output.extend_from_slice(&aad_len.to_be_bytes());
        output.extend_from_slice(&ciphertext_len.to_be_bytes());
        output.extend_from_slice(&self.binding.nonce());
        output.extend_from_slice(self.binding.aad_bytes());
        output.extend_from_slice(&self.ciphertext);
        Ok(output)
    }
    /// Rejects old versions, malformed/trailing data and lengths before allocation.
    pub fn decode(encoded: &[u8]) -> Result<Self, SelectedRevisionErrorV2> {
        if encoded.len() < SELECTED_REVISION_HEADER_BYTES + REVISION_TAG_BYTES
            || encoded.len() > SELECTED_REVISION_MAX_BYTES
        {
            return Err(SelectedRevisionErrorV2::InvalidEnvelope);
        }
        let mut cursor = 0;
        if take::<8>(encoded, &mut cursor)? != *MAGIC
            || u16::from_be_bytes(take::<2>(encoded, &mut cursor)?) != SELECTED_REVISION_VERSION
        {
            return Err(SelectedRevisionErrorV2::InvalidEnvelope);
        }
        let aad_len = usize::try_from(u32::from_be_bytes(take::<4>(encoded, &mut cursor)?))
            .map_err(|_| SelectedRevisionErrorV2::InvalidEnvelope)?;
        let ciphertext_len = usize::try_from(u64::from_be_bytes(take::<8>(encoded, &mut cursor)?))
            .map_err(|_| SelectedRevisionErrorV2::InvalidEnvelope)?;
        let observed_nonce = take::<REVISION_NONCE_BYTES>(encoded, &mut cursor)?;
        if aad_len > SELECTED_REVISION_AAD_MAX_BYTES
            || ciphertext_len > MAX_REVISION_PLAINTEXT_BYTES + REVISION_TAG_BYTES
        {
            return Err(SelectedRevisionErrorV2::InvalidEnvelope);
        }
        let aad_end = cursor
            .checked_add(aad_len)
            .ok_or(SelectedRevisionErrorV2::InvalidEnvelope)?;
        if aad_end.checked_add(ciphertext_len) != Some(encoded.len()) {
            return Err(SelectedRevisionErrorV2::InvalidEnvelope);
        }
        let aad = encoded
            .get(cursor..aad_end)
            .ok_or(SelectedRevisionErrorV2::InvalidEnvelope)?;
        let binding = SelectedRevisionBindingV2::decode(aad)?;
        if observed_nonce != binding.nonce()
            || ciphertext_len as u64 != binding.plaintext_length + REVISION_TAG_BYTES as u64
        {
            return Err(SelectedRevisionErrorV2::InvalidEnvelope);
        }
        Ok(Self {
            binding,
            ciphertext: encoded[aad_end..].to_vec(),
        })
    }
}
impl fmt::Debug for SelectedRevisionEnvelopeV2 {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str("SelectedRevisionEnvelopeV2(<redacted>)")
    }
}

/// Exact decrypted bytes, privately owned and wiped once on every return path.
pub struct SelectedPlaintextV2 {
    bytes: Vec<u8>,
}
impl SelectedPlaintextV2 {
    /// Borrow authenticated bytes; this grants no source access or durable lease.
    #[must_use]
    pub fn as_bytes(&self) -> &[u8] {
        &self.bytes
    }
}
impl Drop for SelectedPlaintextV2 {
    fn drop(&mut self) {
        zeroize::Zeroize::zeroize(self.bytes.as_mut_slice());
    }
}
impl fmt::Debug for SelectedPlaintextV2 {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str("SelectedPlaintextV2(<redacted>)")
    }
}

/// Stateless untrusted primitive; only the closed Windows consumer may use it in product.
/// No nonce, key-use, source or permission receipt is created here.
pub fn seal_selected_revision_v2(
    key: &[u8; REVISION_KEY_BYTES],
    binding: &SelectedRevisionBindingV2,
    plaintext: &[u8],
) -> Result<SelectedRevisionEnvelopeV2, SelectedRevisionErrorV2> {
    validate_key(key)?;
    validate_content(binding, plaintext)?;
    let cipher = Aes256Gcm::new_from_slice(key).map_err(|_| SelectedRevisionErrorV2::InvalidKey)?;
    let nonce = binding.nonce();
    let ciphertext = cipher
        .encrypt(
            Nonce::from_slice(&nonce),
            Payload {
                msg: plaintext,
                aad: binding.aad_bytes(),
            },
        )
        .map_err(|_| SelectedRevisionErrorV2::Encryption)?;
    Ok(SelectedRevisionEnvelopeV2 {
        binding: binding.clone(),
        ciphertext,
    })
}

/// Stateless exact-binding open primitive inside the finite Windows operation.
/// The consumer must load the expected binding from committed object/source owners.
pub fn open_selected_revision_v2(
    key: &[u8; REVISION_KEY_BYTES],
    expected: &SelectedRevisionBindingV2,
    envelope: &SelectedRevisionEnvelopeV2,
) -> Result<SelectedPlaintextV2, SelectedRevisionErrorV2> {
    validate_key(key)?;
    if expected != envelope.binding() {
        return Err(SelectedRevisionErrorV2::Authentication);
    }
    let cipher = Aes256Gcm::new_from_slice(key).map_err(|_| SelectedRevisionErrorV2::InvalidKey)?;
    let nonce = expected.nonce();
    let bytes = cipher
        .decrypt(
            Nonce::from_slice(&nonce),
            Payload {
                msg: envelope.ciphertext_and_tag(),
                aad: expected.aad_bytes(),
            },
        )
        .map_err(|_| SelectedRevisionErrorV2::Authentication)?;
    let plaintext = SelectedPlaintextV2 { bytes };
    validate_content(expected, plaintext.as_bytes())?;
    Ok(plaintext)
}

fn validate_key(key: &[u8; REVISION_KEY_BYTES]) -> Result<(), SelectedRevisionErrorV2> {
    if key.iter().all(|n| *n == 0) {
        Err(SelectedRevisionErrorV2::InvalidKey)
    } else {
        Ok(())
    }
}
fn validate_content(
    binding: &SelectedRevisionBindingV2,
    plaintext: &[u8],
) -> Result<(), SelectedRevisionErrorV2> {
    if plaintext.len() > MAX_REVISION_PLAINTEXT_BYTES
        || plaintext.len() as u64 != binding.plaintext_length
    {
        return Err(SelectedRevisionErrorV2::ContentMismatch);
    }
    let budget = c::Blake3ByteBudget::new(MAX_REVISION_PLAINTEXT_BYTES)
        .map_err(|_| SelectedRevisionErrorV2::Encoding)?;
    let actual = c::blake3_bounded_bytes(plaintext, budget)
        .map_err(|_| SelectedRevisionErrorV2::Encoding)?;
    if actual != binding.content_digest {
        return Err(SelectedRevisionErrorV2::ContentMismatch);
    }
    Ok(())
}
fn nonce(counter: u64) -> [u8; REVISION_NONCE_BYTES] {
    let mut result = [0; REVISION_NONCE_BYTES];
    result[4..].copy_from_slice(&counter.to_be_bytes());
    result
}
fn validate_record(record: &[u8]) -> Result<(), SelectedRevisionErrorV2> {
    if record.is_empty() || record.len() > SELECTED_OWNER_RECORD_MAX_BYTES {
        return Err(SelectedRevisionErrorV2::InvalidMetadata);
    }
    let value =
        c::parse_canonical_cbor(record).map_err(|_| SelectedRevisionErrorV2::InvalidMetadata)?;
    if !matches!(&value, V::Array(fields) if !fields.as_slice().is_empty()) {
        return Err(SelectedRevisionErrorV2::InvalidMetadata);
    }
    if c::to_canonical_cbor(&value)
        .map_err(|_| SelectedRevisionErrorV2::InvalidMetadata)?
        .as_slice()
        != record
    {
        return Err(SelectedRevisionErrorV2::InvalidMetadata);
    }
    Ok(())
}
fn array(values: Vec<V>) -> Result<V, SelectedRevisionErrorV2> {
    c::BoundedList::new(values)
        .map(V::Array)
        .map_err(|_| SelectedRevisionErrorV2::Encoding)
}
fn bytes(value: &[u8]) -> Result<V, SelectedRevisionErrorV2> {
    c::BoundedBytes::new(value.to_vec())
        .map(V::Bytes)
        .map_err(|_| SelectedRevisionErrorV2::Encoding)
}
fn text(value: &str) -> Result<V, SelectedRevisionErrorV2> {
    c::BoundedText::new(value)
        .map(V::Text)
        .map_err(|_| SelectedRevisionErrorV2::Encoding)
}
fn take<const N: usize>(
    input: &[u8],
    cursor: &mut usize,
) -> Result<[u8; N], SelectedRevisionErrorV2> {
    let end = cursor
        .checked_add(N)
        .ok_or(SelectedRevisionErrorV2::InvalidEnvelope)?;
    let value = input
        .get(*cursor..end)
        .ok_or(SelectedRevisionErrorV2::InvalidEnvelope)?
        .try_into()
        .map_err(|_| SelectedRevisionErrorV2::InvalidEnvelope)?;
    *cursor = end;
    Ok(value)
}

#[cfg(test)]
mod tests;

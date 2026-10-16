//! Pure revision-DEK candidate metadata. Decoding or constructing it proves no OS/control effect.

use core::fmt;
use search_contracts::{self as c, CanonicalValue as V};

/// Maximum DPAPI blob inside the SDK's 2560-byte credential frame (44-byte header).
pub const REVISION_DEK_MAX_PROTECTED_BYTES: usize = 2516;
/// Finite canonical metadata record ceiling; records never contain protected or key bytes.
pub const REVISION_DEK_MAX_RECORD_BYTES: usize = 1024;
/// Maximum actual Windows SID byte length supplied by the Native observer.
pub const REVISION_DEK_MAX_SID_BYTES: usize = 68;
/// Fixed purpose for this non-serving candidate profile.
pub const REVISION_DEK_PURPOSE: &str = "revision-aes256-gcm-dek";
/// Fixed current-user wrapping and credential backend profile.
pub const REVISION_DEK_BACKEND: &str = "windows-user-dpapi-credential.v1";
const MARKER: &str = "eliot.revision-dek.record.v1";
const VERSION: u64 = 1;

/// Closed content-free metadata refusal.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum RevisionDekError {
    /// Required binding identifiers or user-scope digest are zero.
    InvalidBinding,
    /// Protected bytes are empty or exceed the fixed credential-frame ceiling.
    InvalidProtectedBlob,
    /// Actual protected bytes or expected binding differ from the retained intent.
    ReadbackMismatch,
    /// Record framing, field types/counts, derived identity or fixed profile differs.
    InvalidRecord,
    /// Shared bounded canonical encoding or hashing refused.
    Encoding,
}
impl RevisionDekError {
    /// Stable reason without SID, target, key or protected bytes.
    #[must_use]
    pub const fn code(self) -> &'static str {
        match self {
            Self::InvalidBinding => "REVISION_DEK_BINDING_INVALID",
            Self::InvalidProtectedBlob => "REVISION_DEK_PROTECTED_BLOB_INVALID",
            Self::ReadbackMismatch => "REVISION_DEK_READBACK_MISMATCH",
            Self::InvalidRecord => "REVISION_DEK_RECORD_INVALID",
            Self::Encoding => "REVISION_DEK_ENCODING_REFUSED",
        }
    }
}
impl fmt::Display for RevisionDekError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result { f.write_str(self.code()) }
}
impl std::error::Error for RevisionDekError {}

/// Bounded SID-byte equality fingerprint, not evidence of a Windows principal.
///
/// The preimage is one canonical CBOR byte string. Native must validate/observe the SID.
/// # Panics
/// Panics when the caller violates the documented 68-byte SID bound.
#[must_use]
pub fn hash_native_revision_user_sid(sid: &[u8]) -> [u8; 32] {
    assert!(sid.len() <= REVISION_DEK_MAX_SID_BYTES, "revision SID byte bound");
    hash("eliot/cbor/windows-user-scope/v1", &bytes(sid).expect("bounded SID bytes"))
        .expect("fixed bounded SID preimage")
}

/// Immutable public metadata; identifiers and digests grant no Native or key-use authority.
#[derive(Clone, Eq, PartialEq)]
pub struct RevisionDekBinding {
    root_id: c::DataRootId,
    installation_id: c::InstallationId,
    incarnation_id: c::InstallationIncarnationId,
    user_scope_digest: [u8; 32],
    operation_id: [u8; 16],
    reference_digest: [u8; 32],
    binding_digest: [u8; 32],
    entropy: Vec<u8>,
}
impl RevisionDekBinding {
    /// Computes a fresh planned version-one reference from the complete fixed scope.
    pub fn new(root_id: c::DataRootId, installation_id: c::InstallationId,
        incarnation_id: c::InstallationIncarnationId, user_scope_digest: [u8; 32],
        operation_id: [u8; 16]) -> Result<Self, RevisionDekError> {
        if [root_id.as_bytes(), installation_id.as_bytes(), incarnation_id.as_bytes(), &operation_id]
            .iter().any(|id| id.iter().all(|byte| *byte == 0))
            || user_scope_digest == [0; 32] {
            return Err(RevisionDekError::InvalidBinding);
        }
        let mut fields = vec![bytes(root_id.as_bytes())?, bytes(installation_id.as_bytes())?,
            bytes(incarnation_id.as_bytes())?, bytes(&user_scope_digest)?, bytes(&operation_id)?,
            V::U64(VERSION), V::U64(VERSION), text(REVISION_DEK_PURPOSE)?,
            text(REVISION_DEK_BACKEND)?, V::U64(VERSION)];
        let reference_digest = hash("eliot/cbor/revision-dek-reference/v1", &array(fields.clone())?)?;
        fields.insert(5, bytes(&reference_digest)?);
        let binding = array(fields)?;
        let binding_digest = hash("eliot/cbor/revision-dek-binding/v1", &binding)?;
        let entropy = encode_value(&binding)?;
        Ok(Self { root_id, installation_id, incarnation_id, user_scope_digest, operation_id,
            reference_digest, binding_digest, entropy })
    }
    /// Exact data-root identity supplied to this metadata constructor.
    #[must_use]
    pub const fn root_id(&self) -> c::DataRootId { self.root_id }
    /// Exact installation identity.
    #[must_use]
    pub const fn installation_id(&self) -> c::InstallationId { self.installation_id }
    /// Exact installation incarnation.
    #[must_use]
    pub const fn installation_incarnation_id(&self) -> c::InstallationIncarnationId { self.incarnation_id }
    /// Equality fingerprint of the supplied user scope; not an OS observation proof.
    #[must_use]
    pub const fn user_scope_digest(&self) -> [u8; 32] { self.user_scope_digest }
    /// Stable logical operation nonce, separate from a Native invocation identity.
    #[must_use]
    pub const fn operation_id(&self) -> [u8; 16] { self.operation_id }
    /// Internally derived opaque reference identity.
    #[must_use]
    pub const fn reference_digest(&self) -> [u8; 32] { self.reference_digest }
    /// Internally computed complete binding fingerprint.
    #[must_use]
    pub const fn binding_digest(&self) -> [u8; 32] { self.binding_digest }
    /// Owned canonical binding bytes used as DPAPI entropy; grants no permission.
    #[must_use]
    pub fn protection_entropy(&self) -> Vec<u8> { self.entropy.clone() }
    /// Planned version of this fresh reference; not evidence of creation.
    #[must_use]
    pub const fn key_version(&self) -> u64 { VERSION }
    /// Planned first record revision.
    #[must_use]
    pub const fn record_revision(&self) -> u64 { VERSION }
    /// Fixed protection-profile revision.
    #[must_use]
    pub const fn profile_revision(&self) -> u64 { VERSION }
}

/// Original candidate intent with a historical Native creator epoch and derived blob metadata.
#[derive(Clone, Eq, PartialEq)]
pub struct RevisionDekIntent {
    binding: RevisionDekBinding,
    creator_epoch: c::OwnerEpoch,
    protected_blob_digest: [u8; 32],
    protected_blob_len: u32,
}
impl RevisionDekIntent {
    /// Hashes exact protected bytes; retains no key or protected blob.
    pub fn new(binding: RevisionDekBinding, creator_epoch: c::OwnerEpoch,
        protected_blob: &[u8]) -> Result<Self, RevisionDekError> {
        let protected_blob_len = valid_blob_len(protected_blob.len())?;
        let digest = c::blake3_bounded_bytes(protected_blob,
            c::Blake3ByteBudget::new(REVISION_DEK_MAX_PROTECTED_BYTES).map_err(|_| RevisionDekError::Encoding)?)
            .map_err(|_| RevisionDekError::Encoding)?;
        Ok(Self { binding, creator_epoch, protected_blob_digest: *digest.as_bytes(), protected_blob_len })
    }
    /// Complete planned reference scope.
    #[must_use]
    pub const fn binding(&self) -> &RevisionDekBinding { &self.binding }
    /// Original Native creator epoch; never a key version or present authority.
    #[must_use]
    pub const fn creator_epoch(&self) -> c::OwnerEpoch { self.creator_epoch }
    /// Raw BLAKE3 of the actual protected bytes supplied at construction.
    #[must_use]
    pub const fn protected_blob_digest(&self) -> [u8; 32] { self.protected_blob_digest }
    /// Exact protected byte count.
    #[must_use]
    pub const fn protected_blob_len(&self) -> u32 { self.protected_blob_len }
}

/// Non-serving candidate metadata. Construction proves equality only, not OS/C02 publication.
#[derive(Clone, Eq, PartialEq)]
pub struct RevisionDekCandidate { intent: RevisionDekIntent }
impl RevisionDekCandidate {
    /// Checks actual protected bytes and the expected binding against the original intent.
    pub fn new(intent: &RevisionDekIntent, expected_binding: &RevisionDekBinding,
        protected_blob: &[u8]) -> Result<Self, RevisionDekError> {
        if intent.binding() != expected_binding { return Err(RevisionDekError::ReadbackMismatch); }
        let actual = RevisionDekIntent::new(expected_binding.clone(), intent.creator_epoch, protected_blob)?;
        if &actual != intent { return Err(RevisionDekError::ReadbackMismatch); }
        Ok(Self { intent: intent.clone() })
    }
    /// Exact retained intent; candidate status grants no Current selection or key use.
    #[must_use]
    pub const fn intent(&self) -> &RevisionDekIntent { &self.intent }
}

/// Closed metadata stages. Neither variant is evidence of an external effect.
#[derive(Clone, Eq, PartialEq)]
pub enum RevisionDekRecord {
    /// Original metadata that must be durably retained before credential publication.
    Intent(RevisionDekIntent),
    /// Metadata for a candidate that must remain non-serving.
    CandidateNonServing(RevisionDekCandidate),
}
impl RevisionDekRecord {
    /// Original metadata shared by both closed stages.
    #[must_use]
    pub const fn intent(&self) -> &RevisionDekIntent {
        match self { Self::Intent(intent) => intent, Self::CandidateNonServing(candidate) => candidate.intent() }
    }
}

/// Encodes exactly seven canonical fields; key/protected bytes never enter the record.
pub fn encode_revision_dek_record(record: &RevisionDekRecord) -> Result<Vec<u8>, RevisionDekError> {
    let intent = record.intent();
    let kind = u64::from(matches!(record, RevisionDekRecord::CandidateNonServing(_)));
    encode_value(&array(vec![text(MARKER)?, V::U64(VERSION), V::U64(kind),
        c::parse_canonical_cbor(&intent.binding.entropy).map_err(|_| RevisionDekError::Encoding)?,
        V::U64(intent.creator_epoch.get()), bytes(&intent.protected_blob_digest)?,
        V::U64(u64::from(intent.protected_blob_len))])?)
}

/// Strictly decodes bounded canonical metadata. A decoded candidate proves no backend existence.
pub fn decode_revision_dek_record(encoded: &[u8]) -> Result<RevisionDekRecord, RevisionDekError> {
    if encoded.len() > REVISION_DEK_MAX_RECORD_BYTES { return Err(RevisionDekError::InvalidRecord); }
    let [marker, codec, kind, binding, epoch, digest, length] = fields::<7>(
        c::parse_canonical_cbor(encoded).map_err(|_| RevisionDekError::InvalidRecord)?)?;
    if marker != text(MARKER)? || codec != V::U64(VERSION) { return Err(RevisionDekError::InvalidRecord); }
    let binding = decode_binding(binding)?;
    let creator_epoch = c::OwnerEpoch::new(number(epoch)?).map_err(|_| RevisionDekError::InvalidRecord)?;
    let protected_blob_digest = fixed(digest)?;
    let length = usize::try_from(number(length)?).map_err(|_| RevisionDekError::InvalidRecord)?;
    let protected_blob_len = valid_blob_len(length).map_err(|_| RevisionDekError::InvalidRecord)?;
    let intent = RevisionDekIntent { binding, creator_epoch, protected_blob_digest, protected_blob_len };
    match number(kind)? {
        0 => Ok(RevisionDekRecord::Intent(intent)),
        1 => Ok(RevisionDekRecord::CandidateNonServing(RevisionDekCandidate { intent })),
        _ => Err(RevisionDekError::InvalidRecord),
    }
}

fn decode_binding(value: V) -> Result<RevisionDekBinding, RevisionDekError> {
    let [root, install, incarnation, user, operation, reference, version, revision, purpose, backend, profile]
        = fields::<11>(value)?;
    if version != V::U64(VERSION) || revision != V::U64(VERSION) || profile != V::U64(VERSION)
        || purpose != text(REVISION_DEK_PURPOSE)? || backend != text(REVISION_DEK_BACKEND)? {
        return Err(RevisionDekError::InvalidRecord);
    }
    let binding = RevisionDekBinding::new(c::DataRootId::from_bytes(fixed(root)?),
        c::InstallationId::from_bytes(fixed(install)?),
        c::InstallationIncarnationId::from_bytes(fixed(incarnation)?), fixed(user)?, fixed(operation)?)
        .map_err(|_| RevisionDekError::InvalidRecord)?;
    if binding.reference_digest != fixed::<32>(reference)? { return Err(RevisionDekError::InvalidRecord); }
    Ok(binding)
}
fn valid_blob_len(length: usize) -> Result<u32, RevisionDekError> {
    if length == 0 || length > REVISION_DEK_MAX_PROTECTED_BYTES { return Err(RevisionDekError::InvalidProtectedBlob); }
    u32::try_from(length).map_err(|_| RevisionDekError::InvalidProtectedBlob)
}
fn bytes(value: &[u8]) -> Result<V, RevisionDekError> {
    c::BoundedBytes::new(value.to_vec()).map(V::Bytes).map_err(|_| RevisionDekError::Encoding)
}
fn text(value: &str) -> Result<V, RevisionDekError> {
    c::CanonicalText::new(value).map(V::Text).map_err(|_| RevisionDekError::Encoding)
}
fn array(values: Vec<V>) -> Result<V, RevisionDekError> {
    c::BoundedList::new(values).map(V::Array).map_err(|_| RevisionDekError::Encoding)
}
fn fields<const N: usize>(value: V) -> Result<[V; N], RevisionDekError> {
    let V::Array(values) = value else { return Err(RevisionDekError::InvalidRecord); };
    values.into_vec().try_into().map_err(|_| RevisionDekError::InvalidRecord)
}
fn fixed<const N: usize>(value: V) -> Result<[u8; N], RevisionDekError> {
    let V::Bytes(value) = value else { return Err(RevisionDekError::InvalidRecord); };
    value.as_slice().try_into().map_err(|_| RevisionDekError::InvalidRecord)
}
fn number(value: V) -> Result<u64, RevisionDekError> {
    let V::U64(value) = value else { return Err(RevisionDekError::InvalidRecord); };
    Ok(value)
}
fn encode_value(value: &V) -> Result<Vec<u8>, RevisionDekError> {
    let encoded = c::to_canonical_cbor(value).map_err(|_| RevisionDekError::Encoding)?;
    if encoded.as_slice().len() > REVISION_DEK_MAX_RECORD_BYTES { return Err(RevisionDekError::Encoding); }
    Ok(encoded.as_slice().to_vec())
}
fn hash(domain: &str, value: &V) -> Result<[u8; 32], RevisionDekError> {
    let domain = c::CanonicalDigestDomain::parse(domain).map_err(|_| RevisionDekError::Encoding)?;
    c::blake3_canonical(&domain, value,
        c::DigestInputLimit::new(REVISION_DEK_MAX_RECORD_BYTES).map_err(|_| RevisionDekError::Encoding)?)
        .map(|digest| *digest.as_bytes()).map_err(|_| RevisionDekError::Encoding)
}
macro_rules! metadata_debug {
    ($($name:ident),+) => { $(impl fmt::Debug for $name {
        fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
            f.write_str(concat!(stringify!($name), "(<metadata>)"))
        }
    })+ };
}
metadata_debug!(RevisionDekBinding, RevisionDekIntent, RevisionDekCandidate, RevisionDekRecord);

#[cfg(test)]
mod tests {
    use super::*;

    fn binding() -> RevisionDekBinding {
        RevisionDekBinding::new(c::DataRootId::from_bytes([1; 16]),
            c::InstallationId::from_bytes([2; 16]), c::InstallationIncarnationId::from_bytes([3; 16]),
            [4; 32], [5; 16]).unwrap()
    }
    fn intent(blob: &[u8]) -> RevisionDekIntent {
        RevisionDekIntent::new(binding(), c::OwnerEpoch::new(7).unwrap(), blob).unwrap()
    }
    fn value() -> Vec<V> {
        let encoded = encode_revision_dek_record(&RevisionDekRecord::Intent(intent(&[0]))).unwrap();
        fields::<7>(c::parse_canonical_cbor(&encoded).unwrap()).unwrap().into_iter().collect()
    }
    fn encoded(rows: Vec<V>) -> Vec<u8> {
        c::to_canonical_cbor(&array(rows).unwrap()).unwrap().as_slice().to_vec()
    }

    #[test]
    fn binding_identity_separates_each_scope_axis_and_preserves_planned_versions() {
        let original = binding();
        assert_eq!(original, binding());
        assert_ne!(original.reference_digest(), original.binding_digest());
        assert_eq!((original.key_version(), original.record_revision(), original.profile_revision()), (1, 1, 1));
        for axis in 0..5 {
            let mut root = [1; 16]; let mut install = [2; 16]; let mut incarnation = [3; 16];
            let mut user = [4; 32]; let mut operation = [5; 16];
            match axis { 0 => root[0] ^= 1, 1 => install[0] ^= 1, 2 => incarnation[0] ^= 1,
                3 => user[0] ^= 1, _ => operation[0] ^= 1 }
            let changed = RevisionDekBinding::new(c::DataRootId::from_bytes(root),
                c::InstallationId::from_bytes(install), c::InstallationIncarnationId::from_bytes(incarnation),
                user, operation).unwrap();
            assert_ne!(original.reference_digest(), changed.reference_digest());
            assert_ne!(original.binding_digest(), changed.binding_digest());
            assert_ne!(original.protection_entropy(), changed.protection_entropy());
        }
        assert_eq!(RevisionDekBinding::new(c::DataRootId::from_bytes([0; 16]),
            original.installation_id(), original.installation_incarnation_id(), [4; 32], [5; 16]),
            Err(RevisionDekError::InvalidBinding));
        let sid = [1, 1, 0, 0, 0, 0, 0, 5, 21, 0, 0, 0];
        let mut other = sid; other[8] = 22;
        assert_ne!(hash_native_revision_user_sid(&sid), hash_native_revision_user_sid(&other));
    }

    #[test]
    fn canonical_record_has_seven_fields_and_the_frozen_eleven_field_binding() {
        let intent = intent(&[0]);
        let candidate = RevisionDekCandidate::new(&intent, intent.binding(), &[0]).unwrap();
        for record in [RevisionDekRecord::Intent(intent.clone()), RevisionDekRecord::CandidateNonServing(candidate)] {
            let encoded = encode_revision_dek_record(&record).unwrap();
            // Independent CBOR framing prefix: array(7), text(28), marker, codec, kind, array(11).
            let mut prefix = vec![0x87, 0x78, 0x1c];
            prefix.extend_from_slice(b"eliot.revision-dek.record.v1");
            prefix.extend_from_slice(&[1, u8::from(matches!(record, RevisionDekRecord::CandidateNonServing(_))), 0x8b, 0x50]);
            prefix.extend_from_slice(&[1; 16]);
            assert!(encoded.starts_with(&prefix));
            assert_eq!(decode_revision_dek_record(&encoded).unwrap(), record);
            let rows = fields::<7>(c::parse_canonical_cbor(&encoded).unwrap()).unwrap();
            let scope = fields::<11>(rows[3].clone()).unwrap();
            assert_eq!(scope[5], bytes(&intent.binding().reference_digest()).unwrap());
            assert_eq!((scope[6].clone(), scope[7].clone(), scope[10].clone()), (V::U64(1), V::U64(1), V::U64(1)));
            assert_eq!(scope[8], text("revision-aes256-gcm-dek").unwrap());
            assert_eq!(scope[9], text("windows-user-dpapi-credential.v1").unwrap());
            assert_eq!(encode_value(&rows[3]).unwrap(), intent.binding().protection_entropy());
        }
    }

    #[test]
    fn decoder_refuses_forged_scope_fixed_profiles_and_record_fields() {
        for axis in 0..11 {
            let mut rows = value();
            let mut scope = fields::<11>(rows[3].clone()).unwrap();
            scope[axis] = match axis {
                0..=2 | 4 => bytes(&[9; 16]).unwrap(),
                3 | 5 => bytes(&[9; 32]).unwrap(),
                6 | 7 | 10 => V::U64(2),
                _ => text("foreign-profile").unwrap(),
            };
            rows[3] = array(scope.into_iter().collect()).unwrap();
            assert!(decode_revision_dek_record(&encoded(rows)).is_err(), "binding field {axis}");
        }
        for (axis, replacement) in [(0, text("unknown-record").unwrap()), (1, V::U64(2)),
            (2, V::U64(2)), (4, V::U64(0)), (5, bytes(&[9; 31]).unwrap()),
            (6, V::U64(0)), (6, V::U64(2517)), (6, V::Bool(true))] {
            let mut rows = value(); rows[axis] = replacement;
            assert!(decode_revision_dek_record(&encoded(rows)).is_err());
        }
        let mut extra = value(); extra.push(V::Null);
        assert!(decode_revision_dek_record(&encoded(extra)).is_err());
        let mut missing = value(); missing.pop();
        assert!(decode_revision_dek_record(&encoded(missing)).is_err());
        for extra in [false, true] {
            let mut rows = value();
            let mut scope = fields::<11>(rows[3].clone()).unwrap().to_vec();
            if extra { scope.push(V::Null); } else { scope.pop(); }
            rows[3] = array(scope).unwrap();
            assert!(decode_revision_dek_record(&encoded(rows)).is_err());
        }
        let mut wrong_length = value();
        let mut scope = fields::<11>(wrong_length[3].clone()).unwrap();
        scope[0] = bytes(&[1; 15]).unwrap();
        wrong_length[3] = array(scope.into_iter().collect()).unwrap();
        assert!(decode_revision_dek_record(&encoded(wrong_length)).is_err());
    }

    #[test]
    fn decoder_refuses_noncanonical_trailing_truncated_and_oversized_bytes() {
        let original = encoded(value());
        for length in 0..original.len() {
            assert!(decode_revision_dek_record(&original[..length]).is_err());
        }
        let mut trailing = original.clone(); trailing.push(0);
        assert!(decode_revision_dek_record(&trailing).is_err());
        let mut noncanonical = original;
        let codec_offset = 3 + MARKER.len();
        noncanonical.splice(codec_offset..codec_offset + 1, [0x18, 1]);
        assert!(decode_revision_dek_record(&noncanonical).is_err());
        assert!(decode_revision_dek_record(&vec![0; REVISION_DEK_MAX_RECORD_BYTES + 1]).is_err());
    }

    #[test]
    fn blob_bounds_and_hash_are_derived_from_actual_protected_bytes() {
        // Published BLAKE3 test vector input[i] = i % 251, length one.
        let expected = c::Blake3Digest32::parse_hex(
            "2d3adedff11b61f14c886e35afa036736dcd87a74d27b5c1510225d0f592e213").unwrap();
        assert_eq!(intent(&[0]).protected_blob_digest(), *expected.as_bytes());
        assert_eq!(intent(&[0]).protected_blob_len(), 1);
        assert_eq!(intent(&vec![7; 2516]).protected_blob_len(), 2516);
        let epoch = c::OwnerEpoch::new(1).unwrap();
        assert_eq!(RevisionDekIntent::new(binding(), epoch, &[]), Err(RevisionDekError::InvalidProtectedBlob));
        assert_eq!(RevisionDekIntent::new(binding(), epoch, &vec![0; 2517]), Err(RevisionDekError::InvalidProtectedBlob));
    }

    #[test]
    fn candidate_rechecks_expected_binding_and_bytes_without_granting_current_authority() {
        let intent = intent(&[0]);
        assert_eq!(RevisionDekCandidate::new(&intent, intent.binding(), &[1]), Err(RevisionDekError::ReadbackMismatch));
        let other = RevisionDekBinding::new(intent.binding().root_id(), intent.binding().installation_id(),
            intent.binding().installation_incarnation_id(), [4; 32], [6; 16]).unwrap();
        assert_eq!(RevisionDekCandidate::new(&intent, &other, &[0]), Err(RevisionDekError::ReadbackMismatch));
        // Canonical metadata can claim a candidate but cannot attest a native effect.
        let mut forged = value(); forged[2] = V::U64(1); forged[5] = bytes(&[9; 32]).unwrap();
        let decoded = decode_revision_dek_record(&encoded(forged)).unwrap();
        assert_eq!(RevisionDekCandidate::new(decoded.intent(), intent.binding(), &[0]), Err(RevisionDekError::ReadbackMismatch));
    }

    #[test]
    fn public_debug_and_metadata_never_contain_protected_canaries() {
        let canary = b"private DPAPI blob canary must remain outside redb";
        let intent = intent(canary);
        let candidate = RevisionDekCandidate::new(&intent, intent.binding(), canary).unwrap();
        let record = RevisionDekRecord::CandidateNonServing(candidate.clone());
        let encoded = encode_revision_dek_record(&record).unwrap();
        assert!(!encoded.windows(canary.len()).any(|window| window == canary));
        let debug = format!("{:?}{intent:?}{candidate:?}{record:?}", intent.binding());
        assert!(!debug.contains("private DPAPI"));
        assert!(!debug.contains("ELIOT Search/"));
        assert!(!debug.contains(REVISION_DEK_BACKEND));
        for error in [RevisionDekError::InvalidBinding, RevisionDekError::InvalidProtectedBlob,
            RevisionDekError::ReadbackMismatch, RevisionDekError::InvalidRecord, RevisionDekError::Encoding] {
            assert!(!format!("{error:?}{error}").contains("private DPAPI"));
        }
    }
}

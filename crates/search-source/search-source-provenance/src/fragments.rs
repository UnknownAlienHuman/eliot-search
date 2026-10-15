//! Bounded metadata fragments. Storage and mutation authority remain external.

use crate::{OriginalAdmissionV2, RetainedPolicyV2, codec as c};
use search_contracts::{
    CanonicalDigestDomain, CanonicalValue as V, DigestInputLimit, blake3_raw, parse_canonical_cbor,
    to_canonical_cbor,
};
use search_source_admission::policy_v2::CheckpointFailureV2;
use search_source_admission::{AcceptedCapabilities, AdmissionError, SectionPlatform};
use std::{collections::BTreeMap, sync::Arc};

/// Body bytes per fragment in the closed original-input layout.
pub const ORIGINAL_INPUT_FRAGMENT_BYTES_V2: usize = 16_384;
const PROFILE: &str = "eliot.original-input-fragments@2";
const MAX_ARCHIVE: usize = 256 * 1024;
const MAX_REFERENCE: usize = 512;
type Probe<'a> = &'a mut dyn FnMut() -> Result<(), CheckpointFailureV2>;

/// Actual borrowed storage bytes. This observation is not a trusted archive.
#[derive(Clone, Copy)]
pub struct ObservedOriginalInputPart<'a> {
    /// Full canonical metadata key, independently read from storage.
    pub key: &'a [u8],
    /// Full canonical metadata value from the same coherent snapshot.
    pub value: &'a [u8],
}

/// Prepared canonical metadata part; it carries no source/native authority.
#[derive(Clone)]
pub struct OriginalInputPartV2 {
    key: Vec<u8>,
    value: Vec<u8>,
}
impl OriginalInputPartV2 {
    /// Exact metadata key, never a source identity or path.
    #[must_use]
    pub fn key(&self) -> &[u8] {
        &self.key
    }
    /// Complete bounded metadata value.
    #[must_use]
    pub fn value(&self) -> &[u8] {
        &self.value
    }
}

/// One logical original split into bounded physical metadata values.
pub struct OriginalInputsLayoutV2 {
    reference: Vec<u8>,
    parts: Vec<OriginalInputPartV2>,
}
impl core::fmt::Debug for OriginalInputsLayoutV2 {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        f.debug_struct("OriginalInputsLayoutV2")
            .field("parts", &self.parts.len())
            .finish_non_exhaustive()
    }
}
impl OriginalInputsLayoutV2 {
    /// Prepare complete original-policy metadata without narrowing its policy limits.
    ///
    /// # Errors
    /// Refuses malformed/oversized metadata or original checkpoint failure.
    pub fn policy(policy: &RetainedPolicyV2, probe: Probe<'_>) -> Result<Self, AdmissionError> {
        prepare("policy", policy.encoded_inputs(), probe)
    }
    /// Prepare one complete original admission, with a policy reference only.
    ///
    /// # Errors
    /// Refuses malformed/oversized metadata or original checkpoint failure.
    pub fn admission(
        original: &OriginalAdmissionV2,
        probe: Probe<'_>,
    ) -> Result<Self, AdmissionError> {
        prepare("admission", original.encoded_source_evidence(), probe)
    }
    /// Exact canonical layout reference for a semantic state/history record.
    #[must_use]
    pub fn reference(&self) -> &[u8] {
        &self.reference
    }
    /// All physical metadata parts; the I/O owner must persist/read them coherently.
    #[must_use]
    pub fn parts(&self) -> &[OriginalInputPartV2] {
        &self.parts
    }

    /// Reconstitute observed policy parts through the actual policy owner.
    ///
    /// # Errors
    /// Missing, duplicate, foreign, altered or noncanonical parts, unsupported
    /// policy inputs and original checkpoint failures return no model.
    pub fn restore_policy(
        reference: &[u8],
        parts: &[ObservedOriginalInputPart<'_>],
        platform: &SectionPlatform,
        capabilities: &AcceptedCapabilities,
        probe: &mut impl FnMut() -> Result<(), CheckpointFailureV2>,
    ) -> Result<RetainedPolicyV2, AdmissionError> {
        let bytes = join("policy", reference, parts, probe)?;
        RetainedPolicyV2::restore(&bytes, platform, capabilities, probe)
    }
    /// Reconstitute observed admission parts through the original semantic owners.
    ///
    /// # Errors
    /// Invalid parts, foreign original policy, altered complete receipt or
    /// original checkpoint failures return no model or currentness grant.
    pub fn restore_admission(
        reference: &[u8],
        parts: &[ObservedOriginalInputPart<'_>],
        policy: Arc<RetainedPolicyV2>,
        probe: &mut impl FnMut() -> Result<(), CheckpointFailureV2>,
    ) -> Result<OriginalAdmissionV2, AdmissionError> {
        let bytes = join("admission", reference, parts, probe)?;
        OriginalAdmissionV2::restore(policy, &bytes, probe)
    }
}

fn prepare(
    kind: &str,
    bytes: &[u8],
    probe: Probe<'_>,
) -> Result<OriginalInputsLayoutV2, AdmissionError> {
    check(probe)?;
    if bytes.is_empty() || bytes.len() > MAX_ARCHIVE {
        return Err(bad());
    }
    let fingerprint = digest(kind, bytes)?;
    check(probe)?;
    let reference = c::array([
        V::U64(2),
        c::text(PROFILE)?,
        c::text(kind)?,
        c::bytes(&fingerprint)?,
        V::U64(bytes.len() as u64),
        V::U64(bytes.len().div_ceil(ORIGINAL_INPUT_FRAGMENT_BYTES_V2) as u64),
    ])?;
    let mut parts = Vec::new();
    for (index, body) in bytes.chunks(ORIGINAL_INPUT_FRAGMENT_BYTES_V2).enumerate() {
        check(probe)?;
        parts.push(OriginalInputPartV2 {
            key: key(kind, &fingerprint, index)?,
            value: encode(&c::array([
                reference.clone(),
                V::U64(index as u64),
                c::bytes(body)?,
            ])?)?,
        });
    }
    let result = OriginalInputsLayoutV2 {
        reference: encode(&reference)?,
        parts,
    };
    check(probe)?;
    Ok(result)
}

fn join(
    kind: &str,
    reference: &[u8],
    parts: &[ObservedOriginalInputPart<'_>],
    probe: Probe<'_>,
) -> Result<Vec<u8>, AdmissionError> {
    check(probe)?;
    if reference.is_empty() || reference.len() > MAX_REFERENCE {
        return Err(bad());
    }
    let expected = parse_canonical_cbor(reference).map_err(|_| bad())?;
    let fields = rows::<6>(&expected)?;
    if fields[0] != V::U64(2) || fields[1] != c::text(PROFILE)? || fields[2] != c::text(kind)? {
        return Err(bad());
    }
    let V::Bytes(fingerprint) = &fields[3] else {
        return Err(bad());
    };
    if fingerprint.as_slice().len() != 32 {
        return Err(bad());
    }
    let length = number(&fields[4])?;
    let count = number(&fields[5])?;
    if length == 0
        || length > MAX_ARCHIVE
        || count != length.div_ceil(ORIGINAL_INPUT_FRAGMENT_BYTES_V2)
        || parts.len() != count
    {
        return Err(bad());
    }
    let mut bodies = BTreeMap::new();
    for part in parts {
        check(probe)?;
        if part.key.len() > MAX_REFERENCE
            || part.value.len() > ORIGINAL_INPUT_FRAGMENT_BYTES_V2 + MAX_REFERENCE
        {
            return Err(bad());
        }
        let value = parse_canonical_cbor(part.value).map_err(|_| bad())?;
        let item = rows::<3>(&value)?;
        let index = number(&item[1])?;
        if item[0] != expected
            || index >= count
            || part.key != key(kind, fingerprint.as_slice(), index)?
        {
            return Err(bad());
        }
        let V::Bytes(body) = &item[2] else {
            return Err(bad());
        };
        let remaining = length - index * ORIGINAL_INPUT_FRAGMENT_BYTES_V2;
        if body.as_slice().len() != remaining.min(ORIGINAL_INPUT_FRAGMENT_BYTES_V2)
            || bodies.insert(index, body.as_slice().to_vec()).is_some()
        {
            return Err(bad());
        }
    }
    let mut joined = Vec::with_capacity(length);
    for body in bodies.values() {
        check(probe)?;
        joined.extend_from_slice(body);
    }
    check(probe)?;
    if digest(kind, &joined)?.as_slice() != fingerprint.as_slice() {
        return Err(bad());
    }
    check(probe)?;
    Ok(joined)
}

fn digest(kind: &str, bytes: &[u8]) -> Result<[u8; 32], AdmissionError> {
    let domain = match kind {
        "policy" => "eliot/raw/original-policy-archive/v2",
        "admission" => "eliot/raw/original-admission-archive/v2",
        _ => return Err(bad()),
    };
    let domain = CanonicalDigestDomain::parse(domain).map_err(|_| bad())?;
    Ok(*blake3_raw(
        &domain,
        bytes,
        DigestInputLimit::new(MAX_ARCHIVE + 128).map_err(|_| bad())?,
    )
    .map_err(|_| bad())?
    .as_bytes())
}
fn key(kind: &str, fingerprint: &[u8], index: usize) -> Result<Vec<u8>, AdmissionError> {
    encode(&c::array([
        c::text(PROFILE)?,
        c::text(kind)?,
        c::bytes(fingerprint)?,
        V::U64(index as u64),
    ])?)
}
fn encode(value: &V) -> Result<Vec<u8>, AdmissionError> {
    to_canonical_cbor(value)
        .map(|bytes| bytes.as_slice().to_vec())
        .map_err(|_| bad())
}
fn rows<const N: usize>(value: &V) -> Result<&[V; N], AdmissionError> {
    let V::Array(values) = value else {
        return Err(bad());
    };
    values.as_slice().try_into().map_err(|_| bad())
}
fn number(value: &V) -> Result<usize, AdmissionError> {
    let V::U64(value) = value else {
        return Err(bad());
    };
    usize::try_from(*value).map_err(|_| bad())
}
fn check(probe: Probe<'_>) -> Result<(), AdmissionError> {
    probe().map_err(|failure| match failure {
        CheckpointFailureV2::Cancelled => AdmissionError::Cancelled,
        CheckpointFailureV2::DeadlineExceeded => AdmissionError::BudgetExhausted,
    })
}
const fn bad() -> AdmissionError {
    AdmissionError::ReceiptMismatch
}

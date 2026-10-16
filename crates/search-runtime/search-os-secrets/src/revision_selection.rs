//! Pure first-Current selection metadata, never Native readback or committed authority.

use crate::{
    RevisionDekCandidate, RevisionDekRecord, ValidatedSecretsConfig, decode_revision_dek_record,
    encode_revision_dek_record,
};
use core::fmt;
use search_config::ConfigFingerprint;
use search_contracts::{self as c, CanonicalValue as V};

/// Finite selection-record profile within the existing journal value ceiling.
pub const REVISION_DEK_SELECTION_MAX_RECORD_BYTES: usize = 8192;
/// The sole supported source-admitted backend for this consumer.
pub const REVISION_DEK_SELECTION_BACKEND: &str = "windows_user_bound";
/// Backend application is limited to this consumer; no lease or key use is applied.
pub const REVISION_DEK_SELECTION_SCOPE: &str =
    "revision-dek-current-selection/windows-user-bound/v1";
const MARKER: &str = "eliot.revision-dek.selection.v1";
const VERSION: u64 = 1;

/// Closed, content-free refusal of selection metadata.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum RevisionDekSelectionError {
    /// Configuration tag, backend or nonzero descriptor/digest identity differs.
    InvalidPolicy,
    /// Selection operation is zero/reuses candidate preparation, or candidate metadata is invalid.
    InvalidIntent,
    /// Canonical framing, fixed profile, field count or first-revision result differs.
    InvalidRecord,
    /// Shared bounded canonical encoding or hashing refused.
    Encoding,
}
impl RevisionDekSelectionError {
    /// Stable content-free reason code.
    #[must_use]
    pub const fn code(self) -> &'static str {
        match self {
            Self::InvalidPolicy => "REVISION_DEK_SELECTION_POLICY_INVALID",
            Self::InvalidIntent => "REVISION_DEK_SELECTION_INTENT_INVALID",
            Self::InvalidRecord => "REVISION_DEK_SELECTION_RECORD_INVALID",
            Self::Encoding => "REVISION_DEK_SELECTION_ENCODING_REFUSED",
        }
    }
}
impl fmt::Display for RevisionDekSelectionError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(self.code())
    }
}
impl std::error::Error for RevisionDekSelectionError {}

/// Backend-only application policy. Digests are equality metadata, not application receipts.
#[derive(Clone, Eq, PartialEq)]
pub struct RevisionDekSelectionPolicy {
    backend: &'static str,
    config_fingerprint: [u8; 32],
    secrets_section_digest: [u8; 32],
    registry_digest: [u8; 32],
}
impl RevisionDekSelectionPolicy {
    /// Captures checked backend metadata after requiring the exact canonical-v2 fingerprint tag.
    pub fn from_config(
        config: &ValidatedSecretsConfig,
        fingerprint: ConfigFingerprint,
    ) -> Result<Self, RevisionDekSelectionError> {
        let tag = (
            fingerprint.algorithm(),
            fingerprint.canonical_profile(),
            fingerprint.revision(),
        );
        if tag != ("blake3", "eliot/cbor/config-snapshot/v2", 2)
            || config.backend() != REVISION_DEK_SELECTION_BACKEND
        {
            return Err(RevisionDekSelectionError::InvalidPolicy);
        }
        policy(
            *fingerprint.as_bytes(),
            *crate::section_digest(config).as_bytes(),
            registry_digest()?,
        )
    }
    /// Fixed configured backend, not proof of present availability.
    #[must_use]
    pub const fn backend(&self) -> &'static str {
        self.backend
    }
    /// Expected bytes under the fixed config-snapshot-v2 profile; not a restored typed fingerprint.
    #[must_use]
    pub const fn config_fingerprint(&self) -> [u8; 32] {
        self.config_fingerprint
    }
    /// Complete source-admitted secrets section equality/provenance digest.
    #[must_use]
    pub const fn secrets_section_digest(&self) -> [u8; 32] {
        self.secrets_section_digest
    }
    /// Exact owner descriptor-v2 registry digest.
    #[must_use]
    pub const fn registry_digest(&self) -> [u8; 32] {
        self.registry_digest
    }
    /// Equality over full canonical Candidate v1 bytes, backend and consumer scope only.
    /// Native must independently construct/read back the actual dependency.
    pub fn expected_backend_identity(
        &self,
        candidate: &RevisionDekCandidate,
    ) -> Result<[u8; 32], RevisionDekSelectionError> {
        hash(
            "eliot/cbor/revision-dek-backend-identity/v1",
            &array(vec![
                bytes(&candidate_bytes(candidate)?)?,
                text(self.backend())?,
                text(REVISION_DEK_SELECTION_SCOPE)?,
            ])?,
        )
    }
}

/// Complete logical application intent with a separately retained historical creator epoch.
#[derive(Clone, Eq, PartialEq)]
pub struct RevisionDekApplicationIntent {
    candidate: RevisionDekCandidate,
    operation_id: [u8; 16],
    policy: RevisionDekSelectionPolicy,
    creator_epoch: c::OwnerEpoch,
    request_digest: [u8; 32],
}
impl RevisionDekApplicationIntent {
    /// Plans application metadata; performs no backend operation or control publication.
    pub fn new(
        candidate: RevisionDekCandidate,
        operation_id: [u8; 16],
        policy: RevisionDekSelectionPolicy,
        creator_epoch: c::OwnerEpoch,
    ) -> Result<Self, RevisionDekSelectionError> {
        if operation_id == [0; 16] || operation_id == candidate.intent().binding().operation_id() {
            return Err(RevisionDekSelectionError::InvalidIntent);
        }
        let request_digest = hash(
            "eliot/cbor/revision-dek-selection-request/v1",
            &array(request_fields(&candidate, operation_id, &policy)?)?,
        )?;
        Ok(Self {
            candidate,
            operation_id,
            policy,
            creator_epoch,
            request_digest,
        })
    }
    /// Exact original canonical candidate metadata; grants no key use.
    #[must_use]
    pub const fn candidate(&self) -> &RevisionDekCandidate {
        &self.candidate
    }
    /// Stable logical selection operation, distinct from candidate preparation and Native invocation.
    #[must_use]
    pub const fn operation_id(&self) -> [u8; 16] {
        self.operation_id
    }
    /// Retained consumer-scoped backend policy.
    #[must_use]
    pub const fn policy(&self) -> &RevisionDekSelectionPolicy {
        &self.policy
    }
    /// Historical application-intent creator, never a current Native fence or replay identity.
    #[must_use]
    pub const fn creator_epoch(&self) -> c::OwnerEpoch {
        self.creator_epoch
    }
    /// Complete logical request digest; excludes only this intent's changing creator epoch.
    /// The candidate's immutable original epoch remains part of the exact candidate bytes.
    #[must_use]
    pub const fn request_digest(&self) -> [u8; 32] {
        self.request_digest
    }
}

/// Planned first-Current metadata. Only an absent-state C02 CAS can assign these revisions durably.
#[derive(Clone, Eq, PartialEq)]
pub struct RevisionDekCurrent {
    intent: RevisionDekApplicationIntent,
    expected_backend_identity: [u8; 32],
}
impl RevisionDekCurrent {
    /// Plans the first scoped application/selection result; proves no Native or control effect.
    pub fn new(intent: &RevisionDekApplicationIntent) -> Result<Self, RevisionDekSelectionError> {
        Ok(Self {
            intent: intent.clone(),
            expected_backend_identity: intent
                .policy
                .expected_backend_identity(intent.candidate())?,
        })
    }
    /// Exact original application intent.
    #[must_use]
    pub const fn intent(&self) -> &RevisionDekApplicationIntent {
        &self.intent
    }
    /// Planned initial consumer application revision, separate from descriptor and config versions.
    #[must_use]
    pub const fn applied_policy_revision(&self) -> u64 {
        VERSION
    }
    /// Planned initial selection revision, separate from application revision and key version.
    #[must_use]
    pub const fn selection_revision(&self) -> u64 {
        VERSION
    }
}

/// Closed selection stages; neither public construction nor decoding proves an external effect.
#[derive(Clone, Eq, PartialEq)]
pub enum RevisionDekSelectionRecord {
    /// Retained intent before actual backend application and Current publication.
    Intent(RevisionDekApplicationIntent),
    /// Planned/decoded result containing the scoped application metadata.
    Current(RevisionDekCurrent),
}
impl RevisionDekSelectionRecord {
    /// Original intent shared by both stages.
    #[must_use]
    pub const fn intent(&self) -> &RevisionDekApplicationIntent {
        match self {
            Self::Intent(intent) => intent,
            Self::Current(current) => current.intent(),
        }
    }
}

/// Encodes the fixed four-field selection profile, embedding unchanged Candidate v1 bytes.
pub fn encode_revision_dek_selection_record(
    record: &RevisionDekSelectionRecord,
) -> Result<Vec<u8>, RevisionDekSelectionError> {
    let mut intent = request_fields(
        record.intent().candidate(),
        record.intent().operation_id(),
        record.intent().policy(),
    )?;
    intent.push(V::U64(record.intent().creator_epoch().get()));
    let intent = array(intent)?;
    let (kind, body) = match record {
        RevisionDekSelectionRecord::Intent(_) => (0, intent),
        RevisionDekSelectionRecord::Current(current) => (
            1,
            array(vec![
                intent,
                V::U64(current.applied_policy_revision()),
                V::U64(current.selection_revision()),
                bytes(&current.expected_backend_identity)?,
            ])?,
        ),
    };
    let value = array(vec![text(MARKER)?, V::U64(VERSION), V::U64(kind), body])?;
    let encoded = c::to_canonical_cbor(&value).map_err(encoding)?;
    if encoded.as_slice().len() > REVISION_DEK_SELECTION_MAX_RECORD_BYTES {
        return Err(RevisionDekSelectionError::Encoding);
    }
    Ok(encoded.as_slice().to_vec())
}

/// Strict bounded canonical decode; returns metadata only, never an application or custody proof.
pub fn decode_revision_dek_selection_record(
    encoded: &[u8],
) -> Result<RevisionDekSelectionRecord, RevisionDekSelectionError> {
    if encoded.is_empty() || encoded.len() > REVISION_DEK_SELECTION_MAX_RECORD_BYTES {
        return Err(RevisionDekSelectionError::InvalidRecord);
    }
    let [marker, codec, kind, body] = fields::<4>(
        c::parse_canonical_cbor(encoded).map_err(|_| RevisionDekSelectionError::InvalidRecord)?,
    )?;
    if marker != text(MARKER)? || codec != V::U64(VERSION) {
        return Err(RevisionDekSelectionError::InvalidRecord);
    }
    match kind {
        V::U64(0) => Ok(RevisionDekSelectionRecord::Intent(decode_intent(body)?)),
        V::U64(1) => {
            let [intent, applied, selection, identity] = fields::<4>(body)?;
            if applied != V::U64(VERSION) || selection != V::U64(VERSION) {
                return Err(RevisionDekSelectionError::InvalidRecord);
            }
            let current = RevisionDekCurrent::new(&decode_intent(intent)?)?;
            if current.expected_backend_identity != fixed::<32>(identity)? {
                return Err(RevisionDekSelectionError::InvalidRecord);
            }
            Ok(RevisionDekSelectionRecord::Current(current))
        }
        _ => Err(RevisionDekSelectionError::InvalidRecord),
    }
}

fn registry_digest() -> Result<[u8; 32], RevisionDekSelectionError> {
    crate::section_descriptor()
        .map(|descriptor| *descriptor.field_registry_digest().as_bytes())
        .map_err(|_| RevisionDekSelectionError::InvalidPolicy)
}
fn policy(
    config: [u8; 32],
    section: [u8; 32],
    registry: [u8; 32],
) -> Result<RevisionDekSelectionPolicy, RevisionDekSelectionError> {
    if [config, section, registry].contains(&[0; 32]) || registry != registry_digest()? {
        return Err(RevisionDekSelectionError::InvalidPolicy);
    }
    Ok(RevisionDekSelectionPolicy {
        backend: REVISION_DEK_SELECTION_BACKEND,
        config_fingerprint: config,
        secrets_section_digest: section,
        registry_digest: registry,
    })
}
fn candidate_bytes(candidate: &RevisionDekCandidate) -> Result<Vec<u8>, RevisionDekSelectionError> {
    if candidate.intent().protected_blob_digest() == [0; 32] {
        return Err(RevisionDekSelectionError::InvalidIntent);
    }
    encode_revision_dek_record(&RevisionDekRecord::CandidateNonServing(candidate.clone()))
        .map_err(|_| RevisionDekSelectionError::InvalidIntent)
}
fn request_fields(
    candidate: &RevisionDekCandidate,
    operation: [u8; 16],
    policy: &RevisionDekSelectionPolicy,
) -> Result<Vec<V>, RevisionDekSelectionError> {
    Ok(vec![
        bytes(&operation)?,
        bytes(&candidate_bytes(candidate)?)?,
        bytes(&policy.config_fingerprint())?,
        bytes(&policy.secrets_section_digest())?,
        bytes(&policy.registry_digest())?,
        text(policy.backend())?,
        text(REVISION_DEK_SELECTION_SCOPE)?,
    ])
}
fn decode_intent(value: V) -> Result<RevisionDekApplicationIntent, RevisionDekSelectionError> {
    let [
        operation,
        candidate,
        config,
        section,
        registry,
        backend,
        scope,
        epoch,
    ] = fields::<8>(value)?;
    if backend != text(REVISION_DEK_SELECTION_BACKEND)?
        || scope != text(REVISION_DEK_SELECTION_SCOPE)?
    {
        return Err(RevisionDekSelectionError::InvalidRecord);
    }
    let V::Bytes(candidate) = candidate else {
        return Err(RevisionDekSelectionError::InvalidRecord);
    };
    let RevisionDekRecord::CandidateNonServing(candidate) =
        decode_revision_dek_record(candidate.as_slice())
            .map_err(|_| RevisionDekSelectionError::InvalidRecord)?
    else {
        return Err(RevisionDekSelectionError::InvalidRecord);
    };
    let V::U64(epoch) = epoch else {
        return Err(RevisionDekSelectionError::InvalidRecord);
    };
    RevisionDekApplicationIntent::new(
        candidate,
        fixed(operation)?,
        policy(fixed(config)?, fixed(section)?, fixed(registry)?)?,
        c::OwnerEpoch::new(epoch).map_err(|_| RevisionDekSelectionError::InvalidRecord)?,
    )
}
fn bytes(value: &[u8]) -> Result<V, RevisionDekSelectionError> {
    c::BoundedBytes::new(value.to_vec())
        .map(V::Bytes)
        .map_err(encoding)
}
fn text(value: &str) -> Result<V, RevisionDekSelectionError> {
    c::CanonicalText::new(value).map(V::Text).map_err(encoding)
}
fn array(values: Vec<V>) -> Result<V, RevisionDekSelectionError> {
    c::BoundedList::new(values).map(V::Array).map_err(encoding)
}
fn fields<const N: usize>(value: V) -> Result<[V; N], RevisionDekSelectionError> {
    let V::Array(values) = value else {
        return Err(RevisionDekSelectionError::InvalidRecord);
    };
    values
        .into_vec()
        .try_into()
        .map_err(|_| RevisionDekSelectionError::InvalidRecord)
}
fn fixed<const N: usize>(value: V) -> Result<[u8; N], RevisionDekSelectionError> {
    let V::Bytes(value) = value else {
        return Err(RevisionDekSelectionError::InvalidRecord);
    };
    value
        .as_slice()
        .try_into()
        .map_err(|_| RevisionDekSelectionError::InvalidRecord)
}
fn hash(domain: &str, value: &V) -> Result<[u8; 32], RevisionDekSelectionError> {
    let domain = c::CanonicalDigestDomain::parse(domain).map_err(encoding)?;
    let limit =
        c::DigestInputLimit::new(REVISION_DEK_SELECTION_MAX_RECORD_BYTES).map_err(encoding)?;
    c::blake3_canonical(&domain, value, limit)
        .map(|digest| *digest.as_bytes())
        .map_err(encoding)
}
const fn encoding(_: c::ContractError) -> RevisionDekSelectionError {
    RevisionDekSelectionError::Encoding
}
macro_rules! metadata_debug {
    ($($name:ident),+) => { $(impl fmt::Debug for $name {
        fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
            formatter.write_str(concat!(stringify!($name), "(<metadata>)"))
        }
    })+ };
}
metadata_debug!(
    RevisionDekSelectionPolicy,
    RevisionDekApplicationIntent,
    RevisionDekCurrent,
    RevisionDekSelectionRecord
);

#[cfg(test)]
#[path = "revision_selection/tests.rs"]
mod tests;

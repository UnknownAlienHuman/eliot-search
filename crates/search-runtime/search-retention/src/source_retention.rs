//! C28 source retention cause record for issue #598.
//!
//! One `UntilExplicitRelease` retention cause bound to one exact physical
//! object identity, its complete residency key digest, its typed source
//! coordinates and its original operation. The record is deliberately small,
//! typed, bounded and canonical so the daemon can commit it as an opaque
//! payload inside the existing atomic control-store compare-and-set.
//!
//! Nothing here is authority. Constructors and codecs bind caller-supplied
//! metadata only: they prove no durability, no access decision and no purge
//! outcome. Durability comes exclusively from the original control-store
//! commit, and the current live purge always dominates this record. This
//! module performs no physical deletion, defaults no unknown value and infers
//! no missing root.
//!
//! The cause projects onto the existing `RetentionRootKind::RetentionPolicy`
//! durable root. No expiring lease, new store, catalog, state machine,
//! controller, clock or identity issuer is introduced.

use core::fmt;

use search_contracts::{
    Blake3Digest32, BoundedBytes, BoundedList, CanonicalText, CanonicalValue,
    ObjectResidencyKeyDigest, OpaqueId, SourceId, SourceNamespaceId, SourceRevisionId,
    parse_canonical_cbor, to_canonical_cbor,
};

use crate::RetentionOperation;
use crate::sweep::{DurableRoot, RetentionRootKind};

/// Magic prefix for one encoded source-retention cause record.
pub const SOURCE_RETENTION_RECORD_MAGIC: &[u8; 8] = b"ELSRRP01";

/// Exact codec version accepted by [`decode_source_retention_record`].
pub const SOURCE_RETENTION_RECORD_VERSION: u32 = 1;

/// Maximum total encoded record bytes, header included.
pub const MAX_SOURCE_RETENTION_RECORD_BYTES: usize = 8_192;

/// Exact fixed header bytes preceding the canonical payload.
pub const SOURCE_RETENTION_RECORD_HEADER_BYTES: usize = 16;

/// Pure technical key prefix for one record.
///
/// The prefix is derived addressing only. It allocates no namespace, mints no
/// identity and reserves nothing in any store.
pub const SOURCE_RETENTION_RECORD_KEY_PREFIX: &str = "eliot-search/source-retention/v1/";

/// Inner canonical payload marker.
const RECORD_MARKER: &str = "eliot.source-retention.cause.v1";

/// Exact canonical array field count for this codec version.
const RECORD_FIELDS: usize = 13;

/// Closed wire spelling of the active state.
const STATE_ACTIVE: &str = "active";

/// Closed wire spelling of the released state.
const STATE_RELEASED: &str = "released";

/// Closed source retention cause.
///
/// Exactly one cause is accepted for this record: protection that survives
/// until one exact explicit release operation removes it.
#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub enum SourceRetentionCause {
    /// Retained until one exact explicit release operation removes this cause.
    UntilExplicitRelease,
}

impl SourceRetentionCause {
    /// All closed causes in canonical order.
    pub const ALL: &'static [Self] = &[Self::UntilExplicitRelease];

    /// Stable wire spelling.
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::UntilExplicitRelease => "until_explicit_release",
        }
    }

    /// Parses the closed cause; anything else is rejected.
    ///
    /// # Errors
    /// Returns [`SourceRetentionRecordError::UnknownCause`] for a foreign cause.
    pub fn parse(value: &str) -> Result<Self, SourceRetentionRecordError> {
        if value == Self::UntilExplicitRelease.as_str() {
            Ok(Self::UntilExplicitRelease)
        } else {
            Err(SourceRetentionRecordError::UnknownCause)
        }
    }
}

/// Closed activation state of one retention cause.
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum SourceRetentionState {
    /// The cause protects its exact object; one durable root may project.
    Active,
    /// One exact release operation removed this cause.
    Released {
        /// Exact release operation identity and canonical request digest.
        release_operation: RetentionOperation,
    },
}

/// One bounded, typed, canonical source retention cause record.
///
/// The struct literal is the constructor and it is untrusted metadata: a
/// constructed or decoded record proves nothing durable. Only the original
/// control-store commit makes a record durable, and the current live purge
/// always dominates it.
///
/// `object_id` is the exact physical object identity. It is never a lease
/// identity and never a source revision identity; those are carried as their
/// own typed fields.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct SourceRetentionRecord {
    /// Exact physical object identity protected by this cause.
    pub object_id: OpaqueId,
    /// Complete residency key digest of that exact object.
    pub residency_digest: ObjectResidencyKeyDigest,
    /// Typed source namespace.
    pub namespace_id: SourceNamespaceId,
    /// Typed source identity.
    pub source_id: SourceId,
    /// Typed immutable source revision identity.
    pub source_revision_id: SourceRevisionId,
    /// Closed protection cause.
    pub cause: SourceRetentionCause,
    /// Original operation that created this cause.
    pub operation: RetentionOperation,
    /// Current activation state.
    pub state: SourceRetentionState,
}

/// Closed record codec and projection failure.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum SourceRetentionRecordError {
    /// Outer or inner marker mismatch: not this record.
    MagicMismatch,
    /// Record version other than [`SOURCE_RETENTION_RECORD_VERSION`].
    VersionUnsupported,
    /// Input is shorter than its header or declared payload.
    Truncated,
    /// Input carries bytes beyond its declared payload.
    TrailingBytes,
    /// Encoded record exceeds [`MAX_SOURCE_RETENTION_RECORD_BYTES`].
    RecordTooLarge,
    /// Payload is not canonical, not an array, or holds a foreign field type.
    Malformed,
    /// Canonical array carries more fields than this version defines.
    UnknownField,
    /// Foreign protection cause.
    UnknownCause,
    /// Foreign activation state.
    UnknownState,
    /// An identity field is empty or malformed.
    InvalidIdentity,
    /// A released record cannot project an active durable root.
    NotActive,
    /// One operation identity was reused with a different release request.
    OperationConflict,
    /// The exact durable root of this cause is absent; it is never inferred.
    RootMissing,
    /// The exact durable root of this cause appears more than once.
    RootAmbiguous,
}

impl SourceRetentionRecordError {
    /// Stable machine-readable reason code.
    #[must_use]
    pub const fn code(self) -> &'static str {
        match self {
            Self::MagicMismatch => "RETENTION_POLICY_RECORD_MAGIC_MISMATCH",
            Self::VersionUnsupported => "RETENTION_POLICY_RECORD_VERSION_UNSUPPORTED",
            Self::Truncated => "RETENTION_POLICY_RECORD_TRUNCATED",
            Self::TrailingBytes => "RETENTION_POLICY_RECORD_TRAILING_BYTES",
            Self::RecordTooLarge => "RETENTION_POLICY_RECORD_TOO_LARGE",
            Self::Malformed => "RETENTION_POLICY_RECORD_MALFORMED",
            Self::UnknownField => "RETENTION_POLICY_RECORD_UNKNOWN_FIELD",
            Self::UnknownCause => "RETENTION_POLICY_RECORD_UNKNOWN_CAUSE",
            Self::UnknownState => "RETENTION_POLICY_RECORD_UNKNOWN_STATE",
            Self::InvalidIdentity => "RETENTION_POLICY_RECORD_INVALID_IDENTITY",
            Self::NotActive => "RETENTION_POLICY_RECORD_NOT_ACTIVE",
            Self::OperationConflict => "RETENTION_POLICY_RECORD_OPERATION_CONFLICT",
            Self::RootMissing => "RETENTION_POLICY_RECORD_ROOT_MISSING",
            Self::RootAmbiguous => "RETENTION_POLICY_RECORD_ROOT_AMBIGUOUS",
        }
    }
}

impl fmt::Display for SourceRetentionRecordError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(self.code())
    }
}

impl std::error::Error for SourceRetentionRecordError {}

/// Deterministic full-identity technical key for one record.
///
/// The key is the exact object identity text hex-encoded at full length: no
/// truncation, no digest substitution, no alias and no namespace allocation.
/// Distinct identities always produce distinct keys.
#[must_use]
pub fn source_retention_record_key(object_id: &OpaqueId) -> String {
    const HEX: &[u8; 16] = b"0123456789abcdef";
    let identity = object_id.as_str();
    let mut key = String::with_capacity(
        SOURCE_RETENTION_RECORD_KEY_PREFIX
            .len()
            .saturating_add(identity.len().saturating_mul(2)),
    );
    key.push_str(SOURCE_RETENTION_RECORD_KEY_PREFIX);
    for byte in identity.as_bytes() {
        key.push(char::from(HEX[usize::from(byte >> 4)]));
        key.push(char::from(HEX[usize::from(byte & 0x0f)]));
    }
    key
}

/// Encodes one record to its canonical bounded bytes.
///
/// Deterministic and bounded: the total encoding never exceeds
/// [`MAX_SOURCE_RETENTION_RECORD_BYTES`]. The result is metadata; it is not
/// durable until the original control-store commit accepts it.
///
/// # Errors
/// Returns [`SourceRetentionRecordError::RecordTooLarge`] when the canonical
/// payload cannot fit, and [`SourceRetentionRecordError::Malformed`] when a
/// field cannot be represented in the closed canonical model.
pub fn encode_source_retention_record(
    record: &SourceRetentionRecord,
) -> Result<Vec<u8>, SourceRetentionRecordError> {
    let (state, release_id, release_digest) = match &record.state {
        SourceRetentionState::Active => (STATE_ACTIVE, CanonicalValue::Null, CanonicalValue::Null),
        SourceRetentionState::Released { release_operation } => (
            STATE_RELEASED,
            text_value(release_operation.operation_id.as_str())?,
            bytes_value(release_operation.request_digest.as_bytes())?,
        ),
    };
    let payload = CanonicalValue::Array(
        BoundedList::new(vec![
            text_value(RECORD_MARKER)?,
            CanonicalValue::U64(u64::from(SOURCE_RETENTION_RECORD_VERSION)),
            text_value(record.object_id.as_str())?,
            bytes_value(record.residency_digest.as_bytes())?,
            bytes_value(record.namespace_id.as_bytes())?,
            bytes_value(record.source_id.as_bytes())?,
            bytes_value(record.source_revision_id.as_bytes())?,
            text_value(record.cause.as_str())?,
            text_value(record.operation.operation_id.as_str())?,
            bytes_value(record.operation.request_digest.as_bytes())?,
            text_value(state)?,
            release_id,
            release_digest,
        ])
        .map_err(|_| SourceRetentionRecordError::Malformed)?,
    );
    let encoded = to_canonical_cbor(&payload).map_err(|_| SourceRetentionRecordError::Malformed)?;
    let payload_bytes = encoded.as_slice();
    let total = payload_bytes
        .len()
        .checked_add(SOURCE_RETENTION_RECORD_HEADER_BYTES)
        .ok_or(SourceRetentionRecordError::RecordTooLarge)?;
    if total > MAX_SOURCE_RETENTION_RECORD_BYTES {
        return Err(SourceRetentionRecordError::RecordTooLarge);
    }
    let declared =
        u32::try_from(payload_bytes.len()).map_err(|_| SourceRetentionRecordError::RecordTooLarge)?;
    let mut out = Vec::with_capacity(total);
    out.extend_from_slice(SOURCE_RETENTION_RECORD_MAGIC);
    out.extend_from_slice(&SOURCE_RETENTION_RECORD_VERSION.to_be_bytes());
    out.extend_from_slice(&declared.to_be_bytes());
    out.extend_from_slice(payload_bytes);
    Ok(out)
}

/// Decodes one record, failing closed on any mismatch.
///
/// The codec preserves bytes only. It performs no authority check, decides no
/// access and never defaults a missing field, root or generation.
///
/// # Errors
/// Returns [`SourceRetentionRecordError`] on marker, version, length,
/// canonical, field-shape, identity, cause or state mismatch.
pub fn decode_source_retention_record(
    bytes: &[u8],
) -> Result<SourceRetentionRecord, SourceRetentionRecordError> {
    if bytes.len() > MAX_SOURCE_RETENTION_RECORD_BYTES {
        return Err(SourceRetentionRecordError::RecordTooLarge);
    }
    if bytes.len() < SOURCE_RETENTION_RECORD_HEADER_BYTES {
        return Err(SourceRetentionRecordError::Truncated);
    }
    if &bytes[..SOURCE_RETENTION_RECORD_MAGIC.len()] != SOURCE_RETENTION_RECORD_MAGIC {
        return Err(SourceRetentionRecordError::MagicMismatch);
    }
    let version = u32::from_be_bytes([bytes[8], bytes[9], bytes[10], bytes[11]]);
    if version != SOURCE_RETENTION_RECORD_VERSION {
        return Err(SourceRetentionRecordError::VersionUnsupported);
    }
    let declared = usize::try_from(u32::from_be_bytes([bytes[12], bytes[13], bytes[14], bytes[15]]))
        .map_err(|_| SourceRetentionRecordError::RecordTooLarge)?;
    let actual = bytes.len() - SOURCE_RETENTION_RECORD_HEADER_BYTES;
    if actual < declared {
        return Err(SourceRetentionRecordError::Truncated);
    }
    if actual > declared {
        return Err(SourceRetentionRecordError::TrailingBytes);
    }
    let payload = &bytes[SOURCE_RETENTION_RECORD_HEADER_BYTES..];
    let value = parse_canonical_cbor(payload).map_err(|_| SourceRetentionRecordError::Malformed)?;
    let CanonicalValue::Array(fields) = value else {
        return Err(SourceRetentionRecordError::Malformed);
    };
    let fields = fields.as_slice();
    if fields.len() < RECORD_FIELDS {
        return Err(SourceRetentionRecordError::Truncated);
    }
    if fields.len() > RECORD_FIELDS {
        return Err(SourceRetentionRecordError::UnknownField);
    }
    let [
        marker,
        version,
        object_id,
        residency,
        namespace,
        source,
        revision,
        cause,
        operation_id,
        request_digest,
        state,
        release_id,
        release_digest,
    ] = fields
    else {
        return Err(SourceRetentionRecordError::Malformed);
    };
    if expect_text(marker)? != RECORD_MARKER {
        return Err(SourceRetentionRecordError::MagicMismatch);
    }
    if expect_u64(version)? != u64::from(SOURCE_RETENTION_RECORD_VERSION) {
        return Err(SourceRetentionRecordError::VersionUnsupported);
    }
    let state = match expect_text(state)? {
        STATE_ACTIVE => {
            if !matches!(release_id, CanonicalValue::Null)
                || !matches!(release_digest, CanonicalValue::Null)
            {
                return Err(SourceRetentionRecordError::Malformed);
            }
            SourceRetentionState::Active
        }
        STATE_RELEASED => {
            let (CanonicalValue::Text(release_id), CanonicalValue::Bytes(release_digest)) =
                (release_id, release_digest)
            else {
                return Err(SourceRetentionRecordError::Malformed);
            };
            let operation_id = OpaqueId::new(release_id.as_str())
                .map_err(|_| SourceRetentionRecordError::InvalidIdentity)?;
            let request_digest =
                Blake3Digest32::from_bytes(expect_bytes32(release_digest.as_slice())?);
            SourceRetentionState::Released {
                release_operation: RetentionOperation {
                    operation_id,
                    request_digest,
                },
            }
        }
        _ => return Err(SourceRetentionRecordError::UnknownState),
    };
    Ok(SourceRetentionRecord {
        object_id: OpaqueId::new(expect_text(object_id)?)
            .map_err(|_| SourceRetentionRecordError::InvalidIdentity)?,
        residency_digest: ObjectResidencyKeyDigest::from_bytes(expect_bytes32(expect_bytes(
            residency,
        )?)?),
        namespace_id: SourceNamespaceId::from_bytes(expect_bytes16(expect_bytes(namespace)?)?),
        source_id: SourceId::from_bytes(expect_bytes16(expect_bytes(source)?)?),
        source_revision_id: SourceRevisionId::from_bytes(expect_bytes16(expect_bytes(revision)?)?),
        cause: SourceRetentionCause::parse(expect_text(cause)?)?,
        operation: RetentionOperation {
            operation_id: OpaqueId::new(expect_text(operation_id)?)
                .map_err(|_| SourceRetentionRecordError::InvalidIdentity)?,
            request_digest: Blake3Digest32::from_bytes(expect_bytes32(expect_bytes(request_digest)?)?),
        },
        state,
    })
}

/// Applies one exact release operation, removing only this cause.
///
/// Pure and idempotent. An active record becomes `Released` with the exact
/// release operation, every other field preserved byte for byte. Releasing an
/// already released record with the same operation returns it unchanged; a
/// different operation refuses instead of overwriting the recorded one. No
/// object is deleted and no other root, pin or hold is touched.
///
/// # Errors
/// Returns [`SourceRetentionRecordError::OperationConflict`] when a released
/// record is presented with a different release operation.
pub fn release_source_retention_record(
    record: &SourceRetentionRecord,
    release_operation: &RetentionOperation,
) -> Result<SourceRetentionRecord, SourceRetentionRecordError> {
    match &record.state {
        SourceRetentionState::Active => {
            let mut released = record.clone();
            released.state = SourceRetentionState::Released {
                release_operation: release_operation.clone(),
            };
            Ok(released)
        }
        SourceRetentionState::Released {
            release_operation: recorded,
        } => {
            if recorded == release_operation {
                Ok(record.clone())
            } else {
                Err(SourceRetentionRecordError::OperationConflict)
            }
        }
    }
}

/// Projects one committed active record to its existing durable root.
///
/// `control_generation` must be the actual coherent control readback
/// generation the caller observed. This function never invents, advances or
/// defaults a generation. A released record never yields an active root.
///
/// # Errors
/// Returns [`SourceRetentionRecordError::NotActive`] for a released record.
pub fn project_source_retention_root(
    record: &SourceRetentionRecord,
    control_generation: u64,
) -> Result<DurableRoot, SourceRetentionRecordError> {
    if !matches!(record.state, SourceRetentionState::Active) {
        return Err(SourceRetentionRecordError::NotActive);
    }
    Ok(DurableRoot {
        object_id: record.object_id.clone(),
        kind: RetentionRootKind::RetentionPolicy,
        residency_digest: record.residency_digest,
        control_generation,
    })
}

/// Returns the same roots with only this cause's exact root removed.
///
/// The exact root is identified by physical object identity, the closed
/// [`RetentionRootKind::RetentionPolicy`] kind, the complete residency digest
/// and the caller's coherent readback generation. It must be present exactly
/// once: a missing root refuses instead of being inferred, and a duplicated
/// root refuses instead of being collapsed. Every other root is preserved in
/// its original order, and no pin or hold is consulted or altered.
///
/// # Errors
/// Returns [`SourceRetentionRecordError::RootMissing`] when the exact root is
/// absent and [`SourceRetentionRecordError::RootAmbiguous`] when it is
/// duplicated.
pub fn retained_roots_without_cause(
    roots: &[DurableRoot],
    record: &SourceRetentionRecord,
    control_generation: u64,
) -> Result<Vec<DurableRoot>, SourceRetentionRecordError> {
    let exact = |root: &DurableRoot| {
        root.object_id == record.object_id
            && root.kind == RetentionRootKind::RetentionPolicy
            && root.residency_digest == record.residency_digest
            && root.control_generation == control_generation
    };
    let mut matching = roots.iter().filter(|root| exact(root));
    let Some(found) = matching.next() else {
        return Err(SourceRetentionRecordError::RootMissing);
    };
    if matching.next().is_some() {
        return Err(SourceRetentionRecordError::RootAmbiguous);
    }
    let found = found.clone();
    Ok(roots
        .iter()
        .filter(|root| **root != found)
        .cloned()
        .collect())
}

fn text_value(value: &str) -> Result<CanonicalValue, SourceRetentionRecordError> {
    CanonicalText::new(value)
        .map(CanonicalValue::Text)
        .map_err(|_| SourceRetentionRecordError::Malformed)
}

fn bytes_value(value: &[u8]) -> Result<CanonicalValue, SourceRetentionRecordError> {
    BoundedBytes::new(value.to_vec())
        .map(CanonicalValue::Bytes)
        .map_err(|_| SourceRetentionRecordError::Malformed)
}

fn expect_text(value: &CanonicalValue) -> Result<&str, SourceRetentionRecordError> {
    match value {
        CanonicalValue::Text(text) => Ok(text.as_str()),
        _ => Err(SourceRetentionRecordError::Malformed),
    }
}

const fn expect_u64(value: &CanonicalValue) -> Result<u64, SourceRetentionRecordError> {
    match value {
        CanonicalValue::U64(number) => Ok(*number),
        _ => Err(SourceRetentionRecordError::Malformed),
    }
}

fn expect_bytes(value: &CanonicalValue) -> Result<&[u8], SourceRetentionRecordError> {
    match value {
        CanonicalValue::Bytes(bytes) => Ok(bytes.as_slice()),
        _ => Err(SourceRetentionRecordError::Malformed),
    }
}

fn expect_bytes32(value: &[u8]) -> Result<[u8; 32], SourceRetentionRecordError> {
    <[u8; 32]>::try_from(value).map_err(|_| SourceRetentionRecordError::Malformed)
}

fn expect_bytes16(value: &[u8]) -> Result<[u8; 16], SourceRetentionRecordError> {
    <[u8; 16]>::try_from(value).map_err(|_| SourceRetentionRecordError::Malformed)
}


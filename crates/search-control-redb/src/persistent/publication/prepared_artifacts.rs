//! Versioned technical references to opaque C07 set objects in the existing journal.

use super::{
    Blake3Digest32, Check, ConditionalControlMutation, ControlError, ControlKey,
    ControlRecordClass, ControlRecordCondition, ControlValue, ControlWrite, JournalLimits, Point,
};
use crate::persistent::bootstrap::OriginalCheck;
use crate::{
    JournalIdentity, JournalReadSnapshot, MigrationBootstrapFailure, MigrationBootstrapWork as Work,
};
use search_contracts::{
    self as c, CanonicalValue as V, MaterializationId, OpaqueId, ReceiptRef, RepresentationId,
    SourceId, SourceNamespaceId, SourceRevisionId, SourceRevisionRef,
};
use std::time::Instant;

const PREFIX: &[u8] = b"publication_intents/prepared-artifact-set/v1/";
const REF_FAMILY: &str = "c02:prepared-artifact-set:";
const REF_PREFIX: &str = "c02:prepared-artifact-set:v1:";
const DOMAIN: &str = "eliot/raw/c02-prepared-artifact-set-record/v1";
const MARKER: &str = "eliot.c02.prepared-artifact-set.v1";
const MAX_RECORD: usize = 48 * 1024;
/// Maximum opaque canonical C07 set-object reference accepted by this profile.
pub const MAX_PREPARED_ARTIFACT_SET_REFERENCE_BYTES: usize = 20 * 1024;
const MAX_RESIDENCY: usize = 8 * 1024;

/// Owner-supplied observations, never a C07 write/read or live-owner proof.
///
/// Root must compare all fields with the actual C07 receipt, full descriptor,
/// genuine source/MatRep identities and current six-domain policy before use.
#[derive(Clone, Eq, PartialEq)]
pub struct PreparedArtifactSetMetadataV1 {
    /// Original Root operation, distinct from C02 phase mutation IDs.
    pub operation_id: OpaqueId,
    /// Full immutable original source revision provenance.
    pub source_revision: SourceRevisionRef,
    /// Genuine Root-issued materialization identity, retained as data.
    pub materialization_id: MaterializationId,
    /// Genuine Root-issued representation identity, retained as data.
    pub representation_id: RepresentationId,
    /// Full six-domain canonical owner encoding, retained verbatim and opaque.
    pub residency_canonical: Vec<u8>,
    /// Complete actual C07 set-object reference encoding, never the set body.
    pub object_reference_canonical: Vec<u8>,
    /// Digest of the actual stored C07 set-object bytes, not its semantic digest.
    pub stored_object_digest: Blake3Digest32,
    /// Length of the actual stored C07 set-object bytes.
    pub stored_object_length: u64,
}
impl core::fmt::Debug for PreparedArtifactSetMetadataV1 {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        f.write_str("PreparedArtifactSetMetadataV1([REDACTED])")
    }
}

/// Exact C02 record candidate. Its reference names these full persisted bytes.
///
/// Construction is pure: neither this value nor its reference proves persistence,
/// C07 I/O, source truth, access, retention reachability or visible publication.
#[derive(Clone, Eq, PartialEq)]
pub struct PreparedArtifactSetRecordV1 {
    metadata: PreparedArtifactSetMetadataV1,
    canonical: Vec<u8>,
    reference: ReceiptRef,
}
impl core::fmt::Debug for PreparedArtifactSetRecordV1 {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        f.write_str("PreparedArtifactSetRecordV1([REDACTED])")
    }
}
impl PreparedArtifactSetRecordV1 {
    /// Encodes and hashes bounded technical metadata under the original allowance.
    /// The fixed C00 domain identifies the actual full C02 record, not C07 I/O.
    /// # Errors
    /// Refuses over-limit/noncanonical opaque metadata or the original context refusal.
    pub fn prepare_with_original_context<F, E>(
        metadata: PreparedArtifactSetMetadataV1,
        limits: JournalLimits,
        deadline: Instant,
        work: &F,
    ) -> Result<Self, MigrationBootstrapFailure<E>>
    where
        F: Fn(Work) -> Result<(), E>,
    {
        let check = OriginalCheck::registry(deadline, work);
        let result = (|| {
            check.check(Point::Start)?;
            let record = Self::prepare(metadata, limits, &check)?;
            check.check(Point::ReadComplete)?;
            Ok(record)
        })();
        result.map_err(|error| check.failure(error))
    }
    /// Exact technical metadata, requiring independent Root/C07 verification.
    #[must_use]
    pub const fn metadata(&self) -> &PreparedArtifactSetMetadataV1 {
        &self.metadata
    }
    /// Versioned full-record locator, usable only when this record is actually durable.
    #[must_use]
    pub const fn reference(&self) -> &ReceiptRef {
        &self.reference
    }

    fn prepare(
        metadata: PreparedArtifactSetMetadataV1,
        limits: JournalLimits,
        check: &dyn Check,
    ) -> Result<Self, ControlError> {
        if metadata.residency_canonical.is_empty()
            || metadata.residency_canonical.len() > MAX_RESIDENCY
            || metadata.object_reference_canonical.is_empty()
            || metadata.object_reference_canonical.len() > MAX_PREPARED_ARTIFACT_SET_REFERENCE_BYTES
            || metadata.stored_object_length == 0
        {
            return Err(ControlError::InvalidValue);
        }
        reserve(MAX_RECORD, check)?;
        // Canonical framing only: C02 never parses C07's descriptor or domain codec.
        c::parse_canonical_cbor(&metadata.residency_canonical)
            .map_err(|_| ControlError::InvalidValue)?;
        c::parse_canonical_cbor(&metadata.object_reference_canonical)
            .map_err(|_| ControlError::InvalidValue)?;
        let source = metadata.source_revision;
        let fields = vec![
            text(MARKER)?,
            V::U64(1),
            text(metadata.operation_id.as_str())?,
            bytes(source.source_namespace_id.as_bytes())?,
            bytes(source.source_id.as_bytes())?,
            bytes(source.revision_id.as_bytes())?,
            bytes(source.content_digest.as_bytes())?,
            V::U64(source.byte_length),
            bytes(metadata.materialization_id.as_bytes())?,
            bytes(metadata.representation_id.as_bytes())?,
            bytes(&metadata.residency_canonical)?,
            bytes(&metadata.object_reference_canonical)?,
            bytes(metadata.stored_object_digest.as_bytes())?,
            V::U64(metadata.stored_object_length),
        ];
        let value = V::Array(c::BoundedList::new(fields).map_err(|_| ControlError::InvalidValue)?);
        let canonical = c::to_canonical_cbor(&value).map_err(|_| ControlError::InvalidValue)?;
        if canonical.len() > MAX_RECORD || canonical.len() > limits.max_value_bytes {
            return Err(ControlError::BudgetExceeded);
        }
        let domain =
            c::CanonicalDigestDomain::parse(DOMAIN).map_err(|_| ControlError::InvalidValue)?;
        let limit = c::DigestInputLimit::new(c::MAX_CANONICAL_BYTES)
            .map_err(|_| ControlError::InvalidValue)?;
        let digest = c::blake3_raw(&domain, canonical.as_slice(), limit)
            .map_err(|_| ControlError::InvalidValue)?;
        let reference = ReceiptRef::new(format!("{REF_PREFIX}{}", hex(digest.as_bytes())))
            .map_err(|_| ControlError::InvalidValue)?;
        Ok(Self {
            metadata,
            canonical: canonical.as_slice().to_vec(),
            reference,
        })
    }

    pub(in crate::persistent) fn value(
        &self,
        limits: JournalLimits,
        check: &dyn Check,
    ) -> Result<ControlValue, ControlError> {
        reserve(self.canonical.len(), check)?;
        ControlValue::new(ControlRecordClass::State, self.canonical.clone(), limits)
    }
    pub(in crate::persistent) fn key(
        &self,
        limits: JournalLimits,
    ) -> Result<ControlKey, ControlError> {
        reference_key(&self.reference, limits)
    }
    pub(in crate::persistent) fn decode(
        value: &ControlValue,
        limits: JournalLimits,
        check: &dyn Check,
    ) -> Result<Self, ControlError> {
        if value.class() != ControlRecordClass::State || value.len() > MAX_RECORD {
            return Err(ControlError::StoreCorrupt);
        }
        reserve(value.len(), check)?;
        let V::Array(fields) =
            c::parse_canonical_cbor(value.as_bytes()).map_err(|_| ControlError::StoreCorrupt)?
        else {
            return Err(ControlError::StoreCorrupt);
        };
        let [
            V::Text(marker),
            V::U64(1),
            V::Text(operation),
            V::Bytes(namespace),
            V::Bytes(source),
            V::Bytes(revision),
            V::Bytes(source_digest),
            V::U64(source_length),
            V::Bytes(materialization),
            V::Bytes(representation),
            V::Bytes(residency),
            V::Bytes(object),
            V::Bytes(stored_digest),
            V::U64(stored_length),
        ] = fields.as_slice()
        else {
            return Err(ControlError::StoreCorrupt);
        };
        if marker.as_str() != MARKER {
            return Err(ControlError::StoreCorrupt);
        }
        let metadata = PreparedArtifactSetMetadataV1 {
            operation_id: OpaqueId::new(operation.as_str())
                .map_err(|_| ControlError::StoreCorrupt)?,
            source_revision: SourceRevisionRef {
                source_namespace_id: SourceNamespaceId::from_bytes(fixed(namespace.as_slice())?),
                source_id: SourceId::from_bytes(fixed(source.as_slice())?),
                revision_id: SourceRevisionId::from_bytes(fixed(revision.as_slice())?),
                content_digest: Blake3Digest32::from_stored_bytes(fixed(source_digest.as_slice())?),
                byte_length: *source_length,
            },
            materialization_id: MaterializationId::from_bytes(fixed(materialization.as_slice())?),
            representation_id: RepresentationId::from_bytes(fixed(representation.as_slice())?),
            residency_canonical: residency.as_slice().to_vec(),
            object_reference_canonical: object.as_slice().to_vec(),
            stored_object_digest: Blake3Digest32::from_stored_bytes(fixed(
                stored_digest.as_slice(),
            )?),
            stored_object_length: *stored_length,
        };
        let record = Self::prepare(metadata, limits, check).map_err(|error| match error {
            ControlError::ReadCancelled | ControlError::BudgetExceeded => error,
            _ => ControlError::StoreCorrupt,
        })?;
        if record.canonical != value.as_bytes() {
            return Err(ControlError::StoreCorrupt);
        }
        Ok(record)
    }
}

/// Actual coherent journal readback; this still grants no C07 or Root authority.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct PreparedArtifactSetReadbackV1 {
    pub(in crate::persistent) identity: JournalIdentity,
    pub(in crate::persistent) generation: u64,
    pub(in crate::persistent) record: PreparedArtifactSetRecordV1,
}
impl PreparedArtifactSetReadbackV1 {
    /// Actual journal identity from this disk read.
    #[must_use]
    pub const fn identity(&self) -> JournalIdentity {
        self.identity
    }
    /// Exact generation supplying these bytes.
    #[must_use]
    pub const fn generation(&self) -> u64 {
        self.generation
    }
    /// Full exact persisted technical record, not an external I/O receipt.
    #[must_use]
    pub const fn record(&self) -> &PreparedArtifactSetRecordV1 {
        &self.record
    }
}

pub(in crate::persistent) fn reserved(key: &[u8]) -> bool {
    key.starts_with(PREFIX) || super::preparation_input::reserved(key)
}
pub(in crate::persistent) fn is_reference(reference: &ReceiptRef) -> bool {
    reference.as_str().starts_with(REF_FAMILY)
}
pub(in crate::persistent) fn reference_key(
    reference: &ReceiptRef,
    limits: JournalLimits,
) -> Result<ControlKey, ControlError> {
    let suffix = reference
        .as_str()
        .strip_prefix(REF_PREFIX)
        .ok_or(ControlError::InvalidValue)?;
    if suffix.len() != 64
        || !suffix
            .bytes()
            .all(|b| b.is_ascii_digit() || (b'a'..=b'f').contains(&b))
    {
        return Err(ControlError::InvalidValue);
    }
    let mut key = PREFIX.to_vec();
    key.extend_from_slice(suffix.as_bytes());
    ControlKey::new(key, limits)
}

pub(in crate::persistent) fn attach(
    reference: &ReceiptRef,
    record: Option<&PreparedArtifactSetRecordV1>,
    create: bool,
    limits: JournalLimits,
    check: &dyn Check,
    command: ConditionalControlMutation,
) -> Result<ConditionalControlMutation, ControlError> {
    let Some(record) = record else {
        return if is_reference(reference) {
            Err(ControlError::InvalidValue)
        } else {
            Ok(command)
        };
    };
    if reference != record.reference() {
        return Err(ControlError::OperationConflict);
    }
    let key = record.key(limits)?;
    let value = record.value(limits, check)?;
    let (mutation, mut conditions) = command.into_parts();
    let mut writes = mutation.writes().to_vec();
    if create {
        conditions.push(ControlRecordCondition::absent(key.clone()));
        writes.push(ControlWrite { key, value });
    } else {
        conditions.push(ControlRecordCondition::exact(key, value));
    }
    Ok(ConditionalControlMutation::new(
        crate::ControlMutation::new(
            mutation.id(),
            mutation.command_digest(),
            mutation.expected_generation(),
            writes,
            mutation.deletes().to_vec(),
        ),
        conditions,
    ))
}

pub(in crate::persistent) fn validate_reference(
    snapshot: &JournalReadSnapshot,
    reference: &ReceiptRef,
    limits: JournalLimits,
    check: &dyn Check,
) -> Result<(), ControlError> {
    read_reference(snapshot, reference, limits, check).map(|_| ())
}

/// Resolve existing full metadata from this exact verified read transaction.
pub(in crate::persistent) fn read_reference(
    snapshot: &JournalReadSnapshot,
    reference: &ReceiptRef,
    limits: JournalLimits,
    check: &dyn Check,
) -> Result<Option<PreparedArtifactSetRecordV1>, ControlError> {
    if !is_reference(reference) {
        return Ok(None);
    }
    let key = reference_key(reference, limits).map_err(|_| ControlError::StoreCorrupt)?;
    let record = PreparedArtifactSetRecordV1::decode(
        snapshot.get(&key).ok_or(ControlError::StoreCorrupt)?,
        limits,
        check,
    )?;
    if record.reference() != reference {
        return Err(ControlError::StoreCorrupt);
    }
    Ok(Some(record))
}

pub(in crate::persistent) fn validate_records(
    snapshot: &JournalReadSnapshot,
    limits: JournalLimits,
    check: &dyn Check,
) -> Result<(), ControlError> {
    let first = snapshot
        .records
        .partition_point(|(key, _)| key.as_bytes() < PREFIX);
    for (key, value) in snapshot
        .records
        .iter()
        .skip(first)
        .take_while(|(key, _)| reserved(key.as_bytes()))
    {
        check.check(Point::ReadRecord)?;
        if !crate::has_visibility_schema(snapshot.identity.schema_version) {
            return Err(ControlError::SchemaMismatch);
        }
        let record = PreparedArtifactSetRecordV1::decode(value, limits, check)?;
        if record.key(limits)? != *key {
            return Err(ControlError::StoreCorrupt);
        }
    }
    super::preparation_input::validate_records(snapshot, limits, check)
}

fn reserve(size: usize, check: &dyn Check) -> Result<(), ControlError> {
    let bound = size.checked_mul(16).ok_or(ControlError::BudgetExceeded)?;
    check.reserve(Work::Allocate(bound))?;
    check.reserve(Work::Process(bound))
}
fn fixed<const N: usize>(bytes: &[u8]) -> Result<[u8; N], ControlError> {
    bytes.try_into().map_err(|_| ControlError::StoreCorrupt)
}
fn text(value: &str) -> Result<V, ControlError> {
    Ok(V::Text(
        c::CanonicalText::new(value).map_err(|_| ControlError::InvalidValue)?,
    ))
}
fn bytes(value: &[u8]) -> Result<V, ControlError> {
    Ok(V::Bytes(
        c::BoundedBytes::new(value.to_vec()).map_err(|_| ControlError::InvalidValue)?,
    ))
}
fn hex(bytes: &[u8]) -> String {
    const DIGITS: &[u8; 16] = b"0123456789abcdef";
    let mut result = String::with_capacity(bytes.len() * 2);
    for byte in bytes {
        result.push(char::from(DIGITS[usize::from(byte >> 4)]));
        result.push(char::from(DIGITS[usize::from(byte & 15)]));
    }
    result
}

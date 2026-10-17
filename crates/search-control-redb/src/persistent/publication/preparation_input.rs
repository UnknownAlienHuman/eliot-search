//! Immutable, operation-keyed preparation inputs for the pre-effect C02 boundary.
//!
//! This record binds the original source, membership, materialization,
//! representation, projection and collection-schema inputs before a C07 worker
//! may perform effects. It stores bounded canonical technical metadata only;
//! source and vector bodies remain outside redb.

use super::{
    Blake3Digest32, Check, ControlError, ControlKey, ControlRecordClass, ControlValue,
    JournalLimits, Point,
};
use crate::persistent::bootstrap::OriginalCheck;
use crate::{
    ConditionalControlMutation, ControlMutation, ControlRecordCondition, ControlWrite,
    JournalIdentity, MigrationBootstrapFailure, MigrationBootstrapWork as Work, MutationId,
};
use search_contracts::indexed::CollectionSchema;
use search_contracts::schema::AssuranceClass;
use search_contracts::{
    self as c, AccessPartitionId, AccessPolicyBindingId, BoundedList, CanonicalValue as V,
    CorpusId, Materialization, MaterializationId, MembershipRevision, MembershipRole, OpaqueId,
    ObjectResidencyKeyDigest, ProfileId, ProjectionMembership, ProjectionMembershipId,
    Representation, RepresentationId, ResidencyPolicyBindingId, ScoringPartitionId, SourceId,
    SourceMembership, SourceMembershipId, SourceNamespaceId, SourceRevisionId, SourceRevisionRef,
    WorkspaceId,
};
use std::time::Instant;

const PREFIX: &[u8] = b"publication_intents/preparation-input/v1/";
const MARKER: &str = "eliot.c02.preparation-input.v1";
const DOMAIN: &str = "eliot/raw/c02-preparation-input-record/v1";

/// Maximum bytes in one complete immutable C02 preparation-input record.
pub const MAX_PREPARATION_INPUT_RECORD_BYTES: usize = 48 * 1024;
/// Maximum canonical residency metadata retained by one input record.
pub const MAX_PREPARATION_INPUT_RESIDENCY_BYTES: usize = 8 * 1024;
/// Maximum canonical policy metadata retained by one input record.
pub const MAX_PREPARATION_INPUT_POLICY_BYTES: usize = 8 * 1024;
const MAX_COLLECTION_SCHEMA_BYTES: usize = 16 * 1024;

/// Exact inputs captured before any C07 preparation effect.
///
/// The nested contract values carry the real registry and profile identities.
/// The canonical policy fields are complete bounded metadata supplied by their
/// owners. This type does not authenticate those observations; the Root caller
/// must first obtain and verify them from their authoritative owners.
#[derive(Clone, Eq, PartialEq)]
pub struct PreparationInputMetadataV1 {
    /// Original Root operation; this exact UTF-8 identity also addresses the redb row.
    pub original_operation_id: OpaqueId,
    /// Stable C02 transaction identity retained for exact retry and recovery.
    pub c02_mutation_id: MutationId,
    /// Original journal generation used by the C02 compare-and-swap.
    pub expected_generation: u64,
    /// Complete immutable source-revision reference.
    pub source_revision: SourceRevisionRef,
    /// Exact accepted source membership, including its workspace and corpus.
    pub source_membership: SourceMembership,
    /// Exact accepted materialization and qualified materializer profile.
    pub materialization: Materialization,
    /// Exact accepted representation, unitizer and enrichment profiles.
    pub representation: Representation,
    /// Exact accepted projection membership, access and scoring partitions.
    pub projection_membership: ProjectionMembership,
    /// Complete qualified collection schema, including generation and profile set.
    pub collection_schema: CollectionSchema,
    /// Full canonical residency metadata; never source or object bytes.
    pub residency_canonical: Vec<u8>,
    /// Full canonical policy metadata; never source or vector bytes.
    pub policy_canonical: Vec<u8>,
}

impl core::fmt::Debug for PreparationInputMetadataV1 {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        f.write_str("PreparationInputMetadataV1([REDACTED])")
    }
}

/// Exact canonical preparation input, keyed by its original operation.
///
/// Construction is a bounded codec operation and performs no storage or C07
/// I/O. Its key is the original operation identifier itself, never a digest
/// alias or a generated UUID.
#[derive(Clone, Eq, PartialEq)]
pub struct PreparationInputRecordV1 {
    metadata: PreparationInputMetadataV1,
    canonical: Vec<u8>,
    command_digest: Blake3Digest32,
}

impl core::fmt::Debug for PreparationInputRecordV1 {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        f.write_str("PreparationInputRecordV1([REDACTED])")
    }
}

impl PreparationInputRecordV1 {
    /// Validates and canonically encodes bounded metadata under the original allowance.
    ///
    /// # Errors
    /// Refuses conflicting typed identities, invalid schema, oversized or noncanonical
    /// metadata, or the first original-context refusal.
    pub fn prepare_with_original_context<F, E>(
        metadata: PreparationInputMetadataV1,
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
            reserve(MAX_PREPARATION_INPUT_RECORD_BYTES, &check)?;
            let record = Self::prepare(metadata, limits, &check)?;
            check.check(Point::ReadComplete)?;
            Ok(record)
        })();
        result.map_err(|error| check.failure(error))
    }

    /// Exact retained metadata. It remains an observation for Root verification.
    #[must_use]
    pub const fn metadata(&self) -> &PreparationInputMetadataV1 {
        &self.metadata
    }

    /// The original operation used as the persistent record key.
    #[must_use]
    pub const fn original_operation_id(&self) -> &OpaqueId {
        &self.metadata.original_operation_id
    }

    /// Stable C02 operation identity used by the original transaction.
    #[must_use]
    pub const fn c02_mutation_id(&self) -> MutationId {
        self.metadata.c02_mutation_id
    }

    /// Original expected generation retained for exact replay and recovery.
    #[must_use]
    pub const fn expected_generation(&self) -> u64 {
        self.metadata.expected_generation
    }

    pub(in crate::persistent) fn command(
        &self,
        limits: JournalLimits,
        check: &dyn Check,
    ) -> Result<ConditionalControlMutation, ControlError> {
        check.check(Point::Start)?;
        let key_len = operation_key_len(&self.metadata.original_operation_id);
        let key_allocation = key_len
            .checked_add(core::mem::size_of::<ControlKey>())
            .ok_or(ControlError::BudgetExceeded)?;
        check.reserve(Work::Allocate(key_allocation))?;
        check.reserve(Work::Process(key_len))?;
        let key = key_for_operation(&self.metadata.original_operation_id, limits)?;
        reserve(self.canonical.len(), check)?;
        let value = ControlValue::new(
            ControlRecordClass::State,
            self.canonical.clone(),
            limits,
        )?;
        let mutation = ControlMutation::new(
            self.metadata.c02_mutation_id,
            self.command_digest,
            self.metadata.expected_generation,
            vec![ControlWrite {
                key: key.clone(),
                value,
            }],
            vec![],
        );
        Ok(ConditionalControlMutation::new(
            mutation,
            vec![ControlRecordCondition::absent(key)],
        ))
    }

    pub(in crate::persistent) fn key(
        &self,
        limits: JournalLimits,
    ) -> Result<ControlKey, ControlError> {
        key_for_operation(&self.metadata.original_operation_id, limits)
    }

    fn prepare(
        metadata: PreparationInputMetadataV1,
        limits: JournalLimits,
        check: &dyn Check,
    ) -> Result<Self, ControlError> {
        validate_metadata(&metadata)?;
        if metadata.residency_canonical.is_empty()
            || metadata.residency_canonical.len() > MAX_PREPARATION_INPUT_RESIDENCY_BYTES
            || metadata.policy_canonical.is_empty()
            || metadata.policy_canonical.len() > MAX_PREPARATION_INPUT_POLICY_BYTES
        {
            return Err(ControlError::InvalidValue);
        }
        validate_canonical_metadata(&metadata.residency_canonical)?;
        validate_canonical_metadata(&metadata.policy_canonical)?;

        let schema_bytes = metadata
            .collection_schema
            .to_canonical_cbor()
            .map_err(|_| ControlError::InvalidValue)?;
        if schema_bytes.as_slice().len() > MAX_COLLECTION_SCHEMA_BYTES {
            return Err(ControlError::BudgetExceeded);
        }
        let schema_identity = metadata
            .collection_schema
            .identity()
            .map_err(|_| ControlError::InvalidValue)?;

        let fields = vec![
            text(MARKER)?,
            V::U64(1),
            text(metadata.original_operation_id.as_str())?,
            bytes(&metadata.c02_mutation_id.0)?,
            V::U64(metadata.expected_generation),
            source_revision_value(&metadata.source_revision)?,
            source_membership_value(&metadata.source_membership)?,
            materialization_value(&metadata.materialization)?,
            representation_value(&metadata.representation)?,
            projection_membership_value(&metadata.projection_membership)?,
            bytes(schema_bytes.as_slice())?,
            bytes(schema_identity.as_digest().as_bytes())?,
            bytes(&metadata.residency_canonical)?,
            bytes(&metadata.policy_canonical)?,
        ];
        let value = V::Array(
            c::BoundedList::new(fields).map_err(|_| ControlError::InvalidValue)?,
        );
        let canonical = c::to_canonical_cbor(&value)
            .map_err(|_| ControlError::InvalidValue)?
            .as_slice()
            .to_vec();
        if canonical.len() > MAX_PREPARATION_INPUT_RECORD_BYTES
            || canonical.len() > limits.max_value_bytes
        {
            return Err(ControlError::BudgetExceeded);
        }
        let command_digest = record_digest(&canonical, check)?;
        Ok(Self {
            metadata,
            canonical,
            command_digest,
        })
    }

    pub(in crate::persistent) fn decode(
        value: &ControlValue,
        expected_operation: &OpaqueId,
        limits: JournalLimits,
        check: &dyn Check,
    ) -> Result<Self, ControlError> {
        if value.class() != ControlRecordClass::State
            || value.len() > MAX_PREPARATION_INPUT_RECORD_BYTES
        {
            return Err(ControlError::StoreCorrupt);
        }
        reserve(value.len(), check)?;
        let parsed = c::parse_canonical_cbor(value.as_bytes())
            .map_err(|_| ControlError::StoreCorrupt)?;
        let fields = array_fields(&parsed, 14).map_err(|_| ControlError::StoreCorrupt)?;
        if text_value(&fields[0]).map_err(|_| ControlError::StoreCorrupt)? != MARKER
            || uint_value(&fields[1]).map_err(|_| ControlError::StoreCorrupt)? != 1
        {
            return Err(ControlError::StoreCorrupt);
        }
        let operation = OpaqueId::new(
            text_value(&fields[2]).map_err(|_| ControlError::StoreCorrupt)?,
        )
        .map_err(|_| ControlError::StoreCorrupt)?;
        if &operation != expected_operation {
            return Err(ControlError::StoreCorrupt);
        }
        let mutation_bytes =
            fixed_bytes::<32>(&fields[3]).map_err(|_| ControlError::StoreCorrupt)?;
        let metadata = PreparationInputMetadataV1 {
            original_operation_id: operation,
            c02_mutation_id: MutationId(mutation_bytes),
            expected_generation: uint_value(&fields[4]).map_err(|_| ControlError::StoreCorrupt)?,
            source_revision: decode_source_revision(&fields[5])?,
            source_membership: decode_source_membership(&fields[6])?,
            materialization: decode_materialization(&fields[7])?,
            representation: decode_representation(&fields[8])?,
            projection_membership: decode_projection_membership(&fields[9])?,
            collection_schema: {
                let schema_bytes =
                    byte_slice(&fields[10]).map_err(|_| ControlError::StoreCorrupt)?;
                let schema_digest = Blake3Digest32::from_bytes(
                    fixed_bytes::<32>(&fields[11]).map_err(|_| ControlError::StoreCorrupt)?,
                );
                CollectionSchema::restore(schema_bytes, schema_digest)
                    .map_err(|_| ControlError::StoreCorrupt)?
            },
            residency_canonical: byte_slice(&fields[12])
                .map_err(|_| ControlError::StoreCorrupt)?
                .to_vec(),
            policy_canonical: byte_slice(&fields[13])
                .map_err(|_| ControlError::StoreCorrupt)?
                .to_vec(),
        };
        let record =
            Self::prepare(metadata, limits, check).map_err(|_| ControlError::StoreCorrupt)?;
        if record.canonical.as_slice() != value.as_bytes() {
            return Err(ControlError::StoreCorrupt);
        }
        Ok(record)
    }
}

/// Exact operation-keyed readback from one bounded journal point read.
#[derive(Clone, Eq, PartialEq)]
pub struct PreparationInputReadbackV1 {
    identity: JournalIdentity,
    generation: u64,
    record: PreparationInputRecordV1,
}

impl core::fmt::Debug for PreparationInputReadbackV1 {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        f.debug_struct("PreparationInputReadbackV1")
            .field("generation", &self.generation)
            .field("record", &self.record)
            .finish_non_exhaustive()
    }
}

impl PreparationInputReadbackV1 {
    pub(in crate::persistent) const fn new(
        identity: JournalIdentity,
        generation: u64,
        record: PreparationInputRecordV1,
    ) -> Self {
        Self { identity, generation, record }
    }

    /// Exact journal identity that supplied the read.
    #[must_use]
    pub const fn identity(&self) -> JournalIdentity {
        self.identity
    }

    /// Journal generation read with the immutable input row.
    #[must_use]
    pub const fn generation(&self) -> u64 {
        self.generation
    }

    /// Full canonical input record recovered from the operation key.
    #[must_use]
    pub const fn record(&self) -> &PreparationInputRecordV1 {
        &self.record
    }
}

pub(in crate::persistent) fn reserved(key: &[u8]) -> bool {
    key.starts_with(PREFIX)
}

pub(in crate::persistent) fn validate_records(
    snapshot: &crate::JournalReadSnapshot,
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
        validate_record(key, value, limits, check)?;
    }
    Ok(())
}

pub(in crate::persistent) fn validate_record(
    key: &ControlKey,
    value: &ControlValue,
    limits: JournalLimits,
    check: &dyn Check,
) -> Result<(), ControlError> {
    let suffix = key
        .as_bytes()
        .strip_prefix(PREFIX)
        .ok_or(ControlError::StoreCorrupt)?;
    let operation = std::str::from_utf8(suffix).map_err(|_| ControlError::StoreCorrupt)?;
    let operation = OpaqueId::new(operation).map_err(|_| ControlError::StoreCorrupt)?;
    let record = PreparationInputRecordV1::decode(value, &operation, limits, check)?;
    if record.key(limits)? != *key {
        return Err(ControlError::StoreCorrupt);
    }
    Ok(())
}

pub(in crate::persistent) fn operation_key_len(operation: &OpaqueId) -> usize {
    PREFIX.len() + operation.as_str().len()
}

pub(in crate::persistent) fn key_for_operation(
    operation: &OpaqueId,
    limits: JournalLimits,
) -> Result<ControlKey, ControlError> {
    let mut key = PREFIX.to_vec();
    key.extend_from_slice(operation.as_str().as_bytes());
    ControlKey::new(key, limits)
}

fn validate_metadata(metadata: &PreparationInputMetadataV1) -> Result<(), ControlError> {
    if metadata.source_revision.source_id != metadata.source_membership.source_id {
        return Err(ControlError::OperationConflict);
    }
    if metadata.materialization.source_revision_id != metadata.source_revision.revision_id {
        return Err(ControlError::OperationConflict);
    }
    if metadata.representation.materialization_id != metadata.materialization.materialization_id {
        return Err(ControlError::OperationConflict);
    }
    if metadata.projection_membership.source_membership_id
        != metadata.source_membership.source_membership_id
    {
        return Err(ControlError::OperationConflict);
    }
    if metadata.projection_membership.representation_id != metadata.representation.representation_id {
        return Err(ControlError::OperationConflict);
    }
    metadata
        .collection_schema
        .validate()
        .map_err(|_| ControlError::InvalidValue)?;
    Ok(())
}
fn validate_canonical_metadata(value: &[u8]) -> Result<(), ControlError> {
    c::parse_canonical_cbor(value).map_err(|_| ControlError::InvalidValue)?;
    Ok(())
}

fn source_revision_value(source: &SourceRevisionRef) -> Result<V, ControlError> {
    array_value(vec![
        bytes(source.source_namespace_id.as_bytes())?,
        bytes(source.source_id.as_bytes())?,
        bytes(source.revision_id.as_bytes())?,
        bytes(source.content_digest.as_bytes())?,
        V::U64(source.byte_length),
    ])
}

fn source_membership_value(member: &SourceMembership) -> Result<V, ControlError> {
    array_value(vec![
        bytes(member.source_membership_id.as_bytes())?,
        bytes(member.corpus_id.as_bytes())?,
        bytes(member.source_id.as_bytes())?,
        bytes(member.workspace_id.as_bytes())?,
        text(membership_role(member.role))?,
        text(member.preparation_profile_id.as_str())?,
        bytes(member.access_policy_binding_id.as_bytes())?,
        text(member.retention_policy_id.as_str())?,
        bytes(member.residency_policy_binding_id.as_bytes())?,
        V::U64(member.membership_revision.get()),
    ])
}

fn materialization_value(value: &Materialization) -> Result<V, ControlError> {
    array_value(vec![
        bytes(value.materialization_id.as_bytes())?,
        bytes(value.source_revision_id.as_bytes())?,
        text(value.materializer_profile_id.as_str())?,
        bytes(value.canonical_object_digest.as_bytes())?,
        bytes(value.object_residency_key_digest.as_bytes())?,
        bytes(value.native_coordinate_map_digest.as_bytes())?,
        bytes(value.loss_map_digest.as_bytes())?,
        text(value.assurance_ceiling.as_str())?,
    ])
}

fn representation_value(value: &Representation) -> Result<V, ControlError> {
    let profiles = value
        .enrichment_profile_ids
        .iter()
        .map(|profile| text(profile.as_str()))
        .collect::<Result<Vec<_>, _>>()?;
    array_value(vec![
        bytes(value.representation_id.as_bytes())?,
        bytes(value.materialization_id.as_bytes())?,
        text(value.unitizer_profile_id.as_str())?,
        array_value(profiles)?,
        bytes(value.unit_manifest_digest.as_bytes())?,
    ])
}

fn projection_membership_value(value: &ProjectionMembership) -> Result<V, ControlError> {
    array_value(vec![
        bytes(value.projection_membership_id.as_bytes())?,
        bytes(value.source_membership_id.as_bytes())?,
        bytes(value.representation_id.as_bytes())?,
        bytes(value.access_partition_id.as_bytes())?,
        bytes(value.scoring_partition_id.as_bytes())?,
        text(value.projection_schema_id.as_str())?,
    ])
}

fn decode_source_revision(value: &V) -> Result<SourceRevisionRef, ControlError> {
    let fields = array_fields(value, 5)?;
    Ok(SourceRevisionRef {
        source_namespace_id: uuid16(&fields[0], SourceNamespaceId::from_bytes)?,
        source_id: uuid16(&fields[1], SourceId::from_bytes)?,
        revision_id: uuid16(&fields[2], SourceRevisionId::from_bytes)?,
        content_digest: digest32(&fields[3])?,
        byte_length: uint_value(&fields[4])?,
    })
}

fn decode_source_membership(value: &V) -> Result<SourceMembership, ControlError> {
    let fields = array_fields(value, 10)?;
    Ok(SourceMembership {
        source_membership_id: uuid16(&fields[0], SourceMembershipId::from_bytes)?,
        corpus_id: uuid16(&fields[1], CorpusId::from_bytes)?,
        source_id: uuid16(&fields[2], SourceId::from_bytes)?,
        workspace_id: uuid16(&fields[3], WorkspaceId::from_bytes)?,
        role: parse_membership_role(text_value(&fields[4])?)?,
        preparation_profile_id: profile_id(&fields[5])?,
        access_policy_binding_id: uuid16(&fields[6], AccessPolicyBindingId::from_bytes)?,
        retention_policy_id: profile_id(&fields[7])?,
        residency_policy_binding_id: uuid16(&fields[8], ResidencyPolicyBindingId::from_bytes)?,
        membership_revision: MembershipRevision::new(uint_value(&fields[9])?),
    })
}

fn decode_materialization(value: &V) -> Result<Materialization, ControlError> {
    let fields = array_fields(value, 8)?;
    Ok(Materialization {
        materialization_id: uuid16(&fields[0], MaterializationId::from_bytes)?,
        source_revision_id: uuid16(&fields[1], SourceRevisionId::from_bytes)?,
        materializer_profile_id: profile_id(&fields[2])?,
        canonical_object_digest: digest32(&fields[3])?,
        object_residency_key_digest: object_residency_digest(&fields[4])?,
        native_coordinate_map_digest: digest32(&fields[5])?,
        loss_map_digest: digest32(&fields[6])?,
        assurance_ceiling: AssuranceClass::parse(text_value(&fields[7])?)
            .map_err(|_| ControlError::StoreCorrupt)?,
    })
}

fn decode_representation(value: &V) -> Result<Representation, ControlError> {
    let fields = array_fields(value, 5)?;
    let V::Array(profile_values) = &fields[3] else {
        return Err(ControlError::StoreCorrupt);
    };
    let profiles = profile_values
        .iter()
        .map(|value| match value {
            V::Text(profile) => {
                ProfileId::new(profile.as_str()).map_err(|_| ControlError::StoreCorrupt)
            }
            _ => Err(ControlError::StoreCorrupt),
        })
        .collect::<Result<Vec<_>, _>>()?;
    Ok(Representation {
        representation_id: uuid16(&fields[0], RepresentationId::from_bytes)?,
        materialization_id: uuid16(&fields[1], MaterializationId::from_bytes)?,
        unitizer_profile_id: profile_id(&fields[2])?,
        enrichment_profile_ids: BoundedList::new(profiles)
            .map_err(|_| ControlError::StoreCorrupt)?,
        unit_manifest_digest: digest32(&fields[4])?,
    })
}

fn decode_projection_membership(value: &V) -> Result<ProjectionMembership, ControlError> {
    let fields = array_fields(value, 6)?;
    Ok(ProjectionMembership {
        projection_membership_id: uuid16(&fields[0], ProjectionMembershipId::from_bytes)?,
        source_membership_id: uuid16(&fields[1], SourceMembershipId::from_bytes)?,
        representation_id: uuid16(&fields[2], RepresentationId::from_bytes)?,
        access_partition_id: uuid16(&fields[3], AccessPartitionId::from_bytes)?,
        scoring_partition_id: uuid16(&fields[4], ScoringPartitionId::from_bytes)?,
        projection_schema_id: profile_id(&fields[5])?,
    })
}

const fn membership_role(role: MembershipRole) -> &'static str {
    match role {
        MembershipRole::Source => "source",
        MembershipRole::Test => "test",
        MembershipRole::Documentation => "documentation",
        MembershipRole::Generated => "generated",
        MembershipRole::Vendor => "vendor",
        MembershipRole::Reference => "reference",
    }
}

fn parse_membership_role(value: &str) -> Result<MembershipRole, ControlError> {
    match value {
        "source" => Ok(MembershipRole::Source),
        "test" => Ok(MembershipRole::Test),
        "documentation" => Ok(MembershipRole::Documentation),
        "generated" => Ok(MembershipRole::Generated),
        "vendor" => Ok(MembershipRole::Vendor),
        "reference" => Ok(MembershipRole::Reference),
        _ => Err(ControlError::StoreCorrupt),
    }
}

const fn uint_value(value: &V) -> Result<u64, ControlError> {
    match value {
        V::U64(value) => Ok(*value),
        _ => Err(ControlError::StoreCorrupt),
    }
}

fn record_digest(value: &[u8], check: &dyn Check) -> Result<Blake3Digest32, ControlError> {
    check.reserve(Work::Process(value.len()))?;
    let domain = c::CanonicalDigestDomain::parse(DOMAIN)
        .map_err(|_| ControlError::InvalidValue)?;
    let limit = c::DigestInputLimit::new(c::MAX_CANONICAL_BYTES)
        .map_err(|_| ControlError::InvalidValue)?;
    c::blake3_raw(&domain, value, limit).map_err(|_| ControlError::InvalidValue)
}

fn reserve(size: usize, check: &dyn Check) -> Result<(), ControlError> {
    let bound = size.checked_mul(16).ok_or(ControlError::BudgetExceeded)?;
    check.reserve(Work::Allocate(bound))?;
    check.reserve(Work::Process(bound))
}

fn array_value(values: Vec<V>) -> Result<V, ControlError> {
    Ok(V::Array(
        c::BoundedList::new(values).map_err(|_| ControlError::InvalidValue)?,
    ))
}

fn array_fields(value: &V, expected: usize) -> Result<&[V], ControlError> {
    match value {
        V::Array(values) if values.len() == expected => Ok(values.as_slice()),
        _ => Err(ControlError::StoreCorrupt),
    }
}

fn bytes(value: &[u8]) -> Result<V, ControlError> {
    Ok(V::Bytes(
        c::BoundedBytes::new(value.to_vec()).map_err(|_| ControlError::InvalidValue)?,
    ))
}

fn text(value: &str) -> Result<V, ControlError> {
    Ok(V::Text(
        c::CanonicalText::new(value).map_err(|_| ControlError::InvalidValue)?,
    ))
}

fn text_value(value: &V) -> Result<&str, ControlError> {
    match value {
        V::Text(value) => Ok(value.as_str()),
        _ => Err(ControlError::StoreCorrupt),
    }
}

fn byte_slice(value: &V) -> Result<&[u8], ControlError> {
    match value {
        V::Bytes(value) => Ok(value.as_slice()),
        _ => Err(ControlError::StoreCorrupt),
    }
}

fn fixed_bytes<const N: usize>(value: &V) -> Result<[u8; N], ControlError> {
    byte_slice(value)?
        .try_into()
        .map_err(|_| ControlError::StoreCorrupt)
}

fn uuid16<T>(value: &V, from_bytes: fn([u8; 16]) -> T) -> Result<T, ControlError> {
    Ok(from_bytes(fixed_bytes::<16>(value)?))
}

fn digest32(value: &V) -> Result<Blake3Digest32, ControlError> {
    Ok(Blake3Digest32::from_bytes(fixed_bytes::<32>(value)?))
}

fn object_residency_digest(value: &V) -> Result<ObjectResidencyKeyDigest, ControlError> {
    Ok(ObjectResidencyKeyDigest::from_bytes(
        fixed_bytes::<32>(value)?,
    ))
}

fn profile_id(value: &V) -> Result<ProfileId, ControlError> {
    ProfileId::new(text_value(value)?).map_err(|_| ControlError::StoreCorrupt)
}

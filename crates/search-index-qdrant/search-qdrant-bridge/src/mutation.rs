//! Vendor-neutral mutation records and pure validation shared with the live adapter.

mod oracle;

use std::collections::BTreeMap;

use search_contracts::{
    AccessPartitionId, Blake3Digest32, BoundedSymbolKey, CollectionGenerationId, EntityKind,
    Epoch, InstallationIncarnationId, Modality, OpaqueId, ProfileId, ProjectionMembershipId,
    ProjectionProfileSetId, RepositoryLineageId, RepresentationId, ScoringDocumentId,
    ScoringPartitionId, SourceId, SourceRevisionId, UnitId, UnitKind,
};

use crate::{BridgeError, BridgeLimits, CollectionRoute, CollectionSchema, VectorSchema};

/// Provider-neutral exact 128-bit point ID.
#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub struct QdrantPointId(pub [u8; 16]);

/// Exact S9.5 opaque Qdrant point payload.
///
/// Source-membership arrays/names, ACL subjects, source text, paths, query text,
/// payload digests and vector digests are deliberately absent. Expected
/// payload/vector digests remain in the immutable projection manifest; exact
/// bridge readback returns the typed fields and vector values needed to verify it.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct PointPayload {
    pub installation_incarnation_id: InstallationIncarnationId,
    pub collection_generation_id: CollectionGenerationId,
    pub projection_membership_id: ProjectionMembershipId,
    pub access_partition_id: AccessPartitionId,
    pub scoring_partition_id: ScoringPartitionId,
    pub source_id: SourceId,
    pub source_revision_id: SourceRevisionId,
    pub representation_id: RepresentationId,
    pub unit_id: UnitId,
    pub point_identity_digest_256: Blake3Digest32,
    pub scoring_document_id: ScoringDocumentId,
    pub projection_profile_set_id: ProjectionProfileSetId,
    pub unit_kind: UnitKind,
    pub modality: Modality,
    pub language_or_format: ProfileId,
    pub entity_kind: Option<EntityKind>,
    pub normalized_symbol_key: Option<BoundedSymbolKey>,
    pub repository_lineage_id: Option<RepositoryLineageId>,
    pub valid_from_epoch: Epoch,
    pub valid_until_epoch_exclusive: Option<Epoch>,
}

impl PointPayload {
    pub const INSTALLATION_INCARNATION_FIELD: &'static str = "installation_incarnation_id";
    pub const COLLECTION_GENERATION_FIELD: &'static str = "collection_generation_id";
    pub const PROJECTION_MEMBERSHIP_FIELD: &'static str = "projection_membership_id";
    pub const ACCESS_PARTITION_FIELD: &'static str = "access_partition_id";
    pub const SCORING_PARTITION_FIELD: &'static str = "scoring_partition_id";
    pub const SOURCE_ID_FIELD: &'static str = "source_id";
    pub const SOURCE_REVISION_FIELD: &'static str = "source_revision_id";
    pub const REPRESENTATION_FIELD: &'static str = "representation_id";
    pub const UNIT_FIELD: &'static str = "unit_id";
    pub const POINT_IDENTITY_DIGEST_FIELD: &'static str = "point_identity_digest_256";
    pub const SCORING_DOCUMENT_FIELD: &'static str = "scoring_document_id";
    pub const PROJECTION_PROFILE_SET_FIELD: &'static str = "projection_profile_set_id";
    pub const UNIT_KIND_FIELD: &'static str = "unit_kind";
    pub const MODALITY_FIELD: &'static str = "modality";
    pub const LANGUAGE_OR_FORMAT_FIELD: &'static str = "language_or_format";
    pub const ENTITY_KIND_FIELD: &'static str = "entity_kind";
    pub const NORMALIZED_SYMBOL_FIELD: &'static str = "normalized_symbol_key";
    pub const REPOSITORY_LINEAGE_FIELD: &'static str = "repository_lineage_id";
    pub const VALID_FROM_FIELD: &'static str = "valid_from_epoch";
    pub const VALID_UNTIL_FIELD: &'static str = "valid_until_epoch_exclusive";

    /// Exact S9.5 payload field set. Unknown fields fail closed during decode.
    pub const PAYLOAD_FIELDS: [&'static str; 20] = [
        Self::INSTALLATION_INCARNATION_FIELD,
        Self::COLLECTION_GENERATION_FIELD,
        Self::PROJECTION_MEMBERSHIP_FIELD,
        Self::ACCESS_PARTITION_FIELD,
        Self::SCORING_PARTITION_FIELD,
        Self::SOURCE_ID_FIELD,
        Self::SOURCE_REVISION_FIELD,
        Self::REPRESENTATION_FIELD,
        Self::UNIT_FIELD,
        Self::POINT_IDENTITY_DIGEST_FIELD,
        Self::SCORING_DOCUMENT_FIELD,
        Self::PROJECTION_PROFILE_SET_FIELD,
        Self::UNIT_KIND_FIELD,
        Self::MODALITY_FIELD,
        Self::LANGUAGE_OR_FORMAT_FIELD,
        Self::ENTITY_KIND_FIELD,
        Self::NORMALIZED_SYMBOL_FIELD,
        Self::REPOSITORY_LINEAGE_FIELD,
        Self::VALID_FROM_FIELD,
        Self::VALID_UNTIL_FIELD,
    ];

    /// Exact baseline payload-index set from the qualified schema.
    pub const INDEXED_FIELDS: [&'static str; 19] = [
        Self::INSTALLATION_INCARNATION_FIELD,
        Self::COLLECTION_GENERATION_FIELD,
        Self::PROJECTION_MEMBERSHIP_FIELD,
        Self::ACCESS_PARTITION_FIELD,
        Self::SCORING_PARTITION_FIELD,
        Self::SOURCE_ID_FIELD,
        Self::SOURCE_REVISION_FIELD,
        Self::REPRESENTATION_FIELD,
        Self::UNIT_FIELD,
        Self::SCORING_DOCUMENT_FIELD,
        Self::PROJECTION_PROFILE_SET_FIELD,
        Self::UNIT_KIND_FIELD,
        Self::MODALITY_FIELD,
        Self::LANGUAGE_OR_FORMAT_FIELD,
        Self::ENTITY_KIND_FIELD,
        Self::NORMALIZED_SYMBOL_FIELD,
        Self::REPOSITORY_LINEAGE_FIELD,
        Self::VALID_FROM_FIELD,
        Self::VALID_UNTIL_FIELD,
    ];

    pub(crate) fn validate(&self) -> Result<(), BridgeError> {
        if self
            .valid_until_epoch_exclusive
            .is_some_and(|until| until <= self.valid_from_epoch)
        {
            return Err(BridgeError::PointPayloadInvalid);
        }
        Ok(())
    }
}

/// Exact named sparse vector values.
///
/// Vector digests are manifest metadata, not Qdrant payload. Exact readback
/// returns these values so the publication owner can verify its manifest.
#[derive(Clone, Debug, PartialEq)]
pub struct StoredVector {
    pub dimensions: u32,
    pub sparse: bool,
    pub values: Vec<(u32, f32)>,
}

impl StoredVector {
    pub(crate) fn validate(&self, schema: VectorSchema) -> Result<(), BridgeError> {
        if self.dimensions != schema.dimensions || self.sparse != schema.sparse {
            return Err(BridgeError::VectorDimensionMismatch);
        }
        if self.values.is_empty()
            || self.values.iter().any(|(_, value)| !value.is_finite())
            || self.values.windows(2).any(|pair| pair[0].0 >= pair[1].0)
            || self
                .values
                .last()
                .is_some_and(|(index, _)| *index >= self.dimensions)
        {
            return Err(BridgeError::VectorDimensionMismatch);
        }
        Ok(())
    }
}

/// Exact point record.
#[derive(Clone, Debug, PartialEq)]
pub struct PointRecord {
    pub point_id: QdrantPointId,
    pub payload: PointPayload,
    pub vectors: BTreeMap<String, StoredVector>,
}

/// Whether an existing point names the same immutable S11.1 logical identity.
///
/// The full 256-bit digest covers the point role. The remaining comparisons
/// bind every canonical key coordinate that is represented independently in
/// S9.5. Validity and vector values are excluded because they are mutation
/// state for that already-proven identity.
pub(crate) fn same_point_identity(existing: &PointRecord, expected: &PointRecord) -> bool {
    existing.point_id == expected.point_id
        && existing.payload.point_identity_digest_256
            == expected.payload.point_identity_digest_256
        && existing.payload.installation_incarnation_id
            == expected.payload.installation_incarnation_id
        && existing.payload.collection_generation_id
            == expected.payload.collection_generation_id
        && existing.payload.projection_membership_id
            == expected.payload.projection_membership_id
        && existing.payload.representation_id == expected.payload.representation_id
        && existing.payload.unit_id == expected.payload.unit_id
        && existing.payload.projection_profile_set_id
            == expected.payload.projection_profile_set_id
}

/// Validates an exact close as an immutable epoch transition.
///
/// An open point may be closed once. Retrying the same close value is allowed
/// so unknown-outcome recovery can converge through exact readback. Replacing
/// an already-published upper epoch with a different value is stale authority
/// and fails before mutation dispatch.
pub(crate) fn validate_close_epoch(
    payload: &PointPayload,
    requested: Epoch,
) -> Result<(), BridgeError> {
    if requested <= payload.valid_from_epoch {
        return Err(BridgeError::ExactReadbackMismatch);
    }
    match payload.valid_until_epoch_exclusive {
        None => Ok(()),
        Some(current) if current == requested => Ok(()),
        Some(_) => Err(BridgeError::ExactReadbackMismatch),
    }
}

/// Immutable exact mutation identity.
#[derive(Clone, Debug, Eq, Ord, PartialEq, PartialOrd)]
pub struct BridgeMutation {
    pub operation_id: OpaqueId,
    pub canonical_input_digest: Blake3Digest32,
}

/// Exact mutation receipt.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct MutationReceipt {
    pub operation_id: OpaqueId,
    pub canonical_input_digest: Blake3Digest32,
    pub route: CollectionRoute,
    pub affected_ids: Vec<QdrantPointId>,
    pub replayed: bool,
}

pub(crate) fn validate_point(
    point: &PointRecord,
    schema: &CollectionSchema,
    limits: BridgeLimits,
) -> Result<(), BridgeError> {
    point.payload.validate()?;
    if point.vectors.len() != schema.named_vectors.len() {
        return Err(BridgeError::NamedVectorMissing);
    }
    let mut stored_values = 0_usize;
    for (name, vector_schema) in &schema.named_vectors {
        let vector = point
            .vectors
            .get(name)
            .ok_or(BridgeError::NamedVectorMissing)?;
        vector.validate(*vector_schema)?;
        stored_values = stored_values
            .checked_add(vector.values.len())
            .ok_or(BridgeError::MutationTooLarge)?;
    }
    if stored_values > limits.max_vector_values_per_point {
        return Err(BridgeError::MutationTooLarge);
    }
    Ok(())
}

pub(crate) fn validate_exact_ids(
    mut ids: Vec<QdrantPointId>,
    limit: usize,
) -> Result<Vec<QdrantPointId>, BridgeError> {
    if ids.is_empty() || ids.len() > limit {
        return Err(BridgeError::MutationTooLarge);
    }
    ids.sort();
    if ids.windows(2).any(|pair| pair[0] == pair[1]) {
        return Err(BridgeError::DuplicatePointId);
    }
    Ok(ids)
}

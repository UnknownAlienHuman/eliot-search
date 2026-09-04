//! Vendor-neutral exact Qdrant data-plane semantics.
//!
//! This package does not discover or start Qdrant. A process supervisor supplies
//! an authenticated endpoint and exact process receipt. The in-memory model here
//! defines capability admission, collection schema, exact point mutation,
//! readback, count, and filtered nomination semantics for a concrete adapter.

#![forbid(unsafe_code)]
#![allow(
    clippy::large_enum_variant,
    clippy::missing_errors_doc,
    clippy::module_name_repetitions,
    clippy::too_many_arguments,
    clippy::too_many_lines
)]

use core::cmp::Ordering;
use core::fmt;
use std::collections::{BTreeMap, BTreeSet};

use search_contracts::{
    Blake3Digest32, CollectionGenerationId, Epoch, OpaqueId, OwnerEpoch, ReceiptRef,
};

/// Closed Qdrant bridge failure.
#[derive(Clone, Copy, Debug, Eq, Ord, PartialEq, PartialOrd)]
pub enum BridgeError {
    EndpointNotLoopback,
    AuthenticationInvalid,
    SupervisorReceiptMismatch,
    CapabilityProbeFailed,
    CapabilityReceiptMismatch,
    CollectionAlreadyExists,
    CollectionNotFound,
    CollectionSchemaMismatch,
    PayloadIndexMissing,
    NamedVectorMissing,
    VectorDimensionMismatch,
    StrictModeRequired,
    MutationTooLarge,
    DuplicatePointId,
    PointNotFound,
    OperationConflict,
    MutationOutcomeUnknown,
    ExactReadbackMismatch,
    UnexpectedPoint,
    InvalidFilter,
    UnindexedFilter,
    QueryBudgetExceeded,
    InvalidScore,
}

impl BridgeError {
    /// Stable machine-readable reason code.
    #[must_use]
    pub const fn code(self) -> &'static str {
        match self {
            Self::EndpointNotLoopback => "QDRANT_ENDPOINT_NOT_LOOPBACK",
            Self::AuthenticationInvalid => "QDRANT_AUTHENTICATION_INVALID",
            Self::SupervisorReceiptMismatch => "QDRANT_SUPERVISOR_RECEIPT_MISMATCH",
            Self::CapabilityProbeFailed => "QDRANT_CAPABILITY_PROBE_FAILED",
            Self::CapabilityReceiptMismatch => "QDRANT_CAPABILITY_RECEIPT_MISMATCH",
            Self::CollectionAlreadyExists => "QDRANT_COLLECTION_ALREADY_EXISTS",
            Self::CollectionNotFound => "QDRANT_COLLECTION_NOT_FOUND",
            Self::CollectionSchemaMismatch => "QDRANT_COLLECTION_SCHEMA_MISMATCH",
            Self::PayloadIndexMissing => "QDRANT_PAYLOAD_INDEX_MISSING",
            Self::NamedVectorMissing => "QDRANT_NAMED_VECTOR_MISSING",
            Self::VectorDimensionMismatch => "QDRANT_VECTOR_DIMENSION_MISMATCH",
            Self::StrictModeRequired => "QDRANT_STRICT_MODE_REQUIRED",
            Self::MutationTooLarge => "QDRANT_MUTATION_TOO_LARGE",
            Self::DuplicatePointId => "QDRANT_DUPLICATE_POINT_ID",
            Self::PointNotFound => "QDRANT_POINT_NOT_FOUND",
            Self::OperationConflict => "QDRANT_OPERATION_CONFLICT",
            Self::MutationOutcomeUnknown => "QDRANT_MUTATION_OUTCOME_UNKNOWN",
            Self::ExactReadbackMismatch => "QDRANT_EXACT_READBACK_MISMATCH",
            Self::UnexpectedPoint => "QDRANT_UNEXPECTED_POINT",
            Self::InvalidFilter => "QDRANT_INVALID_FILTER",
            Self::UnindexedFilter => "QDRANT_UNINDEXED_FILTER",
            Self::QueryBudgetExceeded => "QDRANT_QUERY_BUDGET_EXCEEDED",
            Self::InvalidScore => "QDRANT_INVALID_SCORE",
        }
    }
}

impl fmt::Display for BridgeError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(self.code())
    }
}

impl std::error::Error for BridgeError {}

/// Content-minimized authenticated endpoint identity.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct BridgeEndpoint {
    pub endpoint_digest: Blake3Digest32,
    pub loopback_only: bool,
}

/// Exact process/supervisor proof required before connecting.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct SupervisorReceipt {
    pub owner_epoch: OwnerEpoch,
    pub process_identity_digest: Blake3Digest32,
    pub artifact_digest: Blake3Digest32,
    pub endpoint_digest: Blake3Digest32,
}

/// Purpose-bound authentication-lease proof.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct AuthLeaseEvidence {
    pub reference_digest: Blake3Digest32,
    pub purpose_digest: Blake3Digest32,
    pub valid: bool,
}

/// Executed capability probe results.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct CapabilityProbeResults {
    pub authenticated_health: bool,
    pub single_shard: bool,
    pub signed_i64_ranges: bool,
    pub missing_upper_bound_must_not: bool,
    pub sparse_idf: bool,
    pub independent_idf_corpus: bool,
    pub strict_mode: bool,
    pub payload_indexes: bool,
    pub wait_for_mutations: bool,
    pub strong_ordering: bool,
    pub exact_count_and_readback: bool,
    pub named_sparse_vectors: bool,
}

impl CapabilityProbeResults {
    #[must_use]
    pub const fn all_required(self) -> bool {
        self.authenticated_health
            && self.single_shard
            && self.signed_i64_ranges
            && self.missing_upper_bound_must_not
            && self.sparse_idf
            && self.independent_idf_corpus
            && self.strict_mode
            && self.payload_indexes
            && self.wait_for_mutations
            && self.strong_ordering
            && self.exact_count_and_readback
            && self.named_sparse_vectors
    }
}

/// Exact capability admission receipt.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct QdrantCapabilityReceipt {
    pub process_identity_digest: Blake3Digest32,
    pub artifact_digest: Blake3Digest32,
    pub probe_manifest_digest: Blake3Digest32,
    pub results: CapabilityProbeResults,
}

/// Verifies every mandatory Qdrant capability probe.
pub fn probe_capabilities(
    supervisor: SupervisorReceipt,
    probe_manifest_digest: Blake3Digest32,
    results: CapabilityProbeResults,
) -> Result<QdrantCapabilityReceipt, BridgeError> {
    if !results.all_required() {
        return Err(BridgeError::CapabilityProbeFailed);
    }
    Ok(QdrantCapabilityReceipt {
        process_identity_digest: supervisor.process_identity_digest,
        artifact_digest: supervisor.artifact_digest,
        probe_manifest_digest,
        results,
    })
}

/// Named vector schema.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct VectorSchema {
    pub dimensions: u32,
    pub sparse: bool,
    pub idf_enabled: bool,
}

/// Exact collection schema and strict-mode correctness floors.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct CollectionSchema {
    pub named_vectors: BTreeMap<String, VectorSchema>,
    pub indexed_payload_fields: BTreeSet<String>,
    pub one_shard: bool,
    pub strict_mode: bool,
    pub wait_for_mutations: bool,
    pub strong_ordering: bool,
    pub schema_digest: Blake3Digest32,
}

impl CollectionSchema {
    pub fn validate(&self) -> Result<(), BridgeError> {
        if self.named_vectors.is_empty() {
            return Err(BridgeError::NamedVectorMissing);
        }
        if self
            .named_vectors
            .iter()
            .any(|(name, schema)| name.is_empty() || schema.dimensions == 0)
        {
            return Err(BridgeError::CollectionSchemaMismatch);
        }
        if !self.one_shard
            || !self.strict_mode
            || !self.wait_for_mutations
            || !self.strong_ordering
        {
            return Err(BridgeError::StrictModeRequired);
        }
        for field in EligibilityFilter::INDEXED_FIELDS {
            if !self.indexed_payload_fields.contains(field) {
                return Err(BridgeError::PayloadIndexMissing);
            }
        }
        Ok(())
    }
}

/// Opaque physical collection route.
#[derive(Clone, Debug, Eq, Ord, PartialEq, PartialOrd)]
pub struct CollectionRoute {
    pub generation: CollectionGenerationId,
    pub physical_name: OpaqueId,
}

/// Provider-neutral exact 128-bit point ID.
#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub struct QdrantPointId(pub [u8; 16]);

/// Minimal filterable payload.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct PointPayload {
    pub source_membership_id: OpaqueId,
    pub projection_membership_id: OpaqueId,
    pub access_partition_digest: Blake3Digest32,
    pub source_revision: u64,
    pub unit_ordinal: u64,
    pub valid_from_epoch: Epoch,
    pub valid_until_epoch_exclusive: Option<Epoch>,
    pub payload_digest: Blake3Digest32,
    pub identity_digest: Blake3Digest32,
}

/// Exact named vector values and digest.
#[derive(Clone, Debug, PartialEq)]
pub struct StoredVector {
    pub dimensions: u32,
    pub sparse: bool,
    pub values: Vec<(u32, f32)>,
    pub digest: Blake3Digest32,
}

impl StoredVector {
    fn validate(&self, schema: VectorSchema) -> Result<(), BridgeError> {
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

/// Finite bridge limits.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct BridgeLimits {
    pub max_points_per_mutation: usize,
    pub max_query_candidates: usize,
    pub max_vector_values_per_point: usize,
    pub max_operation_receipts: usize,
}

impl BridgeLimits {
    pub const BASELINE: Self = Self {
        max_points_per_mutation: 1_024,
        max_query_candidates: 4_096,
        max_vector_values_per_point: 65_536,
        max_operation_receipts: 65_536,
    };

    pub const fn validate(self) -> Result<Self, BridgeError> {
        if self.max_points_per_mutation == 0
            || self.max_query_candidates == 0
            || self.max_vector_values_per_point == 0
            || self.max_operation_receipts == 0
        {
            Err(BridgeError::MutationTooLarge)
        } else {
            Ok(self)
        }
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

/// Exact point readback with explicit missing/unexpected IDs.
#[derive(Clone, Debug, PartialEq)]
pub struct BoundedPointReadback {
    pub points: Vec<PointRecord>,
    pub missing_ids: Vec<QdrantPointId>,
    pub unexpected_ids: Vec<QdrantPointId>,
}

/// Closed indexed eligibility filter.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct EligibilityFilter {
    pub access_partition_digest: Blake3Digest32,
    pub allowed_source_memberships: BTreeSet<OpaqueId>,
    pub visible_epoch: Epoch,
}

impl EligibilityFilter {
    pub const INDEXED_FIELDS: [&'static str; 4] = [
        "access_partition_digest",
        "source_membership_id",
        "valid_from_epoch",
        "valid_until_epoch_exclusive",
    ];

    fn matches(&self, payload: &PointPayload) -> bool {
        payload.access_partition_digest == self.access_partition_digest
            && self
                .allowed_source_memberships
                .contains(&payload.source_membership_id)
            && payload.valid_from_epoch <= self.visible_epoch
            && payload
                .valid_until_epoch_exclusive
                .is_none_or(|until| self.visible_epoch < until)
    }
}

/// One bounded nomination returned by filtered retrieval.
#[derive(Clone, Debug, PartialEq)]
pub struct CandidateNomination {
    pub point_id: QdrantPointId,
    pub score: f32,
    pub payload_digest: Blake3Digest32,
    pub identity_digest: Blake3Digest32,
}

/// Exact-count result for the same closed filter language.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct ExactCount {
    pub count: usize,
}

#[derive(Clone, Debug)]
struct CollectionState {
    schema: CollectionSchema,
    points: BTreeMap<QdrantPointId, PointRecord>,
}

/// Deterministic reference bridge after capability admission.
#[derive(Clone, Debug)]
pub struct QdrantBridge {
    supervisor: SupervisorReceipt,
    capability: QdrantCapabilityReceipt,
    limits: BridgeLimits,
    collections: BTreeMap<CollectionRoute, CollectionState>,
    operations: BTreeMap<OpaqueId, MutationReceipt>,
}

impl QdrantBridge {
    /// Connects only to the exact authenticated loopback process.
    pub fn connect(
        endpoint: BridgeEndpoint,
        auth: AuthLeaseEvidence,
        supervisor: SupervisorReceipt,
        capability: QdrantCapabilityReceipt,
        limits: BridgeLimits,
    ) -> Result<Self, BridgeError> {
        if !endpoint.loopback_only {
            return Err(BridgeError::EndpointNotLoopback);
        }
        if !auth.valid {
            return Err(BridgeError::AuthenticationInvalid);
        }
        if endpoint.endpoint_digest != supervisor.endpoint_digest {
            return Err(BridgeError::SupervisorReceiptMismatch);
        }
        if capability.process_identity_digest != supervisor.process_identity_digest
            || capability.artifact_digest != supervisor.artifact_digest
            || !capability.results.all_required()
        {
            return Err(BridgeError::CapabilityReceiptMismatch);
        }
        Ok(Self {
            supervisor,
            capability,
            limits: limits.validate()?,
            collections: BTreeMap::new(),
            operations: BTreeMap::new(),
        })
    }

    #[must_use]
    pub const fn capability_receipt(&self) -> QdrantCapabilityReceipt {
        self.capability
    }

    #[must_use]
    pub const fn supervisor_receipt(&self) -> SupervisorReceipt {
        self.supervisor
    }

    /// Creates a new opaque physical generation after complete schema validation.
    pub fn create_candidate_collection(
        &mut self,
        route: CollectionRoute,
        schema: CollectionSchema,
    ) -> Result<ReceiptRef, BridgeError> {
        schema.validate()?;
        if self.collections.contains_key(&route) {
            return Err(BridgeError::CollectionAlreadyExists);
        }
        let receipt = ReceiptRef::new(format!(
            "qdrant:collection:{}",
            route.physical_name.as_str()
        ))
        .map_err(|_| BridgeError::CollectionSchemaMismatch)?;
        self.collections.insert(
            route,
            CollectionState {
                schema,
                points: BTreeMap::new(),
            },
        );
        Ok(receipt)
    }

    /// Verifies exact readback schema identity.
    pub fn verify_collection_schema(
        &self,
        route: &CollectionRoute,
        expected: &CollectionSchema,
    ) -> Result<ReceiptRef, BridgeError> {
        let actual = &self
            .collections
            .get(route)
            .ok_or(BridgeError::CollectionNotFound)?
            .schema;
        actual.validate()?;
        if actual != expected {
            return Err(BridgeError::CollectionSchemaMismatch);
        }
        ReceiptRef::new(format!("qdrant:schema:{}", route.physical_name.as_str()))
            .map_err(|_| BridgeError::CollectionSchemaMismatch)
    }

    /// Upserts only explicit point IDs with exact idempotency.
    pub fn upsert_exact(
        &mut self,
        route: &CollectionRoute,
        points: Vec<PointRecord>,
        mutation: BridgeMutation,
    ) -> Result<MutationReceipt, BridgeError> {
        if let Some(replay) = self.replay(&mutation)? {
            return Ok(replay);
        }
        if points.is_empty() || points.len() > self.limits.max_points_per_mutation {
            return Err(BridgeError::MutationTooLarge);
        }
        let collection = self
            .collections
            .get_mut(route)
            .ok_or(BridgeError::CollectionNotFound)?;
        let mut seen = BTreeSet::new();
        for point in &points {
            if !seen.insert(point.point_id) {
                return Err(BridgeError::DuplicatePointId);
            }
            validate_point(point, &collection.schema, self.limits)?;
        }
        let mut affected_ids = Vec::with_capacity(points.len());
        for point in points {
            affected_ids.push(point.point_id);
            collection.points.insert(point.point_id, point);
        }
        affected_ids.sort();
        self.record_mutation(route.clone(), mutation, affected_ids)
    }

    /// Sets the exact exclusive upper epoch on explicit point IDs.
    pub fn close_exact(
        &mut self,
        route: &CollectionRoute,
        ids: Vec<QdrantPointId>,
        valid_until_epoch_exclusive: Epoch,
        mutation: BridgeMutation,
    ) -> Result<MutationReceipt, BridgeError> {
        if let Some(replay) = self.replay(&mutation)? {
            return Ok(replay);
        }
        let ids = validate_exact_ids(ids, self.limits.max_points_per_mutation)?;
        let collection = self
            .collections
            .get_mut(route)
            .ok_or(BridgeError::CollectionNotFound)?;
        for id in &ids {
            let point = collection.points.get(id).ok_or(BridgeError::PointNotFound)?;
            if valid_until_epoch_exclusive <= point.payload.valid_from_epoch {
                return Err(BridgeError::ExactReadbackMismatch);
            }
        }
        for id in &ids {
            collection
                .points
                .get_mut(id)
                .expect("validated point exists")
                .payload
                .valid_until_epoch_exclusive = Some(valid_until_epoch_exclusive);
        }
        self.record_mutation(route.clone(), mutation, ids)
    }

    /// Deletes only explicit exact point IDs.
    pub fn delete_exact(
        &mut self,
        route: &CollectionRoute,
        ids: Vec<QdrantPointId>,
        mutation: BridgeMutation,
    ) -> Result<MutationReceipt, BridgeError> {
        if let Some(replay) = self.replay(&mutation)? {
            return Ok(replay);
        }
        let ids = validate_exact_ids(ids, self.limits.max_points_per_mutation)?;
        let collection = self
            .collections
            .get_mut(route)
            .ok_or(BridgeError::CollectionNotFound)?;
        for id in &ids {
            collection.points.remove(id);
        }
        self.record_mutation(route.clone(), mutation, ids)
    }

    /// Reads back exactly the requested identifiers.
    pub fn readback_exact(
        &self,
        route: &CollectionRoute,
        ids: Vec<QdrantPointId>,
    ) -> Result<BoundedPointReadback, BridgeError> {
        let ids = validate_exact_ids(ids, self.limits.max_points_per_mutation)?;
        let collection = self
            .collections
            .get(route)
            .ok_or(BridgeError::CollectionNotFound)?;
        let mut points = Vec::new();
        let mut missing_ids = Vec::new();
        for id in ids {
            match collection.points.get(&id) {
                Some(point) => points.push(point.clone()),
                None => missing_ids.push(id),
            }
        }
        Ok(BoundedPointReadback {
            points,
            missing_ids,
            unexpected_ids: Vec::new(),
        })
    }

    /// Counts points matching the closed indexed filter.
    pub fn count_exact(
        &self,
        route: &CollectionRoute,
        filter: &EligibilityFilter,
    ) -> Result<ExactCount, BridgeError> {
        validate_filter(filter)?;
        let collection = self
            .collections
            .get(route)
            .ok_or(BridgeError::CollectionNotFound)?;
        ensure_filter_indexes(&collection.schema)?;
        Ok(ExactCount {
            count: collection
                .points
                .values()
                .filter(|point| filter.matches(&point.payload))
                .count(),
        })
    }

    /// Returns bounded filtered nominations. These are not evidence until exact
    /// candidate readback and access revalidation occur outside the bridge.
    pub fn query_filtered(
        &self,
        route: &CollectionRoute,
        filter: &EligibilityFilter,
        vector_name: &str,
        query: &[(u32, f32)],
        limit: usize,
    ) -> Result<Vec<CandidateNomination>, BridgeError> {
        validate_filter(filter)?;
        if limit == 0 || limit > self.limits.max_query_candidates {
            return Err(BridgeError::QueryBudgetExceeded);
        }
        let collection = self
            .collections
            .get(route)
            .ok_or(BridgeError::CollectionNotFound)?;
        ensure_filter_indexes(&collection.schema)?;
        let vector_schema = collection
            .schema
            .named_vectors
            .get(vector_name)
            .ok_or(BridgeError::NamedVectorMissing)?;
        validate_query_vector(query, *vector_schema)?;

        let mut candidates = Vec::new();
        for point in collection.points.values() {
            if !filter.matches(&point.payload) {
                continue;
            }
            let vector = point
                .vectors
                .get(vector_name)
                .ok_or(BridgeError::NamedVectorMissing)?;
            let score = dot_sparse(query, &vector.values);
            if !score.is_finite() {
                return Err(BridgeError::InvalidScore);
            }
            candidates.push(CandidateNomination {
                point_id: point.point_id,
                score,
                payload_digest: point.payload.payload_digest,
                identity_digest: point.payload.identity_digest,
            });
        }
        candidates.sort_by(|left, right| {
            right
                .score
                .partial_cmp(&left.score)
                .unwrap_or(Ordering::Equal)
                .then_with(|| left.point_id.cmp(&right.point_id))
        });
        candidates.truncate(limit);
        Ok(candidates)
    }

    fn replay(
        &self,
        mutation: &BridgeMutation,
    ) -> Result<Option<MutationReceipt>, BridgeError> {
        let Some(existing) = self.operations.get(&mutation.operation_id) else {
            return Ok(None);
        };
        if existing.canonical_input_digest != mutation.canonical_input_digest {
            return Err(BridgeError::OperationConflict);
        }
        let mut replay = existing.clone();
        replay.replayed = true;
        Ok(Some(replay))
    }

    fn record_mutation(
        &mut self,
        route: CollectionRoute,
        mutation: BridgeMutation,
        affected_ids: Vec<QdrantPointId>,
    ) -> Result<MutationReceipt, BridgeError> {
        if self.operations.len() >= self.limits.max_operation_receipts {
            return Err(BridgeError::MutationTooLarge);
        }
        let receipt = MutationReceipt {
            operation_id: mutation.operation_id.clone(),
            canonical_input_digest: mutation.canonical_input_digest,
            route,
            affected_ids,
            replayed: false,
        };
        self.operations
            .insert(mutation.operation_id, receipt.clone());
        Ok(receipt)
    }
}

fn validate_point(
    point: &PointRecord,
    schema: &CollectionSchema,
    limits: BridgeLimits,
) -> Result<(), BridgeError> {
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

fn validate_exact_ids(
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

fn validate_filter(filter: &EligibilityFilter) -> Result<(), BridgeError> {
    if filter.allowed_source_memberships.is_empty() {
        return Err(BridgeError::InvalidFilter);
    }
    Ok(())
}

fn ensure_filter_indexes(schema: &CollectionSchema) -> Result<(), BridgeError> {
    for field in EligibilityFilter::INDEXED_FIELDS {
        if !schema.indexed_payload_fields.contains(field) {
            return Err(BridgeError::UnindexedFilter);
        }
    }
    Ok(())
}

fn validate_query_vector(
    query: &[(u32, f32)],
    schema: VectorSchema,
) -> Result<(), BridgeError> {
    if query.is_empty()
        || query.iter().any(|(_, value)| !value.is_finite())
        || query.windows(2).any(|pair| pair[0].0 >= pair[1].0)
        || query
            .last()
            .is_some_and(|(index, _)| *index >= schema.dimensions)
    {
        return Err(BridgeError::VectorDimensionMismatch);
    }
    Ok(())
}

fn dot_sparse(left: &[(u32, f32)], right: &[(u32, f32)]) -> f32 {
    let mut left_index = 0;
    let mut right_index = 0;
    let mut score = 0.0_f32;
    while left_index < left.len() && right_index < right.len() {
        match left[left_index].0.cmp(&right[right_index].0) {
            Ordering::Less => left_index += 1,
            Ordering::Greater => right_index += 1,
            Ordering::Equal => {
                score += left[left_index].1 * right[right_index].1;
                left_index += 1;
                right_index += 1;
            }
        }
    }
    score
}

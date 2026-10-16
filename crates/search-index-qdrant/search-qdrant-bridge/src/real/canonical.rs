//! Architecture 8.4 Qdrant path over the existing authenticated SDK client.
//! It consumes the shared S9.5/S10.3 contracts and S11 point ID directly.

use std::collections::{BTreeMap, BTreeSet, HashMap};
use std::time::{Duration, Instant};

use qdrant_client::qdrant::{
    CollectionInfo, CountPoints, CreateCollection, CreateFieldIndexCollection, FieldType, Filter,
    GetPoints, IdfParams, Modifier, PayloadSchemaType, PointId, Query, QueryPoints, ScrollPoints,
    SearchParams, SparseVectorConfig, SparseVectorParams, StrictModeConfig, UpdateCollection,
    Value, VectorInput, point_id, value, vectors_config,
};
use search_contracts::indexed::{
    CollectionInvariants, CollectionSchema as IndexedCollectionSchema, EligibilityPopulation,
    PAYLOAD_FIELDS, PayloadField, PayloadIndexKind, PointPayload as IndexedPointPayload,
    VectorMode, VectorName, VectorRequirement, VectorValue, payload_indexes,
};
use search_contracts::{
    Blake3Digest32, BoundedMap, CanonicalKey, CanonicalText, CanonicalValue, MAX_MAP_ENTRIES,
    MAX_QDRANT_EPOCH, MIN_QDRANT_EPOCH,
};
use search_point_identity::s11::PointId128;

#[cfg(test)]
use search_contracts::{
    AccessPartitionId, BoundedName, CollectionGenerationId, Epoch, InstallationIncarnationId,
    Modality, ProjectionMembershipId, ProjectionProfileSetId, ScoringDocumentId,
    ScoringPartitionId, SourceId, SourceRevisionId, UnitId, UnitKind,
};

use super::{BridgeError, CollectionRoute, OpContext, RealDataPlane};

const META_SCHEMA_ID: &str = "eliot_search_indexed_schema_id_v1";
const META_INSTALLATION: &str = "eliot_search_installation_incarnation_id_v1";
const META_GENERATION: &str = "eliot_search_collection_generation_id_v1";
const META_PROFILE: &str = "eliot_search_projection_profile_set_id_v1";
const META_ROUTE: &str = "eliot_search_full_route_v1";

/// One untrusted Qdrant nomination. Its score and address are not evidence.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct CanonicalCandidate {
    /// Shared S11 point address; exact identity still requires readback.
    pub point_id: PointId128,
    /// Qdrant score for ranking only.
    pub score: f32,
}

/// Exact typed S9.5 payload readback for one S11 address.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct CanonicalPointReadback {
    /// Shared S11 point address, parsed losslessly from the Qdrant UUID.
    pub point_id: PointId128,
    /// The accepted closed payload type, decoded from Qdrant's private values.
    pub payload: IndexedPointPayload,
}

/// Exact point readback keeps missing and unexpected IDs explicit.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct CanonicalReadback {
    /// Returned points in the caller's requested order.
    pub points: Vec<CanonicalPointReadback>,
    /// Requested IDs absent from Qdrant.
    pub missing_ids: Vec<PointId128>,
}

/// One bounded exact-population scroll page.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct CanonicalScrollPage {
    /// Untrusted point addresses nominated by the exact population filter.
    pub point_ids: Vec<PointId128>,
    /// Opaque continuation address for the next page.
    pub next_offset: Option<PointId128>,
}

#[derive(Clone, Debug)]
struct CanonicalAdmission {
    collection_name: String,
    schema: IndexedCollectionSchema,
}

/// Admission keyed by the complete route, not its truncated generated name.
#[derive(Default)]
pub(super) struct CanonicalState {
    admitted: BTreeMap<CollectionRoute, CanonicalAdmission>,
}

impl CanonicalState {
    fn admit(
        &mut self,
        route: CollectionRoute,
        collection_name: String,
        schema: &IndexedCollectionSchema,
    ) -> Result<(), BridgeError> {
        if self.admitted.iter().any(|(existing_route, admission)| {
            existing_route != &route && admission.collection_name == collection_name
        }) {
            return Err(BridgeError::CollectionSchemaMismatch);
        }
        if self
            .admitted
            .get(&route)
            .is_some_and(|admission| admission.schema != *schema)
        {
            return Err(BridgeError::CollectionSchemaMismatch);
        }
        self.admitted.insert(
            route,
            CanonicalAdmission {
                collection_name,
                schema: schema.clone(),
            },
        );
        Ok(())
    }

    fn resolve(&self, route: &CollectionRoute) -> Result<&CanonicalAdmission, BridgeError> {
        self.admitted
            .get(route)
            .ok_or(BridgeError::CollectionNotFound)
    }
}

/// One monotonic deadline shared by all create and verify phases.
#[derive(Clone, Debug)]
struct IndexedOperationBudget {
    started: Instant,
    total: Duration,
}

impl IndexedOperationBudget {
    fn begin(context: &OpContext) -> Result<Self, BridgeError> {
        context.check()?;
        Ok(Self {
            started: Instant::now(),
            total: context.deadline(),
        })
    }

    fn remaining(&self, context: &OpContext) -> Result<Duration, BridgeError> {
        context.check()?;
        self.total
            .checked_sub(self.started.elapsed())
            .filter(|remaining| !remaining.is_zero())
            .ok_or(BridgeError::TransportFailed)
    }

    fn remaining_after_dispatch(&self, context: &OpContext) -> Result<Duration, BridgeError> {
        self.remaining(context)
            .map_err(|_| BridgeError::MutationOutcomeUnknown)
    }
}

fn validate_schema_route(
    route: &CollectionRoute,
    schema: &IndexedCollectionSchema,
) -> Result<(String, Blake3Digest32), BridgeError> {
    if &route.generation != schema.collection_generation_id() {
        return Err(BridgeError::CollectionSchemaMismatch);
    }
    if schema.mode() != VectorMode::SparseOnly {
        return Err(BridgeError::UnsupportedVectorProfile);
    }
    if schema.invariants() != CollectionInvariants::BASELINE {
        return Err(BridgeError::CollectionSchemaMismatch);
    }
    let name = super::collection_name(route)?;
    let identity = schema
        .identity()
        .map_err(|_| BridgeError::CollectionSchemaMismatch)?;
    Ok((name, *identity.as_digest()))
}

fn strict_mode_config(enabled: bool) -> StrictModeConfig {
    StrictModeConfig {
        enabled: Some(enabled),
        unindexed_filtering_retrieve: Some(false),
        unindexed_filtering_update: Some(false),
        ..Default::default()
    }
}

const fn vendor_field_type(kind: PayloadIndexKind) -> FieldType {
    match kind {
        PayloadIndexKind::Uuid => FieldType::Uuid,
        PayloadIndexKind::Keyword => FieldType::Keyword,
        PayloadIndexKind::Integer => FieldType::Integer,
    }
}

const fn vendor_schema_type(kind: PayloadSchemaType) -> Option<PayloadIndexKind> {
    match kind {
        PayloadSchemaType::Uuid => Some(PayloadIndexKind::Uuid),
        PayloadSchemaType::Keyword => Some(PayloadIndexKind::Keyword),
        PayloadSchemaType::Integer => Some(PayloadIndexKind::Integer),
        _ => None,
    }
}

fn indexed_metadata(
    route: &CollectionRoute,
    schema: &IndexedCollectionSchema,
    schema_id: Blake3Digest32,
) -> HashMap<String, Value> {
    [
        (META_SCHEMA_ID, schema_id.to_string()),
        (
            META_INSTALLATION,
            schema.installation_incarnation_id().to_string(),
        ),
        (
            META_GENERATION,
            schema.collection_generation_id().to_string(),
        ),
        (META_PROFILE, schema.projection_profile_set_id().to_string()),
        (META_ROUTE, route.physical_name.to_string()),
    ]
    .into_iter()
    .map(|(key, value)| (key.to_owned(), super::str_value(&value)))
    .collect()
}

fn indexed_schema_info_matches(
    info: &CollectionInfo,
    route: &CollectionRoute,
    schema: &IndexedCollectionSchema,
    schema_id: Blake3Digest32,
) -> Result<(), BridgeError> {
    let config = info
        .config
        .as_ref()
        .ok_or(BridgeError::CollectionSchemaMismatch)?;
    let expected_metadata = indexed_metadata(route, schema, schema_id);
    if config.metadata != expected_metadata {
        return Err(BridgeError::CollectionSchemaMismatch);
    }
    let params = config
        .params
        .as_ref()
        .ok_or(BridgeError::CollectionSchemaMismatch)?;
    let invariants = schema.invariants();
    if params.shard_number != invariants.shards()
        || params.replication_factor != Some(invariants.replication_factor())
        || params.write_consistency_factor != Some(invariants.write_consistency_factor())
    {
        return Err(BridgeError::CollectionSchemaMismatch);
    }
    if params
        .vectors_config
        .as_ref()
        .and_then(|vectors| vectors.config.as_ref())
        .is_some_and(|vectors| match vectors {
            vectors_config::Config::ParamsMap(map) => !map.map.is_empty(),
            vectors_config::Config::Params(_) => true,
        })
    {
        return Err(BridgeError::UnsupportedVectorProfile);
    }
    let sparse = params
        .sparse_vectors_config
        .as_ref()
        .ok_or(BridgeError::CollectionSchemaMismatch)?;
    if sparse.map.len() != schema.vectors().len() {
        return Err(BridgeError::CollectionSchemaMismatch);
    }
    for vector in schema.vectors() {
        let remote = sparse
            .map
            .get(vector.name().as_str())
            .ok_or(BridgeError::NamedVectorMissing)?;
        if remote.modifier != Some(Modifier::Idf as i32) {
            return Err(BridgeError::CollectionSchemaMismatch);
        }
    }

    let mut indexes = Vec::with_capacity(info.payload_schema.len());
    for (name, index) in &info.payload_schema {
        let kind =
            vendor_schema_type(index.data_type()).ok_or(BridgeError::CollectionSchemaMismatch)?;
        indexes.push((name.clone(), kind));
    }
    schema
        .validate_readback(&indexes, schema.vectors(), invariants)
        .map_err(|_| BridgeError::CollectionSchemaMismatch)?;

    let strict = config
        .strict_mode_config
        .as_ref()
        .ok_or(BridgeError::StrictModeRequired)?;
    if !strict.enabled.unwrap_or_default()
        || strict.unindexed_filtering_retrieve.unwrap_or(true)
        || strict.unindexed_filtering_update.unwrap_or(true)
    {
        return Err(BridgeError::StrictModeRequired);
    }
    Ok(())
}

const fn post_create_error(error: BridgeError) -> BridgeError {
    match error {
        BridgeError::CollectionSchemaMismatch | BridgeError::UnsupportedVectorProfile => error,
        _ => BridgeError::MutationOutcomeUnknown,
    }
}

fn encode_payload(payload: &IndexedPointPayload) -> Result<HashMap<String, Value>, BridgeError> {
    let CanonicalValue::Object(fields) = payload
        .to_canonical_value()
        .map_err(|_| BridgeError::CollectionSchemaMismatch)?
    else {
        return Err(BridgeError::CollectionSchemaMismatch);
    };
    let mut output = HashMap::with_capacity(fields.len());
    for (field, canonical) in &fields {
        let value = match canonical {
            CanonicalValue::Text(text) => super::str_value(text.as_str()),
            CanonicalValue::U64(number) => Value {
                kind: Some(value::Kind::IntegerValue(
                    i64::try_from(*number).map_err(|_| BridgeError::CollectionSchemaMismatch)?,
                )),
            },
            _ => return Err(BridgeError::CollectionSchemaMismatch),
        };
        output.insert(field.as_str().to_owned(), value);
    }
    Ok(output)
}

fn payload_to_canonical_value(
    payload: &HashMap<String, Value>,
) -> Result<CanonicalValue, BridgeError> {
    let mut fields = Vec::with_capacity(payload.len());
    for (name, entry) in payload {
        let descriptor = PAYLOAD_FIELDS
            .iter()
            .find(|field| field.name == name.as_str())
            .ok_or(BridgeError::MalformedResponse)?;
        let key = CanonicalKey::new_non_empty(name).map_err(|_| BridgeError::MalformedResponse)?;
        let value = match (
            descriptor.kind,
            entry.kind.as_ref().ok_or(BridgeError::MalformedResponse)?,
        ) {
            (
                PayloadIndexKind::Uuid | PayloadIndexKind::Keyword,
                value::Kind::StringValue(text),
            ) => CanonicalValue::Text(
                CanonicalText::new(text.clone()).map_err(|_| BridgeError::MalformedResponse)?,
            ),
            (PayloadIndexKind::Integer, value::Kind::IntegerValue(number)) => CanonicalValue::U64(
                u64::try_from(*number).map_err(|_| BridgeError::MalformedResponse)?,
            ),
            _ => return Err(BridgeError::MalformedResponse),
        };
        fields.push((key, value));
    }
    BoundedMap::<CanonicalKey, CanonicalValue, MAX_MAP_ENTRIES>::from_entries(fields)
        .map(CanonicalValue::Object)
        .map_err(|_| BridgeError::MalformedResponse)
}

fn decode_payload(payload: &HashMap<String, Value>) -> Result<IndexedPointPayload, BridgeError> {
    let decoded = IndexedPointPayload::from_canonical_value(payload_to_canonical_value(payload)?)
        .map_err(|_| BridgeError::MalformedResponse)?;
    if encode_payload(&decoded)? != *payload {
        return Err(BridgeError::MalformedResponse);
    }
    Ok(decoded)
}

fn canonical_epoch_bound(epoch: i64) -> Result<f64, BridgeError> {
    if !(MIN_QDRANT_EPOCH..=MAX_QDRANT_EPOCH).contains(&epoch) {
        return Err(BridgeError::InvalidFilter);
    }
    #[allow(clippy::cast_precision_loss)]
    Ok(epoch as f64)
}

fn base_filter(
    population: &EligibilityPopulation,
    schema: &IndexedCollectionSchema,
) -> Result<Filter, BridgeError> {
    population
        .validate_for_schema(schema)
        .map_err(|_| BridgeError::InvalidFilter)?;
    let visible_epoch = canonical_epoch_bound(population.visible_epoch().get())?;
    let memberships = population
        .allowed_projection_memberships()
        .iter()
        .map(ToString::to_string)
        .collect();
    Ok(Filter {
        must: vec![
            super::keyword_condition(
                PayloadField::InstallationIncarnationId.as_str(),
                population.installation_incarnation_id().to_string(),
            ),
            super::keyword_condition(
                PayloadField::CollectionGenerationId.as_str(),
                population.collection_generation_id().to_string(),
            ),
            super::keywords_condition(PayloadField::ProjectionMembershipId.as_str(), memberships),
            super::keyword_condition(
                PayloadField::AccessPartitionId.as_str(),
                population.access_partition_id().to_string(),
            ),
            super::keyword_condition(
                PayloadField::ScoringPartitionId.as_str(),
                population.scoring_partition_id().to_string(),
            ),
            super::keyword_condition(
                PayloadField::ProjectionProfileSetId.as_str(),
                population.projection_profile_set_id().as_str().to_owned(),
            ),
            super::range_condition(
                PayloadField::ValidFromEpoch.as_str(),
                None,
                Some(visible_epoch),
            ),
        ],
        must_not: vec![super::range_condition(
            PayloadField::ValidUntilEpochExclusive.as_str(),
            None,
            Some(visible_epoch),
        )],
        ..Default::default()
    })
}

fn retrieval_and_idf_filters(
    population: &EligibilityPopulation,
    schema: &IndexedCollectionSchema,
) -> Result<(Filter, Filter), BridgeError> {
    let retrieval = base_filter(population, schema)?;
    let idf_corpus = retrieval.clone();
    Ok((retrieval, idf_corpus))
}

fn canonical_vendor_point_id(id: PointId128) -> PointId {
    PointId {
        point_id_options: Some(point_id::PointIdOptions::Uuid(id.to_string())),
    }
}

fn canonical_bridge_point_id(id: &PointId) -> Result<PointId128, BridgeError> {
    match id.point_id_options.as_ref() {
        Some(point_id::PointIdOptions::Uuid(uuid)) => {
            PointId128::parse(uuid).map_err(|_| BridgeError::MalformedResponse)
        }
        Some(point_id::PointIdOptions::Num(_)) | None => Err(BridgeError::MalformedResponse),
    }
}

fn decode_point(
    id: &PointId,
    payload: &HashMap<String, Value>,
    population: &EligibilityPopulation,
) -> Result<CanonicalPointReadback, BridgeError> {
    let point_id = canonical_bridge_point_id(id)?;
    let payload = decode_payload(payload)?;
    if !population.matches(&payload) {
        return Err(BridgeError::UnexpectedPoint);
    }
    Ok(CanonicalPointReadback { point_id, payload })
}

impl RealDataPlane {
    /// Creates a fresh sparse generation and admits it only after exact schema readback.
    pub async fn create_indexed_generation(
        &mut self,
        route: &CollectionRoute,
        schema: &IndexedCollectionSchema,
        context: &OpContext,
    ) -> Result<(), BridgeError> {
        let budget = IndexedOperationBudget::begin(context)?;
        let (name, schema_id) = validate_schema_route(route, schema)?;
        self.authorize_dispatch()?;
        let exists = tokio::time::timeout(
            budget.remaining(context)?,
            self.client.collection_exists(name.clone()),
        )
        .await
        .map_err(|_| BridgeError::TransportFailed)?
        .map_err(super::map_read_error)?;
        if exists {
            return Err(BridgeError::CollectionAlreadyExists);
        }

        let mut sparse_vectors = HashMap::new();
        for vector in schema.vectors() {
            sparse_vectors.insert(
                vector.name().as_str().to_owned(),
                SparseVectorParams {
                    modifier: Some(Modifier::Idf as i32),
                    ..Default::default()
                },
            );
        }
        let create = CreateCollection {
            collection_name: name.clone(),
            shard_number: Some(schema.invariants().shards()),
            replication_factor: Some(schema.invariants().replication_factor()),
            write_consistency_factor: Some(schema.invariants().write_consistency_factor()),
            sparse_vectors_config: Some(SparseVectorConfig {
                map: sparse_vectors,
            }),
            strict_mode_config: Some(strict_mode_config(false)),
            metadata: indexed_metadata(route, schema, schema_id),
            ..Default::default()
        };
        self.authorize_dispatch()?;
        let created = tokio::time::timeout(
            budget.remaining(context)?,
            self.client.create_collection(create),
        )
        .await
        .map_err(|_| BridgeError::MutationOutcomeUnknown)?
        .map_err(super::map_create_error)?;
        if !created.result {
            return Err(BridgeError::MutationOutcomeUnknown);
        }

        for field in payload_indexes() {
            budget.remaining_after_dispatch(context)?;
            self.authorize_dispatch().map_err(post_create_error)?;
            let indexed = tokio::time::timeout(
                budget.remaining_after_dispatch(context)?,
                self.client.create_field_index(CreateFieldIndexCollection {
                    collection_name: name.clone(),
                    field_name: field.name.to_owned(),
                    field_type: Some(vendor_field_type(field.kind) as i32),
                    wait: Some(true),
                    ordering: Some(super::strong_ordering()),
                    ..Default::default()
                }),
            )
            .await
            .map_err(|_| BridgeError::MutationOutcomeUnknown)?
            .map_err(super::map_create_error)
            .map_err(post_create_error)?;
            if !indexed
                .result
                .as_ref()
                .is_some_and(|result| super::update_completed(result.status))
            {
                return Err(BridgeError::MutationOutcomeUnknown);
            }
        }

        budget.remaining_after_dispatch(context)?;
        self.authorize_dispatch().map_err(post_create_error)?;
        let hardened = tokio::time::timeout(
            budget.remaining_after_dispatch(context)?,
            self.client.update_collection(UpdateCollection {
                collection_name: name.clone(),
                strict_mode_config: Some(strict_mode_config(true)),
                ..Default::default()
            }),
        )
        .await
        .map_err(|_| BridgeError::MutationOutcomeUnknown)?
        .map_err(super::map_create_error)
        .map_err(post_create_error)?;
        if !hardened.result {
            return Err(BridgeError::MutationOutcomeUnknown);
        }

        budget.remaining_after_dispatch(context)?;
        self.verify_indexed_schema_at(route, schema, &name, schema_id, &budget, context)
            .await
            .map_err(post_create_error)?;
        self.canonical
            .admit(route.clone(), name, schema)
            .map_err(post_create_error)?;
        Ok(())
    }

    /// Re-reads exact settings and metadata before restart admission.
    pub async fn verify_indexed_generation(
        &mut self,
        route: &CollectionRoute,
        schema: &IndexedCollectionSchema,
        context: &OpContext,
    ) -> Result<(), BridgeError> {
        let budget = IndexedOperationBudget::begin(context)?;
        let (name, schema_id) = validate_schema_route(route, schema)?;
        self.verify_indexed_schema_at(route, schema, &name, schema_id, &budget, context)
            .await?;
        budget.remaining(context)?;
        self.canonical.admit(route.clone(), name, schema)?;
        Ok(())
    }

    async fn verify_indexed_schema_at(
        &self,
        route: &CollectionRoute,
        schema: &IndexedCollectionSchema,
        name: &str,
        schema_id: Blake3Digest32,
        budget: &IndexedOperationBudget,
        context: &OpContext,
    ) -> Result<(), BridgeError> {
        self.authorize_dispatch()?;
        let info = tokio::time::timeout(
            budget.remaining(context)?,
            self.client.collection_info(name),
        )
        .await
        .map_err(|_| BridgeError::TransportFailed)?
        .map_err(super::map_read_error)?
        .result
        .ok_or(BridgeError::MalformedResponse)?;
        budget.remaining(context)?;
        indexed_schema_info_matches(&info, route, schema, schema_id)
    }

    /// Uses one exact population for retrieval and `idf.corpus`.
    pub async fn query_indexed(
        &self,
        route: &CollectionRoute,
        population: &EligibilityPopulation,
        vector_name: &VectorName,
        query: &VectorValue,
        limit: usize,
        context: &OpContext,
    ) -> Result<Vec<CanonicalCandidate>, BridgeError> {
        let budget = IndexedOperationBudget::begin(context)?;
        if limit == 0 || limit > self.limits.max_query_candidates {
            return Err(BridgeError::QueryBudgetExceeded);
        }
        let admission = self.canonical.resolve(route)?;
        let schema = &admission.schema;
        let requirement = schema
            .vectors()
            .iter()
            .find(|vector| vector.name() == vector_name)
            .ok_or(BridgeError::NamedVectorMissing)?;
        if !matches!(requirement.requirement(), VectorRequirement::Sparse { .. }) {
            return Err(BridgeError::UnsupportedVectorProfile);
        }
        query
            .validate(&requirement.requirement())
            .map_err(|_| BridgeError::VectorDimensionMismatch)?;
        let VectorValue::Sparse { indices, values } = query else {
            return Err(BridgeError::UnsupportedVectorProfile);
        };
        let (filter, corpus) = retrieval_and_idf_filters(population, schema)?;
        self.authorize_dispatch()?;
        let answered = tokio::time::timeout(
            budget.remaining(context)?,
            self.client.query(QueryPoints {
                collection_name: admission.collection_name.clone(),
                query: Some(Query::new_nearest(VectorInput::new_sparse(
                    indices.clone(),
                    values.clone(),
                ))),
                using: Some(vector_name.as_str().to_owned()),
                filter: Some(filter),
                params: Some(SearchParams {
                    exact: Some(true),
                    idf: Some(IdfParams {
                        corpus: Some(corpus),
                    }),
                    ..Default::default()
                }),
                limit: Some(u64::try_from(limit).map_err(|_| BridgeError::QueryBudgetExceeded)?),
                with_payload: Some(false.into()),
                ..Default::default()
            }),
        )
        .await
        .map_err(|_| BridgeError::TransportFailed)?
        .map_err(super::map_read_error)?;
        if answered.result.len() > limit {
            return Err(BridgeError::MalformedResponse);
        }
        let mut candidates = Vec::with_capacity(answered.result.len());
        let mut seen = BTreeSet::new();
        for scored in &answered.result {
            budget.remaining(context)?;
            if !scored.score.is_finite() {
                return Err(BridgeError::InvalidScore);
            }
            let point_id = canonical_bridge_point_id(
                scored.id.as_ref().ok_or(BridgeError::MalformedResponse)?,
            )?;
            if !seen.insert(point_id) {
                return Err(BridgeError::MalformedResponse);
            }
            candidates.push(CanonicalCandidate {
                point_id,
                score: scored.score,
            });
        }
        budget.remaining(context)?;
        Ok(candidates)
    }

    /// Exact count using the same closed S10.3 population.
    pub async fn count_indexed(
        &self,
        route: &CollectionRoute,
        population: &EligibilityPopulation,
        context: &OpContext,
    ) -> Result<u64, BridgeError> {
        let budget = IndexedOperationBudget::begin(context)?;
        let admission = self.canonical.resolve(route)?;
        let filter = base_filter(population, &admission.schema)?;
        self.authorize_dispatch()?;
        let counted = tokio::time::timeout(
            budget.remaining(context)?,
            self.client.count(CountPoints {
                collection_name: admission.collection_name.clone(),
                filter: Some(filter),
                exact: Some(true),
                ..Default::default()
            }),
        )
        .await
        .map_err(|_| BridgeError::TransportFailed)?
        .map_err(super::map_read_error)?;
        budget.remaining(context)?;
        counted
            .result
            .map(|result| result.count)
            .ok_or(BridgeError::MalformedResponse)
    }

    /// Reads one bounded page of IDs through the exact population filter.
    pub async fn scroll_indexed(
        &self,
        route: &CollectionRoute,
        population: &EligibilityPopulation,
        offset: Option<PointId128>,
        limit: usize,
        context: &OpContext,
    ) -> Result<CanonicalScrollPage, BridgeError> {
        let budget = IndexedOperationBudget::begin(context)?;
        if limit == 0 || limit > self.limits.max_query_candidates {
            return Err(BridgeError::QueryBudgetExceeded);
        }
        let admission = self.canonical.resolve(route)?;
        let schema = &admission.schema;
        let filter = base_filter(population, schema)?;
        self.authorize_dispatch()?;
        let scrolled = tokio::time::timeout(
            budget.remaining(context)?,
            self.client.scroll(ScrollPoints {
                collection_name: admission.collection_name.clone(),
                filter: Some(filter),
                offset: offset.map(canonical_vendor_point_id),
                limit: Some(u32::try_from(limit).map_err(|_| BridgeError::QueryBudgetExceeded)?),
                with_payload: Some(false.into()),
                with_vectors: Some(false.into()),
                ..Default::default()
            }),
        )
        .await
        .map_err(|_| BridgeError::TransportFailed)?
        .map_err(super::map_read_error)?;
        if scrolled.result.len() > limit {
            return Err(BridgeError::MalformedResponse);
        }
        let mut point_ids = Vec::with_capacity(scrolled.result.len());
        let mut seen = BTreeSet::new();
        for point in &scrolled.result {
            budget.remaining(context)?;
            let point_id = canonical_bridge_point_id(
                point.id.as_ref().ok_or(BridgeError::MalformedResponse)?,
            )?;
            if !seen.insert(point_id) {
                return Err(BridgeError::MalformedResponse);
            }
            point_ids.push(point_id);
        }
        let next_offset = scrolled
            .next_page_offset
            .as_ref()
            .map(canonical_bridge_point_id)
            .transpose()?;
        if next_offset.is_some() && next_offset == offset {
            return Err(BridgeError::MalformedResponse);
        }
        budget.remaining(context)?;
        Ok(CanonicalScrollPage {
            point_ids,
            next_offset,
        })
    }

    /// Reads exact S11 IDs and checks each full identity digest against the manifest.
    pub async fn readback_indexed_exact(
        &self,
        route: &CollectionRoute,
        population: &EligibilityPopulation,
        expected: &[(PointId128, Blake3Digest32)],
        context: &OpContext,
    ) -> Result<CanonicalReadback, BridgeError> {
        let budget = IndexedOperationBudget::begin(context)?;
        if expected.len() > self.limits.max_points_per_mutation {
            return Err(BridgeError::MutationTooLarge);
        }
        let admission = self.canonical.resolve(route)?;
        population
            .validate_for_schema(&admission.schema)
            .map_err(|_| BridgeError::InvalidFilter)?;
        let mut expected_by_id = BTreeMap::new();
        for (id, digest) in expected {
            if expected_by_id.insert(*id, *digest).is_some() {
                return Err(BridgeError::DuplicatePointId);
            }
        }
        self.authorize_dispatch()?;
        let readback = tokio::time::timeout(
            budget.remaining(context)?,
            self.client.get_points(GetPoints {
                collection_name: admission.collection_name.clone(),
                ids: expected
                    .iter()
                    .map(|(id, _)| canonical_vendor_point_id(*id))
                    .collect(),
                with_payload: Some(true.into()),
                with_vectors: Some(false.into()),
                ..Default::default()
            }),
        )
        .await
        .map_err(|_| BridgeError::TransportFailed)?
        .map_err(super::map_read_error)?;
        if readback.result.len() > expected.len() {
            return Err(BridgeError::MalformedResponse);
        }

        let mut returned = BTreeMap::new();
        for point in &readback.result {
            budget.remaining(context)?;
            let id = canonical_bridge_point_id(
                point.id.as_ref().ok_or(BridgeError::MalformedResponse)?,
            )?;
            let Some(expected_digest) = expected_by_id.get(&id) else {
                return Err(BridgeError::UnexpectedPoint);
            };
            if returned.contains_key(&id) {
                return Err(BridgeError::MalformedResponse);
            }
            let decoded = decode_point(
                point.id.as_ref().ok_or(BridgeError::MalformedResponse)?,
                &point.payload,
                population,
            )?;
            if decoded.payload.point_identity_digest_256 != *expected_digest {
                return Err(BridgeError::ExactReadbackMismatch);
            }
            returned.insert(id, decoded);
        }

        let mut points = Vec::with_capacity(returned.len());
        let mut missing_ids = Vec::new();
        for (id, _) in expected {
            match returned.remove(id) {
                Some(point) => points.push(point),
                None => missing_ids.push(*id),
            }
        }
        budget.remaining(context)?;
        Ok(CanonicalReadback {
            points,
            missing_ids,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    macro_rules! fixture_id {
        ($kind:ty, $number:literal) => {{
            let text = format!("00000000-0000-4000-8000-{number:012}", number = $number);
            <$kind>::parse(&text).expect("fixture ID")
        }};
    }

    fn sample_payload() -> IndexedPointPayload {
        IndexedPointPayload::new(
            fixture_id!(InstallationIncarnationId, 1),
            fixture_id!(CollectionGenerationId, 2),
            fixture_id!(ProjectionMembershipId, 3),
            fixture_id!(AccessPartitionId, 4),
            fixture_id!(ScoringPartitionId, 5),
            fixture_id!(SourceId, 6),
            fixture_id!(SourceRevisionId, 7),
            fixture_id!(search_contracts::RepresentationId, 8),
            fixture_id!(UnitId, 9),
            Blake3Digest32::from_bytes([0xA5; 32]),
            fixture_id!(ScoringDocumentId, 10),
            ProjectionProfileSetId::new("lex_code_v1").expect("profile"),
            UnitKind::Symbol,
            Modality::Code,
            BoundedName::new("rust").expect("language"),
            None,
            None,
            None,
            Epoch::new(1).expect("visible epoch"),
            None,
        )
    }

    fn sample_schema() -> IndexedCollectionSchema {
        let payload = sample_payload();
        let vector = search_contracts::indexed::NamedVectorRequirement::new(
            VectorName::new("lex_code_v1").expect("vector name"),
            VectorRequirement::Sparse {
                index_ceiling: 65_536,
                idf_enabled: true,
            },
        )
        .expect("vector requirement");
        IndexedCollectionSchema::initial(
            payload.installation_incarnation_id,
            payload.collection_generation_id,
            payload.projection_profile_set_id,
            &[vector],
        )
        .expect("schema")
    }

    fn sample_population(schema: &IndexedCollectionSchema) -> EligibilityPopulation {
        EligibilityPopulation::new(
            *schema.installation_incarnation_id(),
            *schema.collection_generation_id(),
            &[fixture_id!(ProjectionMembershipId, 3)],
            fixture_id!(AccessPartitionId, 4),
            fixture_id!(ScoringPartitionId, 5),
            schema.projection_profile_set_id().clone(),
            Epoch::new(1).expect("epoch"),
        )
        .expect("population")
    }

    fn route(schema: &IndexedCollectionSchema, physical_name: &str) -> CollectionRoute {
        CollectionRoute {
            generation: *schema.collection_generation_id(),
            physical_name: search_contracts::OpaqueId::new(physical_name).expect("route"),
        }
    }

    #[test]
    fn canonical_payload_codec_round_trips_only_shared_closed_fields() {
        let payload = sample_payload();
        let encoded = encode_payload(&payload).expect("encode");
        let decoded = decode_payload(&encoded).expect("decode");
        assert_eq!(decoded, payload);
        assert_eq!(encoded.len(), 16);
        assert!(!encoded.contains_key("source_membership_id"));
        assert!(!encoded.contains_key("payload_digest"));
        assert!(!encoded.contains_key("vector_digest"));
        assert!(!encoded.contains_key("source_text"));

        let mut extra = encoded;
        extra.insert(
            "source_membership_id".to_owned(),
            super::super::str_value("forbidden"),
        );
        assert_eq!(decode_payload(&extra), Err(BridgeError::MalformedResponse));
    }

    #[test]
    fn population_filter_is_shared_byte_for_byte_with_idf() {
        let schema = sample_schema();
        let population = sample_population(&schema);
        let (retrieval, idf) = retrieval_and_idf_filters(&population, &schema).expect("filters");
        assert_eq!(retrieval, idf);
        assert_eq!(retrieval.must.len(), 7);
        assert_eq!(retrieval.must_not.len(), 1);
    }

    #[test]
    fn route_metadata_and_admission_reject_physical_name_collisions() {
        let schema = sample_schema();
        let left = route(&schema, "portfolio-a");
        let right = route(&schema, "portfolio-b");
        let mut state = CanonicalState::default();
        state
            .admit(left, "same-qdrant-name".to_owned(), &schema)
            .expect("first full route");
        assert_eq!(
            state.admit(right, "same-qdrant-name".to_owned(), &schema),
            Err(BridgeError::CollectionSchemaMismatch)
        );
    }

    #[test]
    fn qdrant_epoch_domain_is_checked_before_float_conversion() {
        assert_eq!(canonical_epoch_bound(MIN_QDRANT_EPOCH), Ok(0.0));
        assert_eq!(
            canonical_epoch_bound(MAX_QDRANT_EPOCH - 1),
            Ok(9_007_199_254_740_991.0)
        );
        assert_eq!(
            canonical_epoch_bound(MAX_QDRANT_EPOCH),
            Ok(9_007_199_254_740_992.0)
        );
        for invalid in [MAX_QDRANT_EPOCH + 1, i64::MAX, -1] {
            assert_eq!(
                canonical_epoch_bound(invalid),
                Err(BridgeError::InvalidFilter)
            );
        }
    }
}

//! Real T24 data-plane adapter over the pinned `qdrant-client` 1.19.0 transport.
//!
//! The in-memory [`QdrantBridge`](crate::QdrantBridge) stays as the behavioral
//! test oracle only. Every production data-plane operation here executes
//! against a real Qdrant server through the exact qualified client and
//! returns verified typed results, never model success.
//!
//! Admission: [`RealDataPlane::connect`] requires an executed
//! [`QualifiedGate`](crate::qualified::QualifiedGate) (all T22 mandatory live
//! probes passed) and rechecks the live server identity on connect. Collection
//! names, filters and batches are validated before dispatch.
//!
//! Single-contract retrieval + IDF (invariant 5): [`RealDataPlane::query_filtered`]
//! takes one [`EligibilityFilter`](crate::EligibilityFilter) and renders both
//! the retrieval `filter` and the `idf.corpus` population filter from that
//! same value. [`IdfScope::Global`] omits the corpus (collection-wide IDF);
//! [`IdfScope::ScopedToRetrieval`] clones the retrieval filter as the corpus.
//! A diverged corpus is unrepresentable: there is no second filter argument.
//!
//! Pre-dispatch versus possible-write failures: validation, cancellation and
//! connect-time failures are definite typed errors (no commit was possible).
//! Any mutation dispatch that may have reached the server — transport loss,
//! deadline expiry, ambiguous acknowledgement — reports
//! [`BridgeError::MutationOutcomeUnknown`](crate::BridgeError::MutationOutcomeUnknown)
//! and must be resolved through [`RealDataPlane::readback_exact`] with the
//! same mutation identity. Reads never report unknown outcomes: they commit
//! nothing, so loss maps to [`BridgeError::TransportFailed`](crate::BridgeError).
//!
//! Vendor (`qdrant_client`) types never appear in public signatures and vendor
//! status text never reaches errors: every failure maps to a stable
//! [`BridgeError`](crate::BridgeError) code.
//!
//! The T24 scope is sparse vectors only: a schema carrying a dense vector is
//! rejected pre-dispatch with
//! [`BridgeError::CollectionSchemaMismatch`](crate::BridgeError::CollectionSchemaMismatch)
//! because dense layouts need a distinct accepted scoring profile.

use std::collections::{BTreeMap, BTreeSet, HashMap};
use std::sync::{
    Arc,
    atomic::{AtomicBool, Ordering},
};
use std::time::Duration;

use qdrant_client::Qdrant;
use qdrant_client::qdrant::{
    CollectionInfo, Condition, CountPoints, CreateCollection, CreateFieldIndexCollection,
    DeletePoints, FieldCondition, FieldType, Filter, GetPoints, IdfParams, Match, Modifier,
    PointId, PointStruct, PointsIdsList, PointsSelector, Query, QueryPoints, Range,
    RepeatedStrings, ScrollPoints, SearchParams, SetPayloadPoints, SparseVectorConfig,
    SparseVectorParams, StrictModeConfig, UpdateStatus, UpsertPoints, Value, Vector, VectorInput,
    Vectors, WriteOrdering, WriteOrderingType, condition, r#match, point_id, points_selector,
    value, vector_output, vectors, vectors_output,
};
use search_contracts::{OpaqueId, ReceiptRef};

use crate::live::LiveEndpoint;
use crate::qualified::{QUALIFIED_SERVER_BUILD, QUALIFIED_SERVER_VERSION, QualifiedGate};
use crate::{
    BoundedPointReadback, BridgeError, BridgeLimits, BridgeMutation, CandidateNomination,
    CollectionRoute, CollectionSchema, EligibilityFilter, ExactCount, MutationReceipt,
    PointPayload, PointRecord, QdrantPointId, StoredVector,
};

/// gRPC canonical status numbers (`google.rpc.Code`), matched without a
/// `tonic` dependency: only the stable numeric values travel across the
/// crate boundary, never vendor status text.
const CODE_INVALID_ARGUMENT: i32 = 3;
const CODE_NOT_FOUND: i32 = 5;
const CODE_ALREADY_EXISTS: i32 = 6;
const CODE_PERMISSION_DENIED: i32 = 7;
const CODE_RESOURCE_EXHAUSTED: i32 = 8;
const CODE_FAILED_PRECONDITION: i32 = 9;
const CODE_OUT_OF_RANGE: i32 = 11;
const CODE_UNAUTHENTICATED: i32 = 16;

/// Bounded per-operation context.
///
/// A finite deadline plus an optional cancellation flag. Cancellation is
/// checked before dispatch and between bounded pages/batches; expiry after a
/// mutation dispatch reports `MutationOutcomeUnknown` because the write may
/// have committed.
#[derive(Clone, Debug)]
pub struct OpContext {
    deadline: Duration,
    cancelled: Option<Arc<AtomicBool>>,
}

impl OpContext {
    /// Builds a context with a finite deadline and no cancellation flag.
    #[must_use]
    pub const fn new(deadline: Duration) -> Self {
        Self {
            deadline,
            cancelled: None,
        }
    }

    /// Builds a context with a finite deadline and a shared cancellation flag.
    #[must_use]
    pub const fn with_cancel(deadline: Duration, cancelled: Arc<AtomicBool>) -> Self {
        Self {
            deadline,
            cancelled: Some(cancelled),
        }
    }

    /// Finite per-operation deadline.
    #[must_use]
    pub const fn deadline(&self) -> Duration {
        self.deadline
    }

    /// Fails with [`BridgeError::Cancelled`] when the flag is set.
    pub fn check(&self) -> Result<(), BridgeError> {
        if self
            .cancelled
            .as_ref()
            .is_some_and(|flag| flag.load(Ordering::SeqCst))
        {
            return Err(BridgeError::Cancelled);
        }
        Ok(())
    }
}

impl Default for OpContext {
    fn default() -> Self {
        Self::new(Duration::from_secs(10))
    }
}

/// Which IDF population a filtered query scores with.
///
/// `Global` omits `idf.corpus` (collection-wide denominators).
/// `ScopedToRetrieval` sets `idf.corpus` to the exact retrieval filter built
/// from the same single contract, so denied documents can never move
/// permitted denominators.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum IdfScope {
    Global,
    ScopedToRetrieval,
}

/// One bounded scroll page with an opaque continuation offset.
#[derive(Clone, Debug, PartialEq)]
pub struct ScrollPage {
    pub points: Vec<PointRecord>,
    pub next_offset: Option<QdrantPointId>,
}

/// Validates a Qdrant collection name without touching the network.
///
/// Exactly `1..=128` ASCII characters from `[A-Za-z0-9_.-]`. Anything else is
/// rejected pre-dispatch so an opaque physical name can never become a
/// vendor-side surprise.
pub fn validate_collection_name(name: &str) -> Result<(), BridgeError> {
    if name.is_empty() || name.len() > 128 {
        return Err(BridgeError::CollectionSchemaMismatch);
    }
    if name
        .bytes()
        .all(|byte| byte.is_ascii_alphanumeric() || matches!(byte, b'_' | b'-' | b'.'))
    {
        Ok(())
    } else {
        Err(BridgeError::CollectionSchemaMismatch)
    }
}

/// Deterministic vendor collection name for one exact route.
///
/// Fixed 28-byte form `t24c` + 24 lowercase hex digits (96 bits of SHA-256
/// over a domain-separated physical-name/generation preimage). Both route
/// halves are bound in: the same physical name at a different generation
/// addresses a different collection, so a wrong generation reads as
/// [`BridgeError::CollectionNotFound`], never as another generation's data.
///
/// Fixed length is load-bearing on Windows: the native server stores payload
/// indexes under deep per-collection gridstore paths, and names around 40
/// bytes already fail index creation on disposable temp storage with a
/// server-side `Internal` error. Any operator can recompute this name from
/// the route with this function; it carries no secrets.
pub fn collection_name(route: &CollectionRoute) -> Result<String, BridgeError> {
    use sha2::{Digest, Sha256};
    let mut hasher = Sha256::new();
    hasher.update(b"eliot-qdrant-collection/v1\x00");
    hasher.update(route.physical_name.as_str().as_bytes());
    hasher.update(b"\x00");
    hasher.update(route.generation.as_bytes());
    let digest = hasher.finalize();
    let mut name = String::with_capacity(28);
    name.push_str("t24c");
    for byte in digest.iter().take(12) {
        name.push(char::from_digit(u32::from(byte >> 4), 16).unwrap_or('?'));
        name.push(char::from_digit(u32::from(byte & 0x0F), 16).unwrap_or('?'));
    }
    validate_collection_name(&name)?;
    Ok(name)
}

fn hex_from_32(bytes: &[u8; 32]) -> String {
    let mut out = String::with_capacity(64);
    for byte in bytes {
        out.push(char::from_digit(u32::from(byte >> 4), 16).unwrap_or('?'));
        out.push(char::from_digit(u32::from(byte & 0x0F), 16).unwrap_or('?'));
    }
    out
}

const fn hex_val(byte: u8) -> Option<u8> {
    match byte {
        b'0'..=b'9' => Some(byte - b'0'),
        b'a'..=b'f' => Some(byte - b'a' + 10),
        b'A'..=b'F' => Some(byte - b'A' + 10),
        _ => None,
    }
}

fn hex_to_32(text: &str) -> Result<[u8; 32], BridgeError> {
    let bytes = text.as_bytes();
    if bytes.len() != 64 {
        return Err(BridgeError::MalformedResponse);
    }
    let mut out = [0_u8; 32];
    for (index, pair) in bytes.chunks(2).enumerate() {
        let high = hex_val(pair[0]).ok_or(BridgeError::MalformedResponse)?;
        let low = hex_val(pair[1]).ok_or(BridgeError::MalformedResponse)?;
        out[index] = (high << 4) | low;
    }
    Ok(out)
}

/// Provider-neutral 128-bit point ID rendered as a lowercase UUID string for
/// the vendor transport (the full 128 bits survive the round trip).
fn uuid_string(id: &QdrantPointId) -> String {
    let mut hex = String::with_capacity(32);
    for byte in id.0 {
        hex.push(char::from_digit(u32::from(byte >> 4), 16).unwrap_or('?'));
        hex.push(char::from_digit(u32::from(byte & 0x0F), 16).unwrap_or('?'));
    }
    format!(
        "{}-{}-{}-{}-{}",
        &hex[0..8],
        &hex[8..12],
        &hex[12..16],
        &hex[16..20],
        &hex[20..32]
    )
}

fn parse_uuid(text: &str) -> Result<QdrantPointId, BridgeError> {
    let bytes = text.as_bytes();
    if bytes.len() != 36 {
        return Err(BridgeError::MalformedResponse);
    }
    for dash in [8, 13, 18, 23] {
        if bytes[dash] != b'-' {
            return Err(BridgeError::MalformedResponse);
        }
    }
    let mut compact = [0_u8; 32];
    let mut next = 0_usize;
    for (index, byte) in bytes.iter().enumerate() {
        if matches!(index, 8 | 13 | 18 | 23) {
            continue;
        }
        compact[next] = *byte;
        next += 1;
    }
    let mut out = [0_u8; 16];
    for (index, pair) in compact.chunks(2).enumerate() {
        let high = hex_val(pair[0]).ok_or(BridgeError::MalformedResponse)?;
        let low = hex_val(pair[1]).ok_or(BridgeError::MalformedResponse)?;
        out[index] = (high << 4) | low;
    }
    Ok(QdrantPointId(out))
}

fn vendor_point_id(id: &QdrantPointId) -> PointId {
    PointId {
        point_id_options: Some(point_id::PointIdOptions::Uuid(uuid_string(id))),
    }
}

fn bridge_point_id(id: &PointId) -> Result<QdrantPointId, BridgeError> {
    match id.point_id_options.as_ref() {
        Some(point_id::PointIdOptions::Num(number)) => {
            let mut bytes = [0_u8; 16];
            bytes[8..16].copy_from_slice(&number.to_be_bytes());
            Ok(QdrantPointId(bytes))
        }
        Some(point_id::PointIdOptions::Uuid(text)) => parse_uuid(text),
        None => Err(BridgeError::MalformedResponse),
    }
}

const fn strong_ordering() -> WriteOrdering {
    WriteOrdering {
        r#type: WriteOrderingType::Strong as i32,
    }
}

fn update_completed(status: i32) -> bool {
    UpdateStatus::try_from(status).is_ok_and(|parsed| parsed == UpdateStatus::Completed)
}

fn str_value(text: &str) -> Value {
    Value {
        kind: Some(value::Kind::StringValue(text.to_owned())),
    }
}

const fn int_value(number: i64) -> Value {
    Value {
        kind: Some(value::Kind::IntegerValue(number)),
    }
}

fn get_string(payload: &HashMap<String, Value>, key: &str) -> Result<String, BridgeError> {
    match payload.get(key).and_then(|entry| entry.kind.as_ref()) {
        Some(value::Kind::StringValue(text)) => Ok(text.clone()),
        _ => Err(BridgeError::MalformedResponse),
    }
}

fn get_int(payload: &HashMap<String, Value>, key: &str) -> Result<i64, BridgeError> {
    match payload.get(key).and_then(|entry| entry.kind.as_ref()) {
        Some(value::Kind::IntegerValue(number)) => Ok(*number),
        _ => Err(BridgeError::MalformedResponse),
    }
}

fn encode_payload(point: &PointRecord) -> Result<HashMap<String, Value>, BridgeError> {
    let payload = &point.payload;
    let mut map = HashMap::new();
    map.insert(
        EligibilityFilter::INDEXED_FIELDS[0].to_owned(),
        str_value(&hex_from_32(payload.access_partition_digest.as_bytes())),
    );
    map.insert(
        EligibilityFilter::INDEXED_FIELDS[1].to_owned(),
        str_value(payload.source_membership_id.as_str()),
    );
    map.insert(
        "projection_membership_id".to_owned(),
        str_value(payload.projection_membership_id.as_str()),
    );
    map.insert(
        "source_revision".to_owned(),
        int_value(
            i64::try_from(payload.source_revision).map_err(|_| BridgeError::MutationTooLarge)?,
        ),
    );
    map.insert(
        "unit_ordinal".to_owned(),
        int_value(i64::try_from(payload.unit_ordinal).map_err(|_| BridgeError::MutationTooLarge)?),
    );
    map.insert(
        EligibilityFilter::INDEXED_FIELDS[2].to_owned(),
        int_value(payload.valid_from_epoch.get()),
    );
    if let Some(until) = payload.valid_until_epoch_exclusive {
        map.insert(
            EligibilityFilter::INDEXED_FIELDS[3].to_owned(),
            int_value(until.get()),
        );
    }
    map.insert(
        "payload_digest".to_owned(),
        str_value(&hex_from_32(payload.payload_digest.as_bytes())),
    );
    map.insert(
        "identity_digest".to_owned(),
        str_value(&hex_from_32(payload.identity_digest.as_bytes())),
    );
    for (name, vector) in &point.vectors {
        map.insert(
            format!("vector_digest_{name}"),
            str_value(&hex_from_32(vector.digest.as_bytes())),
        );
    }
    Ok(map)
}

fn decode_payload(payload: &HashMap<String, Value>) -> Result<PointPayload, BridgeError> {
    let access = hex_to_32(&get_string(payload, EligibilityFilter::INDEXED_FIELDS[0])?)?;
    let source_membership =
        OpaqueId::new(get_string(payload, EligibilityFilter::INDEXED_FIELDS[1])?)
            .map_err(|_| BridgeError::MalformedResponse)?;
    let projection_membership = OpaqueId::new(get_string(payload, "projection_membership_id")?)
        .map_err(|_| BridgeError::MalformedResponse)?;
    let source_revision = u64::try_from(get_int(payload, "source_revision")?)
        .map_err(|_| BridgeError::MalformedResponse)?;
    let unit_ordinal = u64::try_from(get_int(payload, "unit_ordinal")?)
        .map_err(|_| BridgeError::MalformedResponse)?;
    let valid_from =
        search_contracts::Epoch::new(get_int(payload, EligibilityFilter::INDEXED_FIELDS[2])?)
            .map_err(|_| BridgeError::MalformedResponse)?;
    let valid_until = if payload.contains_key(EligibilityFilter::INDEXED_FIELDS[3]) {
        Some(
            search_contracts::Epoch::new(get_int(payload, EligibilityFilter::INDEXED_FIELDS[3])?)
                .map_err(|_| BridgeError::MalformedResponse)?,
        )
    } else {
        None
    };
    let payload_digest = hex_to_32(&get_string(payload, "payload_digest")?)?;
    let identity_digest = hex_to_32(&get_string(payload, "identity_digest")?)?;
    Ok(PointPayload {
        source_membership_id: source_membership,
        projection_membership_id: projection_membership,
        access_partition_digest: search_contracts::Blake3Digest32::from_bytes(access),
        source_revision,
        unit_ordinal,
        valid_from_epoch: valid_from,
        valid_until_epoch_exclusive: valid_until,
        payload_digest: search_contracts::Blake3Digest32::from_bytes(payload_digest),
        identity_digest: search_contracts::Blake3Digest32::from_bytes(identity_digest),
    })
}

fn encode_vectors(point: &PointRecord) -> Vectors {
    let mut map = HashMap::new();
    for (name, stored) in &point.vectors {
        let (indices, values): (Vec<u32>, Vec<f32>) = stored.values.iter().copied().unzip();
        map.insert(name.clone(), Vector::new_sparse(indices, values));
    }
    Vectors {
        vectors_options: Some(vectors::VectorsOptions::Vectors(
            qdrant_client::qdrant::NamedVectors { vectors: map },
        )),
    }
}

fn decode_vectors(
    output: Option<&qdrant_client::qdrant::VectorsOutput>,
    payload: &HashMap<String, Value>,
    schema: &CollectionSchema,
) -> Result<BTreeMap<String, StoredVector>, BridgeError> {
    let named = match output.and_then(|vectors| vectors.vectors_options.as_ref()) {
        Some(vectors_output::VectorsOptions::Vectors(named)) => &named.vectors,
        _ => return Err(BridgeError::MalformedResponse),
    };
    if named.len() != schema.named_vectors.len() {
        return Err(BridgeError::MalformedResponse);
    }
    let mut decoded = BTreeMap::new();
    for (name, vector_schema) in &schema.named_vectors {
        let entry = named.get(name).ok_or(BridgeError::MalformedResponse)?;
        let Some(vector_output::Vector::Sparse(sparse)) = entry.vector.as_ref() else {
            return Err(BridgeError::MalformedResponse);
        };
        if sparse.indices.len() != sparse.values.len() {
            return Err(BridgeError::MalformedResponse);
        }
        let mut values = Vec::with_capacity(sparse.indices.len());
        for (index, score) in sparse.indices.iter().zip(sparse.values.iter()) {
            if !score.is_finite() {
                return Err(BridgeError::MalformedResponse);
            }
            values.push((*index, *score));
        }
        if values.is_empty()
            || values.windows(2).any(|pair| pair[0].0 >= pair[1].0)
            || values
                .last()
                .is_some_and(|(index, _)| *index >= vector_schema.dimensions)
        {
            return Err(BridgeError::MalformedResponse);
        }
        let digest = hex_to_32(&get_string(payload, &format!("vector_digest_{name}"))?)?;
        decoded.insert(
            name.clone(),
            StoredVector {
                dimensions: vector_schema.dimensions,
                sparse: vector_schema.sparse,
                values,
                digest: search_contracts::Blake3Digest32::from_bytes(digest),
            },
        );
    }
    Ok(decoded)
}

fn decode_point(
    id: &PointId,
    payload: &HashMap<String, Value>,
    vectors: Option<&qdrant_client::qdrant::VectorsOutput>,
    schema: &CollectionSchema,
) -> Result<PointRecord, BridgeError> {
    Ok(PointRecord {
        point_id: bridge_point_id(id)?,
        payload: decode_payload(payload)?,
        vectors: decode_vectors(vectors, payload, schema)?,
    })
}

fn keyword_condition(key: &str, text: String) -> Condition {
    Condition {
        condition_one_of: Some(condition::ConditionOneOf::Field(FieldCondition {
            key: key.to_owned(),
            r#match: Some(Match {
                match_value: Some(r#match::MatchValue::Keyword(text)),
            }),
            ..Default::default()
        })),
    }
}

fn keywords_condition(key: &str, texts: Vec<String>) -> Condition {
    Condition {
        condition_one_of: Some(condition::ConditionOneOf::Field(FieldCondition {
            key: key.to_owned(),
            r#match: Some(Match {
                match_value: Some(r#match::MatchValue::Keywords(RepeatedStrings {
                    strings: texts,
                })),
            }),
            ..Default::default()
        })),
    }
}

fn range_condition(key: &str, gte: Option<f64>, lte: Option<f64>) -> Condition {
    Condition {
        condition_one_of: Some(condition::ConditionOneOf::Field(FieldCondition {
            key: key.to_owned(),
            range: Some(Range {
                gte,
                lte,
                ..Default::default()
            }),
            ..Default::default()
        })),
    }
}

/// Range-bound epoch as an exactly representable `f64`.
///
/// Qdrant `Range` bounds travel as doubles; only exactly representable
/// integers are admitted so a large epoch can never silently truncate into a
/// wider filter.
const fn epoch_bound(number: i64) -> Result<f64, BridgeError> {
    #[allow(clippy::cast_precision_loss)]
    let as_float = number as f64;
    #[allow(clippy::cast_possible_truncation)]
    if as_float as i64 != number {
        return Err(BridgeError::InvalidFilter);
    }
    Ok(as_float)
}

/// Canonical base eligibility plan: the one closed filter value shared
/// verbatim by retrieval and the IDF corpus.
fn base_filter(filter: &EligibilityFilter) -> Result<Filter, BridgeError> {
    if filter.allowed_source_memberships.is_empty() {
        return Err(BridgeError::InvalidFilter);
    }
    let members: Vec<String> = filter
        .allowed_source_memberships
        .iter()
        .map(|member| member.as_str().to_owned())
        .collect();
    let from = epoch_bound(filter.visible_epoch.get())?;
    let until = epoch_bound(filter.visible_epoch.get())?;
    Ok(Filter {
        must: vec![
            keyword_condition(
                EligibilityFilter::INDEXED_FIELDS[0],
                hex_from_32(filter.access_partition_digest.as_bytes()),
            ),
            keywords_condition(EligibilityFilter::INDEXED_FIELDS[1], members),
            range_condition(EligibilityFilter::INDEXED_FIELDS[2], None, Some(from)),
        ],
        must_not: vec![range_condition(
            EligibilityFilter::INDEXED_FIELDS[3],
            None,
            Some(until),
        )],
        ..Default::default()
    })
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
        if vector.dimensions != vector_schema.dimensions || vector.sparse != vector_schema.sparse {
            return Err(BridgeError::VectorDimensionMismatch);
        }
        if vector.values.is_empty()
            || vector.values.iter().any(|(_, score)| !score.is_finite())
            || vector.values.windows(2).any(|pair| pair[0].0 >= pair[1].0)
            || vector
                .values
                .last()
                .is_some_and(|(index, _)| *index >= vector_schema.dimensions)
        {
            return Err(BridgeError::VectorDimensionMismatch);
        }
        stored_values = stored_values
            .checked_add(vector.values.len())
            .ok_or(BridgeError::MutationTooLarge)?;
    }
    if stored_values > limits.max_vector_values_per_point {
        return Err(BridgeError::MutationTooLarge);
    }
    encode_payload(point)?;
    Ok(())
}

fn validate_exact_ids(
    ids: Vec<QdrantPointId>,
    limit: usize,
) -> Result<Vec<QdrantPointId>, BridgeError> {
    if ids.is_empty() || ids.len() > limit {
        return Err(BridgeError::MutationTooLarge);
    }
    let mut sorted = ids;
    sorted.sort();
    if sorted.windows(2).any(|pair| pair[0] == pair[1]) {
        return Err(BridgeError::DuplicatePointId);
    }
    Ok(sorted)
}

fn validate_query_vector(query: &[(u32, f32)], dimensions: u32) -> Result<(), BridgeError> {
    if query.is_empty()
        || query.iter().any(|(_, score)| !score.is_finite())
        || query.windows(2).any(|pair| pair[0].0 >= pair[1].0)
        || query.last().is_some_and(|(index, _)| *index >= dimensions)
    {
        return Err(BridgeError::VectorDimensionMismatch);
    }
    Ok(())
}

/// Maps a vendor failure on a read path (no commit is possible) to a stable
/// typed error. Status numbers are gRPC canonical codes; status text never
/// crosses into errors.
fn map_read_error(error: qdrant_client::QdrantError) -> BridgeError {
    match error {
        qdrant_client::QdrantError::ResponseError { status }
        | qdrant_client::QdrantError::ResourceExhaustedError { status, .. } => {
            map_read_status(status.code() as i32)
        }
        qdrant_client::QdrantError::ConversionError(_)
        | qdrant_client::QdrantError::InvalidUri(_)
        | qdrant_client::QdrantError::NoSnapshotFound(_) => BridgeError::MalformedResponse,
        qdrant_client::QdrantError::Io(_) => BridgeError::TransportFailed,
    }
}

const fn map_read_status(code: i32) -> BridgeError {
    match code {
        CODE_NOT_FOUND => BridgeError::CollectionNotFound,
        CODE_ALREADY_EXISTS => BridgeError::CollectionAlreadyExists,
        CODE_INVALID_ARGUMENT | CODE_FAILED_PRECONDITION | CODE_OUT_OF_RANGE => {
            BridgeError::UnindexedFilter
        }
        CODE_PERMISSION_DENIED | CODE_UNAUTHENTICATED => BridgeError::AuthenticationInvalid,
        CODE_RESOURCE_EXHAUSTED => BridgeError::QueryBudgetExceeded,
        _ => BridgeError::TransportFailed,
    }
}

/// Maps a vendor failure after a mutation dispatch. Definite server
/// rejections keep their typed codes; any loss, deadline or ambiguous status
/// becomes `MutationOutcomeUnknown` because the write may have committed.
fn map_mutation_error(error: qdrant_client::QdrantError) -> BridgeError {
    match error {
        qdrant_client::QdrantError::ResponseError { status }
        | qdrant_client::QdrantError::ResourceExhaustedError { status, .. } => {
            map_mutation_status(status.code() as i32)
        }
        qdrant_client::QdrantError::ConversionError(_)
        | qdrant_client::QdrantError::InvalidUri(_)
        | qdrant_client::QdrantError::NoSnapshotFound(_) => BridgeError::MalformedResponse,
        qdrant_client::QdrantError::Io(_) => BridgeError::MutationOutcomeUnknown,
    }
}

const fn map_mutation_status(code: i32) -> BridgeError {
    match code {
        CODE_NOT_FOUND => BridgeError::CollectionNotFound,
        CODE_ALREADY_EXISTS => BridgeError::CollectionAlreadyExists,
        CODE_INVALID_ARGUMENT | CODE_FAILED_PRECONDITION | CODE_OUT_OF_RANGE => {
            BridgeError::VectorDimensionMismatch
        }
        CODE_PERMISSION_DENIED | CODE_UNAUTHENTICATED => BridgeError::AuthenticationInvalid,
        CODE_RESOURCE_EXHAUSTED => BridgeError::MutationTooLarge,
        _ => BridgeError::MutationOutcomeUnknown,
    }
}

/// Maps a vendor failure on collection creation. Creation carries no mutation
/// identity, so loss reports `MutationOutcomeUnknown`; a retry then converges
/// through `CollectionAlreadyExists` plus [`RealDataPlane::verify_schema`].
fn map_create_error(error: qdrant_client::QdrantError) -> BridgeError {
    match error {
        qdrant_client::QdrantError::ResponseError { status }
        | qdrant_client::QdrantError::ResourceExhaustedError { status, .. } => {
            match status.code() as i32 {
                CODE_NOT_FOUND => BridgeError::CollectionNotFound,
                CODE_ALREADY_EXISTS => BridgeError::CollectionAlreadyExists,
                CODE_INVALID_ARGUMENT | CODE_FAILED_PRECONDITION | CODE_OUT_OF_RANGE => {
                    BridgeError::CollectionSchemaMismatch
                }
                CODE_PERMISSION_DENIED | CODE_UNAUTHENTICATED => BridgeError::AuthenticationInvalid,
                CODE_RESOURCE_EXHAUSTED => BridgeError::MutationTooLarge,
                _ => BridgeError::MutationOutcomeUnknown,
            }
        }
        qdrant_client::QdrantError::ConversionError(_)
        | qdrant_client::QdrantError::InvalidUri(_)
        | qdrant_client::QdrantError::NoSnapshotFound(_) => BridgeError::CollectionSchemaMismatch,
        qdrant_client::QdrantError::Io(_) => BridgeError::MutationOutcomeUnknown,
    }
}

fn verify_server_schema(
    info: &CollectionInfo,
    schema: &CollectionSchema,
) -> Result<(), BridgeError> {
    let params = info
        .config
        .as_ref()
        .and_then(|config| config.params.as_ref())
        .ok_or(BridgeError::CollectionSchemaMismatch)?;
    if params.shard_number != 1 {
        return Err(BridgeError::CollectionSchemaMismatch);
    }
    let sparse = params
        .sparse_vectors_config
        .as_ref()
        .ok_or(BridgeError::CollectionSchemaMismatch)?;
    if sparse.map.len() != schema.named_vectors.len() {
        return Err(BridgeError::CollectionSchemaMismatch);
    }
    for (name, vector_schema) in &schema.named_vectors {
        let remote = sparse
            .map
            .get(name)
            .ok_or(BridgeError::NamedVectorMissing)?;
        let want = if vector_schema.idf_enabled {
            Some(Modifier::Idf as i32)
        } else {
            None
        };
        if remote.modifier != want {
            return Err(BridgeError::CollectionSchemaMismatch);
        }
    }
    for field in EligibilityFilter::INDEXED_FIELDS {
        if !info.payload_schema.contains_key(field) {
            return Err(BridgeError::PayloadIndexMissing);
        }
    }
    let strict = info
        .config
        .as_ref()
        .and_then(|config| config.strict_mode_config.as_ref())
        .ok_or(BridgeError::StrictModeRequired)?;
    if !strict.enabled.unwrap_or_default()
        || strict.unindexed_filtering_retrieve.unwrap_or(true)
        || strict.unindexed_filtering_update.unwrap_or(true)
    {
        return Err(BridgeError::StrictModeRequired);
    }
    Ok(())
}

/// Real Qdrant data plane over the pinned client transport.
///
/// Owns the vendor client opaquely (the type never appears in public
/// signatures), the schemas it created, and a bounded mutation-identity
/// ledger for exact replay. There is no fallback to the in-memory oracle.
pub struct RealDataPlane {
    client: Qdrant,
    gate: QualifiedGate,
    limits: BridgeLimits,
    schemas: BTreeMap<String, CollectionSchema>,
    operations: BTreeMap<OpaqueId, MutationReceipt>,
}

impl RealDataPlane {
    /// Connects to a disposable loopback server admitted by an executed
    /// qualification gate and rechecks the live server identity.
    ///
    /// A dead endpoint fails with [`BridgeError::TransportFailed`] (definite:
    /// nothing was sent); a version/build drift fails with
    /// [`BridgeError::CapabilityReceiptMismatch`].
    pub async fn connect(
        endpoint: &LiveEndpoint,
        gate: QualifiedGate,
        limits: BridgeLimits,
    ) -> Result<Self, BridgeError> {
        let limits = limits.validate()?;
        let client = Qdrant::from_url(&endpoint.grpc_url())
            .timeout(Duration::from_secs(10))
            .connect_timeout(Duration::from_secs(5))
            .skip_compatibility_check()
            .build()
            .map_err(|_| BridgeError::TransportFailed)?;
        let reply = tokio::time::timeout(Duration::from_secs(15), client.health_check())
            .await
            .map_err(|_| BridgeError::TransportFailed)?
            .map_err(|_| BridgeError::TransportFailed)?;
        if reply.version != QUALIFIED_SERVER_VERSION
            || reply.version != gate.server_version()
            || reply
                .commit
                .as_deref()
                .is_none_or(|commit| !commit.starts_with(QUALIFIED_SERVER_BUILD))
        {
            return Err(BridgeError::CapabilityReceiptMismatch);
        }
        Ok(Self {
            client,
            gate,
            limits,
            schemas: BTreeMap::new(),
            operations: BTreeMap::new(),
        })
    }

    /// Admitted gate bound to this data plane (evidence, not transport).
    #[must_use]
    pub const fn gate(&self) -> &QualifiedGate {
        &self.gate
    }

    /// Creates one new opaque physical generation with mandatory payload
    /// indexes, strict-mode floors and post-creation schema verification.
    pub async fn create_collection(
        &mut self,
        route: &CollectionRoute,
        schema: &CollectionSchema,
        context: &OpContext,
    ) -> Result<ReceiptRef, BridgeError> {
        context.check()?;
        schema.validate()?;
        for vector_schema in schema.named_vectors.values() {
            if !vector_schema.sparse {
                return Err(BridgeError::CollectionSchemaMismatch);
            }
        }
        let name = collection_name(route)?;
        if self.schemas.contains_key(&name) {
            return Err(BridgeError::CollectionAlreadyExists);
        }
        if self
            .client
            .collection_exists(name.clone())
            .await
            .map_err(map_create_error)?
        {
            return Err(BridgeError::CollectionAlreadyExists);
        }
        let mut sparse = HashMap::new();
        for (vector_name, vector_schema) in &schema.named_vectors {
            sparse.insert(
                vector_name.clone(),
                SparseVectorParams {
                    modifier: if vector_schema.idf_enabled {
                        Some(Modifier::Idf as i32)
                    } else {
                        None
                    },
                    ..Default::default()
                },
            );
        }
        let create = CreateCollection {
            collection_name: name.clone(),
            shard_number: Some(1),
            replication_factor: Some(1),
            write_consistency_factor: Some(1),
            sparse_vectors_config: Some(SparseVectorConfig { map: sparse }),
            strict_mode_config: Some(StrictModeConfig {
                enabled: Some(true),
                unindexed_filtering_retrieve: Some(false),
                unindexed_filtering_update: Some(false),
                ..Default::default()
            }),
            ..Default::default()
        };
        let created =
            tokio::time::timeout(context.deadline(), self.client.create_collection(create))
                .await
                .map_err(|_| BridgeError::MutationOutcomeUnknown)?
                .map_err(map_create_error)?;
        if !created.result {
            return Err(BridgeError::MutationOutcomeUnknown);
        }
        for (field, field_type) in [
            (EligibilityFilter::INDEXED_FIELDS[0], FieldType::Keyword),
            (EligibilityFilter::INDEXED_FIELDS[1], FieldType::Keyword),
            (EligibilityFilter::INDEXED_FIELDS[2], FieldType::Integer),
            (EligibilityFilter::INDEXED_FIELDS[3], FieldType::Integer),
        ] {
            context.check()?;
            let indexed = tokio::time::timeout(
                context.deadline(),
                self.client.create_field_index(CreateFieldIndexCollection {
                    collection_name: name.clone(),
                    field_name: (*field).to_owned(),
                    field_type: Some(field_type as i32),
                    wait: Some(true),
                    ordering: Some(strong_ordering()),
                    ..Default::default()
                }),
            )
            .await
            .map_err(|_| BridgeError::TransportFailed)?
            .map_err(|_| BridgeError::TransportFailed)?;
            if !indexed
                .result
                .as_ref()
                .is_some_and(|result| update_completed(result.status))
            {
                return Err(BridgeError::TransportFailed);
            }
        }
        context.check()?;
        self.verify_server_schema(&name, schema, context).await?;
        self.schemas.insert(name.clone(), schema.clone());
        ReceiptRef::new(format!("qdrant:collection:{name}"))
            .map_err(|_| BridgeError::CollectionSchemaMismatch)
    }

    async fn verify_server_schema(
        &self,
        name: &str,
        schema: &CollectionSchema,
        context: &OpContext,
    ) -> Result<(), BridgeError> {
        let info = tokio::time::timeout(context.deadline(), self.client.collection_info(name))
            .await
            .map_err(|_| BridgeError::TransportFailed)?
            .map_err(map_read_error)?
            .result
            .ok_or(BridgeError::MalformedResponse)?;
        verify_server_schema(&info, schema)
    }

    /// Verifies exact readback schema identity for a managed collection.
    pub async fn verify_schema(
        &self,
        route: &CollectionRoute,
        expected: &CollectionSchema,
        context: &OpContext,
    ) -> Result<ReceiptRef, BridgeError> {
        context.check()?;
        expected.validate()?;
        let name = collection_name(route)?;
        let actual = self
            .schemas
            .get(&name)
            .ok_or(BridgeError::CollectionNotFound)?;
        actual.validate()?;
        if actual != expected {
            return Err(BridgeError::CollectionSchemaMismatch);
        }
        self.verify_server_schema(&name, expected, context).await?;
        ReceiptRef::new(format!("qdrant:schema:{name}"))
            .map_err(|_| BridgeError::CollectionSchemaMismatch)
    }

    /// Upserts only explicit point IDs with `wait=true`, strong ordering and
    /// exact readback before success. Same identity plus same canonical batch
    /// replays without a second write; same identity plus different input is
    /// [`BridgeError::OperationConflict`].
    pub async fn upsert_exact(
        &mut self,
        route: &CollectionRoute,
        points: Vec<PointRecord>,
        mutation: BridgeMutation,
        context: &OpContext,
    ) -> Result<MutationReceipt, BridgeError> {
        context.check()?;
        if let Some(replay) = self.replay(&mutation)? {
            return Ok(replay);
        }
        if self.operations.len() >= self.limits.max_operation_receipts {
            return Err(BridgeError::MutationTooLarge);
        }
        if points.is_empty() || points.len() > self.limits.max_points_per_mutation {
            return Err(BridgeError::MutationTooLarge);
        }
        let name = collection_name(route)?;
        let schema = self
            .schemas
            .get(&name)
            .ok_or(BridgeError::CollectionNotFound)?
            .clone();
        let mut seen = BTreeSet::new();
        for point in &points {
            if !seen.insert(point.point_id) {
                return Err(BridgeError::DuplicatePointId);
            }
            validate_point(point, &schema, self.limits)?;
        }
        let mut vendor_points = Vec::with_capacity(points.len());
        for point in &points {
            vendor_points.push(PointStruct {
                id: Some(vendor_point_id(&point.point_id)),
                payload: encode_payload(point)?,
                vectors: Some(encode_vectors(point)),
            });
        }
        let acked = tokio::time::timeout(
            context.deadline(),
            self.client.upsert_points(UpsertPoints {
                collection_name: name.clone(),
                wait: Some(true),
                ordering: Some(strong_ordering()),
                points: vendor_points,
                ..Default::default()
            }),
        )
        .await
        .map_err(|_| BridgeError::MutationOutcomeUnknown)?
        .map_err(map_mutation_error)?;
        if !acked
            .result
            .as_ref()
            .is_some_and(|result| update_completed(result.status))
        {
            return Err(BridgeError::MutationOutcomeUnknown);
        }
        context.check()?;
        let mut affected: Vec<QdrantPointId> = points.iter().map(|point| point.point_id).collect();
        affected.sort();
        self.verify_upsert_readback(&name, &points, &schema, context)
            .await?;
        self.record_mutation(route.clone(), mutation, affected)
    }

    async fn verify_upsert_readback(
        &self,
        name: &str,
        expected: &[PointRecord],
        schema: &CollectionSchema,
        context: &OpContext,
    ) -> Result<(), BridgeError> {
        let ids: Vec<PointId> = expected
            .iter()
            .map(|point| vendor_point_id(&point.point_id))
            .collect();
        let readback = tokio::time::timeout(
            context.deadline(),
            self.client.get_points(GetPoints {
                collection_name: name.to_owned(),
                ids,
                with_payload: Some(true.into()),
                with_vectors: Some(true.into()),
                ..Default::default()
            }),
        )
        .await
        .map_err(|_| BridgeError::MutationOutcomeUnknown)?
        .map_err(|_| BridgeError::MutationOutcomeUnknown)?;
        if readback.result.len() != expected.len() {
            return Err(BridgeError::ExactReadbackMismatch);
        }
        for point in expected {
            let found = readback
                .result
                .iter()
                .find(|retrieved| {
                    retrieved.id.as_ref().is_some_and(|id| {
                        bridge_point_id(id).is_ok_and(|parsed| parsed == point.point_id)
                    })
                })
                .ok_or(BridgeError::ExactReadbackMismatch)?;
            let decoded = decode_point(
                found.id.as_ref().ok_or(BridgeError::MalformedResponse)?,
                &found.payload,
                found.vectors.as_ref(),
                schema,
            )
            .map_err(|_| BridgeError::ExactReadbackMismatch)?;
            if decoded != *point {
                return Err(BridgeError::ExactReadbackMismatch);
            }
        }
        Ok(())
    }

    /// Sets the exact exclusive upper epoch on explicit point IDs via
    /// payload-only update (no broad-filter closure exists). The current
    /// upper bound is read first, so a stale close fails pre-dispatch with
    /// [`BridgeError::ExactReadbackMismatch`] and a missing ID with
    /// [`BridgeError::PointNotFound`].
    pub async fn close_exact(
        &mut self,
        route: &CollectionRoute,
        ids: Vec<QdrantPointId>,
        valid_until_epoch_exclusive: search_contracts::Epoch,
        mutation: BridgeMutation,
        context: &OpContext,
    ) -> Result<MutationReceipt, BridgeError> {
        context.check()?;
        if let Some(replay) = self.replay(&mutation)? {
            return Ok(replay);
        }
        if self.operations.len() >= self.limits.max_operation_receipts {
            return Err(BridgeError::MutationTooLarge);
        }
        let ids = validate_exact_ids(ids, self.limits.max_points_per_mutation)?;
        let name = collection_name(route)?;
        let schema = self
            .schemas
            .get(&name)
            .ok_or(BridgeError::CollectionNotFound)?
            .clone();
        let current = self
            .fetch_points(&name, &ids, &schema, context)
            .await
            .map_err(|error| match error {
                BridgeError::TransportFailed => BridgeError::TransportFailed,
                BridgeError::CollectionNotFound => BridgeError::CollectionNotFound,
                _ => BridgeError::ExactReadbackMismatch,
            })?;
        for id in &ids {
            let point = current
                .iter()
                .find(|point| point.point_id == *id)
                .ok_or(BridgeError::PointNotFound)?;
            if valid_until_epoch_exclusive <= point.payload.valid_from_epoch {
                return Err(BridgeError::ExactReadbackMismatch);
            }
        }
        let mut payload = HashMap::new();
        payload.insert(
            EligibilityFilter::INDEXED_FIELDS[3].to_owned(),
            int_value(valid_until_epoch_exclusive.get()),
        );
        let acked = tokio::time::timeout(
            context.deadline(),
            self.client.set_payload(SetPayloadPoints {
                collection_name: name.clone(),
                wait: Some(true),
                payload,
                points_selector: Some(PointsSelector {
                    points_selector_one_of: Some(points_selector::PointsSelectorOneOf::Points(
                        PointsIdsList {
                            ids: ids.iter().map(vendor_point_id).collect(),
                        },
                    )),
                }),
                ordering: Some(strong_ordering()),
                ..Default::default()
            }),
        )
        .await
        .map_err(|_| BridgeError::MutationOutcomeUnknown)?
        .map_err(map_mutation_error)?;
        if !acked
            .result
            .as_ref()
            .is_some_and(|result| update_completed(result.status))
        {
            return Err(BridgeError::MutationOutcomeUnknown);
        }
        context.check()?;
        let after = self
            .fetch_points(&name, &ids, &schema, context)
            .await
            .map_err(|_| BridgeError::MutationOutcomeUnknown)?;
        for id in &ids {
            let point = after
                .iter()
                .find(|point| point.point_id == *id)
                .ok_or(BridgeError::MutationOutcomeUnknown)?;
            if point.payload.valid_until_epoch_exclusive != Some(valid_until_epoch_exclusive) {
                return Err(BridgeError::ExactReadbackMismatch);
            }
        }
        self.record_mutation(route.clone(), mutation, ids)
    }

    /// Deletes only explicit exact point IDs with `wait=true` and strong
    /// ordering, then proves absence through exact readback.
    pub async fn delete_exact(
        &mut self,
        route: &CollectionRoute,
        ids: Vec<QdrantPointId>,
        mutation: BridgeMutation,
        context: &OpContext,
    ) -> Result<MutationReceipt, BridgeError> {
        context.check()?;
        if let Some(replay) = self.replay(&mutation)? {
            return Ok(replay);
        }
        if self.operations.len() >= self.limits.max_operation_receipts {
            return Err(BridgeError::MutationTooLarge);
        }
        let ids = validate_exact_ids(ids, self.limits.max_points_per_mutation)?;
        let name = collection_name(route)?;
        let acked = tokio::time::timeout(
            context.deadline(),
            self.client.delete_points(DeletePoints {
                collection_name: name.clone(),
                wait: Some(true),
                points: Some(PointsSelector {
                    points_selector_one_of: Some(points_selector::PointsSelectorOneOf::Points(
                        PointsIdsList {
                            ids: ids.iter().map(vendor_point_id).collect(),
                        },
                    )),
                }),
                ordering: Some(strong_ordering()),
                ..Default::default()
            }),
        )
        .await
        .map_err(|_| BridgeError::MutationOutcomeUnknown)?
        .map_err(map_mutation_error)?;
        if !acked
            .result
            .as_ref()
            .is_some_and(|result| update_completed(result.status))
        {
            return Err(BridgeError::MutationOutcomeUnknown);
        }
        context.check()?;
        let present = tokio::time::timeout(
            context.deadline(),
            self.client.get_points(GetPoints {
                collection_name: name.clone(),
                ids: ids.iter().map(vendor_point_id).collect(),
                with_payload: Some(false.into()),
                with_vectors: Some(false.into()),
                ..Default::default()
            }),
        )
        .await
        .map_err(|_| BridgeError::MutationOutcomeUnknown)?
        .map_err(|_| BridgeError::MutationOutcomeUnknown)?;
        if !present.result.is_empty() {
            return Err(BridgeError::ExactReadbackMismatch);
        }
        self.record_mutation(route.clone(), mutation, ids)
    }

    async fn fetch_points(
        &self,
        name: &str,
        ids: &[QdrantPointId],
        schema: &CollectionSchema,
        context: &OpContext,
    ) -> Result<Vec<PointRecord>, BridgeError> {
        let readback = tokio::time::timeout(
            context.deadline(),
            self.client.get_points(GetPoints {
                collection_name: name.to_owned(),
                ids: ids.iter().map(vendor_point_id).collect(),
                with_payload: Some(true.into()),
                with_vectors: Some(true.into()),
                ..Default::default()
            }),
        )
        .await
        .map_err(|_| BridgeError::TransportFailed)?
        .map_err(map_read_error)?;
        let mut points = Vec::with_capacity(readback.result.len());
        for retrieved in &readback.result {
            points.push(decode_point(
                retrieved
                    .id
                    .as_ref()
                    .ok_or(BridgeError::MalformedResponse)?,
                &retrieved.payload,
                retrieved.vectors.as_ref(),
                schema,
            )?);
        }
        Ok(points)
    }

    /// Reads back exactly the requested identifiers with explicit
    /// missing/unexpected sets.
    pub async fn readback_exact(
        &self,
        route: &CollectionRoute,
        ids: Vec<QdrantPointId>,
        context: &OpContext,
    ) -> Result<BoundedPointReadback, BridgeError> {
        context.check()?;
        let ids = validate_exact_ids(ids, self.limits.max_points_per_mutation)?;
        let name = collection_name(route)?;
        let schema = self
            .schemas
            .get(&name)
            .ok_or(BridgeError::CollectionNotFound)?;
        let readback = tokio::time::timeout(
            context.deadline(),
            self.client.get_points(GetPoints {
                collection_name: name.clone(),
                ids: ids.iter().map(vendor_point_id).collect(),
                with_payload: Some(true.into()),
                with_vectors: Some(true.into()),
                ..Default::default()
            }),
        )
        .await
        .map_err(|_| BridgeError::TransportFailed)?
        .map_err(map_read_error)?;
        if readback.result.len() > ids.len() {
            return Err(BridgeError::MalformedResponse);
        }
        let requested: BTreeSet<QdrantPointId> = ids.into_iter().collect();
        let mut points = Vec::new();
        let mut seen = BTreeSet::new();
        let mut unexpected_ids = Vec::new();
        for retrieved in &readback.result {
            let id = bridge_point_id(
                retrieved
                    .id
                    .as_ref()
                    .ok_or(BridgeError::MalformedResponse)?,
            )?;
            if !requested.contains(&id) {
                unexpected_ids.push(id);
                continue;
            }
            seen.insert(id);
            points.push(decode_point(
                retrieved
                    .id
                    .as_ref()
                    .ok_or(BridgeError::MalformedResponse)?,
                &retrieved.payload,
                retrieved.vectors.as_ref(),
                schema,
            )?);
        }
        let missing_ids: Vec<QdrantPointId> = requested.difference(&seen).copied().collect();
        Ok(BoundedPointReadback {
            points,
            missing_ids,
            unexpected_ids,
        })
    }

    /// Counts the exact already-authorized filter population with
    /// `exact=true`. Unindexed or strict-rejected filters fail with
    /// [`BridgeError::UnindexedFilter`] rather than scanning.
    pub async fn count_exact(
        &self,
        route: &CollectionRoute,
        filter: &EligibilityFilter,
        context: &OpContext,
    ) -> Result<ExactCount, BridgeError> {
        context.check()?;
        if filter.allowed_source_memberships.is_empty() {
            return Err(BridgeError::InvalidFilter);
        }
        let name = collection_name(route)?;
        let vendor_filter = base_filter(filter)?;
        let counted = tokio::time::timeout(
            context.deadline(),
            self.client.count(CountPoints {
                collection_name: name,
                filter: Some(vendor_filter),
                exact: Some(true),
                ..Default::default()
            }),
        )
        .await
        .map_err(|_| BridgeError::TransportFailed)?
        .map_err(map_read_error)?;
        let count = counted
            .result
            .as_ref()
            .ok_or(BridgeError::MalformedResponse)?
            .count;
        Ok(ExactCount {
            count: usize::try_from(count).map_err(|_| BridgeError::MalformedResponse)?,
        })
    }

    /// Scrolls one bounded page of the exact filter population. The caller
    /// iterates with [`ScrollPage::next_offset`] and checks cancellation
    /// between pages; an empty page ends the walk with no continuation.
    pub async fn scroll_exact(
        &self,
        route: &CollectionRoute,
        filter: &EligibilityFilter,
        offset: Option<QdrantPointId>,
        limit: usize,
        context: &OpContext,
    ) -> Result<ScrollPage, BridgeError> {
        context.check()?;
        if filter.allowed_source_memberships.is_empty() {
            return Err(BridgeError::InvalidFilter);
        }
        if limit == 0 || limit > self.limits.max_query_candidates {
            return Err(BridgeError::QueryBudgetExceeded);
        }
        let name = collection_name(route)?;
        let schema = self
            .schemas
            .get(&name)
            .ok_or(BridgeError::CollectionNotFound)?;
        let vendor_filter = base_filter(filter)?;
        let scrolled = tokio::time::timeout(
            context.deadline(),
            self.client.scroll(ScrollPoints {
                collection_name: name,
                filter: Some(vendor_filter),
                offset: offset.as_ref().map(vendor_point_id),
                limit: Some(u32::try_from(limit).map_err(|_| BridgeError::QueryBudgetExceeded)?),
                with_payload: Some(true.into()),
                with_vectors: Some(true.into()),
                ..Default::default()
            }),
        )
        .await
        .map_err(|_| BridgeError::TransportFailed)?
        .map_err(map_read_error)?;
        if scrolled.result.len() > limit {
            return Err(BridgeError::MalformedResponse);
        }
        let mut points = Vec::with_capacity(scrolled.result.len());
        for retrieved in &scrolled.result {
            points.push(decode_point(
                retrieved
                    .id
                    .as_ref()
                    .ok_or(BridgeError::MalformedResponse)?,
                &retrieved.payload,
                retrieved.vectors.as_ref(),
                schema,
            )?);
        }
        let next_offset = if scrolled.result.is_empty() {
            None
        } else {
            scrolled
                .next_page_offset
                .as_ref()
                .map(bridge_point_id)
                .transpose()?
        };
        Ok(ScrollPage {
            points,
            next_offset,
        })
    }

    /// Returns bounded filtered nominations for one already-authorized leg.
    /// Retrieval and `idf.corpus` are rendered from the single `filter`
    /// contract: [`IdfScope::ScopedToRetrieval`] clones the retrieval filter
    /// as the corpus, [`IdfScope::Global`] omits it. Scores are finite
    /// nominations, never evidence.
    pub async fn query_filtered(
        &self,
        route: &CollectionRoute,
        filter: &EligibilityFilter,
        vector_name: &str,
        query: &[(u32, f32)],
        limit: usize,
        idf: IdfScope,
        context: &OpContext,
    ) -> Result<Vec<CandidateNomination>, BridgeError> {
        context.check()?;
        if filter.allowed_source_memberships.is_empty() {
            return Err(BridgeError::InvalidFilter);
        }
        if limit == 0 || limit > self.limits.max_query_candidates {
            return Err(BridgeError::QueryBudgetExceeded);
        }
        let name = collection_name(route)?;
        let schema = self
            .schemas
            .get(&name)
            .ok_or(BridgeError::CollectionNotFound)?;
        let vector_schema = schema
            .named_vectors
            .get(vector_name)
            .ok_or(BridgeError::NamedVectorMissing)?;
        validate_query_vector(query, vector_schema.dimensions)?;
        let vendor_filter = base_filter(filter)?;
        let corpus = match idf {
            IdfScope::Global => None,
            IdfScope::ScopedToRetrieval => Some(vendor_filter.clone()),
        };
        let (indices, values): (Vec<u32>, Vec<f32>) = query.iter().copied().unzip();
        let answered = tokio::time::timeout(
            context.deadline(),
            self.client.query(QueryPoints {
                collection_name: name,
                query: Some(Query::new_nearest(VectorInput::new_sparse(indices, values))),
                using: Some(vector_name.to_owned()),
                filter: Some(vendor_filter),
                params: Some(SearchParams {
                    exact: Some(true),
                    idf: corpus.map(|population| IdfParams {
                        corpus: Some(population),
                    }),
                    ..Default::default()
                }),
                limit: Some(u64::try_from(limit).map_err(|_| BridgeError::QueryBudgetExceeded)?),
                with_payload: Some(true.into()),
                ..Default::default()
            }),
        )
        .await
        .map_err(|_| BridgeError::TransportFailed)?
        .map_err(map_read_error)?;
        if answered.result.len() > limit {
            return Err(BridgeError::MalformedResponse);
        }
        let mut nominations = Vec::with_capacity(answered.result.len());
        for scored in &answered.result {
            if !scored.score.is_finite() {
                return Err(BridgeError::InvalidScore);
            }
            let point_id =
                bridge_point_id(scored.id.as_ref().ok_or(BridgeError::MalformedResponse)?)?;
            let payload_digest = hex_to_32(&get_string(&scored.payload, "payload_digest")?)?;
            let identity_digest = hex_to_32(&get_string(&scored.payload, "identity_digest")?)?;
            nominations.push(CandidateNomination {
                point_id,
                score: scored.score,
                payload_digest: search_contracts::Blake3Digest32::from_bytes(payload_digest),
                identity_digest: search_contracts::Blake3Digest32::from_bytes(identity_digest),
            });
        }
        Ok(nominations)
    }

    fn replay(&self, mutation: &BridgeMutation) -> Result<Option<MutationReceipt>, BridgeError> {
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn collection_name_binds_physical_and_generation() {
        let left = CollectionRoute {
            generation: search_contracts::CollectionGenerationId::from_bytes([0x11; 16]),
            physical_name: OpaqueId::new("t24_unit").expect("name"),
        };
        let right = CollectionRoute {
            generation: search_contracts::CollectionGenerationId::from_bytes([0x12; 16]),
            physical_name: OpaqueId::new("t24_unit").expect("name"),
        };
        let renamed = CollectionRoute {
            generation: search_contracts::CollectionGenerationId::from_bytes([0x11; 16]),
            physical_name: OpaqueId::new("t24_other").expect("name"),
        };
        let left_name = collection_name(&left).expect("left");
        // Fixed short budget for Windows gridstore paths (see `collection_name`).
        assert_eq!(left_name.len(), 28);
        assert!(left_name.starts_with("t24c"));
        assert_eq!(collection_name(&left).expect("deterministic"), left_name);
        assert_ne!(
            collection_name(&right).expect("generation bound"),
            left_name
        );
        assert_ne!(
            collection_name(&renamed).expect("physical bound"),
            left_name
        );
    }

    #[test]
    fn uuid_round_trip_preserves_128_bits() {
        let id = QdrantPointId([
            0x00, 0x11, 0x22, 0x33, 0x44, 0x55, 0x66, 0x77, 0x88, 0x99, 0xAA, 0xBB, 0xCC, 0xDD,
            0xEE, 0xFF,
        ]);
        let text = uuid_string(&id);
        assert_eq!(text, "00112233-4455-6677-8899-aabbccddeeff");
        assert_eq!(parse_uuid(&text).expect("round trip"), id);
        assert_eq!(
            parse_uuid("not-a-uuid").expect_err("rejects garbage"),
            BridgeError::MalformedResponse
        );
        assert_eq!(
            parse_uuid("00112233-4455-6677-8899-aabbccddeefg").expect_err("rejects non-hex"),
            BridgeError::MalformedResponse
        );
    }

    #[test]
    fn hex_32_round_trip() {
        let bytes = [0xABu8; 32];
        let text = hex_from_32(&bytes);
        assert_eq!(text.len(), 64);
        assert_eq!(hex_to_32(&text).expect("round trip"), bytes);
        assert_eq!(
            hex_to_32("short").expect_err("rejects short"),
            BridgeError::MalformedResponse
        );
    }

    #[test]
    fn empty_filter_and_inexact_epoch_rejected_before_dispatch() {
        let mut allowed = BTreeSet::new();
        allowed.insert(OpaqueId::new("member").expect("member"));
        let filter = EligibilityFilter {
            access_partition_digest: search_contracts::Blake3Digest32::from_bytes([0x01; 32]),
            allowed_source_memberships: allowed,
            visible_epoch: search_contracts::Epoch::new(42).expect("epoch"),
        };
        assert!(base_filter(&filter).is_ok());
        let mut empty = filter;
        empty.allowed_source_memberships.clear();
        assert_eq!(
            base_filter(&empty).expect_err("empty"),
            BridgeError::InvalidFilter
        );
        // 2^53 + 1 is not exactly representable as f64: the filter must fail,
        // never silently widen.
        assert_eq!(
            epoch_bound(9_007_199_254_740_993).expect_err("inexact"),
            BridgeError::InvalidFilter
        );
        assert!(epoch_bound(42).is_ok());
    }

    #[test]
    fn error_codes_are_stable_and_redacted() {
        for error in [
            BridgeError::Cancelled,
            BridgeError::TransportFailed,
            BridgeError::MalformedResponse,
            BridgeError::MutationOutcomeUnknown,
            BridgeError::CollectionNotFound,
            BridgeError::InvalidFilter,
        ] {
            let rendered = error.to_string();
            assert_eq!(rendered, error.code());
            assert!(!rendered.contains("127.0.0.1"));
            assert!(!rendered.contains("http"));
        }
        assert_eq!(BridgeError::Cancelled.code(), "QDRANT_OPERATION_CANCELLED");
        assert_eq!(
            BridgeError::TransportFailed.code(),
            "QDRANT_TRANSPORT_FAILED"
        );
        assert_eq!(
            BridgeError::MalformedResponse.code(),
            "QDRANT_MALFORMED_RESPONSE"
        );
    }

    #[test]
    fn cancelled_context_fails_before_dispatch() {
        let flag = Arc::new(AtomicBool::new(true));
        let context = OpContext::with_cancel(Duration::from_secs(5), Arc::clone(&flag));
        assert_eq!(
            context.check().expect_err("cancelled"),
            BridgeError::Cancelled
        );
        flag.store(false, Ordering::SeqCst);
        assert!(context.check().is_ok());
        assert_eq!(OpContext::default().deadline(), Duration::from_secs(10));
    }

    #[test]
    fn indexed_field_constants_match_filter_translation() {
        assert_eq!(
            EligibilityFilter::INDEXED_FIELDS,
            [
                "access_partition_digest",
                "source_membership_id",
                "valid_from_epoch",
                "valid_until_epoch_exclusive"
            ]
        );
    }
}

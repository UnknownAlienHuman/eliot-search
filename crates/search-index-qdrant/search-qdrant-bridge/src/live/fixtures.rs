use std::collections::HashMap;

use qdrant_client::qdrant::{
    Condition, FieldCondition, Filter, Match, PointId, PointStruct, Range, Value,
    Vector, Vectors, WriteOrdering, WriteOrderingType, condition, point_id,
    r#match, value, vectors,
};

use crate::qualified::BaseEligibility;

use super::LiveError;

/// Disposable qualification collection. The server itself is disposable, so a
/// fixed name is collision-free by construction.
pub const QUALIFICATION_COLLECTION: &str = "t22_qual_probe";
/// Qualified sparse vector names (frozen lexical legs).
pub const VECTOR_CODE: &str = "lex_code_v1";
pub const VECTOR_TEXT: &str = "lex_text_neutral_v1";
/// Payload fields.
pub const FIELD_TENANT: &str = "tenant";
pub const FIELD_ACCESS: &str = "access_partition";
pub const FIELD_FROM: &str = "valid_from_epoch";
pub const FIELD_UNTIL: &str = "valid_until_epoch_exclusive";
/// Payload field that is ingested but deliberately never indexed: the strict
/// negative fixture.
pub const FIELD_UNINDEXED: &str = "unit_kind";
/// Tenant populations.
pub const TENANT_A: &str = "tenant-a";
pub const TENANT_B: &str = "tenant-b";
pub const ACCESS_A: &str = "partition-a";
/// Visible epoch shared by the retrieval plan and the IDF corpus plan.
pub const VISIBLE_EPOCH: u64 = 42;
/// Same epoch as `i64` for integer payload/range construction without casts.
pub const VISIBLE_EPOCH_I64: i64 = 42;
/// Signed epoch extremes as integer payload.
///
/// `Range` bounds travel as `double`, so extremes stay within exactly
/// representable `f64` integers (|v| < 2^53). [`exact_f64`] rechecks exactness
/// instead of assuming it.
pub const EPOCH_MIN: i64 = -9_007_199_254_740_000;
pub const EPOCH_MAX: i64 = 9_007_199_254_740_000;
/// UUID point proving UUID transport without the client `uuid` feature.
pub const UUID_POINT: &str = "550e8400-e29b-41d4-a716-446655440000";

pub(super) fn base_eligibility() -> BaseEligibility {
    BaseEligibility {
        access_partition: ACCESS_A.to_owned(),
        tenant: TENANT_A.to_owned(),
        visible_epoch: VISIBLE_EPOCH,
    }
}

pub(super) const fn exact_f64(value: i64) -> Result<f64, LiveError> {
    #[allow(clippy::cast_precision_loss)]
    let as_float = value as f64;
    #[allow(clippy::cast_possible_truncation)]
    if as_float as i64 != value {
        return Err(LiveError::FixtureNotRepresentable);
    }
    Ok(as_float)
}

pub(super) fn keyword_condition(key: &str, value: &str) -> Condition {
    Condition {
        condition_one_of: Some(condition::ConditionOneOf::Field(FieldCondition {
            key: key.to_owned(),
            r#match: Some(Match {
                match_value: Some(r#match::MatchValue::Keyword(value.to_owned())),
            }),
            ..Default::default()
        })),
    }
}

pub(super) fn range_condition(
    key: &str,
    gte: Option<f64>,
    lte: Option<f64>,
) -> Condition {
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

/// Canonical base eligibility plan, shared verbatim by retrieval and the IDF
/// corpus: tenant-a, access partition A, `valid_from <= 42`, with the
/// open-ended upper bound expressed as `must_not(valid_until <= 42)`.
pub(super) fn base_filter() -> Result<Filter, LiveError> {
    Ok(Filter {
        must: vec![
            keyword_condition(FIELD_TENANT, TENANT_A),
            keyword_condition(FIELD_ACCESS, ACCESS_A),
            range_condition(
                FIELD_FROM,
                None,
                Some(exact_f64(VISIBLE_EPOCH_I64)?),
            ),
        ],
        must_not: vec![range_condition(
            FIELD_UNTIL,
            None,
            Some(exact_f64(VISIBLE_EPOCH_I64)?),
        )],
        ..Default::default()
    })
}

pub(super) const fn strong_ordering() -> WriteOrdering {
    WriteOrdering {
        r#type: WriteOrderingType::Strong as i32,
    }
}

pub(super) const fn int_value(value: i64) -> Value {
    Value {
        kind: Some(value::Kind::IntegerValue(value)),
    }
}

pub(super) fn string_value(value: &str) -> Value {
    Value {
        kind: Some(value::Kind::StringValue(value.to_owned())),
    }
}

pub(super) fn sparse_named(
    code: Vec<(u32, f32)>,
    text: Vec<(u32, f32)>,
) -> Vectors {
    let (code_idx, code_val): (Vec<u32>, Vec<f32>) = code.into_iter().unzip();
    let (text_idx, text_val): (Vec<u32>, Vec<f32>) = text.into_iter().unzip();
    let mut map = HashMap::new();
    map.insert(
        VECTOR_CODE.to_owned(),
        Vector::new_sparse(code_idx, code_val),
    );
    map.insert(
        VECTOR_TEXT.to_owned(),
        Vector::new_sparse(text_idx, text_val),
    );
    Vectors {
        vectors_options: Some(vectors::VectorsOptions::Vectors(
            qdrant_client::qdrant::NamedVectors { vectors: map },
        )),
    }
}

pub(super) fn point(
    id: u64,
    tenant: &str,
    from: i64,
    until: Option<i64>,
    code: Vec<(u32, f32)>,
    text: Vec<(u32, f32)>,
) -> PointStruct {
    let mut payload = HashMap::new();
    payload.insert(FIELD_TENANT.to_owned(), string_value(tenant));
    payload.insert(FIELD_ACCESS.to_owned(), string_value(ACCESS_A));
    payload.insert(FIELD_FROM.to_owned(), int_value(from));
    if let Some(until) = until {
        payload.insert(FIELD_UNTIL.to_owned(), int_value(until));
    }
    payload.insert(FIELD_UNINDEXED.to_owned(), string_value("code_unit"));
    PointStruct {
        id: Some(PointId {
            point_id_options: Some(point_id::PointIdOptions::Num(id)),
        }),
        payload,
        vectors: Some(sparse_named(code, text)),
    }
}

pub(super) const fn num_point_id(id: u64) -> PointId {
    PointId {
        point_id_options: Some(point_id::PointIdOptions::Num(id)),
    }
}

pub(super) fn snapshot_id(id: Option<&PointId>) -> String {
    match id.and_then(|point| point.point_id_options.as_ref()) {
        Some(point_id::PointIdOptions::Num(number)) => number.to_string(),
        Some(point_id::PointIdOptions::Uuid(uuid)) => uuid.clone(),
        None => "<missing>".to_owned(),
    }
}

pub(super) fn update_completed(status: i32) -> bool {
    use qdrant_client::qdrant::UpdateStatus;
    UpdateStatus::try_from(status)
        .is_ok_and(|parsed| parsed == UpdateStatus::Completed)
}

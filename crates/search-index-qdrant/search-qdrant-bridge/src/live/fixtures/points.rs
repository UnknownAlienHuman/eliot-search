//! Qualification point, payload, vector and write-order construction.

use std::collections::HashMap;

use qdrant_client::qdrant::{
    PointId, PointStruct, UpdateStatus, Value, Vector, Vectors, WriteOrdering,
    WriteOrderingType, point_id, value, vectors,
};

use super::spec::{
    ACCESS_A, FIELD_ACCESS, FIELD_FROM, FIELD_TENANT, FIELD_UNINDEXED,
    FIELD_UNTIL, VECTOR_CODE, VECTOR_TEXT,
};

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
        id: Some(num_point_id(id)),
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
    UpdateStatus::try_from(status)
        .is_ok_and(|parsed| parsed == UpdateStatus::Completed)
}

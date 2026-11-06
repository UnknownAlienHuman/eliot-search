//! Private SDK message construction and strict routing conversions.

use std::collections::{BTreeSet, HashMap};
use std::sync::atomic::AtomicBool;
use std::time::Instant;

use prost::Message;
use qdrant_client::qdrant::{
    NamedVectors, PointId, PointStruct, UpdateMode, UpsertPoints, Value, point_id, value,
};
use search_contracts::SourceId;

use crate::backend::check;
use crate::model::collection_name;
use crate::{BackendError, MAX_PAGE_POINTS, MAX_POINT_TEXT_BYTES, MAX_REQUEST_BYTES, PointInput};

pub fn encode_batch(
    name: &str,
    points: &[PointInput<'_>],
    deadline: Instant,
    cancelled: &AtomicBool,
) -> Result<UpsertPoints, BackendError> {
    check(deadline, cancelled)?;
    collection_name(name)?;
    if points.is_empty() {
        return Err(BackendError::InvalidInput);
    }
    if points.len() > MAX_PAGE_POINTS {
        return Err(BackendError::RequestTooLarge);
    }
    let mut lower_bound = name.len();
    let mut ids = BTreeSet::new();
    for point in points {
        check(deadline, cancelled)?;
        if point.retrieval_text.is_empty() || !ids.insert(point.id) {
            return Err(BackendError::InvalidInput);
        }
        if point.retrieval_text.len() > MAX_POINT_TEXT_BYTES {
            return Err(BackendError::RequestTooLarge);
        }
        lower_bound = lower_bound
            .checked_add(point.retrieval_text.len())
            .ok_or(BackendError::RequestTooLarge)?;
        if lower_bound > MAX_REQUEST_BYTES {
            return Err(BackendError::RequestTooLarge);
        }
    }
    let mut encoded = Vec::new();
    encoded
        .try_reserve_exact(points.len())
        .map_err(|_| BackendError::AllocationFailed)?;
    for point in points {
        check(deadline, cancelled)?;
        encoded.push(PointStruct::new(
            point_id(point.id),
            NamedVectors {
                vectors: HashMap::default(),
            },
            HashMap::from([
                (
                    "source_id".to_owned(),
                    string_value(point.source_id.to_string()),
                ),
                (
                    "retrieval_text".to_owned(),
                    string_value(point.retrieval_text.to_owned()),
                ),
            ]),
        ));
    }
    let request = UpsertPoints {
        collection_name: name.to_owned(),
        wait: Some(true),
        points: encoded,
        update_mode: Some(UpdateMode::InsertOnly.into()),
        ..Default::default()
    };
    if request.encoded_len() > MAX_REQUEST_BYTES {
        return Err(BackendError::RequestTooLarge);
    }
    check(deadline, cancelled)?;
    Ok(request)
}

const fn string_value(text: String) -> Value {
    Value {
        kind: Some(value::Kind::StringValue(text)),
    }
}

pub fn point_id(bytes: [u8; 16]) -> PointId {
    PointId {
        point_id_options: Some(point_id::PointIdOptions::Uuid(uuid_text(bytes))),
    }
}

fn uuid_text(bytes: [u8; 16]) -> String {
    const HEX: &[u8; 16] = b"0123456789abcdef";
    let mut text = String::with_capacity(36);
    for (index, byte) in bytes.into_iter().enumerate() {
        if matches!(index, 4 | 6 | 8 | 10) {
            text.push('-');
        }
        text.push(char::from(HEX[usize::from(byte >> 4)]));
        text.push(char::from(HEX[usize::from(byte & 15)]));
    }
    text
}

pub fn decode_id(id: Option<&PointId>) -> Result<[u8; 16], BackendError> {
    let Some(point_id::PointIdOptions::Uuid(text)) = id.and_then(|id| id.point_id_options.as_ref())
    else {
        return Err(BackendError::MalformedResponse);
    };
    let bytes = text.as_bytes();
    if bytes.len() != 36 {
        return Err(BackendError::MalformedResponse);
    }
    let mut decoded = [0; 16];
    let mut cursor = 0;
    for (index, byte) in decoded.iter_mut().enumerate() {
        if matches!(index, 4 | 6 | 8 | 10) {
            if bytes.get(cursor) != Some(&b'-') {
                return Err(BackendError::MalformedResponse);
            }
            cursor += 1;
        }
        let high = hex(*bytes.get(cursor).ok_or(BackendError::MalformedResponse)?)?;
        let low = hex(*bytes
            .get(cursor + 1)
            .ok_or(BackendError::MalformedResponse)?)?;
        *byte = high * 16 + low;
        cursor += 2;
    }
    Ok(decoded)
}

const fn hex(byte: u8) -> Result<u8, BackendError> {
    match byte {
        b'0'..=b'9' => Ok(byte - b'0'),
        b'a'..=b'f' => Ok(byte - b'a' + 10),
        _ => Err(BackendError::MalformedResponse),
    }
}

pub fn payload_string<'a>(
    payload: &'a HashMap<String, Value>,
    key: &str,
) -> Result<&'a str, BackendError> {
    match payload.get(key).and_then(|value| value.kind.as_ref()) {
        Some(value::Kind::StringValue(text)) => Ok(text),
        _ => Err(BackendError::MalformedResponse),
    }
}

pub fn source_id(payload: &HashMap<String, Value>) -> Result<SourceId, BackendError> {
    let text = payload_string(payload, "source_id")?;
    if text.len() != 36 {
        return Err(BackendError::MalformedResponse);
    }
    text.parse().map_err(|_| BackendError::MalformedResponse)
}

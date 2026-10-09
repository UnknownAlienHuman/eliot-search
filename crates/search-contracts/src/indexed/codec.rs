//! Canonical-value assembly and closed scalar decoding, never a byte encoder.

use crate::{
    BoundedList, BoundedMap, CanonicalKey, CanonicalText, CanonicalValue, ClosedCanonicalObject,
    ContractError, ContractErrorKind, Epoch,
};

pub(super) const fn error(kind: ContractErrorKind, field: &'static str) -> ContractError {
    ContractError::new(kind, field)
}

pub(super) fn object(
    fields: impl IntoIterator<Item = (&'static str, CanonicalValue)>,
) -> Result<CanonicalValue, ContractError> {
    let mut bounded = Vec::new();
    for (name, value) in fields {
        if bounded.len() == crate::MAX_MAP_ENTRIES {
            return Err(error(ContractErrorKind::TooManyItems, "indexed_object"));
        }
        bounded.push((CanonicalKey::new_non_empty(name)?, value));
    }
    Ok(CanonicalValue::Object(BoundedMap::from_entries(bounded)?))
}

pub(super) fn text(value: &str) -> Result<CanonicalValue, ContractError> {
    Ok(CanonicalValue::Text(CanonicalText::new(value)?))
}

pub(super) fn array(values: Vec<CanonicalValue>) -> Result<CanonicalValue, ContractError> {
    Ok(CanonicalValue::Array(BoundedList::new(values)?))
}

pub(super) fn decode_text(
    value: CanonicalValue,
    field: &'static str,
) -> Result<String, ContractError> {
    match value {
        CanonicalValue::Text(value) => Ok(value.into_string()),
        _ => Err(error(ContractErrorKind::MalformedPayload, field)),
    }
}

// Scalar extraction consumes the removed field, just like text/array extraction.
#[allow(clippy::needless_pass_by_value)]
pub(super) fn decode_u64(value: CanonicalValue, field: &'static str) -> Result<u64, ContractError> {
    match value {
        CanonicalValue::U64(value) => Ok(value),
        _ => Err(error(ContractErrorKind::MalformedPayload, field)),
    }
}

#[allow(clippy::needless_pass_by_value)]
pub(super) fn decode_bool(
    value: CanonicalValue,
    field: &'static str,
) -> Result<bool, ContractError> {
    match value {
        CanonicalValue::Bool(value) => Ok(value),
        _ => Err(error(ContractErrorKind::MalformedPayload, field)),
    }
}

pub(super) fn decode_array(
    value: CanonicalValue,
    field: &'static str,
) -> Result<Vec<CanonicalValue>, ContractError> {
    match value {
        CanonicalValue::Array(value) => Ok(value.into_vec()),
        _ => Err(error(ContractErrorKind::MalformedPayload, field)),
    }
}

pub(super) fn take_text(
    value: &mut ClosedCanonicalObject,
    field: &'static str,
) -> Result<String, ContractError> {
    decode_text(value.take_required(field)?, field)
}

pub(super) fn take_u64(
    value: &mut ClosedCanonicalObject,
    field: &'static str,
) -> Result<u64, ContractError> {
    decode_u64(value.take_required(field)?, field)
}

pub(super) fn take_bool(
    value: &mut ClosedCanonicalObject,
    field: &'static str,
) -> Result<bool, ContractError> {
    decode_bool(value.take_required(field)?, field)
}

pub(super) fn epoch(value: CanonicalValue, field: &'static str) -> Result<Epoch, ContractError> {
    let value = i64::try_from(decode_u64(value, field)?)
        .map_err(|_| error(ContractErrorKind::EpochOutOfRange, field))?;
    Epoch::new(value)
}

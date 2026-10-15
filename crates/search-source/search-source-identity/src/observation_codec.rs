//! Bounded shared-value construction for the native directory identity owner.
use crate::IdentityError;
use search_contracts::{self as c, CanonicalValue as V};

/// Construct one bounded value tuple through the sole shared canonical vocabulary.
pub fn tuple(values: Vec<V>) -> Result<V, IdentityError> {
    c::BoundedList::new(values)
        .map(V::Array)
        .map_err(|_| IdentityError::IdentityObservationInvalid)
}

/// Construct bounded canonical bytes; no digest or native evidence is inferred.
pub fn binary(value: &[u8]) -> Result<V, IdentityError> {
    c::BoundedBytes::new(value.to_vec())
        .map(V::Bytes)
        .map_err(|_| IdentityError::IdentityObservationInvalid)
}

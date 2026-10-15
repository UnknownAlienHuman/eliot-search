//! Canonical CBOR field mapping for retained provenance structures.
//!
//! Only the shared `CanonicalValue` model and the shared deterministic CBOR encoder
//! and parser are used. No local serializer, digest recipe or fingerprint
//! preimage is reimplemented here.

use search_contracts::canonical::CanonicalValue;
use search_contracts::{BoundedList, BoundedMap, CanonicalKey, CanonicalText, MAX_MAP_ENTRIES};

/// Maximum canonical fields one provenance record may carry.
pub const MAX_RECORD_FIELDS: usize = MAX_MAP_ENTRIES;

/// One canonical record: a bounded text-keyed canonical object.
#[derive(Clone, Debug, Eq, PartialEq, Default)]
pub struct CanonicalRecord {
    fields: BoundedMap<CanonicalKey, CanonicalValue, MAX_RECORD_FIELDS>,
}

impl CanonicalRecord {
    /// Creates one empty bounded record.
    pub(crate) fn new() -> Self {
        Self::default()
    }

    /// Inserts one canonical field, rejecting a duplicate key.
    pub(crate) fn insert(&mut self, key: &str, value: CanonicalValue) -> Result<(), CodecError> {
        let key = CanonicalKey::new(key).map_err(|_| CodecError::NonCanonical)?;
        self.fields
            .insert(key, value)
            .map_err(|_| CodecError::NonCanonical)?;
        Ok(())
    }

    /// Reads one canonical field by exact key.
    pub(crate) fn get(&self, key: &str) -> Option<&CanonicalValue> {
        self.fields.get(&CanonicalKey::new(key).ok()?)
    }

    /// Deterministic canonical CBOR bytes for this exact record.
    pub(crate) fn encode(&self) -> Result<Vec<u8>, CodecError> {
        let value = CanonicalValue::Object(self.fields.clone());
        value.validate().map_err(|_| CodecError::NonCanonical)?;
        search_contracts::canonical::to_canonical_cbor(&value)
            .map(|encoded| encoded.as_slice().to_vec())
            .map_err(|_| CodecError::NonCanonical)
    }

    /// Parses exact canonical CBOR bytes back into a bounded record.
    pub(crate) fn decode(bytes: &[u8]) -> Result<Self, CodecError> {
        let value = search_contracts::canonical::parse_canonical_cbor(bytes)
            .map_err(|_| CodecError::NonCanonical)?;
        let CanonicalValue::Object(fields) = value else {
            return Err(CodecError::NonCanonical);
        };
        Ok(Self { fields })
    }

    /// Number of retained canonical fields.
    pub(crate) fn len(&self) -> usize {
        self.fields.len()
    }
}

impl From<search_contracts::ContractError> for CodecError {
    fn from(_: search_contracts::ContractError) -> Self {
        Self::NonCanonical
    }
}
/// Content-free canonical mapping failure.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum CodecError {
    /// Input was not the exact canonical CBOR representation.
    NonCanonical,
}

/// One canonical unsigned integer field.
pub const fn unsigned(value: u64) -> CanonicalValue {
    CanonicalValue::U64(value)
}

/// One canonical text field.
pub fn text(value: &str) -> Result<CanonicalValue, CodecError> {
    CanonicalText::new(value)
        .map(CanonicalValue::Text)
        .map_err(|_| CodecError::NonCanonical)
}

/// One canonical boolean field.
pub const fn boolean(value: bool) -> CanonicalValue {
    CanonicalValue::Bool(value)
}

/// One canonical array of canonical values.
pub fn array<I>(values: I) -> Result<CanonicalValue, CodecError>
where
    I: IntoIterator<Item = CanonicalValue>,
{
    BoundedList::new(values.into_iter().collect())
        .map(CanonicalValue::Array)
        .map_err(|_| CodecError::NonCanonical)
}

impl CanonicalRecord {
    /// Consumes this record into its canonical object value.
    pub(crate) fn into_inner(self) -> BoundedMap<CanonicalKey, CanonicalValue, MAX_RECORD_FIELDS> {
        self.fields
    }
}

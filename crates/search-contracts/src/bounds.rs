use crate::{Blake3Digest32, ContractError, ContractErrorKind, NonZeroRevision, ProfileId};
use core::ops::Deref;
use std::collections::{BTreeMap, BTreeSet};

pub const MAX_ANCHOR_DEPTH: usize = 16;
pub const MAX_BEHAVIOR_SIGNATURE_BYTES: usize = 16_384;
pub const MAX_BOUND_CLASSES: usize = 256;
pub const MAX_CANONICAL_BYTES: usize = 8 * 1024 * 1024;
pub const MAX_COLLECTION_ITEMS: usize = 4_096;
pub const MAX_DISPLAY_NAME_BYTES: usize = 512;
pub const MAX_DISPLAY_PATH_BYTES: usize = 32_768;
pub const MAX_EXPRESSION_BYTES: usize = 16_384;
pub const MAX_FACET_VALUE_BYTES: usize = 4_096;
pub const MAX_FRAME_BYTES: usize = 8 * 1024 * 1024;
pub const MAX_JSON_DEPTH: usize = 64;
pub const MAX_MAP_ENTRIES: usize = 1_024;
pub const MAX_METADATA_ENTRIES: usize = 64;
pub const MAX_METADATA_KEY_BYTES: usize = 128;
pub const MAX_NAME_BYTES: usize = 1_024;
pub const MAX_OBSERVATION_BYTES: usize = 65_536;
pub const MAX_OPAQUE_ID_BYTES: usize = 256;
pub const MAX_OPAQUE_REF_BYTES: usize = 4_096;
pub const MAX_PROFILE_ID_BYTES: usize = 256;
pub const MAX_PROTOCOL_IN_FLIGHT: usize = 32;
pub const MAX_RAW_BYTES: usize = 8 * 1024 * 1024;
pub const MAX_REASON_CODES: usize = 64;
pub const MAX_SET_ITEMS: usize = 4_096;
pub const MAX_SYMBOL_KEY_BYTES: usize = 4_096;
pub const MIN_HANDLE_TOKEN_BYTES: usize = 32;
pub const MAX_HANDLE_TOKEN_BYTES: usize = 64;

// Semantic aliases map only to registered P00 classes; they do not create
// hidden limits outside the digest-bound table.
pub const MAX_CANONICAL_DEPTH: usize = MAX_JSON_DEPTH;
pub const MAX_CANONICAL_KEY_BYTES: usize = MAX_METADATA_KEY_BYTES;
pub const MAX_LIST_ITEMS: usize = MAX_COLLECTION_ITEMS;
pub const MAX_PROTOCOL_MESSAGE_BYTES: usize = MAX_FRAME_BYTES;
pub const MAX_TEXT_BYTES: usize = MAX_RAW_BYTES;

/// One row in the immutable W0 limit table.
#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub struct LimitClass {
    pub max_items: Option<u32>,
    pub max_bytes: Option<u64>,
    pub max_depth: Option<u16>,
}

impl LimitClass {
    #[must_use]
    pub const fn items(max_items: u32) -> Self {
        Self {
            max_items: Some(max_items),
            max_bytes: None,
            max_depth: None,
        }
    }

    #[must_use]
    pub const fn bytes(max_bytes: u64) -> Self {
        Self {
            max_items: None,
            max_bytes: Some(max_bytes),
            max_depth: None,
        }
    }

    #[must_use]
    pub const fn depth(max_depth: u16) -> Self {
        Self {
            max_items: None,
            max_bytes: None,
            max_depth: Some(max_depth),
        }
    }

    /// Zero disables a dimension. An entirely absent row is malformed.
    pub fn validate(self) -> Result<(), ContractError> {
        if self.max_items.is_none() && self.max_bytes.is_none() && self.max_depth.is_none() {
            return Err(ContractError::new(
                ContractErrorKind::InvalidRange,
                "limit_class",
            ));
        }
        Ok(())
    }
}

/// Frozen, digest-bound P00 limit table.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ContractBoundsV1 {
    pub bounds_revision: NonZeroRevision,
    pub classes: BoundedMap<ProfileId, LimitClass, MAX_BOUND_CLASSES>,
    pub table_digest: Blake3Digest32,
}

impl ContractBoundsV1 {
    /// Construct the exact W0 table declared by the accepted P00 pack.
    pub fn p00() -> Result<Self, ContractError> {
        let rows = vec![
            class_depth("anchor_depth", MAX_ANCHOR_DEPTH)?,
            class_bytes("behavior_signature", MAX_BEHAVIOR_SIGNATURE_BYTES)?,
            class_items("bound_classes", MAX_BOUND_CLASSES)?,
            class_bytes("canonical", MAX_CANONICAL_BYTES)?,
            class_items("collection", MAX_COLLECTION_ITEMS)?,
            class_bytes("display_name", MAX_DISPLAY_NAME_BYTES)?,
            class_bytes("display_path", MAX_DISPLAY_PATH_BYTES)?,
            class_bytes("expression", MAX_EXPRESSION_BYTES)?,
            class_bytes("facet_value", MAX_FACET_VALUE_BYTES)?,
            class_bytes("frame", MAX_FRAME_BYTES)?,
            class_depth("json_depth", MAX_JSON_DEPTH)?,
            class_items("map", MAX_MAP_ENTRIES)?,
            class_items("metadata_entries", MAX_METADATA_ENTRIES)?,
            class_bytes("metadata_key", MAX_METADATA_KEY_BYTES)?,
            class_bytes("name", MAX_NAME_BYTES)?,
            class_bytes("observation", MAX_OBSERVATION_BYTES)?,
            class_bytes("opaque_id", MAX_OPAQUE_ID_BYTES)?,
            class_bytes("opaque_ref", MAX_OPAQUE_REF_BYTES)?,
            class_bytes("profile_id", MAX_PROFILE_ID_BYTES)?,
            class_items("protocol_in_flight", MAX_PROTOCOL_IN_FLIGHT)?,
            class_bytes("raw", MAX_RAW_BYTES)?,
            class_items("reason_codes", MAX_REASON_CODES)?,
            class_items("set", MAX_SET_ITEMS)?,
            class_bytes("symbol_key", MAX_SYMBOL_KEY_BYTES)?,
        ];
        let table_digest = Blake3Digest32::from_bytes([
            0x8a, 0xb6, 0x11, 0x00, 0x6a, 0x1f, 0x8c, 0xdd, 0x5d, 0xec, 0x9a, 0x71, 0x43, 0x3f,
            0xbb, 0x61, 0xbd, 0x5e, 0x24, 0xcc, 0x2d, 0x12, 0x56, 0x9a, 0x4f, 0xdd, 0xf7, 0x8d,
            0x85, 0x9f, 0x2f, 0x82,
        ]);
        Ok(Self {
            bounds_revision: NonZeroRevision::new(1)?,
            classes: BoundedMap::from_entries(rows)?,
            table_digest,
        })
    }

    pub fn class(&self, id: &ProfileId) -> Result<LimitClass, ContractError> {
        self.classes.get(id).copied().ok_or_else(|| {
            ContractError::new(ContractErrorKind::InvalidRange, "contract_bound_class")
        })
    }
}

fn class_items(name: &'static str, value: usize) -> Result<(ProfileId, LimitClass), ContractError> {
    Ok((
        ProfileId::new(name)?,
        LimitClass::items(u32::try_from(value).map_err(|_| {
            ContractError::new(ContractErrorKind::InvalidRange, "bound_class_items")
        })?),
    ))
}

fn class_bytes(name: &'static str, value: usize) -> Result<(ProfileId, LimitClass), ContractError> {
    Ok((
        ProfileId::new(name)?,
        LimitClass::bytes(u64::try_from(value).map_err(|_| {
            ContractError::new(ContractErrorKind::InvalidRange, "bound_class_bytes")
        })?),
    ))
}

fn class_depth(name: &'static str, value: usize) -> Result<(ProfileId, LimitClass), ContractError> {
    Ok((
        ProfileId::new(name)?,
        LimitClass::depth(u16::try_from(value).map_err(|_| {
            ContractError::new(ContractErrorKind::InvalidRange, "bound_class_depth")
        })?),
    ))
}

/// Validated UTF-8 with a byte cap known at compile time.
#[derive(Clone, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub struct BoundedText<const LIMIT: usize>(String);

impl<const LIMIT: usize> BoundedText<LIMIT> {
    pub fn new(value: impl Into<String>) -> Result<Self, ContractError> {
        let value = value.into();
        check_len("bounded_text", value.len(), LIMIT)?;
        Ok(Self(value))
    }

    pub fn new_non_empty(value: impl Into<String>) -> Result<Self, ContractError> {
        let value = Self::new(value)?;
        if value.is_empty() {
            return Err(ContractError::new(ContractErrorKind::Empty, "bounded_text"));
        }
        Ok(value)
    }

    #[must_use]
    pub fn as_str(&self) -> &str {
        &self.0
    }

    #[must_use]
    pub fn as_bytes(&self) -> &[u8] {
        self.0.as_bytes()
    }

    #[must_use]
    pub fn len(&self) -> usize {
        self.0.len()
    }

    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.0.is_empty()
    }

    #[must_use]
    pub fn into_string(self) -> String {
        self.0
    }
}

impl<const LIMIT: usize> AsRef<str> for BoundedText<LIMIT> {
    fn as_ref(&self) -> &str {
        self.as_str()
    }
}

impl<const LIMIT: usize> Deref for BoundedText<LIMIT> {
    type Target = str;

    fn deref(&self) -> &Self::Target {
        self.as_str()
    }
}

impl<const LIMIT: usize> core::fmt::Display for BoundedText<LIMIT> {
    fn fmt(&self, formatter: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        formatter.write_str(self.as_str())
    }
}

impl<const LIMIT: usize> TryFrom<String> for BoundedText<LIMIT> {
    type Error = ContractError;

    fn try_from(value: String) -> Result<Self, Self::Error> {
        Self::new(value)
    }
}

/// Bytes whose length is capped at compile time.
#[derive(Clone, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub struct BoundedBytes<const LIMIT: usize>(Vec<u8>);

impl<const LIMIT: usize> BoundedBytes<LIMIT> {
    pub fn new(value: impl Into<Vec<u8>>) -> Result<Self, ContractError> {
        let value = value.into();
        check_len("bounded_bytes", value.len(), LIMIT)?;
        Ok(Self(value))
    }

    #[must_use]
    pub fn as_slice(&self) -> &[u8] {
        &self.0
    }

    #[must_use]
    pub fn len(&self) -> usize {
        self.0.len()
    }

    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.0.is_empty()
    }

    #[must_use]
    pub fn into_vec(self) -> Vec<u8> {
        self.0
    }
}

impl<const LIMIT: usize> AsRef<[u8]> for BoundedBytes<LIMIT> {
    fn as_ref(&self) -> &[u8] {
        self.as_slice()
    }
}

/// Producer-validated canonical codec bytes with an independent cap.
#[derive(Clone, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub struct BoundedCanonicalBytes<const LIMIT: usize>(BoundedBytes<LIMIT>);

impl<const LIMIT: usize> BoundedCanonicalBytes<LIMIT> {
    pub fn from_validated(value: impl Into<Vec<u8>>) -> Result<Self, ContractError> {
        BoundedBytes::new(value).map(Self)
    }

    #[must_use]
    pub fn as_slice(&self) -> &[u8] {
        self.0.as_slice()
    }

    #[must_use]
    pub fn len(&self) -> usize {
        self.0.len()
    }

    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.0.is_empty()
    }

    #[must_use]
    pub fn into_vec(self) -> Vec<u8> {
        self.0.into_vec()
    }
}

/// Opaque bytes that shared consumers must not parse.
#[derive(Clone, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub struct BoundedOpaqueBytes<const LIMIT: usize>(BoundedBytes<LIMIT>);

impl<const LIMIT: usize> BoundedOpaqueBytes<LIMIT> {
    pub fn new(value: impl Into<Vec<u8>>) -> Result<Self, ContractError> {
        let value = BoundedBytes::new(value)?;
        if value.is_empty() {
            return Err(ContractError::new(
                ContractErrorKind::Empty,
                "bounded_opaque_bytes",
            ));
        }
        Ok(Self(value))
    }

    #[must_use]
    pub fn as_slice(&self) -> &[u8] {
        self.0.as_slice()
    }

    #[must_use]
    pub fn len(&self) -> usize {
        self.0.len()
    }

    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.0.is_empty()
    }
}

/// A bounded sequence that never exposes mutable access to its backing `Vec`.
#[derive(Clone, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub struct BoundedList<T, const LIMIT: usize>(Vec<T>);

impl<T, const LIMIT: usize> BoundedList<T, LIMIT> {
    pub fn new(value: Vec<T>) -> Result<Self, ContractError> {
        check_items("bounded_list", value.len(), LIMIT)?;
        Ok(Self(value))
    }

    #[must_use]
    pub const fn empty() -> Self {
        Self(Vec::new())
    }

    #[must_use]
    pub fn as_slice(&self) -> &[T] {
        &self.0
    }

    pub fn iter(&self) -> core::slice::Iter<'_, T> {
        self.0.iter()
    }

    #[must_use]
    pub fn len(&self) -> usize {
        self.0.len()
    }

    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.0.is_empty()
    }

    #[must_use]
    pub fn into_vec(self) -> Vec<T> {
        self.0
    }

    pub fn try_push(&mut self, value: T) -> Result<(), ContractError> {
        check_items("bounded_list", self.0.len().saturating_add(1), LIMIT)?;
        self.0.push(value);
        Ok(())
    }
}

impl<T, const LIMIT: usize> Default for BoundedList<T, LIMIT> {
    fn default() -> Self {
        Self::empty()
    }
}

impl<'a, T, const LIMIT: usize> IntoIterator for &'a BoundedList<T, LIMIT> {
    type Item = &'a T;
    type IntoIter = core::slice::Iter<'a, T>;

    fn into_iter(self) -> Self::IntoIter {
        self.0.iter()
    }
}

/// Canonically ordered bounded set. Duplicate input is rejected rather than
/// silently normalized, preserving strict contract validation.
#[derive(Clone, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub struct BoundedSet<T, const LIMIT: usize>(BTreeSet<T>);

impl<T: Ord, const LIMIT: usize> BoundedSet<T, LIMIT> {
    pub fn from_items(value: impl IntoIterator<Item = T>) -> Result<Self, ContractError> {
        let mut output = BTreeSet::new();
        for item in value {
            check_items("bounded_set", output.len().saturating_add(1), LIMIT)?;
            if !output.insert(item) {
                return Err(ContractError::new(
                    ContractErrorKind::Duplicate,
                    "bounded_set",
                ));
            }
        }
        Ok(Self(output))
    }

    #[must_use]
    pub const fn empty() -> Self {
        Self(BTreeSet::new())
    }

    pub fn insert(&mut self, value: T) -> Result<(), ContractError> {
        if self.0.contains(&value) {
            return Err(ContractError::new(
                ContractErrorKind::Duplicate,
                "bounded_set",
            ));
        }
        check_items("bounded_set", self.0.len().saturating_add(1), LIMIT)?;
        self.0.insert(value);
        Ok(())
    }

    #[must_use]
    pub fn contains(&self, value: &T) -> bool {
        self.0.contains(value)
    }

    pub fn iter(&self) -> std::collections::btree_set::Iter<'_, T> {
        self.0.iter()
    }

    #[must_use]
    pub fn len(&self) -> usize {
        self.0.len()
    }

    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.0.is_empty()
    }
}

impl<T: Ord, const LIMIT: usize> Default for BoundedSet<T, LIMIT> {
    fn default() -> Self {
        Self::empty()
    }
}

impl<'a, T: Ord, const LIMIT: usize> IntoIterator for &'a BoundedSet<T, LIMIT> {
    type Item = &'a T;
    type IntoIter = std::collections::btree_set::Iter<'a, T>;

    fn into_iter(self) -> Self::IntoIter {
        self.0.iter()
    }
}

/// Canonically ordered bounded map with strict duplicate-key rejection.
#[derive(Clone, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub struct BoundedMap<K, V, const LIMIT: usize>(BTreeMap<K, V>);

impl<K: Ord, V, const LIMIT: usize> BoundedMap<K, V, LIMIT> {
    pub fn from_entries(value: impl IntoIterator<Item = (K, V)>) -> Result<Self, ContractError> {
        let mut output = BTreeMap::new();
        for (key, item) in value {
            check_items("bounded_map", output.len().saturating_add(1), LIMIT)?;
            if output.insert(key, item).is_some() {
                return Err(ContractError::new(
                    ContractErrorKind::Duplicate,
                    "bounded_map",
                ));
            }
        }
        Ok(Self(output))
    }

    #[must_use]
    pub const fn empty() -> Self {
        Self(BTreeMap::new())
    }

    #[must_use]
    pub fn get(&self, key: &K) -> Option<&V> {
        self.0.get(key)
    }

    /// Remove one exact key while preserving the original cardinality bound.
    pub fn remove(&mut self, key: &K) -> Option<V> {
        self.0.remove(key)
    }

    pub fn iter(&self) -> std::collections::btree_map::Iter<'_, K, V> {
        self.0.iter()
    }

    #[must_use]
    pub fn len(&self) -> usize {
        self.0.len()
    }

    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.0.is_empty()
    }

    pub fn insert(&mut self, key: K, value: V) -> Result<(), ContractError> {
        if self.0.contains_key(&key) {
            return Err(ContractError::new(
                ContractErrorKind::Duplicate,
                "bounded_map",
            ));
        }
        check_items("bounded_map", self.0.len().saturating_add(1), LIMIT)?;
        self.0.insert(key, value);
        Ok(())
    }
}

impl<K: Ord, V, const LIMIT: usize> Default for BoundedMap<K, V, LIMIT> {
    fn default() -> Self {
        Self::empty()
    }
}

impl<'a, K: Ord, V, const LIMIT: usize> IntoIterator for &'a BoundedMap<K, V, LIMIT> {
    type Item = (&'a K, &'a V);
    type IntoIter = std::collections::btree_map::Iter<'a, K, V>;

    fn into_iter(self) -> Self::IntoIter {
        self.0.iter()
    }
}

/// Text or raw bytes with explicit encoding and independently enforced bounds.
#[derive(Clone, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub enum BoundedTextOrBytes<
    const TEXT_LIMIT: usize = MAX_RAW_BYTES,
    const BYTES_LIMIT: usize = MAX_RAW_BYTES,
> {
    Text(BoundedText<TEXT_LIMIT>),
    Bytes(BoundedBytes<BYTES_LIMIT>),
}

fn check_len(field: &'static str, actual: usize, limit: usize) -> Result<(), ContractError> {
    if actual > limit {
        return Err(ContractError::bounded(
            ContractErrorKind::TooLong,
            field,
            usize_to_u64(limit),
            usize_to_u64(actual),
        ));
    }
    Ok(())
}

fn check_items(field: &'static str, actual: usize, limit: usize) -> Result<(), ContractError> {
    if actual > limit {
        return Err(ContractError::bounded(
            ContractErrorKind::TooManyItems,
            field,
            usize_to_u64(limit),
            usize_to_u64(actual),
        ));
    }
    Ok(())
}

fn usize_to_u64(value: usize) -> u64 {
    u64::try_from(value).unwrap_or(u64::MAX)
}

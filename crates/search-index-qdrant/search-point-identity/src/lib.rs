//! Deterministic provider-neutral point identities for W3 publication.
//!
//! A point identity is derived only from immutable logical inputs: namespace,
//! stable source identity, retained source revision, exact unit range, projection
//! kind, projection configuration fingerprint, and schema revision. It does not
//! depend on Qdrant collection names, insertion order, process identity, wall
//! time, or current routing. The compact identifier uses a frozen two-lane
//! digest profile and every use is guarded by the complete canonical key so a
//! digest collision is detected rather than silently aliasing another point.

#![forbid(unsafe_code)]
#![deny(missing_docs)]
#![allow(
    clippy::doc_markdown,
    clippy::large_enum_variant,
    clippy::manual_let_else,
    clippy::match_same_arms,
    clippy::missing_const_for_fn,
    clippy::missing_errors_doc,
    clippy::module_name_repetitions,
    clippy::must_use_candidate,
    clippy::needless_pass_by_value,
    clippy::option_if_let_else,
    clippy::similar_names,
    clippy::struct_excessive_bools,
    clippy::too_many_arguments,
    clippy::too_many_lines,
    clippy::trivially_copy_pass_by_ref
)]

use core::fmt;
use std::collections::BTreeMap;

use search_contracts::{Blake3Digest32, NonZeroRevision, OpaqueId};

/// Frozen point-identity profile revision.
pub const POINT_IDENTITY_PROFILE_REVISION: u16 = 1;

const DOMAIN_TAG: &[u8] = b"eliot-search.point-id.v1\0";
const FNV_OFFSET_A: u64 = 0xcbf2_9ce4_8422_2325;
const FNV_OFFSET_B: u64 = 0x8422_2325_cbf2_9ce4;
const FNV_PRIME: u64 = 0x0000_0100_0000_01b3;
const MIX_A: u64 = 0x9e37_79b9_7f4a_7c15;
const MIX_B: u64 = 0xc2b2_ae3d_27d4_eb4f;

/// Conservative finite point-identity limits.
pub const DEFAULT_POINT_IDENTITY_LIMITS: PointIdentityLimits = PointIdentityLimits {
    max_identifier_bytes: 4_096,
    max_canonical_bytes: 32_768,
    max_registered_points: 16_000_000,
};

/// Closed content-free point-identity failure.
#[derive(Clone, Copy, Debug, Eq, Ord, PartialEq, PartialOrd)]
pub enum PointIdentityError {
    /// Limits are zero or internally inconsistent.
    InvalidLimits,
    /// Namespace or source identifier exceeds the finite boundary.
    IdentifierTooLong,
    /// Unit byte range is empty or inverted.
    InvalidUnitRange,
    /// Canonical preimage exceeds its finite byte ceiling.
    CanonicalBytesExceeded,
    /// Canonical length or offset conversion overflowed.
    LengthOverflow,
    /// The same compact point identifier maps to another complete key.
    DigestCollision,
    /// The same complete key was registered with a different compact identifier.
    IdentityMismatch,
    /// Finite collision registry is full.
    RegistryCapacityExceeded,
    /// Requested point is absent from the collision registry.
    PointNotFound,
}

impl PointIdentityError {
    /// Stable machine-readable reason code.
    pub const fn code(self) -> &'static str {
        match self {
            Self::InvalidLimits => "POINT_ID_INVALID_LIMITS",
            Self::IdentifierTooLong => "POINT_ID_IDENTIFIER_TOO_LONG",
            Self::InvalidUnitRange => "POINT_ID_INVALID_UNIT_RANGE",
            Self::CanonicalBytesExceeded => "POINT_ID_CANONICAL_BYTES_EXCEEDED",
            Self::LengthOverflow => "POINT_ID_LENGTH_OVERFLOW",
            Self::DigestCollision => "POINT_ID_DIGEST_COLLISION",
            Self::IdentityMismatch => "POINT_ID_IDENTITY_MISMATCH",
            Self::RegistryCapacityExceeded => "POINT_ID_REGISTRY_CAPACITY_EXCEEDED",
            Self::PointNotFound => "POINT_ID_NOT_FOUND",
        }
    }
}

impl fmt::Display for PointIdentityError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(self.code())
    }
}

impl std::error::Error for PointIdentityError {}

/// Finite point-identity limits.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct PointIdentityLimits {
    /// Maximum UTF-8 bytes in one opaque identifier.
    pub max_identifier_bytes: usize,
    /// Maximum bytes in the complete canonical identity preimage.
    pub max_canonical_bytes: usize,
    /// Maximum collision-checked identities retained by one registry instance.
    pub max_registered_points: usize,
}

impl PointIdentityLimits {
    /// Validates every finite dimension as non-zero.
    pub const fn validate(self) -> Result<Self, PointIdentityError> {
        if self.max_identifier_bytes == 0
            || self.max_canonical_bytes == 0
            || self.max_registered_points == 0
        {
            Err(PointIdentityError::InvalidLimits)
        } else {
            Ok(self)
        }
    }
}

/// Closed logical projection family.
#[derive(Clone, Copy, Debug, Eq, Ord, PartialEq, PartialOrd)]
pub enum ProjectionKind {
    /// Lexical searchable unit projection.
    Lexical,
    /// Exact-source metadata projection.
    ExactMetadata,
    /// Optional code-structure projection.
    CodeStructure,
    /// Optional model-produced projection.
    ModelDerived,
}

impl ProjectionKind {
    const fn tag(self) -> u8 {
        match self {
            Self::Lexical => 1,
            Self::ExactMetadata => 2,
            Self::CodeStructure => 3,
            Self::ModelDerived => 4,
        }
    }
}

/// Complete immutable logical key for one index point.
#[derive(Clone, Debug, Eq, Ord, PartialEq, PartialOrd)]
pub struct PointIdentityKey {
    /// Stable namespace identity.
    pub namespace_id: OpaqueId,
    /// Stable source identity within the namespace.
    pub source_id: OpaqueId,
    /// Retained source revision.
    pub source_revision: NonZeroRevision,
    /// Deterministic unit ordinal within the retained revision.
    pub unit_ordinal: u64,
    /// Inclusive exact source byte start.
    pub source_byte_start: u64,
    /// Exclusive exact source byte end.
    pub source_byte_end: u64,
    /// Logical projection family.
    pub projection_kind: ProjectionKind,
    /// Fingerprint of the complete projection/analyzer configuration.
    pub projection_fingerprint: Blake3Digest32,
    /// Monotone projection schema revision.
    pub projection_schema_revision: NonZeroRevision,
}

impl PointIdentityKey {
    /// Validates immutable key boundaries.
    pub fn validate(&self, limits: PointIdentityLimits) -> Result<(), PointIdentityError> {
        let limits = limits.validate()?;
        if self.namespace_id.as_str().len() > limits.max_identifier_bytes
            || self.source_id.as_str().len() > limits.max_identifier_bytes
        {
            return Err(PointIdentityError::IdentifierTooLong);
        }
        if self.source_byte_start >= self.source_byte_end {
            return Err(PointIdentityError::InvalidUnitRange);
        }
        Ok(())
    }

    /// Exact source byte length represented by this point.
    pub const fn source_byte_len(&self) -> u64 {
        self.source_byte_end - self.source_byte_start
    }

    /// Encodes the complete key using the frozen length-prefixed profile.
    pub fn canonical_bytes(
        &self,
        limits: PointIdentityLimits,
    ) -> Result<Vec<u8>, PointIdentityError> {
        self.validate(limits)?;
        let limits = limits.validate()?;
        let mut bytes = Vec::with_capacity(256);
        append_bytes(&mut bytes, DOMAIN_TAG, limits)?;
        append_u16(&mut bytes, POINT_IDENTITY_PROFILE_REVISION, limits)?;
        append_text(&mut bytes, self.namespace_id.as_str(), limits)?;
        append_text(&mut bytes, self.source_id.as_str(), limits)?;
        append_u64(&mut bytes, self.source_revision.get(), limits)?;
        append_u64(&mut bytes, self.unit_ordinal, limits)?;
        append_u64(&mut bytes, self.source_byte_start, limits)?;
        append_u64(&mut bytes, self.source_byte_end, limits)?;
        append_u8(&mut bytes, self.projection_kind.tag(), limits)?;
        append_bytes(
            &mut bytes,
            self.projection_fingerprint.as_bytes(),
            limits,
        )?;
        append_u64(
            &mut bytes,
            self.projection_schema_revision.get(),
            limits,
        )?;
        Ok(bytes)
    }
}

/// Compact 128-bit provider-neutral point identifier.
///
/// The value can be rendered as a UUID-compatible hexadecimal string, but it is
/// intentionally not assigned UUID version semantics. Correctness relies on
/// collision checking against [`PointIdentityKey`], not on assuming collisions
/// are impossible.
#[derive(Clone, Copy, Eq, Ord, PartialEq, PartialOrd)]
pub struct PointId128([u8; 16]);

impl PointId128 {
    /// Creates an identifier from exact bytes.
    pub const fn from_bytes(bytes: [u8; 16]) -> Self {
        Self(bytes)
    }

    /// Exact 16 identifier bytes.
    pub const fn as_bytes(&self) -> &[u8; 16] {
        &self.0
    }

    /// Deterministic lower-case hexadecimal representation.
    pub fn to_hex(self) -> String {
        let mut output = String::with_capacity(32);
        for byte in self.0 {
            use core::fmt::Write as _;
            write!(&mut output, "{byte:02x}")
                .expect("writing hexadecimal into String cannot fail");
        }
        output
    }

    /// UUID-compatible hyphenated representation accepted by Qdrant UUID IDs.
    pub fn to_hyphenated(self) -> String {
        let hex = self.to_hex();
        format!(
            "{}-{}-{}-{}-{}",
            &hex[0..8],
            &hex[8..12],
            &hex[12..16],
            &hex[16..20],
            &hex[20..32]
        )
    }
}

impl fmt::Debug for PointId128 {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_tuple("PointId128")
            .field(&self.to_hyphenated())
            .finish()
    }
}

impl fmt::Display for PointId128 {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(&self.to_hyphenated())
    }
}

/// Complete derived identity and canonical-key fingerprint.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct PointIdentity {
    /// Compact provider-neutral point ID.
    pub point_id: PointId128,
    /// Complete immutable logical key.
    pub key: PointIdentityKey,
    /// Digest profile revision used for the compact ID.
    pub profile_revision: u16,
}

/// Derives a deterministic point identity from a complete immutable key.
pub fn derive_point_identity(
    key: PointIdentityKey,
    limits: PointIdentityLimits,
) -> Result<PointIdentity, PointIdentityError> {
    let canonical = key.canonical_bytes(limits)?;
    let point_id = PointId128::from_bytes(frozen_digest_128(&canonical));
    Ok(PointIdentity {
        point_id,
        key,
        profile_revision: POINT_IDENTITY_PROFILE_REVISION,
    })
}

/// Finite collision registry required before publishing compact point IDs.
#[derive(Clone, Debug)]
pub struct PointIdentityRegistry {
    max_points: usize,
    by_id: BTreeMap<PointId128, PointIdentityKey>,
    by_key: BTreeMap<PointIdentityKey, PointId128>,
}

impl PointIdentityRegistry {
    /// Creates an empty finite collision registry.
    pub fn new(limits: PointIdentityLimits) -> Result<Self, PointIdentityError> {
        let limits = limits.validate()?;
        Ok(Self {
            max_points: limits.max_registered_points,
            by_id: BTreeMap::new(),
            by_key: BTreeMap::new(),
        })
    }

    /// Registers or exactly replays one derived identity.
    ///
    /// A compact-ID collision with another complete key is a hard error.
    pub fn register(
        &mut self,
        identity: PointIdentity,
    ) -> Result<PointId128, PointIdentityError> {
        if identity.profile_revision != POINT_IDENTITY_PROFILE_REVISION {
            return Err(PointIdentityError::IdentityMismatch);
        }
        if let Some(existing_key) = self.by_id.get(&identity.point_id) {
            if existing_key != &identity.key {
                return Err(PointIdentityError::DigestCollision);
            }
            return Ok(identity.point_id);
        }
        if let Some(existing_id) = self.by_key.get(&identity.key) {
            if existing_id != &identity.point_id {
                return Err(PointIdentityError::IdentityMismatch);
            }
            return Ok(*existing_id);
        }
        if self.by_id.len() >= self.max_points {
            return Err(PointIdentityError::RegistryCapacityExceeded);
        }
        self.by_key
            .insert(identity.key.clone(), identity.point_id);
        self.by_id.insert(identity.point_id, identity.key);
        Ok(identity.point_id)
    }

    /// Returns the complete key for one compact point ID.
    pub fn key(&self, point_id: PointId128) -> Result<&PointIdentityKey, PointIdentityError> {
        self.by_id
            .get(&point_id)
            .ok_or(PointIdentityError::PointNotFound)
    }

    /// Number of registered identities.
    pub fn len(&self) -> usize {
        self.by_id.len()
    }

    /// Returns whether no identities are registered.
    pub fn is_empty(&self) -> bool {
        self.by_id.is_empty()
    }
}

fn append_text(
    output: &mut Vec<u8>,
    value: &str,
    limits: PointIdentityLimits,
) -> Result<(), PointIdentityError> {
    append_bytes(output, value.as_bytes(), limits)
}

fn append_bytes(
    output: &mut Vec<u8>,
    value: &[u8],
    limits: PointIdentityLimits,
) -> Result<(), PointIdentityError> {
    let length = u32::try_from(value.len()).map_err(|_| PointIdentityError::LengthOverflow)?;
    extend_checked(output, &length.to_be_bytes(), limits)?;
    extend_checked(output, value, limits)
}

fn append_u8(
    output: &mut Vec<u8>,
    value: u8,
    limits: PointIdentityLimits,
) -> Result<(), PointIdentityError> {
    extend_checked(output, &[value], limits)
}

fn append_u16(
    output: &mut Vec<u8>,
    value: u16,
    limits: PointIdentityLimits,
) -> Result<(), PointIdentityError> {
    extend_checked(output, &value.to_be_bytes(), limits)
}

fn append_u64(
    output: &mut Vec<u8>,
    value: u64,
    limits: PointIdentityLimits,
) -> Result<(), PointIdentityError> {
    extend_checked(output, &value.to_be_bytes(), limits)
}

fn extend_checked(
    output: &mut Vec<u8>,
    value: &[u8],
    limits: PointIdentityLimits,
) -> Result<(), PointIdentityError> {
    let new_len = output
        .len()
        .checked_add(value.len())
        .ok_or(PointIdentityError::LengthOverflow)?;
    if new_len > limits.max_canonical_bytes {
        return Err(PointIdentityError::CanonicalBytesExceeded);
    }
    output.extend_from_slice(value);
    Ok(())
}

fn frozen_digest_128(bytes: &[u8]) -> [u8; 16] {
    let mut left = FNV_OFFSET_A;
    let mut right = FNV_OFFSET_B;
    for (index, byte) in bytes.iter().copied().enumerate() {
        left ^= u64::from(byte);
        left = left.wrapping_mul(FNV_PRIME);
        left ^= left.rotate_right(29).wrapping_add(MIX_A);

        right ^= u64::from(byte).wrapping_add(
            u64::try_from(index).unwrap_or(u64::MAX).rotate_left(17),
        );
        right = right.wrapping_mul(FNV_PRIME ^ MIX_B);
        right ^= right.rotate_left(31).wrapping_add(MIX_B);
    }
    left ^= u64::try_from(bytes.len()).unwrap_or(u64::MAX).wrapping_mul(MIX_A);
    right ^= u64::try_from(bytes.len()).unwrap_or(u64::MAX).wrapping_mul(MIX_B);
    left = avalanche(left);
    right = avalanche(right ^ left.rotate_left(23));
    let mut output = [0_u8; 16];
    output[..8].copy_from_slice(&left.to_be_bytes());
    output[8..].copy_from_slice(&right.to_be_bytes());
    output
}

const fn avalanche(mut value: u64) -> u64 {
    value ^= value >> 30;
    value = value.wrapping_mul(0xbf58_476d_1ce4_e5b9);
    value ^= value >> 27;
    value = value.wrapping_mul(0x94d0_49bb_1331_11eb);
    value ^ (value >> 31)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn key(unit_ordinal: u64, start: u64, end: u64) -> PointIdentityKey {
        PointIdentityKey {
            namespace_id: OpaqueId::new("namespace:test").expect("namespace"),
            source_id: OpaqueId::new("source:test").expect("source"),
            source_revision: NonZeroRevision::new(3).expect("revision"),
            unit_ordinal,
            source_byte_start: start,
            source_byte_end: end,
            projection_kind: ProjectionKind::Lexical,
            projection_fingerprint: Blake3Digest32::from_bytes([7; 32]),
            projection_schema_revision: NonZeroRevision::new(2).expect("revision"),
        }
    }

    #[test]
    fn exact_same_key_produces_exact_same_point_id() {
        let first = derive_point_identity(key(0, 0, 10), DEFAULT_POINT_IDENTITY_LIMITS)
            .expect("first");
        let second = derive_point_identity(key(0, 0, 10), DEFAULT_POINT_IDENTITY_LIMITS)
            .expect("second");
        assert_eq!(first, second);
        assert_eq!(first.point_id.to_hex().len(), 32);
        assert_eq!(first.point_id.to_hyphenated().len(), 36);
    }

    #[test]
    fn every_load_bearing_dimension_changes_identity() {
        let baseline = derive_point_identity(key(0, 0, 10), DEFAULT_POINT_IDENTITY_LIMITS)
            .expect("baseline");
        let changed_ordinal = derive_point_identity(key(1, 0, 10), DEFAULT_POINT_IDENTITY_LIMITS)
            .expect("ordinal");
        let changed_range = derive_point_identity(key(0, 1, 10), DEFAULT_POINT_IDENTITY_LIMITS)
            .expect("range");
        let mut changed_projection = key(0, 0, 10);
        changed_projection.projection_fingerprint = Blake3Digest32::from_bytes([8; 32]);
        let changed_projection = derive_point_identity(
            changed_projection,
            DEFAULT_POINT_IDENTITY_LIMITS,
        )
        .expect("projection");
        assert_ne!(baseline.point_id, changed_ordinal.point_id);
        assert_ne!(baseline.point_id, changed_range.point_id);
        assert_ne!(baseline.point_id, changed_projection.point_id);
    }

    #[test]
    fn provider_collection_or_insertion_order_is_not_an_input() {
        let one = derive_point_identity(key(5, 100, 200), DEFAULT_POINT_IDENTITY_LIMITS)
            .expect("identity");
        let mut registry_a = PointIdentityRegistry::new(DEFAULT_POINT_IDENTITY_LIMITS)
            .expect("registry");
        let mut registry_b = PointIdentityRegistry::new(DEFAULT_POINT_IDENTITY_LIMITS)
            .expect("registry");
        registry_a.register(one.clone()).expect("register");
        let other = derive_point_identity(key(1, 0, 10), DEFAULT_POINT_IDENTITY_LIMITS)
            .expect("other");
        registry_b.register(other).expect("other");
        assert_eq!(registry_b.register(one.clone()).expect("one"), one.point_id);
        assert_eq!(registry_a.key(one.point_id).expect("key"), &one.key);
        assert_eq!(registry_b.key(one.point_id).expect("key"), &one.key);
    }

    #[test]
    fn compact_id_collision_is_detected_against_complete_key() {
        let first = derive_point_identity(key(0, 0, 10), DEFAULT_POINT_IDENTITY_LIMITS)
            .expect("first");
        let mut forged = derive_point_identity(key(1, 10, 20), DEFAULT_POINT_IDENTITY_LIMITS)
            .expect("forged");
        forged.point_id = first.point_id;
        let mut registry = PointIdentityRegistry::new(DEFAULT_POINT_IDENTITY_LIMITS)
            .expect("registry");
        registry.register(first).expect("first");
        assert_eq!(
            registry.register(forged),
            Err(PointIdentityError::DigestCollision)
        );
    }

    #[test]
    fn invalid_or_oversize_key_fails_closed() {
        let mut invalid = key(0, 10, 10);
        assert_eq!(
            derive_point_identity(invalid.clone(), DEFAULT_POINT_IDENTITY_LIMITS),
            Err(PointIdentityError::InvalidUnitRange)
        );
        invalid.source_byte_end = 11;
        let limits = PointIdentityLimits {
            max_identifier_bytes: 4,
            ..DEFAULT_POINT_IDENTITY_LIMITS
        };
        assert_eq!(
            derive_point_identity(invalid, limits),
            Err(PointIdentityError::IdentifierTooLong)
        );
    }

    #[test]
    fn registry_is_finite_and_exact_replay_is_idempotent() {
        let limits = PointIdentityLimits {
            max_registered_points: 1,
            ..DEFAULT_POINT_IDENTITY_LIMITS
        };
        let first = derive_point_identity(key(0, 0, 10), limits).expect("first");
        let second = derive_point_identity(key(1, 10, 20), limits).expect("second");
        let mut registry = PointIdentityRegistry::new(limits).expect("registry");
        assert_eq!(registry.register(first.clone()).expect("first"), first.point_id);
        assert_eq!(registry.register(first.clone()).expect("replay"), first.point_id);
        assert_eq!(
            registry.register(second),
            Err(PointIdentityError::RegistryCapacityExceeded)
        );
        assert_eq!(registry.len(), 1);
    }
}

//! Canonical, collision-detectable identities for Qdrant projection points.
//!
//! The exact S11.1 key is encoded as deterministic canonical CBOR, hashed with
//! BLAKE3-256, and projected through a separate domain into a 128-bit
//! UUID-compatible Qdrant address. The address is never treated as the full
//! identity: every overwrite/recovery path compares the complete digest and
//! every independently represented identity field.

#![forbid(unsafe_code)]
#![deny(missing_docs)]
#![allow(
    clippy::large_enum_variant,
    clippy::missing_errors_doc,
    clippy::module_name_repetitions,
    clippy::too_many_lines
)]

use core::fmt;
use std::collections::BTreeMap;

use search_contracts::{
    CollectionGenerationId, InstallationIncarnationId, ProjectionMembershipId,
    ProjectionProfileSetId, RepresentationId, UnitId,
};

/// Frozen S11.1 point-key schema version.
pub const POINT_IDENTITY_SCHEMA_VERSION: u16 = 1;

const UUID_PROJECTION_DOMAIN: &[u8] = b"eliot-search/qdrant-point-uuid/v1\0";

/// Conservative finite point-identity limits.
pub const DEFAULT_POINT_IDENTITY_LIMITS: PointIdentityLimits = PointIdentityLimits {
    max_canonical_bytes: 4_096,
    max_registered_points: 16_000_000,
};

/// Closed point-identity failure.
#[derive(Clone, Copy, Debug, Eq, Ord, PartialEq, PartialOrd)]
pub enum PointIdentityError {
    /// A finite bound is zero or internally inconsistent.
    InvalidLimits,
    /// The key names an unsupported canonical schema version.
    UnknownSchemaVersion,
    /// Canonical CBOR encoding exceeded its finite byte ceiling.
    CanonicalBytesExceeded,
    /// A CBOR length or offset conversion overflowed.
    LengthOverflow,
    /// The compact Qdrant address is occupied by another full identity.
    DigestCollision,
    /// An observed address or identity payload does not match the expected point.
    IdentityMismatch,
    /// The bounded in-memory collision registry is full.
    RegistryCapacityExceeded,
}

impl PointIdentityError {
    /// Stable machine-readable reason code.
    #[must_use]
    pub const fn code(self) -> &'static str {
        match self {
            Self::InvalidLimits => "POINT_ID_INVALID_LIMITS",
            Self::UnknownSchemaVersion => "POINT_ID_UNKNOWN_SCHEMA_VERSION",
            Self::CanonicalBytesExceeded => "POINT_ID_CANONICAL_BYTES_EXCEEDED",
            Self::LengthOverflow => "POINT_ID_LENGTH_OVERFLOW",
            Self::DigestCollision => "POINT_ID_COLLISION",
            Self::IdentityMismatch => "POINT_IDENTITY_MISMATCH",
            Self::RegistryCapacityExceeded => "POINT_ID_REGISTRY_CAPACITY_EXCEEDED",
        }
    }
}

impl fmt::Display for PointIdentityError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(self.code())
    }
}

impl std::error::Error for PointIdentityError {}

/// Finite pure point-identity limits.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct PointIdentityLimits {
    /// Maximum deterministic CBOR bytes for one key.
    pub max_canonical_bytes: usize,
    /// Maximum collision-checked identities retained by one registry.
    pub max_registered_points: usize,
}

impl PointIdentityLimits {
    /// Validates every finite dimension as non-zero.
    pub const fn validate(self) -> Result<Self, PointIdentityError> {
        if self.max_canonical_bytes == 0 || self.max_registered_points == 0 {
            Err(PointIdentityError::InvalidLimits)
        } else {
            Ok(self)
        }
    }
}

/// Logical role of one projection point in the S11.1 identity key.
#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub enum PointRole {
    /// One searchable unit and all required named vectors for that unit.
    Unit,
    /// A relation point admitted by a future qualified projection profile.
    Relation,
    /// An auxiliary point admitted by a future qualified projection profile.
    Auxiliary,
}

impl PointRole {
    /// Frozen wire text encoded into canonical CBOR.
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Unit => "unit",
            Self::Relation => "relation",
            Self::Auxiliary => "auxiliary",
        }
    }
}

/// Exact normative S11.1 projection-point key.
#[derive(Clone, Debug, Eq, Ord, PartialEq, PartialOrd)]
pub struct PointIdentityKey {
    /// Canonical key schema version; must equal [`POINT_IDENTITY_SCHEMA_VERSION`].
    pub schema_version: u16,
    /// Current installation incarnation.
    pub installation_incarnation_id: InstallationIncarnationId,
    /// Exact physical collection generation.
    pub collection_generation_id: CollectionGenerationId,
    /// One immutable projection membership.
    pub projection_membership_id: ProjectionMembershipId,
    /// Exact canonical representation.
    pub representation_id: RepresentationId,
    /// Exact occurrence unit within that representation.
    pub unit_id: UnitId,
    /// Immutable vector/analyzer/profile-set identity.
    pub projection_profile_set_id: ProjectionProfileSetId,
    /// Point role within the projection profile.
    pub point_role: PointRole,
}

impl PointIdentityKey {
    /// Validates the frozen key schema version.
    pub const fn validate(&self) -> Result<(), PointIdentityError> {
        if self.schema_version == POINT_IDENTITY_SCHEMA_VERSION {
            Ok(())
        } else {
            Err(PointIdentityError::UnknownSchemaVersion)
        }
    }
}

/// Bounded deterministic canonical CBOR bytes for one point key.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct CanonicalPointKeyBytes(Vec<u8>);

impl CanonicalPointKeyBytes {
    /// Exact encoded bytes.
    #[must_use]
    pub fn as_slice(&self) -> &[u8] {
        &self.0
    }

    /// Consumes the wrapper and returns the exact encoded bytes.
    #[must_use]
    pub fn into_vec(self) -> Vec<u8> {
        self.0
    }
}

/// Full BLAKE3-256 point-identity digest.
#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub struct PointIdentityDigest([u8; 32]);

impl PointIdentityDigest {
    /// Creates a digest from exact bytes.
    #[must_use]
    pub const fn from_bytes(bytes: [u8; 32]) -> Self {
        Self(bytes)
    }

    /// Exact digest bytes.
    #[must_use]
    pub const fn as_bytes(&self) -> &[u8; 32] {
        &self.0
    }
}

/// Compact UUID-compatible Qdrant point address.
///
/// This value is only an address. Correctness always compares the complete
/// [`PointIdentityDigest`] and S11.1 identity payload.
#[derive(Clone, Copy, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub struct PointId128([u8; 16]);

impl PointId128 {
    /// Creates an address from exact bytes.
    #[must_use]
    pub const fn from_bytes(bytes: [u8; 16]) -> Self {
        Self(bytes)
    }

    /// Exact address bytes.
    #[must_use]
    pub const fn as_bytes(&self) -> &[u8; 16] {
        &self.0
    }

    /// Deterministic lower-case hexadecimal representation.
    #[must_use]
    pub fn to_hex(self) -> String {
        let mut output = String::with_capacity(32);
        for byte in self.0 {
            use core::fmt::Write as _;
            write!(&mut output, "{byte:02x}")
                .expect("writing hexadecimal into String cannot fail");
        }
        output
    }

    /// UUID-compatible hyphenated representation accepted by Qdrant.
    #[must_use]
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

/// Complete derived point identity.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct PointIdentity {
    /// Exact canonical key.
    pub key: PointIdentityKey,
    /// Full BLAKE3-256 key digest stored in payload/manifest.
    pub full_digest: PointIdentityDigest,
    /// Namespace-separated 128-bit Qdrant address.
    pub point_id: PointId128,
}

impl PointIdentity {
    /// Identity fields independently represented by the S9.5 point payload.
    #[must_use]
    pub fn payload(&self) -> PointIdentityPayload {
        PointIdentityPayload {
            installation_incarnation_id: self.key.installation_incarnation_id,
            collection_generation_id: self.key.collection_generation_id,
            projection_membership_id: self.key.projection_membership_id,
            representation_id: self.key.representation_id,
            unit_id: self.key.unit_id,
            projection_profile_set_id: self.key.projection_profile_set_id.clone(),
            point_identity_digest_256: self.full_digest,
        }
    }
}

/// S11.2 identity fields read back from one S9.5 point payload.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct PointIdentityPayload {
    /// Installation incarnation carried by the point.
    pub installation_incarnation_id: InstallationIncarnationId,
    /// Collection generation carried by the point.
    pub collection_generation_id: CollectionGenerationId,
    /// Projection membership carried by the point.
    pub projection_membership_id: ProjectionMembershipId,
    /// Representation carried by the point.
    pub representation_id: RepresentationId,
    /// Unit carried by the point.
    pub unit_id: UnitId,
    /// Projection profile set carried by the point.
    pub projection_profile_set_id: ProjectionProfileSetId,
    /// Full BLAKE3-256 canonical key digest.
    pub point_identity_digest_256: PointIdentityDigest,
}

/// Exact observed point address and identity payload.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ObservedPointIdentity {
    /// Address occupied in Qdrant.
    pub point_id: PointId128,
    /// Exact identity payload read back from Qdrant.
    pub payload: PointIdentityPayload,
}

/// Non-destructive collision decision.
#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub enum CollisionDecision {
    /// No point occupies the address; creation is permitted.
    Vacant,
    /// Address and complete identity match; idempotent replay is permitted.
    SameFullIdentity,
    /// The address names another full identity; overwrite is forbidden.
    CollisionBlock,
}

/// Encodes the exact S11.1 key as deterministic canonical CBOR.
///
/// The map has eight text keys in RFC 8949 deterministic order (encoded key
/// length, then bytewise lexical order). UUID-like contract IDs are encoded as
/// 16-byte byte strings, not display text.
pub fn canonical_point_key_bytes(
    key: &PointIdentityKey,
    limits: PointIdentityLimits,
) -> Result<CanonicalPointKeyBytes, PointIdentityError> {
    key.validate()?;
    let limits = limits.validate()?;
    let mut bytes = Vec::with_capacity(320);
    append_map_len(&mut bytes, 8)?;

    // Deterministic CBOR key order:
    // unit_id, point_role, schema_version, representation_id,
    // collection_generation_id, projection_membership_id,
    // projection_profile_set_id, installation_incarnation_id.
    append_text(&mut bytes, "unit_id")?;
    append_bytes(&mut bytes, key.unit_id.as_bytes())?;

    append_text(&mut bytes, "point_role")?;
    append_text(&mut bytes, key.point_role.as_str())?;

    append_text(&mut bytes, "schema_version")?;
    append_unsigned(&mut bytes, u64::from(key.schema_version))?;

    append_text(&mut bytes, "representation_id")?;
    append_bytes(&mut bytes, key.representation_id.as_bytes())?;

    append_text(&mut bytes, "collection_generation_id")?;
    append_bytes(&mut bytes, key.collection_generation_id.as_bytes())?;

    append_text(&mut bytes, "projection_membership_id")?;
    append_bytes(&mut bytes, key.projection_membership_id.as_bytes())?;

    append_text(&mut bytes, "projection_profile_set_id")?;
    append_text(&mut bytes, key.projection_profile_set_id.as_str())?;

    append_text(&mut bytes, "installation_incarnation_id")?;
    append_bytes(&mut bytes, key.installation_incarnation_id.as_bytes())?;

    if bytes.len() > limits.max_canonical_bytes {
        return Err(PointIdentityError::CanonicalBytesExceeded);
    }
    Ok(CanonicalPointKeyBytes(bytes))
}

/// Compatibility-free surface name for canonical key encoding.
pub fn encode_canonical_key(
    key: &PointIdentityKey,
    limits: PointIdentityLimits,
) -> Result<CanonicalPointKeyBytes, PointIdentityError> {
    canonical_point_key_bytes(key, limits)
}

/// Computes the full BLAKE3-256 digest of canonical key bytes.
#[must_use]
pub fn point_identity_digest(canonical_bytes: &[u8]) -> PointIdentityDigest {
    PointIdentityDigest::from_bytes(*blake3::hash(canonical_bytes).as_bytes())
}

/// Surface alias for [`point_identity_digest`].
#[must_use]
pub fn full_digest(canonical_bytes: &[u8]) -> PointIdentityDigest {
    point_identity_digest(canonical_bytes)
}

/// Projects a full digest into a separately domain-separated Qdrant address.
#[must_use]
pub fn project_qdrant_uuid(digest: PointIdentityDigest) -> PointId128 {
    let mut hasher = blake3::Hasher::new();
    hasher.update(UUID_PROJECTION_DOMAIN);
    hasher.update(digest.as_bytes());
    let projected = hasher.finalize();
    let mut bytes = [0_u8; 16];
    bytes.copy_from_slice(&projected.as_bytes()[..16]);
    PointId128::from_bytes(bytes)
}

/// Surface alias for [`project_qdrant_uuid`].
#[must_use]
pub fn derive_qdrant_uuid(digest: PointIdentityDigest) -> PointId128 {
    project_qdrant_uuid(digest)
}

/// Derives one complete point identity from the exact S11.1 key.
pub fn derive_point_identity(
    key: PointIdentityKey,
    limits: PointIdentityLimits,
) -> Result<PointIdentity, PointIdentityError> {
    let canonical = canonical_point_key_bytes(&key, limits)?;
    let full_digest = point_identity_digest(canonical.as_slice());
    let point_id = project_qdrant_uuid(full_digest);
    Ok(PointIdentity {
        key,
        full_digest,
        point_id,
    })
}

/// Validates the complete digest and every independently represented S11.1 field.
pub fn validate_identity_payload(
    expected: &PointIdentity,
    observed: &PointIdentityPayload,
) -> Result<(), PointIdentityError> {
    let expected_payload = expected.payload();
    if &expected_payload == observed {
        Ok(())
    } else {
        Err(PointIdentityError::IdentityMismatch)
    }
}

/// Compares a possibly occupied Qdrant address without destructive assumptions.
pub fn compare_existing_identity(
    expected: &PointIdentity,
    observed: Option<&ObservedPointIdentity>,
) -> Result<CollisionDecision, PointIdentityError> {
    let Some(observed) = observed else {
        return Ok(CollisionDecision::Vacant);
    };
    if observed.point_id != expected.point_id {
        return Err(PointIdentityError::IdentityMismatch);
    }
    Ok(if validate_identity_payload(expected, &observed.payload).is_ok() {
        CollisionDecision::SameFullIdentity
    } else {
        CollisionDecision::CollisionBlock
    })
}

/// Bounded in-memory collision registry used while composing one exact plan.
#[derive(Clone, Debug)]
pub struct PointIdentityRegistry {
    max_points: usize,
    points: BTreeMap<PointId128, PointIdentity>,
}

impl PointIdentityRegistry {
    /// Creates an empty bounded registry.
    pub fn new(limits: PointIdentityLimits) -> Result<Self, PointIdentityError> {
        let limits = limits.validate()?;
        Ok(Self {
            max_points: limits.max_registered_points,
            points: BTreeMap::new(),
        })
    }

    /// Registers one derived identity and blocks any compact-address collision.
    pub fn register(
        &mut self,
        identity: PointIdentity,
    ) -> Result<CollisionDecision, PointIdentityError> {
        if let Some(existing) = self.points.get(&identity.point_id) {
            if existing.full_digest == identity.full_digest && existing.key == identity.key {
                return Ok(CollisionDecision::SameFullIdentity);
            }
            return Err(PointIdentityError::DigestCollision);
        }
        if self.points.len() >= self.max_points {
            return Err(PointIdentityError::RegistryCapacityExceeded);
        }
        self.points.insert(identity.point_id, identity);
        Ok(CollisionDecision::Vacant)
    }

    /// Number of distinct registered point addresses.
    #[must_use]
    pub fn len(&self) -> usize {
        self.points.len()
    }

    /// Whether no identity is registered.
    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.points.is_empty()
    }
}

fn append_map_len(output: &mut Vec<u8>, len: u64) -> Result<(), PointIdentityError> {
    append_head(output, 5, len)
}

fn append_text(output: &mut Vec<u8>, text: &str) -> Result<(), PointIdentityError> {
    let len = u64::try_from(text.len()).map_err(|_| PointIdentityError::LengthOverflow)?;
    append_head(output, 3, len)?;
    output.extend_from_slice(text.as_bytes());
    Ok(())
}

fn append_bytes(output: &mut Vec<u8>, bytes: &[u8]) -> Result<(), PointIdentityError> {
    let len = u64::try_from(bytes.len()).map_err(|_| PointIdentityError::LengthOverflow)?;
    append_head(output, 2, len)?;
    output.extend_from_slice(bytes);
    Ok(())
}

fn append_unsigned(output: &mut Vec<u8>, value: u64) -> Result<(), PointIdentityError> {
    append_head(output, 0, value)
}

fn append_head(
    output: &mut Vec<u8>,
    major: u8,
    value: u64,
) -> Result<(), PointIdentityError> {
    let base = major
        .checked_shl(5)
        .ok_or(PointIdentityError::LengthOverflow)?;
    match value {
        0..=23 => output.push(base | u8::try_from(value).map_err(|_| PointIdentityError::LengthOverflow)?),
        24..=0xff => {
            output.push(base | 0x18);
            output.push(u8::try_from(value).map_err(|_| PointIdentityError::LengthOverflow)?);
        }
        0x100..=0xffff => {
            output.push(base | 0x19);
            output.extend_from_slice(
                &u16::try_from(value)
                    .map_err(|_| PointIdentityError::LengthOverflow)?
                    .to_be_bytes(),
            );
        }
        0x1_0000..=0xffff_ffff => {
            output.push(base | 0x1a);
            output.extend_from_slice(
                &u32::try_from(value)
                    .map_err(|_| PointIdentityError::LengthOverflow)?
                    .to_be_bytes(),
            );
        }
        _ => {
            output.push(base | 0x1b);
            output.extend_from_slice(&value.to_be_bytes());
        }
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn key() -> PointIdentityKey {
        PointIdentityKey {
            schema_version: POINT_IDENTITY_SCHEMA_VERSION,
            installation_incarnation_id: InstallationIncarnationId::from_bytes([1; 16]),
            collection_generation_id: CollectionGenerationId::from_bytes([2; 16]),
            projection_membership_id: ProjectionMembershipId::from_bytes([3; 16]),
            representation_id: RepresentationId::from_bytes([4; 16]),
            unit_id: UnitId::from_bytes([5; 16]),
            projection_profile_set_id: ProjectionProfileSetId::new("profile-v1")
                .expect("profile"),
            point_role: PointRole::Unit,
        }
    }

    fn decode_hex(text: &str) -> Vec<u8> {
        assert_eq!(text.len() % 2, 0);
        text.as_bytes()
            .chunks_exact(2)
            .map(|pair| {
                let pair = core::str::from_utf8(pair).expect("ASCII");
                u8::from_str_radix(pair, 16).expect("hex")
            })
            .collect()
    }

    #[test]
    fn canonical_cbor_digest_and_uuid_match_golden() {
        let canonical = canonical_point_key_bytes(&key(), DEFAULT_POINT_IDENTITY_LIMITS)
            .expect("canonical");
        let expected = decode_hex(
            "a867756e69745f696450050505050505050505050505050505056a706f696e745f726f6c6564756e69746e736368656d615f76657273696f6e0171726570726573656e746174696f6e5f696450040404040404040404040404040404047818636f6c6c656374696f6e5f67656e65726174696f6e5f69645002020202020202020202020202020202781870726f6a656374696f6e5f6d656d626572736869705f69645003030303030303030303030303030303781970726f6a656374696f6e5f70726f66696c655f7365745f69646a70726f66696c652d7631781b696e7374616c6c6174696f6e5f696e6361726e6174696f6e5f69645001010101010101010101010101010101",
        );
        assert_eq!(canonical.as_slice(), expected);
        let identity = derive_point_identity(key(), DEFAULT_POINT_IDENTITY_LIMITS)
            .expect("identity");
        assert_eq!(
            identity.full_digest.as_bytes(),
            &[
                0x43, 0xe4, 0xf1, 0xbf, 0xc6, 0x46, 0x54, 0xe2,
                0x59, 0x67, 0xb7, 0x16, 0x96, 0x9b, 0xeb, 0x61,
                0xd8, 0xf0, 0x93, 0xce, 0xbb, 0x3c, 0xbb, 0xfd,
                0x4b, 0x4f, 0x7e, 0x87, 0x9b, 0x0d, 0xe3, 0x04,
            ]
        );
        assert_eq!(
            identity.point_id.to_hyphenated(),
            "05bfacd2-4f23-45f5-6f5c-aae620e52b1b"
        );
    }

    #[test]
    fn every_s11_1_coordinate_changes_the_identity() {
        let baseline = derive_point_identity(key(), DEFAULT_POINT_IDENTITY_LIMITS)
            .expect("baseline");
        let mut variants = Vec::new();

        let mut value = key();
        value.installation_incarnation_id = InstallationIncarnationId::from_bytes([9; 16]);
        variants.push(value);
        let mut value = key();
        value.collection_generation_id = CollectionGenerationId::from_bytes([9; 16]);
        variants.push(value);
        let mut value = key();
        value.projection_membership_id = ProjectionMembershipId::from_bytes([9; 16]);
        variants.push(value);
        let mut value = key();
        value.representation_id = RepresentationId::from_bytes([9; 16]);
        variants.push(value);
        let mut value = key();
        value.unit_id = UnitId::from_bytes([9; 16]);
        variants.push(value);
        let mut value = key();
        value.projection_profile_set_id =
            ProjectionProfileSetId::new("profile-v2").expect("profile");
        variants.push(value);
        let mut value = key();
        value.point_role = PointRole::Auxiliary;
        variants.push(value);

        for variant in variants {
            let identity = derive_point_identity(variant, DEFAULT_POINT_IDENTITY_LIMITS)
                .expect("variant");
            assert_ne!(identity.full_digest, baseline.full_digest);
            assert_ne!(identity.point_id, baseline.point_id);
        }
    }

    #[test]
    fn unknown_schema_version_is_rejected() {
        let mut invalid = key();
        invalid.schema_version = POINT_IDENTITY_SCHEMA_VERSION + 1;
        assert_eq!(
            derive_point_identity(invalid, DEFAULT_POINT_IDENTITY_LIMITS),
            Err(PointIdentityError::UnknownSchemaVersion)
        );
    }

    #[test]
    fn collision_registry_never_overwrites_another_identity() {
        let first = derive_point_identity(key(), DEFAULT_POINT_IDENTITY_LIMITS)
            .expect("first");
        let mut other_key = key();
        other_key.unit_id = UnitId::from_bytes([8; 16]);
        let mut other = derive_point_identity(other_key, DEFAULT_POINT_IDENTITY_LIMITS)
            .expect("other");
        other.point_id = first.point_id;

        let mut registry = PointIdentityRegistry::new(PointIdentityLimits {
            max_canonical_bytes: 4_096,
            max_registered_points: 1,
        })
        .expect("registry");
        assert_eq!(
            registry.register(first.clone()).expect("vacant"),
            CollisionDecision::Vacant
        );
        assert_eq!(
            registry.register(first).expect("same"),
            CollisionDecision::SameFullIdentity
        );
        assert_eq!(
            registry.register(other),
            Err(PointIdentityError::DigestCollision)
        );
        assert_eq!(registry.len(), 1);
    }

    #[test]
    fn observed_payload_mismatch_blocks_overwrite() {
        let expected = derive_point_identity(key(), DEFAULT_POINT_IDENTITY_LIMITS)
            .expect("identity");
        let same = ObservedPointIdentity {
            point_id: expected.point_id,
            payload: expected.payload(),
        };
        assert_eq!(
            compare_existing_identity(&expected, Some(&same)).expect("same"),
            CollisionDecision::SameFullIdentity
        );

        let mut foreign = same;
        foreign.payload.unit_id = UnitId::from_bytes([0xff; 16]);
        assert_eq!(
            compare_existing_identity(&expected, Some(&foreign)).expect("decision"),
            CollisionDecision::CollisionBlock
        );
    }
}

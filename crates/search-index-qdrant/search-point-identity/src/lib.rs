//! Canonical collision-detectable Qdrant point identities.
//!
//! The complete identity is the BLAKE3-256 digest of one versioned canonical
//! CBOR [`ProjectionPointKey`]. The Qdrant UUID is only a namespace-separated
//! 128-bit projection of that digest. Correctness therefore always compares the
//! full digest and every canonical identity field before an existing UUID may
//! be reused.

#![forbid(unsafe_code)]
#![deny(missing_docs)]
#![allow(
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

/// Supported canonical point-key schema version.
pub const POINT_KEY_SCHEMA_VERSION: u16 = 1;

const UUID_PROJECTION_DOMAIN: &[u8] = b"eliot-search/qdrant-point-uuid/v1\0";

/// Conservative finite point-identity limits.
pub const DEFAULT_POINT_IDENTITY_LIMITS: PointIdentityLimits = PointIdentityLimits {
    max_identifier_bytes: 4_096,
    max_canonical_bytes: 32_768,
    max_registered_points: 16_000_000,
};

/// Closed point-identity failure.
#[derive(Clone, Copy, Debug, Eq, Ord, PartialEq, PartialOrd)]
pub enum PointIdentityError {
    /// Limits are zero or internally inconsistent.
    InvalidLimits,
    /// The key schema version is not accepted.
    UnsupportedKeyVersion,
    /// The profile-set identifier exceeds the finite boundary.
    IdentifierTooLong,
    /// Canonical encoding exceeded its finite ceiling.
    CanonicalBytesExceeded,
    /// Canonical encoding length conversion overflowed.
    LengthOverflow,
    /// One compact UUID maps to another full identity.
    DigestCollision,
    /// Full digest, UUID or canonical fields differ.
    IdentityMismatch,
    /// Finite collision registry is full.
    RegistryCapacityExceeded,
    /// Requested UUID is absent from the collision registry.
    PointNotFound,
}

impl PointIdentityError {
    /// Stable machine-readable reason code.
    #[must_use]
    pub const fn code(self) -> &'static str {
        match self {
            Self::InvalidLimits => "POINT_ID_INVALID_LIMITS",
            Self::UnsupportedKeyVersion => "POINT_KEY_VERSION_UNSUPPORTED",
            Self::IdentifierTooLong => "POINT_ID_IDENTIFIER_TOO_LONG",
            Self::CanonicalBytesExceeded => "CANONICAL_ENCODING_FAILED",
            Self::LengthOverflow => "CANONICAL_ENCODING_FAILED",
            Self::DigestCollision => "POINT_ID_COLLISION",
            Self::IdentityMismatch => "POINT_IDENTITY_MISMATCH",
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
    /// Maximum UTF-8 bytes in the profile-set identifier.
    pub max_identifier_bytes: usize,
    /// Maximum canonical CBOR bytes for one key.
    pub max_canonical_bytes: usize,
    /// Maximum collision-checked identities retained by one registry.
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

/// Logical role of one point within a projection profile set.
#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub enum PointRole {
    /// Searchable unit point.
    Unit,
    /// Relation point between units/entities.
    Relation,
    /// Auxiliary profile-owned point.
    Auxiliary,
}

impl PointRole {
    const fn cbor_value(self) -> u8 {
        match self {
            Self::Unit => 0,
            Self::Relation => 1,
            Self::Auxiliary => 2,
        }
    }
}

/// Complete immutable S11.1 logical key for one Qdrant point.
///
/// The key intentionally excludes source membership names, paths, policy
/// revisions, epochs, vector values and vendor collection names. Changes to
/// access/scoring meaning mint new immutable partition/projection identities
/// before this key is constructed.
#[derive(Clone, Debug, Eq, Ord, PartialEq, PartialOrd)]
pub struct ProjectionPointKey {
    /// Canonical key schema version; only version 1 is accepted.
    pub schema_version: u16,
    /// Current installation incarnation.
    pub installation_incarnation_id: InstallationIncarnationId,
    /// Physical collection generation identity.
    pub collection_generation_id: CollectionGenerationId,
    /// Exactly one projection membership.
    pub projection_membership_id: ProjectionMembershipId,
    /// Immutable representation identity.
    pub representation_id: RepresentationId,
    /// Immutable unit occurrence identity.
    pub unit_id: UnitId,
    /// Immutable projection-profile-set identity.
    pub projection_profile_set_id: ProjectionProfileSetId,
    /// Point role within that profile set.
    pub point_role: PointRole,
}

impl ProjectionPointKey {
    /// Validates the exact key version and finite profile identifier.
    pub fn validate(&self, limits: PointIdentityLimits) -> Result<(), PointIdentityError> {
        let limits = limits.validate()?;
        if self.schema_version != POINT_KEY_SCHEMA_VERSION {
            return Err(PointIdentityError::UnsupportedKeyVersion);
        }
        if self.projection_profile_set_id.as_str().len() > limits.max_identifier_bytes {
            return Err(PointIdentityError::IdentifierTooLong);
        }
        Ok(())
    }
}

/// Compatibility name for the complete canonical point key.
pub type PointIdentityKey = ProjectionPointKey;

/// Frozen canonical CBOR bytes for one [`ProjectionPointKey`].
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct CanonicalPointKeyBytes(Vec<u8>);

impl CanonicalPointKeyBytes {
    /// Borrows the exact canonical bytes.
    #[must_use]
    pub fn as_slice(&self) -> &[u8] {
        &self.0
    }

    /// Consumes the wrapper.
    #[must_use]
    pub fn into_vec(self) -> Vec<u8> {
        self.0
    }
}

/// Full BLAKE3-256 point identity digest.
#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub struct PointIdentityDigest([u8; 32]);

impl PointIdentityDigest {
    /// Creates a digest from exact bytes.
    #[must_use]
    pub const fn from_bytes(bytes: [u8; 32]) -> Self {
        Self(bytes)
    }

    /// Exact 32 digest bytes.
    #[must_use]
    pub const fn as_bytes(&self) -> &[u8; 32] {
        &self.0
    }
}

/// Compact 128-bit provider-neutral Qdrant UUID bytes.
#[derive(Clone, Copy, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub struct PointId128([u8; 16]);

impl PointId128 {
    /// Creates an identifier from exact bytes.
    #[must_use]
    pub const fn from_bytes(bytes: [u8; 16]) -> Self {
        Self(bytes)
    }

    /// Exact 16 identifier bytes.
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
    /// Compact Qdrant UUID projection.
    pub point_id: PointId128,
    /// Full BLAKE3-256 digest stored in payload and manifest.
    pub full_digest: PointIdentityDigest,
    /// Complete immutable canonical key.
    pub key: ProjectionPointKey,
}

/// Existing point identity read before a possible overwrite.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ExistingPointIdentity {
    /// Compact UUID occupied in Qdrant.
    pub point_id: PointId128,
    /// Full payload identity digest.
    pub full_digest: PointIdentityDigest,
    /// Canonical identity fields reconstructed from typed payload/control data.
    pub key: ProjectionPointKey,
}

/// Non-destructive collision decision.
#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub enum CollisionDecision {
    /// No point occupies the UUID; creation is permitted.
    Vacant,
    /// UUID, full digest and every canonical field match.
    SameFullIdentity,
    /// Any mismatch blocks the write.
    CollisionBlock,
}

/// Encodes one key as a deterministic canonical CBOR array.
///
/// The fixed version-1 schema is:
/// `[version, installation, collection, membership, representation, unit,
/// profile_set, role]`. UUIDs are 16-byte CBOR byte strings, profile set is a
/// CBOR text string and role is an unsigned integer. Definite lengths and the
/// shortest integer encoding make the representation canonical.
pub fn canonical_point_key_bytes(
    key: &ProjectionPointKey,
    limits: PointIdentityLimits,
) -> Result<CanonicalPointKeyBytes, PointIdentityError> {
    key.validate(limits)?;
    let limits = limits.validate()?;
    let mut output = Vec::with_capacity(128);
    push_checked(&mut output, &[0x88], limits)?;
    encode_unsigned(&mut output, u64::from(key.schema_version), limits)?;
    encode_bytes(
        &mut output,
        key.installation_incarnation_id.as_bytes(),
        limits,
    )?;
    encode_bytes(
        &mut output,
        key.collection_generation_id.as_bytes(),
        limits,
    )?;
    encode_bytes(
        &mut output,
        key.projection_membership_id.as_bytes(),
        limits,
    )?;
    encode_bytes(&mut output, key.representation_id.as_bytes(), limits)?;
    encode_bytes(&mut output, key.unit_id.as_bytes(), limits)?;
    encode_text(
        &mut output,
        key.projection_profile_set_id.as_str(),
        limits,
    )?;
    encode_unsigned(
        &mut output,
        u64::from(key.point_role.cbor_value()),
        limits,
    )?;
    Ok(CanonicalPointKeyBytes(output))
}

/// Compatibility spelling for canonical key encoding.
pub fn encode_canonical_key(
    key: &ProjectionPointKey,
    limits: PointIdentityLimits,
) -> Result<Vec<u8>, PointIdentityError> {
    canonical_point_key_bytes(key, limits).map(CanonicalPointKeyBytes::into_vec)
}

/// Computes the full BLAKE3-256 digest of canonical key bytes.
#[must_use]
pub fn point_identity_digest(canonical_bytes: &[u8]) -> PointIdentityDigest {
    PointIdentityDigest(*blake3::hash(canonical_bytes).as_bytes())
}

/// Compatibility spelling for full digest derivation.
#[must_use]
pub fn full_digest(canonical_bytes: &[u8]) -> PointIdentityDigest {
    point_identity_digest(canonical_bytes)
}

/// Projects a full digest into a namespace-separated 128-bit UUID.
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

/// Compatibility spelling for UUID projection.
#[must_use]
pub fn derive_qdrant_uuid(digest: PointIdentityDigest) -> PointId128 {
    project_qdrant_uuid(digest)
}

/// Derives the complete point identity from one canonical key.
pub fn derive_point_identity(
    key: ProjectionPointKey,
    limits: PointIdentityLimits,
) -> Result<PointIdentity, PointIdentityError> {
    let canonical = canonical_point_key_bytes(&key, limits)?;
    let full_digest = point_identity_digest(canonical.as_slice());
    let point_id = project_qdrant_uuid(full_digest);
    Ok(PointIdentity {
        point_id,
        full_digest,
        key,
    })
}

/// Compares an expected identity with an existing occupant.
#[must_use]
pub fn compare_existing_identity(
    expected: &PointIdentity,
    observed: Option<&ExistingPointIdentity>,
) -> CollisionDecision {
    let Some(observed) = observed else {
        return CollisionDecision::Vacant;
    };
    if observed.point_id == expected.point_id
        && observed.full_digest == expected.full_digest
        && observed.key == expected.key
    {
        CollisionDecision::SameFullIdentity
    } else {
        CollisionDecision::CollisionBlock
    }
}

/// Verifies UUID, full digest and every canonical identity field.
pub fn validate_identity_fields(
    expected: &PointIdentity,
    observed: &ExistingPointIdentity,
) -> Result<(), PointIdentityError> {
    if compare_existing_identity(expected, Some(observed))
        == CollisionDecision::SameFullIdentity
    {
        Ok(())
    } else {
        Err(PointIdentityError::IdentityMismatch)
    }
}

/// Compatibility validation for two derived identities.
pub fn validate_identity_payload(
    expected: &PointIdentity,
    observed: &PointIdentity,
) -> Result<(), PointIdentityError> {
    let observed = ExistingPointIdentity {
        point_id: observed.point_id,
        full_digest: observed.full_digest,
        key: observed.key.clone(),
    };
    validate_identity_fields(expected, &observed)
}

/// Finite collision registry required before publishing compact UUIDs.
#[derive(Clone, Debug)]
pub struct PointIdentityRegistry {
    max_points: usize,
    by_id: BTreeMap<PointId128, (PointIdentityDigest, ProjectionPointKey)>,
    by_key: BTreeMap<ProjectionPointKey, PointId128>,
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

    /// Registers or exactly replays one identity.
    pub fn register(
        &mut self,
        identity: PointIdentity,
    ) -> Result<PointId128, PointIdentityError> {
        if let Some((digest, key)) = self.by_id.get(&identity.point_id) {
            if digest != &identity.full_digest || key != &identity.key {
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
        self.by_id.insert(
            identity.point_id,
            (identity.full_digest, identity.key),
        );
        Ok(identity.point_id)
    }

    /// Returns the complete key for one point UUID.
    pub fn key(
        &self,
        point_id: PointId128,
    ) -> Result<&ProjectionPointKey, PointIdentityError> {
        self.by_id
            .get(&point_id)
            .map(|(_, key)| key)
            .ok_or(PointIdentityError::PointNotFound)
    }

    /// Returns the full digest for one point UUID.
    pub fn digest(
        &self,
        point_id: PointId128,
    ) -> Result<PointIdentityDigest, PointIdentityError> {
        self.by_id
            .get(&point_id)
            .map(|(digest, _)| *digest)
            .ok_or(PointIdentityError::PointNotFound)
    }

    /// Number of registered identities.
    #[must_use]
    pub fn len(&self) -> usize {
        self.by_id.len()
    }

    /// Whether no identities are registered.
    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.by_id.is_empty()
    }
}

fn encode_unsigned(
    output: &mut Vec<u8>,
    value: u64,
    limits: PointIdentityLimits,
) -> Result<(), PointIdentityError> {
    encode_major_length(output, 0, value, limits)
}

fn encode_bytes(
    output: &mut Vec<u8>,
    value: &[u8],
    limits: PointIdentityLimits,
) -> Result<(), PointIdentityError> {
    let length = u64::try_from(value.len()).map_err(|_| PointIdentityError::LengthOverflow)?;
    encode_major_length(output, 2, length, limits)?;
    push_checked(output, value, limits)
}

fn encode_text(
    output: &mut Vec<u8>,
    value: &str,
    limits: PointIdentityLimits,
) -> Result<(), PointIdentityError> {
    let length = u64::try_from(value.len()).map_err(|_| PointIdentityError::LengthOverflow)?;
    encode_major_length(output, 3, length, limits)?;
    push_checked(output, value.as_bytes(), limits)
}

fn encode_major_length(
    output: &mut Vec<u8>,
    major: u8,
    value: u64,
    limits: PointIdentityLimits,
) -> Result<(), PointIdentityError> {
    let prefix = major << 5;
    match value {
        0..=23 => push_checked(output, &[prefix | u8::try_from(value).unwrap_or(0)], limits),
        24..=0xff => push_checked(
            output,
            &[prefix | 24, u8::try_from(value).map_err(|_| PointIdentityError::LengthOverflow)?],
            limits,
        ),
        0x100..=0xffff => {
            push_checked(output, &[prefix | 25], limits)?;
            push_checked(
                output,
                &u16::try_from(value)
                    .map_err(|_| PointIdentityError::LengthOverflow)?
                    .to_be_bytes(),
                limits,
            )
        }
        0x1_0000..=0xffff_ffff => {
            push_checked(output, &[prefix | 26], limits)?;
            push_checked(
                output,
                &u32::try_from(value)
                    .map_err(|_| PointIdentityError::LengthOverflow)?
                    .to_be_bytes(),
                limits,
            )
        }
        _ => {
            push_checked(output, &[prefix | 27], limits)?;
            push_checked(output, &value.to_be_bytes(), limits)
        }
    }
}

fn push_checked(
    output: &mut Vec<u8>,
    bytes: &[u8],
    limits: PointIdentityLimits,
) -> Result<(), PointIdentityError> {
    let new_len = output
        .len()
        .checked_add(bytes.len())
        .ok_or(PointIdentityError::LengthOverflow)?;
    if new_len > limits.max_canonical_bytes {
        return Err(PointIdentityError::CanonicalBytesExceeded);
    }
    output.extend_from_slice(bytes);
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn key(profile: &str) -> ProjectionPointKey {
        ProjectionPointKey {
            schema_version: POINT_KEY_SCHEMA_VERSION,
            installation_incarnation_id:
                InstallationIncarnationId::from_bytes([1; 16]),
            collection_generation_id:
                CollectionGenerationId::from_bytes([2; 16]),
            projection_membership_id:
                ProjectionMembershipId::from_bytes([3; 16]),
            representation_id: RepresentationId::from_bytes([4; 16]),
            unit_id: UnitId::from_bytes([5; 16]),
            projection_profile_set_id: ProjectionProfileSetId::new(profile)
                .expect("profile"),
            point_role: PointRole::Unit,
        }
    }

    #[test]
    fn same_key_same_identity_and_full_digest() {
        let first = derive_point_identity(
            key("lex-code-v1"),
            DEFAULT_POINT_IDENTITY_LIMITS,
        )
        .expect("first");
        let second = derive_point_identity(
            key("lex-code-v1"),
            DEFAULT_POINT_IDENTITY_LIMITS,
        )
        .expect("second");
        assert_eq!(first, second);
        assert_eq!(first.full_digest.as_bytes().len(), 32);
        assert_eq!(first.point_id.to_hyphenated().len(), 36);
    }

    #[test]
    fn profile_or_role_change_changes_identity() {
        let baseline = derive_point_identity(
            key("lex-code-v1"),
            DEFAULT_POINT_IDENTITY_LIMITS,
        )
        .expect("baseline");
        let profile = derive_point_identity(
            key("lex-code-v2"),
            DEFAULT_POINT_IDENTITY_LIMITS,
        )
        .expect("profile");
        let mut relation_key = key("lex-code-v1");
        relation_key.point_role = PointRole::Relation;
        let relation = derive_point_identity(
            relation_key,
            DEFAULT_POINT_IDENTITY_LIMITS,
        )
        .expect("relation");
        assert_ne!(baseline.full_digest, profile.full_digest);
        assert_ne!(baseline.point_id, profile.point_id);
        assert_ne!(baseline.point_id, relation.point_id);
    }

    #[test]
    fn canonical_cbor_shape_is_frozen() {
        let encoded = canonical_point_key_bytes(
            &key("p"),
            DEFAULT_POINT_IDENTITY_LIMITS,
        )
        .expect("canonical");
        assert_eq!(encoded.as_slice()[0], 0x88);
        assert_eq!(encoded.as_slice()[1], 0x01);
        assert_eq!(encoded.as_slice()[2], 0x50);
        assert_eq!(encoded.as_slice().last(), Some(&0x00));
        assert_eq!(
            point_identity_digest(encoded.as_slice()),
            point_identity_digest(encoded.as_slice())
        );
    }

    #[test]
    fn unsupported_version_and_finite_limits_fail_closed() {
        let mut unsupported = key("p");
        unsupported.schema_version = 2;
        assert_eq!(
            derive_point_identity(
                unsupported,
                DEFAULT_POINT_IDENTITY_LIMITS
            ),
            Err(PointIdentityError::UnsupportedKeyVersion)
        );
        let limits = PointIdentityLimits {
            max_identifier_bytes: 1,
            ..DEFAULT_POINT_IDENTITY_LIMITS
        };
        assert_eq!(
            derive_point_identity(key("too-long"), limits),
            Err(PointIdentityError::IdentifierTooLong)
        );
    }

    #[test]
    fn fake_truncated_uuid_collision_never_overwrites() {
        let first = derive_point_identity(
            key("lex-code-v1"),
            DEFAULT_POINT_IDENTITY_LIMITS,
        )
        .expect("first");
        let mut forged = derive_point_identity(
            key("lex-code-v2"),
            DEFAULT_POINT_IDENTITY_LIMITS,
        )
        .expect("forged");
        forged.point_id = first.point_id;
        let observed = ExistingPointIdentity {
            point_id: forged.point_id,
            full_digest: forged.full_digest,
            key: forged.key.clone(),
        };
        assert_eq!(
            compare_existing_identity(&first, Some(&observed)),
            CollisionDecision::CollisionBlock
        );
        let mut registry = PointIdentityRegistry::new(
            DEFAULT_POINT_IDENTITY_LIMITS,
        )
        .expect("registry");
        registry.register(first).expect("first");
        assert_eq!(
            registry.register(forged),
            Err(PointIdentityError::DigestCollision)
        );
    }

    #[test]
    fn registry_exact_replay_is_idempotent_and_finite() {
        let limits = PointIdentityLimits {
            max_registered_points: 1,
            ..DEFAULT_POINT_IDENTITY_LIMITS
        };
        let first = derive_point_identity(key("one"), limits).expect("first");
        let second = derive_point_identity(key("two"), limits).expect("second");
        let mut registry = PointIdentityRegistry::new(limits).expect("registry");
        assert_eq!(
            registry.register(first.clone()).expect("first"),
            first.point_id
        );
        assert_eq!(
            registry.register(first.clone()).expect("replay"),
            first.point_id
        );
        assert_eq!(
            registry.register(second),
            Err(PointIdentityError::RegistryCapacityExceeded)
        );
        assert_eq!(registry.digest(first.point_id).expect("digest"), first.full_digest);
    }
}

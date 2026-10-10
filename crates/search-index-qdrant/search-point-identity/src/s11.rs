//! Stateless, bounded Architecture 8.4 S11 identity for new generations.
//!
//! The root-level legacy exports are never consulted by this profile.

use core::fmt;
use search_contracts::{
    Blake3Digest32, BoundedBytes, BoundedMap, CanonicalBytes, CanonicalDigestDomain, CanonicalKey,
    CanonicalText, CanonicalValue, CollectionGenerationId, DigestInputLimit,
    InstallationIncarnationId, ProjectionMembershipId, ProjectionProfileSetId, RepresentationId,
    UnitId, blake3_canonical, to_canonical_cbor,
};

mod address;
mod decode;

pub use address::PointId128;

/// Only accepted S11 key schema version.
pub const POINT_KEY_SCHEMA_VERSION: u16 = 1;
/// Frozen S11 computation profile revision; distinct from the legacy profile.
pub const S11_PROFILE_REVISION: u16 = 1;
/// Full key identity domain, including its representation and revision.
pub const IDENTITY_DOMAIN: &str = "eliot/cbor/point-identity/v1";
/// Independent raw-digest address projection domain.
pub const ADDRESS_DOMAIN: &str = "eliot/raw/point-address/v1";

/// Frozen ceilings; resource budgets may narrow them without changing bytes.
pub const DEFAULT_LIMITS: PointIdentityLimits = PointIdentityLimits {
    max_profile_id_bytes: 256,
    max_canonical_bytes: 1024,
    max_digest_preimage_bytes: 1088,
};

/// Content-free failure at the S11 identity boundary.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum PointIdError {
    /// A limit is zero or exceeds the frozen profile.
    InvalidLimits,
    /// The profile identifier exceeds the admitted budget.
    IdentifierTooLong,
    /// Key bytes exceed the admitted canonical ceiling.
    CanonicalBytesExceeded,
    /// Complete domain, separator and payload exceed the digest budget.
    DigestPreimageExceeded,
    /// A key is malformed, noncanonical or has an unexpected field/type.
    CanonicalEncodingMismatch,
    /// Only key schema version 1 is admitted.
    PointKeyVersionUnsupported,
    /// Only the three closed S11 roles are admitted.
    InvalidPointRole,
    /// The observed point has another address and cannot authorize this write.
    ForeignAddress,
    /// The address is not a canonical byte-preserving UUID spelling.
    InvalidUuid,
}

impl PointIdError {
    /// Stable machine-readable refusal reason.
    pub const fn code(self) -> &'static str {
        match self {
            Self::InvalidLimits => "POINT_ID_INVALID_LIMITS",
            Self::IdentifierTooLong => "POINT_ID_IDENTIFIER_TOO_LONG",
            Self::CanonicalBytesExceeded => "POINT_ID_CANONICAL_BYTES_EXCEEDED",
            Self::DigestPreimageExceeded => "POINT_ID_PREIMAGE_EXCEEDED",
            Self::CanonicalEncodingMismatch => "CANONICAL_ENCODING_MISMATCH",
            Self::PointKeyVersionUnsupported => "POINT_KEY_VERSION_UNSUPPORTED",
            Self::InvalidPointRole => "INVALID_POINT_ROLE",
            Self::ForeignAddress => "FOREIGN_POINT_ADDRESS",
            Self::InvalidUuid => "POINT_ID_INVALID_UUID",
        }
    }
}

impl fmt::Display for PointIdError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.code())
    }
}

impl std::error::Error for PointIdError {}

/// Finite budgets for the frozen profile, never caller-selected algorithms.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct PointIdentityLimits {
    /// UTF-8 bytes in the immutable profile-set identifier.
    pub max_profile_id_bytes: usize,
    /// Canonical key bytes, excluding the digest domain and separator.
    pub max_canonical_bytes: usize,
    /// Complete preimage bytes for each of the two digest computations.
    pub max_digest_preimage_bytes: usize,
}

impl PointIdentityLimits {
    /// Checks nonzero budgets against the frozen upper ceilings.
    pub const fn validate(self) -> Result<Self, PointIdError> {
        if self.max_profile_id_bytes == 0
            || self.max_profile_id_bytes > DEFAULT_LIMITS.max_profile_id_bytes
            || self.max_canonical_bytes == 0
            || self.max_canonical_bytes > DEFAULT_LIMITS.max_canonical_bytes
            || self.max_digest_preimage_bytes == 0
            || self.max_digest_preimage_bytes > DEFAULT_LIMITS.max_digest_preimage_bytes
        {
            Err(PointIdError::InvalidLimits)
        } else {
            Ok(self)
        }
    }
}

impl Default for PointIdentityLimits {
    fn default() -> Self {
        DEFAULT_LIMITS
    }
}

/// Closed role in the eight-field S11 key.
#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub enum PointRole {
    /// One unit carrying its immutable required named-vector set.
    Unit,
    /// A relation bound to the exact unit occurrence.
    Relation,
    /// An auxiliary point required by the immutable projection profile.
    Auxiliary,
}

impl PointRole {
    /// Exact case-sensitive wire spelling.
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Unit => "unit",
            Self::Relation => "relation",
            Self::Auxiliary => "auxiliary",
        }
    }

    /// Rejects every spelling outside the closed role set.
    pub fn parse(value: &str) -> Result<Self, PointIdError> {
        match value {
            "unit" => Ok(Self::Unit),
            "relation" => Ok(Self::Relation),
            "auxiliary" => Ok(Self::Auxiliary),
            _ => Err(PointIdError::InvalidPointRole),
        }
    }
}

/// Exact immutable S11 key; mutable and source/scoring coordinates are absent.
#[derive(Clone, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub struct ProjectionPointKey {
    /// Key schema version, exactly 1.
    pub schema_version: u16,
    /// Installation incarnation, independent of physical root paths.
    pub installation_incarnation_id: InstallationIncarnationId,
    /// New immutable collection generation.
    pub collection_generation_id: CollectionGenerationId,
    /// Exactly one projection membership.
    pub projection_membership_id: ProjectionMembershipId,
    /// Exact representation identity.
    pub representation_id: RepresentationId,
    /// Exact unit occurrence identity.
    pub unit_id: UnitId,
    /// Immutable required projection/profile set.
    pub projection_profile_set_id: ProjectionProfileSetId,
    /// Closed unit, relation or auxiliary role.
    pub point_role: PointRole,
}

impl ProjectionPointKey {
    /// Checks version and identifier budget before canonical assembly.
    pub fn validate(&self, limits: PointIdentityLimits) -> Result<(), PointIdError> {
        let limits = limits.validate()?;
        if self.schema_version != POINT_KEY_SCHEMA_VERSION {
            return Err(PointIdError::PointKeyVersionUnsupported);
        }
        if self.projection_profile_set_id.as_str().len() > limits.max_profile_id_bytes {
            return Err(PointIdError::IdentifierTooLong);
        }
        Ok(())
    }

    /// Assembles eight typed values for the sole shared encoder.
    pub fn to_canonical_value(
        &self,
        limits: PointIdentityLimits,
    ) -> Result<CanonicalValue, PointIdError> {
        self.validate(limits)?;
        let fields = [
            (
                "schema_version",
                CanonicalValue::U64(u64::from(self.schema_version)),
            ),
            (
                "installation_incarnation_id",
                uuid(self.installation_incarnation_id.as_bytes())?,
            ),
            (
                "collection_generation_id",
                uuid(self.collection_generation_id.as_bytes())?,
            ),
            (
                "projection_membership_id",
                uuid(self.projection_membership_id.as_bytes())?,
            ),
            (
                "representation_id",
                uuid(self.representation_id.as_bytes())?,
            ),
            ("unit_id", uuid(self.unit_id.as_bytes())?),
            (
                "projection_profile_set_id",
                text(self.projection_profile_set_id.as_str())?,
            ),
            ("point_role", text(self.point_role.as_str())?),
        ];
        let entries = fields
            .into_iter()
            .map(|(name, value)| {
                CanonicalKey::new_non_empty(name)
                    .map(|key| (key, value))
                    .map_err(|_| PointIdError::CanonicalEncodingMismatch)
            })
            .collect::<Result<Vec<_>, _>>()?;
        BoundedMap::from_entries(entries)
            .map(CanonicalValue::Object)
            .map_err(|_| PointIdError::CanonicalEncodingMismatch)
    }

    /// Encodes deterministic CBOR, enforcing this profile's finite key budget.
    pub fn canonical_bytes(
        &self,
        limits: PointIdentityLimits,
    ) -> Result<CanonicalBytes, PointIdError> {
        let value = self.to_canonical_value(limits)?;
        let bytes =
            to_canonical_cbor(&value).map_err(|_| PointIdError::CanonicalEncodingMismatch)?;
        if bytes.len() > limits.max_canonical_bytes {
            return Err(PointIdError::CanonicalBytesExceeded);
        }
        Ok(bytes)
    }
}

fn uuid(value: &[u8; 16]) -> Result<CanonicalValue, PointIdError> {
    BoundedBytes::new(value.to_vec())
        .map(CanonicalValue::Bytes)
        .map_err(|_| PointIdError::CanonicalEncodingMismatch)
}

fn text(value: &str) -> Result<CanonicalValue, PointIdError> {
    CanonicalText::new(value)
        .map(CanonicalValue::Text)
        .map_err(|_| PointIdError::CanonicalEncodingMismatch)
}

/// Computed evidence, constructible only through the S11 derivation function.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct PointIdentity {
    key: ProjectionPointKey,
    full_digest: Blake3Digest32,
    point_id: PointId128,
}

impl PointIdentity {
    /// Independently represented, exact immutable key coordinates.
    pub const fn key(&self) -> &ProjectionPointKey {
        &self.key
    }

    /// Full 256-bit BLAKE3 identity; the compact address is not a substitute.
    pub const fn full_digest(&self) -> Blake3Digest32 {
        self.full_digest
    }

    /// Byte-preserving 128-bit address.
    pub const fn point_id(&self) -> PointId128 {
        self.point_id
    }
}

/// Untrusted stored readback; its coordinates must not be inferred from digest.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ObservedPointIdentity {
    /// Independently obtained key from exact generation/manifest and payload.
    pub key: ProjectionPointKey,
    /// Stored full identity digest, still requiring comparison.
    pub full_digest: Blake3Digest32,
    /// Exact address of the observed stored point.
    pub point_id: PointId128,
}

/// Derives full identity and a separately domain-bound address without state.
pub fn derive_point_identity(
    key: &ProjectionPointKey,
    limits: PointIdentityLimits,
) -> Result<PointIdentity, PointIdError> {
    let value = key.to_canonical_value(limits)?;
    let bytes = to_canonical_cbor(&value).map_err(|_| PointIdError::CanonicalEncodingMismatch)?;
    if bytes.len() > limits.max_canonical_bytes {
        return Err(PointIdError::CanonicalBytesExceeded);
    }
    let domain = CanonicalDigestDomain::parse(IDENTITY_DOMAIN)
        .map_err(|_| PointIdError::CanonicalEncodingMismatch)?;
    if domain.as_str().len() + 1 + bytes.len() > limits.max_digest_preimage_bytes {
        return Err(PointIdError::DigestPreimageExceeded);
    }
    if ADDRESS_DOMAIN.len() + 1 + 32 > limits.max_digest_preimage_bytes {
        return Err(PointIdError::DigestPreimageExceeded);
    }
    let limit = DigestInputLimit::new(limits.max_digest_preimage_bytes)
        .map_err(|_| PointIdError::InvalidLimits)?;
    let full_digest = blake3_canonical(&domain, &value, limit)
        .map_err(|_| PointIdError::CanonicalEncodingMismatch)?;
    let point_id = address::project_address(&full_digest, limit)?;
    Ok(PointIdentity {
        key: key.clone(),
        full_digest,
        point_id,
    })
}

/// Pure pre-upsert result; collision refusal never authorizes an overwrite.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum CollisionDecision {
    /// No existing point was read at this address; creation is possible.
    Vacant,
    /// Address, full digest and every exact coordinate agree.
    SameFullIdentity,
    /// The same address contains different full identity evidence.
    CollisionBlock,
}

impl CollisionDecision {
    /// Stable machine-readable result; a collision blocks publication.
    pub const fn code(self) -> &'static str {
        match self {
            Self::Vacant => "POINT_ID_VACANT",
            Self::SameFullIdentity => "POINT_ID_SAME_FULL_IDENTITY",
            Self::CollisionBlock => "POINT_ID_COLLISION",
        }
    }
}

/// Compares exact stored evidence without a registry or mutation.
pub fn compare_existing_identity(
    expected: &PointIdentity,
    observed: Option<&ObservedPointIdentity>,
    limits: PointIdentityLimits,
) -> Result<CollisionDecision, PointIdError> {
    // Revalidate the computation under the caller's current finite budget.
    if derive_point_identity(expected.key(), limits)? != *expected {
        return Err(PointIdError::CanonicalEncodingMismatch);
    }
    let Some(observed) = observed else {
        return Ok(CollisionDecision::Vacant);
    };
    if observed.point_id != expected.point_id {
        return Err(PointIdError::ForeignAddress);
    }
    if observed.full_digest != expected.full_digest || observed.key != expected.key {
        return Ok(CollisionDecision::CollisionBlock);
    }
    Ok(CollisionDecision::SameFullIdentity)
}

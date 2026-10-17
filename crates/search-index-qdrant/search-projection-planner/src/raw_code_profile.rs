//! Immutable C13 identity for the current raw-code lexical projection profile.

use search_contracts::indexed::NamedVectorRequirement;
use search_contracts::{
    Blake3Digest32, BoundedBytes, BoundedList, CanonicalDigestDomain, CanonicalText,
    CanonicalValue, DigestInputLimit, MAX_CANONICAL_BYTES, NonZeroRevision, OpaqueId, ProfileId,
    ProjectionProfileSetId, blake3_canonical,
};

use crate::ProjectionError;

const RAW_CODE_PROFILE_SET_DOMAIN: &str = "eliot/cbor/raw-code-projection-profile-set/v1";
const RAW_CODE_PROFILE_SET_PREFIX: &str = "raw-code-c13-v1-";

/// Full immutable C13 profile-set description for one raw-code collection generation.
///
/// The identity is a content commitment, not profile acceptance, registry
/// admission, or publication authority. Callers must supply identities read
/// from the C11, materializer, unitizer, and shared collection-schema owners.
/// Under S9.4, a changed profile-set ID requires a new collection generation;
/// this descriptor does not authorize that transition.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct RawCodeProjectionProfileSetDescriptor {
    profile_set_id: ProjectionProfileSetId,
    identity_digest: Blake3Digest32,
    code_profile_id: OpaqueId,
    code_profile_revision: NonZeroRevision,
    code_profile_fingerprint: Blake3Digest32,
    artifact_set_identity: Blake3Digest32,
    compatibility_fixture_identity: Blake3Digest32,
    collision_fixture_identity: Blake3Digest32,
    vector_requirement: NamedVectorRequirement,
    materializer_profile_identity: Blake3Digest32,
    unitizer_profile_id: ProfileId,
}

impl RawCodeProjectionProfileSetDescriptor {
    /// Builds the versioned descriptor from exact owner-produced profile identities.
    ///
    /// The profile-set ID is the complete lower-case hex BLAKE3 digest of a
    /// canonical CBOR value under `eliot/cbor/raw-code-projection-profile-set/v1`,
    /// prefixed by `raw-code-c13-v1-`. The canonical value is:
    ///
    /// ```text
    /// [1,
    ///  [C11 profile ID, revision, full fingerprint, artifact-set digest,
    ///   compatibility-fixture digest, collision-fixture digest],
    ///  complete named-vector requirement,
    ///  materializer profile digest,
    ///  verified unitizer profile ID]
    /// ```
    ///
    /// This computes identity only. It does not verify external acceptance or
    /// authorize a collection generation.
    #[allow(clippy::too_many_arguments)]
    pub fn new(
        code_profile_id: OpaqueId,
        code_profile_revision: NonZeroRevision,
        code_profile_fingerprint: Blake3Digest32,
        artifact_set_identity: Blake3Digest32,
        compatibility_fixture_identity: Blake3Digest32,
        collision_fixture_identity: Blake3Digest32,
        vector_requirement: NamedVectorRequirement,
        materializer_profile_identity: Blake3Digest32,
        unitizer_profile_id: ProfileId,
    ) -> Result<Self, ProjectionError> {
        let code_profile = CanonicalValue::Array(
            BoundedList::new(vec![
                canonical_text(code_profile_id.as_str())?,
                CanonicalValue::U64(code_profile_revision.get()),
                canonical_digest(code_profile_fingerprint)?,
                canonical_digest(artifact_set_identity)?,
                canonical_digest(compatibility_fixture_identity)?,
                canonical_digest(collision_fixture_identity)?,
            ])
            .map_err(|_| ProjectionError::ProfileSetMismatch)?,
        );
        let value = CanonicalValue::Array(
            BoundedList::new(vec![
                CanonicalValue::U64(1),
                code_profile,
                vector_requirement
                    .to_canonical_value()
                    .map_err(|_| ProjectionError::ProfileSetMismatch)?,
                canonical_digest(materializer_profile_identity)?,
                canonical_text(unitizer_profile_id.as_str())?,
            ])
            .map_err(|_| ProjectionError::ProfileSetMismatch)?,
        );
        let domain = CanonicalDigestDomain::parse(RAW_CODE_PROFILE_SET_DOMAIN)
            .map_err(|_| ProjectionError::ProfileSetMismatch)?;
        let limit = DigestInputLimit::new(MAX_CANONICAL_BYTES)
            .map_err(|_| ProjectionError::ProfileSetMismatch)?;
        let digest = blake3_canonical(&domain, &value, limit)
            .map_err(|_| ProjectionError::ProfileSetMismatch)?;
        let profile_set_id = ProjectionProfileSetId::new(format!(
            "{RAW_CODE_PROFILE_SET_PREFIX}{}",
            lower_hex(digest.as_bytes())
        ))
        .map_err(|_| ProjectionError::ProfileSetMismatch)?;

        Ok(Self {
            profile_set_id,
            identity_digest: digest,
            code_profile_id,
            code_profile_revision,
            code_profile_fingerprint,
            artifact_set_identity,
            compatibility_fixture_identity,
            collision_fixture_identity,
            vector_requirement,
            materializer_profile_identity,
            unitizer_profile_id,
        })
    }

    /// The full content-derived profile-set ID.
    #[must_use]
    pub const fn profile_set_id(&self) -> &ProjectionProfileSetId {
        &self.profile_set_id
    }

    /// Full canonical descriptor digest computed by this owner before rendering
    /// the profile-set identifier. This is data, not profile qualification.
    #[must_use]
    pub const fn identity_digest(&self) -> Blake3Digest32 {
        self.identity_digest
    }

    /// C11 profile ID bound by this descriptor.
    #[must_use]
    pub const fn code_profile_id(&self) -> &OpaqueId {
        &self.code_profile_id
    }

    /// C11 profile revision bound by this descriptor.
    #[must_use]
    pub const fn code_profile_revision(&self) -> NonZeroRevision {
        self.code_profile_revision
    }

    /// Full C11 behavior fingerprint bound by this descriptor.
    #[must_use]
    pub const fn code_profile_fingerprint(&self) -> Blake3Digest32 {
        self.code_profile_fingerprint
    }

    /// Exact C11 provider artifact-set identity.
    #[must_use]
    pub const fn artifact_set_identity(&self) -> Blake3Digest32 {
        self.artifact_set_identity
    }

    /// Exact C11 document/query compatibility fixture-definition identity.
    #[must_use]
    pub const fn compatibility_fixture_identity(&self) -> Blake3Digest32 {
        self.compatibility_fixture_identity
    }

    /// Exact C11 collision fixture-definition identity.
    #[must_use]
    pub const fn collision_fixture_identity(&self) -> Blake3Digest32 {
        self.collision_fixture_identity
    }

    /// Complete shared named-vector requirement bound by this descriptor.
    #[must_use]
    pub const fn vector_requirement(&self) -> &NamedVectorRequirement {
        &self.vector_requirement
    }

    /// Exact materializer profile identity bound by this descriptor.
    #[must_use]
    pub const fn materializer_profile_identity(&self) -> Blake3Digest32 {
        self.materializer_profile_identity
    }

    /// Exact verified unitizer profile identity bound by this descriptor.
    #[must_use]
    pub const fn unitizer_profile_id(&self) -> &ProfileId {
        &self.unitizer_profile_id
    }

    /// Checks an actual C11 encoding receipt against this descriptor's profile identity.
    #[must_use]
    pub fn matches_encoding_profile(
        &self,
        profile_id: &OpaqueId,
        profile_revision: NonZeroRevision,
        profile_fingerprint: Blake3Digest32,
        analyzer_fingerprint: Blake3Digest32,
    ) -> bool {
        self.code_profile_id == *profile_id
            && self.code_profile_revision == profile_revision
            && self.code_profile_fingerprint == profile_fingerprint
            && self.code_profile_fingerprint == analyzer_fingerprint
    }
}

fn canonical_text(value: &str) -> Result<CanonicalValue, ProjectionError> {
    CanonicalText::new(value.to_owned())
        .map(CanonicalValue::Text)
        .map_err(|_| ProjectionError::ProfileSetMismatch)
}

fn canonical_digest(value: Blake3Digest32) -> Result<CanonicalValue, ProjectionError> {
    BoundedBytes::new(value.as_bytes().to_vec())
        .map(CanonicalValue::Bytes)
        .map_err(|_| ProjectionError::ProfileSetMismatch)
}

fn lower_hex(bytes: &[u8; 32]) -> String {
    const HEX: &[u8; 16] = b"0123456789abcdef";
    let mut output = String::with_capacity(64);
    for byte in bytes {
        output.push(char::from(HEX[usize::from(*byte >> 4)]));
        output.push(char::from(HEX[usize::from(*byte & 0x0f)]));
    }
    output
}

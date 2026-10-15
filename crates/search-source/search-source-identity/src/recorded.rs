//! Metadata-only C04 canonical-path replay for retained source records.

use search_contracts::RootBindingId;

use super::{
    CanonicalPathKey, CaseBehavior, FilesystemIdentityProfile, IdentityError,
    validate_relative_lookup_path,
};

/// Reconstitutes a canonical path key from recorded C04 metadata.
///
/// The caller must have compared the persisted `SourceBinding` path with the
/// qualified producer's `ValidatedIdentityObservation.path_key` before the
/// binding was persisted. This function performs no native observation,
/// normalization attestation, currentness check, or access grant.
pub fn reconstitute_recorded_canonical_path_key(
    root_binding_id: RootBindingId,
    lookup_path: &str,
    profile: FilesystemIdentityProfile,
) -> Result<CanonicalPathKey, IdentityError> {
    validate_relative_lookup_path(lookup_path)?;
    if profile.case_behavior() == CaseBehavior::InsensitiveAscii
        && lookup_path != lookup_path.to_ascii_lowercase()
    {
        return Err(IdentityError::IdentityObservationInvalid);
    }
    Ok(CanonicalPathKey {
        root_binding_id,
        profile_revision: profile.revision(),
        profile_schema_digest: profile.schema_digest(),
        lookup_path: lookup_path.to_owned(),
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use search_contracts::{Blake3Digest32, NonZeroRevision};
    use crate::evidence::{
        LinkBehavior, PathObservation, ReparseBehavior, StableFieldPolicy, UnicodeBehavior,
        derive_canonical_path_key,
    };

    fn profile() -> FilesystemIdentityProfile {
        FilesystemIdentityProfile::new(
            NonZeroRevision::new(4).expect("revision"),
            Blake3Digest32::from_bytes([7; 32]),
            CaseBehavior::PreNormalizedInsensitive,
            UnicodeBehavior::PreNormalizedNfc,
            StableFieldPolicy::Required,
            StableFieldPolicy::Required,
            LinkBehavior::StablePhysicalIdentity,
            ReparseBehavior::FinalTargetIdentity,
        )
        .expect("profile")
    }

    #[test]
    fn replayed_prenormalized_metadata_is_not_fresh_native_attestation() {
        let profile = profile();
        let fresh = PathObservation {
            root_binding_id: RootBindingId::from_bytes([1; 16]),
            root_relative_lookup_path: "src/lib.rs".into(),
            profile_revision: profile.revision(),
            profile_schema_digest: profile.schema_digest(),
            normalization_attested: false,
        };
        assert_eq!(
            derive_canonical_path_key(&fresh, profile),
            Err(IdentityError::IdentityObservationInvalid)
        );
        let replayed = reconstitute_recorded_canonical_path_key(
            RootBindingId::from_bytes([1; 16]),
            "src/lib.rs",
            profile,
        )
        .expect("recorded key");
        assert_eq!(replayed.as_str(), "src/lib.rs");
        assert_eq!(replayed.profile_revision(), profile.revision());
    }
}

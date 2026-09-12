//! Adapter from the daemon's qualified SHA-256 primitive to identity ownership.

use search_source_identity::{
    GitIdentityDigest, LegacyDigestPriorIdentity, LegacyIdentityDigest,
    derive_git_stable_identity_digest, derive_legacy_digest_revision_id,
    derive_legacy_digest_source_id, resolve_legacy_digest_identity,
};

use crate::sha256;

pub(super) struct DirectIdentityDigest;

impl LegacyIdentityDigest for DirectIdentityDigest {
    fn digest_parts(domain: &[u8], parts: &[&[u8]]) -> [u8; 32] {
        sha256::digest_parts(domain, parts)
    }
}

impl GitIdentityDigest for DirectIdentityDigest {
    fn digest_parts(domain: &[u8], parts: &[&[u8]]) -> [u8; 32] {
        sha256::digest_parts(domain, parts)
    }
}

pub(super) fn resolve_source_id(
    namespace_digest: &str,
    stable_identity_digest: &str,
    identity_strength: &str,
    prior: &[LegacyDigestPriorIdentity],
) -> Result<String, String> {
    let resolution = resolve_legacy_digest_identity(
        stable_identity_digest,
        identity_strength,
        prior,
    )
    .map_err(|error| error.code().to_owned())?;
    derive_legacy_digest_source_id::<DirectIdentityDigest>(
        namespace_digest,
        stable_identity_digest,
        &resolution,
    )
    .map_err(|error| error.code().to_owned())
}

pub(super) fn derive_revision_id(
    source_id: &str,
    content_digest: &str,
    byte_length: u64,
) -> Result<String, String> {
    derive_legacy_digest_revision_id::<DirectIdentityDigest>(
        source_id,
        content_digest,
        byte_length,
    )
    .map_err(|error| error.code().to_owned())
}

pub(crate) fn derive_git_stable_digest(
    repository_identity_digest: &[u8; 32],
    object_id: &[u8; 20],
) -> [u8; 32] {
    derive_git_stable_identity_digest::<DirectIdentityDigest>(
        repository_identity_digest,
        object_id,
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn path_bound_identity_never_creates_a_source() {
        let result = resolve_source_id(
            &"11".repeat(32),
            &"22".repeat(32),
            "path-bound",
            &[],
        );
        assert_eq!(result, Err("SOURCE_IDENTITY_AMBIGUOUS".to_owned()));
    }

    #[test]
    fn exact_prior_identity_is_preserved() {
        let prior = vec![LegacyDigestPriorIdentity {
            source_id: "33".repeat(32),
            stable_identity_digest: "22".repeat(32),
        }];
        let source = resolve_source_id(
            &"11".repeat(32),
            &"22".repeat(32),
            "native",
            &prior,
        )
        .expect("exact prior identity");
        assert_eq!(source, "33".repeat(32));
    }

    #[test]
    fn git_stable_digest_changes_with_repository_or_object() {
        let repository = [0x11_u8; 32];
        let object = [0x22_u8; 20];
        let first = derive_git_stable_digest(&repository, &object);
        assert_eq!(first, derive_git_stable_digest(&repository, &object));
        assert_ne!(first, derive_git_stable_digest(&[0x12_u8; 32], &object));
        assert_ne!(first, derive_git_stable_digest(&repository, &[0x23_u8; 20]));
    }
}

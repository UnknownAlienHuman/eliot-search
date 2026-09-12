//! Compatibility digest binding for no-execute Git object identity.
//!
//! The source-identity owner defines the domain and framing. Integration code
//! supplies the already-qualified digest primitive plus bounded repository and
//! object identity bytes. This module performs no Git, filesystem, network,
//! registry, clock or content I/O.

/// Domain binding one admitted repository identity to one exact Git object ID.
pub const GIT_STABLE_IDENTITY_DOMAIN: &[u8] =
    b"eliot-searchd/git-stable-identity/v1";

/// Qualified digest primitive supplied by the integration boundary.
///
/// The identity package owns framing and semantics without adding a second
/// cryptographic implementation or dependency.
pub trait GitIdentityDigest {
    /// Domain-separated digest over ordered, length-prefixed parts.
    fn digest_parts(domain: &[u8], parts: &[&[u8]]) -> [u8; 32];
}

/// Derives the exact stable digest for one admitted repository and Git object.
///
/// Repository paths, remote URLs, repository names, refs, HEAD and worktree
/// paths never participate. Callers must validate the repository observation
/// and Git object ID before invoking this pure operation.
#[must_use]
pub fn derive_git_stable_identity_digest<D: GitIdentityDigest>(
    repository_identity_digest: &[u8; 32],
    object_id: &[u8; 20],
) -> [u8; 32] {
    D::digest_parts(
        GIT_STABLE_IDENTITY_DOMAIN,
        &[repository_identity_digest, object_id],
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    struct ToyDigest;

    impl GitIdentityDigest for ToyDigest {
        fn digest_parts(domain: &[u8], parts: &[&[u8]]) -> [u8; 32] {
            let mut output = [0_u8; 32];
            for (index, byte) in domain
                .iter()
                .chain(parts.iter().flat_map(|part| part.iter()))
                .enumerate()
            {
                let slot = index % output.len();
                output[slot] = output[slot]
                    .wrapping_add(*byte)
                    .rotate_left(u32::try_from(index % 8).expect("rotation below eight"));
            }
            output
        }
    }

    #[test]
    fn repository_and_object_are_both_load_bearing() {
        let repository = [0x11_u8; 32];
        let other_repository = [0x12_u8; 32];
        let object = [0x22_u8; 20];
        let other_object = [0x23_u8; 20];
        let first = derive_git_stable_identity_digest::<ToyDigest>(&repository, &object);
        assert_eq!(
            first,
            derive_git_stable_identity_digest::<ToyDigest>(&repository, &object)
        );
        assert_ne!(
            first,
            derive_git_stable_identity_digest::<ToyDigest>(&other_repository, &object)
        );
        assert_ne!(
            first,
            derive_git_stable_identity_digest::<ToyDigest>(&repository, &other_object)
        );
    }
}

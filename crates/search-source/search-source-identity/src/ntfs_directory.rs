//! Fresh Windows NTFS directory profile. Pure encoding never proves FD custody.
use crate::{IdentityError, observation_codec::tuple};
use search_contracts::{self as c, CanonicalValue as V};

/// Accepted physical identity domain; creation time protects against file-ID reuse.
pub const NTFS_DIRECTORY_DOMAIN: &str = "eliot/cbor/source-identity/ntfs-directory/v1";
/// Closed platform/filesystem profile for the full 64-bit NTFS index and FILETIME.
pub const NTFS_DIRECTORY_PROFILE: &str = "ntfs-fileindex64-filetime-v1";

/// Encode observed native values without path, Root UUID, policy or access fields.
pub fn ntfs_directory_value(
    volume_serial: u32,
    file_index: u64,
    creation_time: u64,
) -> Result<V, IdentityError> {
    let text = |s: &str| {
        c::CanonicalText::new(s)
            .map(V::Text)
            .map_err(|_| IdentityError::IdentityObservationInvalid)
    };
    tuple(vec![
        V::U64(1),
        text("windows")?,
        text(NTFS_DIRECTORY_PROFILE)?,
        V::U64(u64::from(volume_serial)),
        V::U64(file_index),
        V::U64(creation_time),
    ])
}

/// Real canonical BLAKE3 of the exact accepted tuple. No native grant is issued.
pub fn ntfs_directory_digest(
    volume_serial: u32,
    file_index: u64,
    creation_time: u64,
) -> Result<c::Blake3Digest32, IdentityError> {
    let bad = |_| IdentityError::IdentityObservationInvalid;
    c::blake3_canonical(
        &c::CanonicalDigestDomain::parse(NTFS_DIRECTORY_DOMAIN).map_err(bad)?,
        &ntfs_directory_value(volume_serial, file_index, creation_time)?,
        c::DigestInputLimit::new(256).map_err(bad)?,
    )
    .map_err(bad)
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn ntfs_directory_profile_keeps_full_index_creation_and_no_logical_id() {
        let digest = ntfs_directory_digest(u32::MAX, u64::MAX, 133_700_000_000_000_000).unwrap();
        assert_ne!(
            digest,
            ntfs_directory_digest(u32::MAX, u64::from(u32::MAX), 133_700_000_000_000_000).unwrap()
        );
        assert_ne!(
            digest,
            ntfs_directory_digest(u32::MAX, u64::MAX, 133_700_000_000_000_001).unwrap()
        );
        let bytes = c::to_canonical_cbor(
            &ntfs_directory_value(u32::MAX, u64::MAX, 133_700_000_000_000_000).unwrap(),
        )
        .unwrap();
        assert_eq!(
            c::parse_canonical_cbor(bytes.as_slice()).unwrap(),
            ntfs_directory_value(u32::MAX, u64::MAX, 133_700_000_000_000_000).unwrap()
        );
        assert_eq!(
            digest.to_string(),
            "01cbd94f10601d95c2786d8726e57dd73505c37acb93b590c0e8b6bec59303ca"
        );
    }
}

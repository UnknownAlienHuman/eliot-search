//! Fixed NTFS Source descriptor and physical digests; no Native proof or I/O.
//!
//! The Native producer must measure the descriptor's requirements on held FDs
//! before attesting a path. Constructing this profile grants no read authority.

use crate::{
    CaseBehavior, FilesystemIdentityProfile, IdentityError, LinkBehavior, ReparseBehavior,
    StableFieldPolicy, UnicodeBehavior, observation_codec::tuple,
};
use search_contracts::{self as c, CanonicalValue as V};

/// Domain of the complete fixed NTFS Source profile descriptor, revision 1.
pub const NTFS_SOURCE_PROFILE_DOMAIN: &str = "eliot/cbor/source-identity/ntfs-source-profile/v1";
/// Physical volume tuple domain, distinct from file and directory identities.
pub const NTFS_VOLUME_DOMAIN: &str = "eliot/cbor/source-identity/ntfs-volume/v1";
/// Physical file tuple domain; creation FILETIME protects against index reuse.
pub const NTFS_FILE_DOMAIN: &str = "eliot/cbor/source-identity/ntfs-file/v1";

const NATIVE_CONTRACT: &[(&str, &str)] = &[
    ("platform", "windows"),
    ("filesystem", "ntfs"),
    (
        "physical_abi",
        "GetFileInformationByHandle:BY_HANDLE_FILE_INFORMATION:52bytes:v1",
    ),
    (
        "physical_fields",
        "dwVolumeSerialNumber:u32;nFileIndexHighLow:u64;ftCreationTimeHighLow:u64:FILETIME-100ns-since-1601",
    ),
    (
        "name_abi",
        "GetFinalPathNameByHandleW:FILE_NAME_NORMALIZED=0|VOLUME_NAME_GUID=1:v1",
    ),
    (
        "case_abi",
        "GetFileInformationByHandleEx:FileCaseSensitiveInfo=23:FILE_CASE_SENSITIVE_INFO:u32:FILE_CS_FLAG_CASE_SENSITIVE_DIR=1:v1",
    ),
    (
        "case_proof",
        "root-and-every-root-relative-naming-directory:case-sensitive-flag-clear;unknown-rejected",
    ),
    (
        "lookup_form",
        "native-normalized-stored-long-names;root-relative;slash-separated;no-case-fold",
    ),
    (
        "encoding",
        "strict-utf16-to-utf8:reject-unpaired-surrogates;no-lossy-replacement;no-nfc-or-nfd:v1",
    ),
    (
        "reparse_proof",
        "root-and-every-selected-component:reparse-rejected",
    ),
    (
        "volume_guid",
        "canonical-ascii49:lowercase-guid-hex:windows-volume-guid-root:v1",
    ),
    (
        "volume_tuple",
        "[1,windows,ntfs,volume_guid_root:ascii49,volume_serial:u32]:v1",
    ),
    (
        "file_tuple",
        "[1,windows,ntfs,file_index:u64,creation_time:u64]:v1",
    ),
    ("generation", "none;creation-time-is-in-file-identity"),
];

#[derive(Clone, Copy)]
struct ProfileFields {
    revision: u64,
    case: CaseBehavior,
    unicode: UnicodeBehavior,
    volume: StableFieldPolicy,
    file: StableFieldPolicy,
    link: LinkBehavior,
    reparse: ReparseBehavior,
}

const PROFILE: ProfileFields = ProfileFields {
    revision: 1,
    case: CaseBehavior::PreNormalizedInsensitive,
    unicode: UnicodeBehavior::PreserveScalarValues,
    volume: StableFieldPolicy::Required,
    file: StableFieldPolicy::Required,
    link: LinkBehavior::StablePhysicalIdentity,
    reparse: ReparseBehavior::Rejected,
};

/// Fixed revision-1 descriptor, hashed through the shared canonical CBOR owner.
///
/// This describes required behavior, not measured support. Before setting
/// `PathObservation::normalization_attested`, the Native caller must establish
/// current Root/FD custody, reject reparses, measure every naming directory from
/// Root through the file's parent as case-insensitive, and supply native stored
/// normalized names decoded with strict UTF-16. No ASCII folding or Unicode
/// normalization substitutes for those measurements. Use `generation: None`:
/// the measured creation time is already part of the file identity digest.
pub fn ntfs_source_profile_v1() -> Result<FilesystemIdentityProfile, IdentityError> {
    FilesystemIdentityProfile::new(
        c::NonZeroRevision::new(PROFILE.revision)
            .map_err(|_| IdentityError::FilesystemProfileUnsupported)?,
        digest(
            NTFS_SOURCE_PROFILE_DOMAIN,
            &descriptor_value(PROFILE)?,
            4096,
        )?,
        PROFILE.case,
        PROFILE.unicode,
        PROFILE.volume,
        PROFILE.file,
        PROFILE.link,
        PROFILE.reparse,
    )
}

/// Hash `[1, "windows", "ntfs", volume_guid, volume_serial]` in its own domain.
///
/// Accepts only the 49-byte `\\?\Volume{xxxxxxxx-xxxx-xxxx-xxxx-xxxxxxxxxxxx}\`
/// root with lowercase ASCII GUID hex, no suffix or NUL. The caller must supply
/// the canonical form of an actual held-FD volume GUID, not a selected path or
/// drive letter. Syntax validation alone cannot establish that Native origin.
pub fn ntfs_volume_identity_v1(
    volume_guid: &str,
    volume_serial: u32,
) -> Result<c::Blake3Digest32, IdentityError> {
    let bytes = volume_guid.as_bytes();
    if bytes.len() != 49
        || !bytes.starts_with(br"\\?\Volume{")
        || !bytes.ends_with(br"}\")
        || !bytes[11..47].iter().enumerate().all(|(index, byte)| {
            if matches!(index, 8 | 13 | 18 | 23) {
                *byte == b'-'
            } else {
                byte.is_ascii_digit() || matches!(byte, b'a'..=b'f')
            }
        })
    {
        return Err(IdentityError::IdentityObservationInvalid);
    }
    digest(
        NTFS_VOLUME_DOMAIN,
        &tuple(vec![
            V::U64(1),
            text("windows")?,
            text("ntfs")?,
            text(volume_guid)?,
            V::U64(u64::from(volume_serial)),
        ])?,
        256,
    )
}

/// Hash `[1, "windows", "ntfs", file_index, creation_time]` in its own domain.
///
/// Both inputs retain all 64 bits from `BY_HANDLE_FILE_INFORMATION` (high DWORD
/// shifted by 32, OR low DWORD). Creation time is the unsigned Windows FILETIME,
/// not a Unix conversion, path, revision or synthetic generation. This digest
/// must be paired with the measured volume identity; it is not globally unique.
pub fn ntfs_file_identity_v1(
    file_index: u64,
    creation_time: u64,
) -> Result<c::Blake3Digest32, IdentityError> {
    digest(
        NTFS_FILE_DOMAIN,
        &tuple(vec![
            V::U64(1),
            text("windows")?,
            text("ntfs")?,
            V::U64(file_index),
            V::U64(creation_time),
        ])?,
        256,
    )
}

fn text(value: &str) -> Result<V, IdentityError> {
    c::CanonicalText::new(value)
        .map(V::Text)
        .map_err(|_| IdentityError::IdentityObservationInvalid)
}

fn digest(domain: &str, value: &V, limit: usize) -> Result<c::Blake3Digest32, IdentityError> {
    let bad = |_| IdentityError::IdentityObservationInvalid;
    c::blake3_canonical(
        &c::CanonicalDigestDomain::parse(domain).map_err(bad)?,
        value,
        c::DigestInputLimit::new(limit).map_err(bad)?,
    )
    .map_err(bad)
}

fn descriptor_value(fields: ProfileFields) -> Result<V, IdentityError> {
    let unsupported = IdentityError::FilesystemProfileUnsupported;
    let case = match fields.case {
        CaseBehavior::Sensitive => "sensitive",
        CaseBehavior::InsensitiveAscii => "insensitive_ascii",
        CaseBehavior::PreNormalizedInsensitive => "pre_normalized_insensitive",
        CaseBehavior::Unsupported => return Err(unsupported),
    };
    let unicode = match fields.unicode {
        UnicodeBehavior::PreserveScalarValues => "preserve_scalar_values",
        UnicodeBehavior::PreNormalizedNfc => "pre_normalized_nfc",
        UnicodeBehavior::PreNormalizedNfd => "pre_normalized_nfd",
        UnicodeBehavior::Unsupported => return Err(unsupported),
    };
    let stable = |policy| match policy {
        StableFieldPolicy::Required => Ok("required"),
        StableFieldPolicy::OptionalExplicitUnavailable => Ok("optional_explicit_unavailable"),
        StableFieldPolicy::Unsupported => Err(unsupported),
    };
    let link = match fields.link {
        LinkBehavior::StablePhysicalIdentity => "stable_physical_identity",
        LinkBehavior::NoStableLinkIdentity => "no_stable_link_identity",
        LinkBehavior::Unsupported => return Err(unsupported),
    };
    let reparse = match fields.reparse {
        ReparseBehavior::FinalTargetIdentity => "final_target_identity",
        ReparseBehavior::Rejected => "rejected",
        ReparseBehavior::Unsupported => return Err(unsupported),
    };
    tuple(vec![
        V::U64(fields.revision),
        text(c::CANONICAL_CBOR_PROFILE)?,
        text(NTFS_SOURCE_PROFILE_DOMAIN)?,
        text(NTFS_VOLUME_DOMAIN)?,
        text(NTFS_FILE_DOMAIN)?,
        tuple(vec![
            text(case)?,
            text(unicode)?,
            text(stable(fields.volume)?)?,
            text(stable(fields.file)?)?,
            text(link)?,
            text(reparse)?,
        ])?,
        tuple(
            NATIVE_CONTRACT
                .iter()
                .map(|(key, value)| tuple(vec![text(key)?, text(value)?]))
                .collect::<Result<_, _>>()?,
        )?,
    ])
}

#[cfg(test)]
mod tests {
    use super::*;

    const GUID: &str = r"\\?\Volume{01234567-89ab-cdef-0123-456789abcdef}\";

    #[test]
    fn ntfs_source_descriptor_has_exact_behavior_and_shared_schema_identity() {
        let profile = ntfs_source_profile_v1().unwrap();
        assert_eq!(profile.revision().get(), 1);
        assert_eq!(
            profile.case_behavior(),
            CaseBehavior::PreNormalizedInsensitive
        );
        assert_eq!(
            profile.unicode_behavior(),
            UnicodeBehavior::PreserveScalarValues
        );
        assert_eq!(profile.volume_identity(), StableFieldPolicy::Required);
        assert_eq!(profile.file_identity(), StableFieldPolicy::Required);
        assert_eq!(
            profile.link_behavior(),
            LinkBehavior::StablePhysicalIdentity
        );
        assert_eq!(profile.reparse_behavior(), ReparseBehavior::Rejected);
        let descriptor = descriptor_value(PROFILE).unwrap();
        let encoded = c::to_canonical_cbor(&descriptor).unwrap();
        assert_eq!(
            c::parse_canonical_cbor(encoded.as_slice()).unwrap(),
            descriptor
        );
        assert_eq!(
            profile.schema_digest(),
            c::blake3_canonical(
                &c::CanonicalDigestDomain::parse(NTFS_SOURCE_PROFILE_DOMAIN).unwrap(),
                &descriptor,
                c::DigestInputLimit::new(4096).unwrap(),
            )
            .unwrap()
        );
        assert_eq!(profile, ntfs_source_profile_v1().unwrap());
    }

    // Every leaf participates, including ABI, encoding, domains and tuple versions.
    fn perturbations(value: &V) -> Vec<V> {
        match value {
            V::Array(items) => items
                .as_slice()
                .iter()
                .enumerate()
                .flat_map(|(index, item)| {
                    perturbations(item).into_iter().map(move |changed| {
                        let mut copy = items.as_slice().to_vec();
                        copy[index] = changed;
                        tuple(copy).unwrap()
                    })
                })
                .collect(),
            V::Text(value) => vec![text(&format!("{}!", value.as_str())).unwrap()],
            V::U64(value) => vec![V::U64(value + 1)],
            _ => panic!("unexpected descriptor vocabulary"),
        }
    }

    #[test]
    fn ntfs_source_schema_binds_every_descriptor_field() {
        let descriptor = descriptor_value(PROFILE).unwrap();
        let expected = ntfs_source_profile_v1().unwrap().schema_digest();
        let changed = perturbations(&descriptor);
        assert_eq!(changed.len(), 39);
        for value in changed {
            assert_ne!(
                digest(NTFS_SOURCE_PROFILE_DOMAIN, &value, 4096).unwrap(),
                expected
            );
        }
        for fields in [
            ProfileFields {
                case: CaseBehavior::Unsupported,
                ..PROFILE
            },
            ProfileFields {
                unicode: UnicodeBehavior::Unsupported,
                ..PROFILE
            },
            ProfileFields {
                volume: StableFieldPolicy::Unsupported,
                ..PROFILE
            },
            ProfileFields {
                file: StableFieldPolicy::Unsupported,
                ..PROFILE
            },
            ProfileFields {
                link: LinkBehavior::Unsupported,
                ..PROFILE
            },
            ProfileFields {
                reparse: ReparseBehavior::Unsupported,
                ..PROFILE
            },
        ] {
            assert_eq!(
                descriptor_value(fields),
                Err(IdentityError::FilesystemProfileUnsupported)
            );
        }
    }

    #[test]
    fn ntfs_source_volume_guid_refuses_paths_aliases_and_unknown_vocabulary() {
        assert!(ntfs_volume_identity_v1(GUID, u32::MAX).is_ok());
        let invalid = [
            String::new(),
            r"C:\".into(),
            r"\\server\share\".into(),
            format!("{GUID}file"),
            GUID[..48].into(),
            GUID.replace("Volume", "volume"),
            GUID.replace('a', "A"),
            GUID.replace('a', "g"),
            GUID.replace('-', "_"),
            GUID.replace('a', "é"),
            GUID.replace('a', "\0"),
            format!("{GUID}\0"),
            "a".repeat(32_769),
        ];
        for value in invalid {
            assert_eq!(
                ntfs_volume_identity_v1(&value, 7),
                Err(IdentityError::IdentityObservationInvalid)
            );
        }
    }

    #[test]
    fn ntfs_source_physical_tuples_bind_full_values_in_separate_domains() {
        let volume = ntfs_volume_identity_v1(GUID, u32::MAX).unwrap();
        assert_ne!(
            volume,
            ntfs_volume_identity_v1(&GUID.replace('0', "1"), u32::MAX).unwrap()
        );
        assert_ne!(volume, ntfs_volume_identity_v1(GUID, u32::MAX - 1).unwrap());
        let file = ntfs_file_identity_v1(u64::MAX, u64::MAX).unwrap();
        assert_ne!(
            file,
            ntfs_file_identity_v1(u64::from(u32::MAX), u64::MAX).unwrap()
        );
        assert_ne!(
            file,
            ntfs_file_identity_v1(u64::MAX, u64::from(u32::MAX)).unwrap()
        );
        assert_ne!(file, ntfs_file_identity_v1(u64::MAX, u64::MAX - 1).unwrap());
        let expected = tuple(vec![
            V::U64(1),
            text("windows").unwrap(),
            text("ntfs").unwrap(),
            V::U64(u64::MAX),
            V::U64(u64::MAX),
        ])
        .unwrap();
        assert_eq!(file, digest(NTFS_FILE_DOMAIN, &expected, 256).unwrap());
        assert_ne!(file, digest(NTFS_VOLUME_DOMAIN, &expected, 256).unwrap());
        assert_ne!(
            file,
            digest(NTFS_SOURCE_PROFILE_DOMAIN, &expected, 256).unwrap()
        );
        assert_ne!(
            file,
            crate::ntfs_directory::ntfs_directory_digest(u32::MAX, u64::MAX, u64::MAX).unwrap()
        );
        assert_ne!(volume, file);
    }

    #[test]
    fn ntfs_source_path_still_requires_attestation_and_preserves_scalar_values() {
        let profile = ntfs_source_profile_v1().unwrap();
        let mut path = crate::PathObservation {
            root_binding_id: c::RootBindingId::from_bytes([2; 16]),
            root_relative_lookup_path: "Dir/É-e\u{301}.rs".into(),
            profile_revision: profile.revision(),
            profile_schema_digest: profile.schema_digest(),
            normalization_attested: false,
        };
        assert_eq!(
            crate::derive_canonical_path_key(&path, profile),
            Err(IdentityError::IdentityObservationInvalid)
        );
        path.normalization_attested = true; // Pure fixture, never Native evidence.
        let key = crate::derive_canonical_path_key(&path, profile).unwrap();
        assert_eq!(key.as_str(), path.root_relative_lookup_path);
        path.root_relative_lookup_path = "Dir/É-é.rs".into();
        assert_ne!(
            key,
            crate::derive_canonical_path_key(&path, profile).unwrap()
        );
    }
}

//! Pure frame/policy/context proofs, never native permission qualification.
use super::*;

const USER: &[u8] = &[1, 2, 0, 0, 0, 0, 0, 5, 21, 0, 0, 0, 99, 0, 0, 0];
const CONTROL: u16 = 0x9004;

fn ace(sid: &[u8], flags: u8, mask: u32) -> Vec<u8> {
    let mut bytes = vec![0, flags];
    bytes.extend_from_slice(&u16::try_from(8 + sid.len()).unwrap().to_le_bytes());
    bytes.extend_from_slice(&mask.to_le_bytes());
    bytes.extend_from_slice(sid);
    bytes
}
fn acl(aces: &[Vec<u8>]) -> Vec<u8> {
    let mut bytes = vec![2, 0];
    bytes.extend_from_slice(
        &u16::try_from(8 + aces.iter().map(Vec::len).sum::<usize>())
            .unwrap()
            .to_le_bytes(),
    );
    bytes.extend_from_slice(&u16::try_from(aces.len()).unwrap().to_le_bytes());
    bytes.extend_from_slice(&[0, 0]);
    for ace in aces {
        bytes.extend_from_slice(ace);
    }
    bytes
}
fn inspect(acl: &[u8]) -> Result<StorageAclObservation, StorageAclError<()>> {
    qualify(
        USER,
        CONTROL,
        acl,
        search_os_secrets::hash_native_revision_user_sid(USER),
    )
}

#[test]
fn closed_trustees_owner_rights_and_creator_inherit_only_are_supported() {
    let bytes = acl(&[
        ace(USER, 3, 0x1f01ff),
        ace(SYSTEM, 3, 0x1f01ff),
        ace(ADMIN, 3, 0x1f01ff),
        ace(OWNER_RIGHTS, 0, 0x20000),
        ace(CREATOR_OWNER, 11, 0x1f01ff),
    ]);
    let observed = inspect(&bytes).unwrap();
    assert_eq!(observed.clone(), observed);
    assert!(core::mem::align_of::<OwnerSid>() >= 4);
    assert_eq!(observed.owner.words[0].to_ne_bytes()[1], USER[1]);
    assert!(!format!("{observed:?}").contains("99"));
    assert!(!format!("{observed:?}").contains(&format!("{:?}", observed.semantic_fingerprint())));
    for flags in [0, 3, 8] {
        assert!(matches!(
            inspect(&acl(&[ace(CREATOR_OWNER, flags, 1)])),
            Err(StorageAclError::PolicyRefused)
        ));
    }
    assert!(matches!(
        qualify::<()>(
            SYSTEM,
            CONTROL,
            &bytes,
            search_os_secrets::hash_native_revision_user_sid(USER)
        ),
        Err(StorageAclError::PolicyRefused)
    ));
}

#[test]
fn foreign_trustees_and_unsupported_ace_types_or_flags_never_qualify() {
    let foreign = &[1, 1, 0, 0, 0, 0, 0, 1, 0, 0, 0, 0]; // Everyone
    for flags in [0, 3, 11, 19] {
        assert!(matches!(
            inspect(&acl(&[ace(foreign, flags, u32::MAX)])),
            Err(StorageAclError::PolicyRefused)
        ));
    }
    for kind in [1, 2, 5, 9, 0xff] {
        let mut entry = ace(USER, 3, 1);
        entry[0] = kind;
        assert!(matches!(
            inspect(&acl(&[entry])),
            Err(StorageAclError::InvalidDescriptor)
        ));
    }
    for flags in [0x20, 0x40, 0x80] {
        assert!(matches!(
            inspect(&acl(&[ace(USER, flags, 1)])),
            Err(StorageAclError::InvalidDescriptor)
        ));
    }
}

#[test]
fn malformed_acl_ace_and_sid_frames_refuse_without_pointer_reads() {
    let valid = acl(&[ace(USER, 3, 0x1f01ff)]);
    for end in 0..valid.len() {
        assert!(inspect(&valid[..end]).is_err());
    }
    for (offset, byte) in [
        (0, 3),
        (1, 1),
        (2, 8),
        (4, 0xff),
        (6, 1),
        (10, 12),
        (10, 17),
        (10, 0xff),
        (16, 0),
        (17, 16),
    ] {
        let mut changed = valid.clone();
        changed[offset] = byte;
        assert!(inspect(&changed).is_err(), "frame byte {offset}");
    }
    assert!(matches!(
        inspect(&acl(&[])),
        Err(StorageAclError::PolicyRefused)
    ));
    for control in [0, 0x9000, 0x9004 | 0x40, 0x9004 | 0x80, 0x9004 | 0x4000] {
        assert!(
            qualify::<()>(
                USER,
                control,
                &valid,
                search_os_secrets::hash_native_revision_user_sid(USER)
            )
            .is_err()
        );
    }
    for sid in [&USER[..7], &USER[..12], &[0; 16][..], &[1; 68][..]] {
        assert!(qualify::<()>(sid, CONTROL, &valid, [1; 32]).is_err());
    }
}

#[test]
fn semantic_fingerprint_binds_owner_control_revision_order_flags_masks_and_sids() {
    let entries = [ace(USER, 3, 0x1f01ff), ace(SYSTEM, 3, 0x1f01ff)];
    let original = acl(&entries);
    let observed = inspect(&original).unwrap();
    assert_eq!(
        qualify::<()>(
            USER,
            CONTROL | UNCLAIMED_SD_BITS,
            &original,
            search_os_secrets::hash_native_revision_user_sid(USER)
        )
        .unwrap(),
        observed
    );
    for mask in [0x001f_01ff, 0xf000_0000, SUPPORTED_ACCESS_MASK] {
        assert!(inspect(&acl(&[ace(USER, 3, mask)])).is_ok());
    }
    for flags in [4, 8, 0x14, 0x18] {
        assert!(inspect(&acl(&[ace(USER, flags, 1)])).is_ok());
    }
    for mask in [0x0200_0000, 0x0100_0000, 0x0000_0200, u32::MAX] {
        assert!(matches!(
            inspect(&acl(&[ace(USER, 3, mask)])),
            Err(StorageAclError::PolicyRefused)
        ));
    }
    let mut changed_flags = entries.clone();
    changed_flags[0][1] = 0x13;
    let mut changed_mask = entries.clone();
    changed_mask[0][4] ^= 1;
    let changed_sid = [ace(USER, 3, 0x1f01ff), ace(ADMIN, 3, 0x1f01ff)];
    for changed in [
        acl(&[entries[1].clone(), entries[0].clone()]),
        acl(&changed_flags),
        acl(&changed_mask),
        acl(&changed_sid),
    ] {
        assert_ne!(inspect(&changed).unwrap(), observed);
    }
    for control in [0x8004, CONTROL | 0x400, CONTROL | 1] {
        assert_ne!(
            qualify::<()>(
                USER,
                control,
                &original,
                search_os_secrets::hash_native_revision_user_sid(USER)
            )
            .unwrap(),
            observed
        );
    }
    let mut revision = original.clone();
    revision[0] = 4;
    assert_ne!(inspect(&revision).unwrap(), observed);
    let mut owner = USER.to_vec();
    owner[12] ^= 1;
    let owner_acl = acl(&[ace(&owner, 3, 0x1f01ff), ace(SYSTEM, 3, 0x1f01ff)]);
    assert_ne!(
        qualify::<()>(
            &owner,
            CONTROL,
            &owner_acl,
            search_os_secrets::hash_native_revision_user_sid(&owner)
        )
        .unwrap(),
        observed
    );
    // ACL capacity is not another ACE; unused padding must not change semantics.
    let mut padded = original;
    padded.extend_from_slice(&[0xa5; 8]);
    let size = u16::try_from(padded.len()).unwrap();
    padded[2..4].copy_from_slice(&size.to_le_bytes());
    assert_eq!(inspect(&padded).unwrap(), observed);
}

struct Refusal;
impl StorageSecurityContext for Refusal {
    type Error = &'static str;
    fn absolute_deadline(&self) -> std::time::Instant {
        std::time::Instant::now()
    }
    fn checkpoint(&mut self) -> Result<(), Self::Error> {
        Err("first caller refusal")
    }
    fn charge(&mut self, _: StorageSecurityWork) -> Result<(), Self::Error> {
        panic!("no work after refusal")
    }
}
#[test]
fn creation_refusal_and_unknown_outcome_remain_distinct_and_redacted() {
    let root = inspect(&acl(&[ace(USER, 3, 0x1f01ff)])).unwrap();
    assert!(matches!(
        create_owner_private_directory(Path::new("unused"), &root, &mut Refusal),
        Err(StorageAclError::Context("first caller refusal"))
    ));
    assert!(creation_result::<()>(true, false, Ok(())).is_ok());
    let unknown = creation_result(true, false, Err("first caller refusal")).unwrap_err();
    assert!(matches!(
        unknown,
        StorageAclError::CreateOutcomeUnknown("first caller refusal")
    ));
    assert_eq!(format!("{unknown:?}"), "STORAGE_ACL_CREATE_OUTCOME_UNKNOWN");
    assert!(matches!(
        creation_result(false, true, Err("later refusal")),
        Err(StorageAclError::AlreadyExists)
    ));
    assert!(matches!(
        creation_result(false, false, Err("later refusal")),
        Err(StorageAclError::CreateFailed)
    ));
}

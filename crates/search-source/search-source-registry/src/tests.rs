//! Durable current-row value encodings and their refusals.
//!
//! These values are written into CONTROL rows, so a silently accepted
//! malformed encoding would be read back as a different identity.

use crate::{
    MAX_PREPARATION_UNIT_BYTES, NativeFileKey, ObjectKind, ObjectRef, PreparationSettings,
    SOURCE_OBJECT_FORMAT_VERSION,
};
use search_contracts::Blake3Digest32;

#[test]
fn a_native_file_key_round_trips_through_its_fixed_row_encoding() {
    for key in [
        NativeFileKey::Unix {
            device: 0,
            inode: 0,
        },
        NativeFileKey::Unix {
            device: u64::MAX,
            inode: u64::MAX,
        },
        NativeFileKey::Unix {
            device: 0x0102_0304_0506_0708,
            inode: 0x1112_1314_1516_1718,
        },
        NativeFileKey::Windows {
            volume_serial: 0,
            file_index: 0,
        },
        NativeFileKey::Windows {
            volume_serial: u32::MAX,
            file_index: u64::MAX,
        },
        NativeFileKey::Windows {
            volume_serial: 0xDEAD_BEEF,
            file_index: 0x2122_2324_2526_2728,
        },
    ] {
        let bytes = key.to_bytes();
        assert_eq!(NativeFileKey::from_bytes(bytes), Some(key));
    }
}

#[test]
fn a_platform_tag_outside_the_closed_set_is_refused() {
    let unix = NativeFileKey::Unix {
        device: 7,
        inode: 9,
    }
    .to_bytes();
    for tag in [0_u8, 3, 4, 0x80, 0xFF] {
        let mut bytes = unix;
        bytes[0] = tag;
        assert_eq!(
            NativeFileKey::from_bytes(bytes),
            None,
            "tag {tag} must not decode"
        );
    }
}

#[test]
fn windows_key_padding_must_be_zero_so_two_encodings_cannot_mean_one_key() {
    let key = NativeFileKey::Windows {
        volume_serial: 0x1122_3344,
        file_index: 0x5566_7788_99AA_BBCC,
    };
    let bytes = key.to_bytes();
    assert_eq!(NativeFileKey::from_bytes(bytes), Some(key));
    // A Windows key uses 4 of the 8 identifier bytes. Padding carrying data
    // would let distinct byte strings decode to the same key.
    assert_eq!(&bytes[5..9], &[0; 4]);
    for index in 5..9 {
        let mut dirty = bytes;
        dirty[index] = 1;
        assert_eq!(
            NativeFileKey::from_bytes(dirty),
            None,
            "non-zero padding at {index} must not decode"
        );
    }
}

#[test]
fn a_unix_key_uses_every_identifier_byte() {
    // The Unix branch has no padding, so every byte change is a different key.
    let key = NativeFileKey::Unix {
        device: 1,
        inode: 2,
    };
    let bytes = key.to_bytes();
    for index in 1..17 {
        let mut other = bytes;
        other[index] = other[index].wrapping_add(1);
        assert_ne!(
            NativeFileKey::from_bytes(other),
            Some(key),
            "byte {index} must change the decoded key"
        );
    }
}

#[test]
fn object_kind_tags_are_stable_and_closed() {
    for kind in [ObjectKind::Source, ObjectKind::Prepared] {
        assert_eq!(ObjectKind::from_tag(kind.tag()), Some(kind));
    }
    assert_eq!(ObjectKind::Source.tag(), 1);
    assert_eq!(ObjectKind::Prepared.tag(), 2);
    assert_eq!(ObjectKind::Source.name(), "source");
    assert_eq!(ObjectKind::Prepared.name(), "prepared");
    for tag in [0_u8, 3, 0xFF] {
        assert_eq!(ObjectKind::from_tag(tag), None);
    }
}

#[test]
fn an_object_reference_carries_its_kind_version_and_integrity_data() {
    let digest = Blake3Digest32::from_bytes([0x5A; 32]);
    let source = ObjectRef::source(digest, 1024);
    let prepared = ObjectRef::prepared(digest, 2048);
    assert_eq!(source.kind, ObjectKind::Source);
    assert_eq!(prepared.kind, ObjectKind::Prepared);
    assert_eq!(source.format_version, SOURCE_OBJECT_FORMAT_VERSION);
    assert_eq!(prepared.format_version, SOURCE_OBJECT_FORMAT_VERSION);
    assert_eq!(source.byte_length, 1024);
    assert_eq!(prepared.byte_length, 2048);
    assert_eq!(source.digest, digest);
    // Equal digests and lengths in different kinds remain different references.
    assert_ne!(ObjectRef::source(digest, 1024), prepared);
}

#[test]
fn only_the_supported_bounded_preparation_settings_are_accepted() {
    let default = PreparationSettings::default();
    assert!(default.is_supported());
    assert_eq!(default.max_unit_bytes, MAX_PREPARATION_UNIT_BYTES);
    assert!(default.preferred_unit_bytes <= default.max_unit_bytes);

    let rejected = [
        PreparationSettings {
            utf8_version: 2,
            ..default
        },
        PreparationSettings {
            line_unitizer_version: 2,
            ..default
        },
        PreparationSettings {
            preferred_unit_bytes: 0,
            ..default
        },
        // A preferred size above the hard ceiling is not a clamp request.
        PreparationSettings {
            preferred_unit_bytes: default.max_unit_bytes + 1,
            ..default
        },
        // The hard ceiling itself cannot exceed the format maximum.
        PreparationSettings {
            max_unit_bytes: MAX_PREPARATION_UNIT_BYTES + 1,
            ..default
        },
    ];
    for settings in rejected {
        assert!(
            !settings.is_supported(),
            "unsupported settings accepted: {settings:?}"
        );
    }
}

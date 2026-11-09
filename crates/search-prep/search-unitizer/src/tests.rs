use std::sync::atomic::AtomicBool;
use std::time::{Duration, Instant};

use search_source_registry::{CurrentSourceRecord, NativeFileKey, ObjectRef, PreparationSettings};

use crate::*;

fn source(bytes: &[u8]) -> CurrentSourceRecord {
    CurrentSourceRecord {
        source_namespace_id: "01010101-0101-0101-0101-010101010101".parse().unwrap(),
        source_id: "02020202-0202-0202-0202-020202020202".parse().unwrap(),
        row_revision: 1,
        registration_revision: 1,
        locator: b"fixture".to_vec(),
        file_key: NativeFileKey::Unix {
            device: 1,
            inode: 2,
        },
        object_ref: ObjectRef::source(
            search_contracts::Blake3Digest32::from_bytes(*blake3::hash(bytes).as_bytes()),
            u64::try_from(bytes.len()).unwrap(),
        ),
    }
}

fn limits() -> PreparationLimits {
    PreparationLimits {
        max_input_bytes: 4096,
        max_output_bytes: 4096,
        max_lines: 256,
        max_units: 256,
        max_encoded_bytes: 16384,
    }
}

fn settings(preferred: u32, hard: u32) -> PreparationSettings {
    PreparationSettings {
        preferred_unit_bytes: preferred,
        max_unit_bytes: hard,
        ..PreparationSettings::default()
    }
}

fn deadline() -> Instant {
    Instant::now() + Duration::from_secs(5)
}

#[test]
fn complete_saved_artifact_contains_and_borrows_exact_text() {
    let text = "α\r\nbeta\nlong_unicode_βγδε\rfinal";
    let input = source(text.as_bytes());
    let selected = settings(5, 9);
    let cancel = AtomicBool::new(false);
    let bytes = prepare_utf8(
        &input,
        text.as_bytes().to_vec(),
        selected,
        limits(),
        deadline(),
        &cancel,
    )
    .unwrap();
    let view =
        decode_prepared_utf8(&bytes, &input, &selected, limits(), deadline(), &cancel).unwrap();
    assert_eq!(view.text(), text);
    let body_start = view.text().as_ptr().addr();
    let artifact_start = bytes.as_ptr().addr();
    assert!((artifact_start..artifact_start + bytes.len()).contains(&body_start));
    let mut previous = 0;
    for unit in view.units() {
        assert_eq!(unit.source_start, previous);
        assert!(unit.source_end - unit.source_start <= 9);
        assert!(text.get(unit.source_start..unit.source_end).is_some());
        previous = unit.source_end;
    }
    assert_eq!(previous, text.len());
}

#[test]
fn supported_empty_has_a_real_artifact_and_zero_units() {
    let input = source(b"");
    let selected = PreparationSettings::default();
    let cancel = AtomicBool::new(false);
    let bytes = prepare_utf8(&input, Vec::new(), selected, limits(), deadline(), &cancel).unwrap();
    assert!(!bytes.is_empty());
    let view =
        decode_prepared_utf8(&bytes, &input, &selected, limits(), deadline(), &cancel).unwrap();
    assert!(view.text().is_empty());
    assert!(view.units().is_empty());
}

#[test]
fn output_identity_excludes_execution_ceilings_and_source_row_cas_revision() {
    let input = source(b"exact\ntext");
    let selected = settings(4, 8);
    let cancel = AtomicBool::new(false);
    let first = prepare_utf8(
        &input,
        b"exact\ntext".to_vec(),
        selected,
        limits(),
        deadline(),
        &cancel,
    )
    .unwrap();
    let mut larger = limits();
    larger.max_lines *= 2;
    larger.max_units *= 2;
    larger.max_encoded_bytes *= 2;
    let mut later = input.clone();
    later.row_revision += 1;
    let second = prepare_utf8(
        &later,
        b"exact\ntext".to_vec(),
        selected,
        larger,
        deadline(),
        &cancel,
    )
    .unwrap();
    assert_eq!(first, second);
    assert!(decode_prepared_utf8(&first, &later, &selected, larger, deadline(), &cancel).is_ok());
    later.registration_revision += 1;
    assert_eq!(
        decode_prepared_utf8(&first, &later, &selected, larger, deadline(), &cancel).unwrap_err(),
        PreparationError::BindingMismatch
    );
    assert_eq!(
        decode_prepared_utf8(&first, &input, &settings(5, 8), larger, deadline(), &cancel)
            .unwrap_err(),
        PreparationError::BindingMismatch
    );
}

#[test]
fn source_content_and_saved_representation_integrity_are_checked() {
    let text = b"unique_saved_representation";
    let input = source(text);
    let selected = settings(8, 16);
    let cancel = AtomicBool::new(false);
    assert_eq!(
        prepare_utf8(
            &input,
            vec![b'x'; text.len()],
            selected,
            limits(),
            deadline(),
            &cancel
        ),
        Err(PreparationError::IntegrityFailure)
    );
    let mut bytes = prepare_utf8(
        &input,
        text.to_vec(),
        selected,
        limits(),
        deadline(),
        &cancel,
    )
    .unwrap();
    let text_start = bytes
        .windows(text.len())
        .position(|window| window == text)
        .unwrap();
    bytes[text_start] = b'X';
    assert_eq!(
        decode_prepared_utf8(&bytes, &input, &selected, limits(), deadline(), &cancel).unwrap_err(),
        PreparationError::IntegrityFailure
    );
}

#[test]
fn corrupt_counts_ranges_flags_and_trailing_bytes_are_unavailable() {
    let text = b"abcdefghijkl";
    let input = source(text);
    let selected = settings(4, 8);
    let cancel = AtomicBool::new(false);
    let bytes = prepare_utf8(
        &input,
        text.to_vec(),
        selected,
        limits(),
        deadline(),
        &cancel,
    )
    .unwrap();
    let count = decode_prepared_utf8(&bytes, &input, &selected, limits(), deadline(), &cancel)
        .unwrap()
        .units()
        .len();
    let header = bytes.len() - text.len() - count * 34;
    let mut bad_count = bytes.clone();
    bad_count[header - 8..header].copy_from_slice(&u64::MAX.to_le_bytes());
    assert!(matches!(
        decode_prepared_utf8(&bad_count, &input, &selected, limits(), deadline(), &cancel),
        Err(PreparationError::TooManyUnits | PreparationError::OffsetOverflow)
    ));
    let mut bad_range = bytes.clone();
    bad_range[header + text.len()..header + text.len() + 8].copy_from_slice(&1_u64.to_le_bytes());
    assert_eq!(
        decode_prepared_utf8(&bad_range, &input, &selected, limits(), deadline(), &cancel)
            .unwrap_err(),
        PreparationError::InvalidArtifact
    );
    let mut bad_flag = bytes.clone();
    *bad_flag.last_mut().unwrap() = 2;
    assert_eq!(
        decode_prepared_utf8(&bad_flag, &input, &selected, limits(), deadline(), &cancel)
            .unwrap_err(),
        PreparationError::InvalidArtifact
    );
    let mut trailing = bytes;
    trailing.push(0);
    assert_eq!(
        decode_prepared_utf8(&trailing, &input, &selected, limits(), deadline(), &cancel)
            .unwrap_err(),
        PreparationError::InvalidArtifact
    );
}

#[test]
fn saved_ranges_are_validated_without_rebuilding_preferred_layout() {
    let text = b"abcdefghijkl";
    let input = source(text);
    let selected = settings(4, 8);
    let cancel = AtomicBool::new(false);
    let mut bytes = prepare_utf8(
        &input,
        text.to_vec(),
        selected,
        limits(),
        deadline(),
        &cancel,
    )
    .unwrap();
    let count = decode_prepared_utf8(&bytes, &input, &selected, limits(), deadline(), &cancel)
        .unwrap()
        .units()
        .len();
    let descriptors = bytes.len() - count * 34;
    bytes[descriptors + 8..descriptors + 16].copy_from_slice(&5_u64.to_le_bytes());
    bytes[descriptors + 34..descriptors + 42].copy_from_slice(&5_u64.to_le_bytes());
    let view =
        decode_prepared_utf8(&bytes, &input, &selected, limits(), deadline(), &cancel).unwrap();
    assert_eq!(view.units()[0].source_end, 5);
    assert_eq!(view.units()[1].source_start, 5);
}

#[test]
fn exact_content_refusals_and_limits_remain_distinct() {
    let cancel = AtomicBool::new(false);
    let selected = settings(2, 4);
    for (text, expected) in [
        (b"a\0b".as_slice(), PreparationError::BinaryContent),
        (&[0xff], PreparationError::InvalidUtf8),
    ] {
        assert_eq!(
            prepare_utf8(
                &source(text),
                text.to_vec(),
                selected,
                limits(),
                deadline(),
                &cancel
            ),
            Err(expected)
        );
    }
    let text = b"a\nb";
    let input = source(text);
    let mut caps = limits();
    caps.max_lines = 1;
    assert_eq!(
        prepare_utf8(&input, text.to_vec(), selected, caps, deadline(), &cancel),
        Err(PreparationError::TooManyLines)
    );
    caps = limits();
    caps.max_encoded_bytes = 1;
    assert_eq!(
        prepare_utf8(&input, text.to_vec(), selected, caps, deadline(), &cancel),
        Err(PreparationError::EncodedTooLarge)
    );
    let scalar = "α".as_bytes();
    assert_eq!(
        prepare_utf8(
            &source(scalar),
            scalar.to_vec(),
            settings(1, 1),
            limits(),
            deadline(),
            &cancel
        ),
        Err(PreparationError::UnitTooLarge)
    );
}

#[test]
fn cancellation_and_deadline_apply_to_preparation_and_saved_acceptance() {
    let input = source(b"abc");
    let selected = PreparationSettings::default();
    let cancel = AtomicBool::new(false);
    let bytes = prepare_utf8(
        &input,
        b"abc".to_vec(),
        selected,
        limits(),
        deadline(),
        &cancel,
    )
    .unwrap();
    let cancelled = AtomicBool::new(true);
    assert_eq!(
        prepare_utf8(
            &input,
            b"abc".to_vec(),
            selected,
            limits(),
            deadline(),
            &cancelled
        ),
        Err(PreparationError::Cancelled)
    );
    assert_eq!(
        decode_prepared_utf8(&bytes, &input, &selected, limits(), deadline(), &cancelled)
            .unwrap_err(),
        PreparationError::Cancelled
    );
    assert_eq!(
        decode_prepared_utf8(&bytes, &input, &selected, limits(), Instant::now(), &cancel)
            .unwrap_err(),
        PreparationError::Deadline
    );
}

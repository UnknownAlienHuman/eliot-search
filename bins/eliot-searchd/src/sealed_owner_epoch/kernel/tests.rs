use super::codec::{OwnerEpochRecord, parse_u64};
use super::identity::{
    object_id, parse_epoch_object_id, require_epoch_capacity,
};
use super::spec::{
    MAX_OWNER_EPOCH_RECORDS, OWNER_EPOCH_FIELD_COUNT,
    OWNER_EPOCH_FORMAT_VERSION, OwnerEpochError, ZERO_DIGEST_HEX,
};
use crate::sealed_digest::Sha256Digest;

fn first_record() -> OwnerEpochRecord {
    OwnerEpochRecord {
        format_version: OWNER_EPOCH_FORMAT_VERSION,
        epoch: 1,
        previous_epoch: 0,
        previous_record_sha256: Sha256Digest::from_hex(ZERO_DIGEST_HEX)
            .expect("zero digest"),
        root_binding_sha256: Sha256Digest::from_hex(&"ab".repeat(32))
            .expect("root digest"),
    }
}

#[test]
fn epoch_record_encoding_has_exact_header_and_round_trips() {
    let record = first_record();
    let encoded = record.encode().expect("encode");
    assert!(encoded.starts_with(
        "ELIOT-SEALED-OWNER-EPOCH-V1\nformat_version=1\nepoch=1\n"
    ));
    assert_eq!(encoded.lines().count(), OWNER_EPOCH_FIELD_COUNT + 1);
    assert!(encoded.ends_with('\n'));
    assert_eq!(
        OwnerEpochRecord::decode(encoded.as_bytes()).expect("decode"),
        record
    );
}

#[test]
fn duplicate_epoch_field_and_missing_terminator_are_rejected() {
    let encoded = first_record().encode().expect("encode");
    let duplicate = format!("{encoded}epoch=1\n");
    assert_eq!(
        OwnerEpochRecord::decode(duplicate.as_bytes()),
        Err(OwnerEpochError::ChainInvalid)
    );
    assert_eq!(
        OwnerEpochRecord::decode(encoded.trim_end().as_bytes()),
        Err(OwnerEpochError::ChainInvalid)
    );
}

#[test]
fn invalid_first_predecessor_cannot_be_encoded() {
    let mut record = first_record();
    record.previous_epoch = 1;
    assert_eq!(
        record.encode(),
        Err(OwnerEpochError::PredecessorMismatch)
    );
}

#[test]
fn generated_epoch_filenames_can_be_reopened() {
    for epoch in [1, 2, 42, 1_000_000, u64::MAX] {
        assert_eq!(parse_epoch_object_id(&object_id(epoch)), Ok(epoch));
    }
}

#[test]
fn invalid_epoch_filenames_remain_rejected() {
    for value in [
        "owner-epoch-00000000000000000000",
        "owner-epoch-1",
        "owner-epoch-0000000000000000000x",
        "owner-epoch-18446744073709551616",
        "owner-epoch-+0000000000000000001",
    ] {
        assert_eq!(
            parse_epoch_object_id(value),
            Err(OwnerEpochError::ChainInvalid)
        );
    }
    assert_eq!(parse_u64("01"), Err(OwnerEpochError::ChainInvalid));
}

#[test]
fn full_history_refuses_append_before_creating_an_unreadable_record() {
    assert_eq!(require_epoch_capacity(MAX_OWNER_EPOCH_RECORDS - 1), Ok(()));
    assert_eq!(
        require_epoch_capacity(MAX_OWNER_EPOCH_RECORDS),
        Err(OwnerEpochError::EpochExhausted)
    );
}

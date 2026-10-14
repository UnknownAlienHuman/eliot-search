use super::*;

fn object(
    source: u8,
    revision: u8,
    sha: u8,
    blake: u8,
    byte_length: u64,
) -> SourceContentObjectReadback {
    SourceContentObjectReadback {
        legacy_source_id: Sha256Digest32::from_bytes([source; 32]),
        legacy_revision_id: Sha256Digest32::from_bytes([revision; 32]),
        content_sha256: Sha256Digest32::from_bytes([sha; 32]),
        byte_length,
        content_blake3: search_contracts::Blake3Digest32::from_bytes([blake; 32]),
    }
}

fn binding(object: &SourceContentObjectReadback) -> ContentRevisionBinding {
    ContentRevisionBinding {
        legacy_pair: legacy_pair(object),
        content_sha256: *object.content_sha256.as_bytes(),
        source_bytes: object.byte_length,
    }
}

#[test]
fn aba_occurrences_join_through_two_reused_content_objects() {
    let a = object(1, 11, 21, 31, 10);
    let b = object(1, 12, 22, 32, 20);
    let a_binding = binding(&a);
    let b_binding = binding(&b);
    let mut seen = [0_u8; 1];
    let mut referenced_objects = 0_u64;

    for (ordinal, revision, content) in [
        (1, &a_binding, &a),
        (2, &b_binding, &b),
        (1, &a_binding, &a),
    ] {
        let value = index_value(ordinal, content);
        validate_join_value(&value, revision, 2).expect("exact tuple");
        referenced_objects += u64::from(mark_seen(&mut seen, ordinal).expect("ordinal"));
    }

    assert_eq!(referenced_objects, 2);
    assert_eq!(validate_join_accounting(3, 0, 2, 3, 2, 3), Ok(0));
}

#[test]
fn legacy_pair_match_with_wrong_sha_or_length_refuses() {
    let content = object(2, 12, 22, 32, 20);
    let exact = index_value(1, &content);
    let mut wrong_sha = binding(&content);
    wrong_sha.content_sha256[0] ^= 1;
    assert_eq!(
        validate_join_value(&exact, &wrong_sha, 1),
        Err(ControlError::MigrationUnverified)
    );

    let mut wrong_length = binding(&content);
    wrong_length.source_bytes += 1;
    assert_eq!(
        validate_join_value(&exact, &wrong_length, 1),
        Err(ControlError::MigrationUnverified)
    );
}

#[test]
fn missing_references_remain_gaps_and_orphan_objects_refuse() {
    let a = object(3, 13, 23, 33, 30);
    let a_binding = binding(&a);
    let mut seen = [0_u8; 1];
    validate_join_value(&index_value(1, &a), &a_binding, 1).expect("exact tuple");
    let referenced = u64::from(mark_seen(&mut seen, 1).expect("ordinal"));

    assert_eq!(validate_join_accounting(2, 1, referenced, 3, 1, 3), Ok(0));
    assert_eq!(
        validate_join_accounting(2, 1, referenced, 3, 2, 3),
        Err(ControlError::MigrationUnverified)
    );
}

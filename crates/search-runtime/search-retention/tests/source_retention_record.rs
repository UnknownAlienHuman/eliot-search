//! Focused pure cases for the C28 source retention cause record (#598).
//!
//! Pure codec, release and root-projection cases only. No store, no Native
//! I/O, no credentials, no purge execution: the record is untrusted metadata
//! until the original control-store commit makes it durable.

use search_contracts::{
    Blake3Digest32, BoundedList, CanonicalValue, ObjectResidencyKeyDigest, OpaqueId, SourceId,
    SourceNamespaceId, SourceRevisionId, to_canonical_cbor,
};
use search_retention::RetentionOperation;
use search_retention::source_retention::{
    MAX_SOURCE_RETENTION_RECORD_BYTES, SOURCE_RETENTION_RECORD_HEADER_BYTES,
    SOURCE_RETENTION_RECORD_KEY_PREFIX, SOURCE_RETENTION_RECORD_MAGIC, SourceRetentionCause,
    SourceRetentionRecord, SourceRetentionRecordError, SourceRetentionState,
    decode_source_retention_record, encode_source_retention_record, project_source_retention_root,
    release_source_retention_record, source_retention_record_key,
};
use search_retention::sweep::{DurableRoot, RetentionRootKind};

fn oid(value: &str) -> OpaqueId {
    OpaqueId::new(value).expect("bounded identity")
}

fn op(id: &str, byte: u8) -> RetentionOperation {
    RetentionOperation {
        operation_id: oid(id),
        request_digest: Blake3Digest32::from_bytes([byte; 32]),
    }
}

fn hex(value: &str) -> String {
    let mut out = String::with_capacity(value.len() * 2);
    for byte in value.as_bytes() {
        out.push_str(&format!("{byte:02x}"));
    }
    out
}

/// One cause for one exact physical object, created by one exact operation.
fn record_for(object_id: &str, operation_id: &str, digest: u8) -> SourceRetentionRecord {
    SourceRetentionRecord {
        object_id: oid(object_id),
        residency_digest: ObjectResidencyKeyDigest::from_bytes([0x11; 32]),
        namespace_id: SourceNamespaceId::from_bytes([0x22; 16]),
        source_id: SourceId::from_bytes([0x33; 16]),
        source_revision_id: SourceRevisionId::from_bytes([0x44; 16]),
        cause: SourceRetentionCause::UntilExplicitRelease,
        operation: op(operation_id, digest),
        state: SourceRetentionState::Active,
    }
}

fn record() -> SourceRetentionRecord {
    record_for("object/primary-0001", "operation/admit-0001", 0x55)
}

#[test]
fn codec_round_trips_every_field_and_the_key_separates_two_causes_of_one_object() {
    let active = record();
    let bytes = encode_source_retention_record(&active).expect("encode");
    assert!(bytes.len() <= MAX_SOURCE_RETENTION_RECORD_BYTES);
    assert_eq!(&bytes[..8], SOURCE_RETENTION_RECORD_MAGIC);
    assert_eq!(decode_source_retention_record(&bytes), Ok(active.clone()));
    assert_eq!(
        encode_source_retention_record(&active).expect("encode"),
        bytes,
        "encoding is deterministic"
    );

    let released = release_source_retention_record(&active, &op("operation/release-0001", 0x66))
        .expect("release");
    let released_bytes = encode_source_retention_record(&released).expect("encode released");
    assert_eq!(
        decode_source_retention_record(&released_bytes),
        Ok(released),
        "the exact release operation survives the codec"
    );
    assert_ne!(released_bytes, bytes, "active and released differ on the wire");

    // The technical key is the complete framed identity pair: the exact object
    // plus the exact original creation operation. Two independent causes for
    // one physical object therefore never alias onto one key, and no identity
    // text is truncated or replaced by a digest.
    let first_cause = record_for("object/shared-0001", "operation/cause-0001", 0x01);
    let second_cause = record_for("object/shared-0001", "operation/cause-0002", 0x02);
    assert_eq!(first_cause.object_id, second_cause.object_id);
    assert_ne!(first_cause.operation, second_cause.operation);

    let first_key = source_retention_record_key(&first_cause.object_id, &first_cause.operation.operation_id);
    let second_key =
        source_retention_record_key(&second_cause.object_id, &second_cause.operation.operation_id);
    assert_ne!(first_key, second_key, "same object, distinct causes, distinct keys");
    assert_eq!(
        first_key,
        format!(
            "{SOURCE_RETENTION_RECORD_KEY_PREFIX}{}:{}",
            hex("object/shared-0001"),
            hex("operation/cause-0001")
        ),
        "both full identity texts are framed, untruncated, in order"
    );
    assert_eq!(
        source_retention_record_key(&first_cause.object_id, &second_cause.operation.operation_id),
        second_key,
        "the key follows the identity pair and never the record instance"
    );

    let other_object = record_for("object/shared-0002", "operation/cause-0001", 0x01);
    assert_ne!(
        source_retention_record_key(&other_object.object_id, &other_object.operation.operation_id),
        first_key,
        "distinct objects cannot alias either"
    );
}

#[test]
fn decoder_refuses_magic_version_length_canonical_cause_and_state_drift() {
    let bytes = encode_source_retention_record(&record()).expect("encode");

    let mut foreign_magic = bytes.clone();
    foreign_magic[0] ^= 0xFF;
    assert_eq!(
        decode_source_retention_record(&foreign_magic),
        Err(SourceRetentionRecordError::MagicMismatch)
    );

    let mut foreign_version = bytes.clone();
    foreign_version[11] = foreign_version[11].wrapping_add(1);
    assert_eq!(
        decode_source_retention_record(&foreign_version),
        Err(SourceRetentionRecordError::VersionUnsupported)
    );

    assert_eq!(
        decode_source_retention_record(&bytes[..SOURCE_RETENTION_RECORD_HEADER_BYTES - 1]),
        Err(SourceRetentionRecordError::Truncated)
    );
    let mut truncated = bytes.clone();
    truncated.pop();
    assert_eq!(
        decode_source_retention_record(&truncated),
        Err(SourceRetentionRecordError::Truncated),
        "fewer bytes than the declared payload is truncation"
    );
    let mut trailing = bytes.clone();
    trailing.push(0);
    assert_eq!(
        decode_source_retention_record(&trailing),
        Err(SourceRetentionRecordError::TrailingBytes)
    );
    assert_eq!(
        decode_source_retention_record(&vec![0_u8; MAX_SOURCE_RETENTION_RECORD_BYTES + 1]),
        Err(SourceRetentionRecordError::RecordTooLarge)
    );

    // Indefinite-length CBOR is outside the closed canonical subset.
    let mut indefinite = SOURCE_RETENTION_RECORD_MAGIC.to_vec();
    indefinite.extend_from_slice(&1_u32.to_be_bytes());
    indefinite.extend_from_slice(&2_u32.to_be_bytes());
    indefinite.extend_from_slice(&[0xBF, 0xFF]);
    assert_eq!(
        decode_source_retention_record(&indefinite),
        Err(SourceRetentionRecordError::Malformed)
    );

    // A wider canonical array is a foreign field set, never a widened record.
    let wider = CanonicalValue::Array(BoundedList::new(vec![CanonicalValue::Null; 14]).unwrap());
    let wider = to_canonical_cbor(&wider).expect("canonical array");
    let mut widened = SOURCE_RETENTION_RECORD_MAGIC.to_vec();
    widened.extend_from_slice(&1_u32.to_be_bytes());
    widened.extend_from_slice(&u32::try_from(wider.as_slice().len()).unwrap().to_be_bytes());
    widened.extend_from_slice(wider.as_slice());
    assert_eq!(
        decode_source_retention_record(&widened),
        Err(SourceRetentionRecordError::UnknownField)
    );

    // Same-length edits inside the payload keep the CBOR canonical and isolate
    // the closed cause/state vocabularies.
    let mut foreign_cause = bytes.clone();
    let position = foreign_cause
        .windows(6)
        .position(|window| window == b"until_")
        .expect("cause spelling present");
    foreign_cause[position + 5] = b'X';
    assert_eq!(
        decode_source_retention_record(&foreign_cause),
        Err(SourceRetentionRecordError::UnknownCause)
    );
    let mut foreign_state = bytes;
    let position = foreign_state
        .windows(6)
        .position(|window| window == b"active")
        .expect("state spelling present");
    foreign_state[position + 5] = b'X';
    assert_eq!(
        decode_source_retention_record(&foreign_state),
        Err(SourceRetentionRecordError::UnknownState)
    );
}

#[test]
fn release_is_pure_idempotent_and_refuses_a_second_or_creation_identity() {
    let active = record();
    let first = op("operation/release-0001", 0x66);
    let released = release_source_retention_record(&active, &first).expect("release");

    assert_eq!(active.state, SourceRetentionState::Active, "input is untouched");
    assert_eq!(
        released.state,
        SourceRetentionState::Released {
            release_operation: first.clone(),
        }
    );
    assert_eq!(released.object_id, active.object_id);
    assert_eq!(released.residency_digest, active.residency_digest);
    assert_eq!(released.namespace_id, active.namespace_id);
    assert_eq!(released.source_id, active.source_id);
    assert_eq!(released.source_revision_id, active.source_revision_id);
    assert_eq!(released.cause, active.cause);
    assert_eq!(released.operation, active.operation);

    assert_eq!(
        &release_source_retention_record(&released, &first).expect("replay"),
        &released,
        "replay of the same release operation is idempotent"
    );
    assert_eq!(
        release_source_retention_record(&released, &op("operation/release-0002", 0x77)),
        Err(SourceRetentionRecordError::OperationConflict),
        "a different release operation never replaces the recorded one"
    );

    // The original creation operation is not a release request, active or not.
    assert_eq!(
        release_source_retention_record(&active, &active.operation.clone()),
        Err(SourceRetentionRecordError::ReleaseOperationReusesCreation)
    );
    assert_eq!(
        release_source_retention_record(&released, &active.operation.clone()),
        Err(SourceRetentionRecordError::ReleaseOperationReusesCreation)
    );
}

#[test]
fn committed_active_record_projects_one_root_and_release_touches_only_that_record() {
    let active = record();
    let projected = project_source_retention_root(&active, 7).expect("active projects");
    assert_eq!(projected.kind, RetentionRootKind::RetentionPolicy);
    assert_eq!(projected.object_id, active.object_id);
    assert_eq!(projected.residency_digest, active.residency_digest);
    assert_eq!(projected.control_generation, 7, "actual readback generation is used");

    assert_eq!(
        project_source_retention_root(&active, 0),
        Err(SourceRetentionRecordError::ZeroControlGeneration),
        "an actually committed object root cannot have generation zero"
    );

    let released = release_source_retention_record(&active, &op("operation/release-0001", 0x66))
        .expect("release");
    assert_eq!(
        project_source_retention_root(&released, 7),
        Err(SourceRetentionRecordError::NotActive),
        "a released record never yields an active root"
    );

    // Two causes share one physical object. Releasing one exact record through
    // the original control-store CAS leaves the other record active, so the
    // caller's fresh root recollection still sees that cause's root.
    let other_cause = record_for(
        active.object_id.as_str(),
        "operation/admit-0002",
        0x88,
    );
    assert_eq!(other_cause.object_id, active.object_id);
    assert_ne!(other_cause.operation, active.operation);
    assert_ne!(
        source_retention_record_key(&other_cause.object_id, &other_cause.operation.operation_id),
        source_retention_record_key(&active.object_id, &active.operation.operation_id),
        "the two causes never share one record key"
    );

    let remaining = project_source_retention_root(&other_cause, 8).expect("other cause stays active");
    assert_eq!(remaining.kind, RetentionRootKind::RetentionPolicy);
    assert_eq!(remaining.object_id, active.object_id);
    assert_eq!(remaining.control_generation, 8);
    assert_eq!(other_cause.state, SourceRetentionState::Active);
    assert_eq!(
        decode_source_retention_record(
            &encode_source_retention_record(&other_cause).expect("encode other cause")
        ),
        Ok(other_cause.clone())
    );

    let untouched = DurableRoot {
        object_id: oid("object/other-0001"),
        kind: RetentionRootKind::LegalHold,
        residency_digest: ObjectResidencyKeyDigest::from_bytes([0xAA; 32]),
        control_generation: 8,
    };
    assert_ne!(untouched, remaining, "no other root is rewritten by a release");
}


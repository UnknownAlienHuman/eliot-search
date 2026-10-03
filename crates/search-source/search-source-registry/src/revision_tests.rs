use std::collections::{BTreeMap, VecDeque};

use search_contracts::{
    AcquisitionKind, Blake3Digest32, ObjectResidencyKeyDigest, OpaqueId, OpaqueRef, ReceiptRef,
    RequestId, SourceId, SourceNamespaceId, SourceRevision, SourceRevisionId, UtcTimestamp,
};
use search_ports::{
    FakeCancellation, IdempotencyClass, MutationIdentity, OperationContext, Port, PortErrorKind,
    PortRetryability,
};

use crate::error::{RegistryError, RegistryPortError, registry_port_error};
use crate::revision::{
    SourceRevisionControlPort, SourceRevisionCurrentness, SourceRevisionHead, SourceRevisionIdPort,
    SourceRevisionMutation, SourceRevisionMutationReadback, SourceRevisionRegistrationRequest,
    SourceRevisionSourceState, decode_source_revision_mutation_readback,
    decode_source_revision_record, encode_source_revision_mutation_readback,
    encode_source_revision_record, register_source_revision,
};

fn context() -> OperationContext<FakeCancellation> {
    OperationContext::new(
        RequestId::from_bytes([0x11; 16]),
        5_000,
        FakeCancellation::new(false),
        OpaqueRef::new("budget:source-revision-test").expect("budget reference"),
    )
    .expect("valid context")
}

fn port_error() -> RegistryPortError {
    registry_port_error(
        PortErrorKind::DependencyUnavailable,
        PortRetryability::AfterReadback,
        RegistryError::DurabilityRejected,
        None,
    )
}

fn uuid_v4(seed: u8) -> SourceRevisionId {
    let mut bytes = [seed; 16];
    bytes[6] = (bytes[6] & 0x0f) | 0x40;
    bytes[8] = (bytes[8] & 0x3f) | 0x80;
    SourceRevisionId::from_bytes(bytes)
}

fn request(
    operation: &str,
    source_id: SourceId,
    expected_head: Option<SourceRevisionId>,
    content_byte: u8,
    observed_at: &str,
    stability_receipt: &str,
) -> SourceRevisionRegistrationRequest {
    SourceRevisionRegistrationRequest::new(
        OpaqueId::new(operation).expect("operation identity"),
        SourceNamespaceId::from_bytes([0x20; 16]),
        source_id,
        expected_head,
        Blake3Digest32::from_bytes([content_byte; 32]),
        37,
        UtcTimestamp::parse(observed_at).expect("UTC timestamp"),
        AcquisitionKind::Filesystem,
        ReceiptRef::new(stability_receipt).expect("stability receipt"),
        ObjectResidencyKeyDigest::from_bytes([0x42; 32]),
    )
}

#[derive(Default)]
struct TestIds {
    ids: VecDeque<SourceRevisionId>,
    issued: usize,
}

impl Port for TestIds {
    type Error = RegistryPortError;
    type Cancellation = FakeCancellation;
}

impl SourceRevisionIdPort for TestIds {
    fn fresh_source_revision_id(
        &mut self,
        _context: &OperationContext<Self::Cancellation>,
    ) -> Result<SourceRevisionId, Self::Error> {
        self.issued += 1;
        self.ids.pop_front().ok_or_else(port_error)
    }
}

#[derive(Default)]
struct TestControl {
    heads: BTreeMap<(SourceNamespaceId, SourceId), SourceRevisionHead>,
    revisions: BTreeMap<(SourceNamespaceId, SourceId, SourceRevisionId), SourceRevision>,
    operations: BTreeMap<OpaqueId, SourceRevisionMutationReadback>,
    fail_reads_after_commit: bool,
    fail_next_commit_readback: bool,
}

impl TestControl {
    fn admit(&mut self, source_namespace_id: SourceNamespaceId, source_id: SourceId) {
        self.heads.insert(
            (source_namespace_id, source_id),
            SourceRevisionHead {
                source_namespace_id,
                source_id,
                state: SourceRevisionSourceState::Active,
                revision: None,
            },
        );
    }

    fn read<T>(&self, value: Option<T>) -> Result<Option<T>, RegistryPortError> {
        if self.fail_reads_after_commit {
            Err(port_error())
        } else {
            Ok(value)
        }
    }
}

impl Port for TestControl {
    type Error = RegistryPortError;
    type Cancellation = FakeCancellation;
}

impl SourceRevisionControlPort for TestControl {
    fn load_source_revision_operation(
        &self,
        operation_id: &OpaqueId,
        _context: &OperationContext<Self::Cancellation>,
    ) -> Result<Option<SourceRevisionMutationReadback>, Self::Error> {
        self.read(self.operations.get(operation_id).cloned())
    }

    fn load_source_revision_head(
        &self,
        source_namespace_id: &SourceNamespaceId,
        source_id: &SourceId,
        _context: &OperationContext<Self::Cancellation>,
    ) -> Result<Option<SourceRevisionHead>, Self::Error> {
        self.read(self.heads.get(&(*source_namespace_id, *source_id)).cloned())
    }

    fn load_source_revision(
        &self,
        source_namespace_id: &SourceNamespaceId,
        source_id: &SourceId,
        revision_id: &SourceRevisionId,
        _context: &OperationContext<Self::Cancellation>,
    ) -> Result<Option<SourceRevision>, Self::Error> {
        self.read(
            self.revisions
                .get(&(*source_namespace_id, *source_id, *revision_id))
                .cloned(),
        )
    }

    fn compare_and_append_source_revision(
        &mut self,
        mutation: &SourceRevisionMutation,
        _context: &OperationContext<Self::Cancellation>,
        identity: &MutationIdentity,
    ) -> Result<(), Self::Error> {
        if identity.operation_id != mutation.operation_id
            || identity.idempotency != IdempotencyClass::RetrySameIdentity
        {
            return Err(port_error());
        }
        if let Some(existing) = self.operations.get(&mutation.operation_id) {
            return if existing.request_digest == mutation.request_digest
                && existing.source_namespace_id == mutation.source_namespace_id
                && existing.source_id == mutation.source_id
                && existing.revision == mutation.revision
                && existing.expected_head == mutation.expected_head
            {
                Ok(())
            } else {
                Err(port_error())
            };
        }
        let head = self
            .heads
            .get(&(mutation.source_namespace_id, mutation.source_id))
            .ok_or_else(port_error)?;
        if head.state != SourceRevisionSourceState::Active
            || head.revision.as_ref().map(|value| value.revision_id) != mutation.expected_head
        {
            return Err(port_error());
        }
        let next_sequence = head
            .revision
            .as_ref()
            .map_or(Some(1), |value| value.occurrence_sequence.checked_add(1));
        if next_sequence != Some(mutation.revision.occurrence_sequence)
            || mutation.revision.source_id != mutation.source_id
            || self.revisions.contains_key(&(
                mutation.source_namespace_id,
                mutation.source_id,
                mutation.revision.revision_id,
            ))
        {
            return Err(port_error());
        }
        let readback = SourceRevisionMutationReadback {
            operation_id: mutation.operation_id.clone(),
            request_digest: mutation.request_digest,
            source_namespace_id: mutation.source_namespace_id,
            source_id: mutation.source_id,
            expected_head: mutation.expected_head,
            revision: mutation.revision.clone(),
            receipt: ReceiptRef::new(format!("test:revision:{}", mutation.operation_id))
                .expect("receipt"),
        };
        self.revisions.insert(
            (
                mutation.source_namespace_id,
                mutation.source_id,
                mutation.revision.revision_id,
            ),
            mutation.revision.clone(),
        );
        self.operations
            .insert(mutation.operation_id.clone(), readback);
        self.heads.insert(
            (mutation.source_namespace_id, mutation.source_id),
            SourceRevisionHead {
                source_namespace_id: mutation.source_namespace_id,
                source_id: mutation.revision.source_id,
                state: SourceRevisionSourceState::Active,
                revision: Some(mutation.revision.clone()),
            },
        );
        if self.fail_next_commit_readback {
            self.fail_next_commit_readback = false;
            self.fail_reads_after_commit = true;
            Err(port_error())
        } else {
            Ok(())
        }
    }
}

#[test]
fn source_revision_occurrences_are_fresh_sequential_and_operation_recoverable() {
    let source_id = SourceId::from_bytes([0x21; 16]);
    let mut control = TestControl::default();
    control.admit(SourceNamespaceId::from_bytes([0x20; 16]), source_id);
    let mut ids = TestIds {
        ids: VecDeque::from([uuid_v4(1), uuid_v4(2), uuid_v4(3)]),
        issued: 0,
    };
    let first_request = request(
        "revision-op:a1",
        source_id,
        None,
        0xa1,
        "2026-10-03T12:00:00.000000Z",
        "stable-read:a1",
    );
    let first = register_source_revision(&first_request, &mut ids, &mut control, &context())
        .expect("first occurrence");
    assert_eq!(first.revision.occurrence_sequence, 1);
    assert_eq!(first.currentness, SourceRevisionCurrentness::Current);
    assert!(!first.replayed);

    let second_request = request(
        "revision-op:b",
        source_id,
        Some(first.revision.revision_id),
        0xb2,
        "2026-10-03T12:01:00.000000Z",
        "stable-read:b",
    );
    let second = register_source_revision(&second_request, &mut ids, &mut control, &context())
        .expect("second occurrence");
    assert_eq!(second.revision.occurrence_sequence, 2);
    assert_eq!(second.currentness, SourceRevisionCurrentness::Current);

    let third_request = request(
        "revision-op:a2",
        source_id,
        Some(second.revision.revision_id),
        0xa1,
        "2026-10-03T12:02:00.000000Z",
        "stable-read:a2",
    );
    let third = register_source_revision(&third_request, &mut ids, &mut control, &context())
        .expect("reverted occurrence");
    assert_eq!(third.revision.occurrence_sequence, 3);
    assert_eq!(third.revision.content_digest, first.revision.content_digest);
    assert_ne!(third.revision.revision_id, first.revision.revision_id);
    assert_ne!(first.revision.revision_id, second.revision.revision_id);
    assert_ne!(second.revision.revision_id, third.revision.revision_id);

    let replay = register_source_revision(&first_request, &mut ids, &mut control, &context())
        .expect("old exact operation receipt replays");
    assert_eq!(replay.revision, first.revision);
    assert_eq!(replay.current_head, third.revision);
    assert_eq!(replay.currentness, SourceRevisionCurrentness::Superseded);
    assert!(replay.replayed);
    assert_eq!(ids.issued, 3, "replay must not mint another occurrence ID");

    let mut changed_request = first_request.clone();
    changed_request.byte_length += 1;
    assert_eq!(
        register_source_revision(&changed_request, &mut ids, &mut control, &context())
            .expect_err("same operation cannot change its request"),
        RegistryError::OperationConflict
    );
    assert_eq!(ids.issued, 3);

    let record_bytes = encode_source_revision_record(&first.revision).expect("record encoding");
    assert_eq!(
        decode_source_revision_record(&record_bytes).expect("record decoding"),
        first.revision
    );
    let mut malformed_record = record_bytes.clone();
    malformed_record.push(0);
    assert_eq!(
        decode_source_revision_record(&malformed_record),
        Err(RegistryError::SourceRevisionRecordInvalid)
    );
    let operation_readback = control
        .operations
        .get(&first_request.operation_id)
        .expect("persisted operation");
    let readback_bytes =
        encode_source_revision_mutation_readback(operation_readback).expect("readback encoding");
    assert_eq!(
        decode_source_revision_mutation_readback(&readback_bytes).expect("readback decoding"),
        *operation_readback
    );
}

#[test]
fn unknown_commit_requires_exact_readback_before_retry_and_does_not_remint() {
    let source_id = SourceId::from_bytes([0x31; 16]);
    let mut control = TestControl::default();
    control.admit(SourceNamespaceId::from_bytes([0x20; 16]), source_id);
    control.fail_next_commit_readback = true;
    let mut ids = TestIds {
        ids: VecDeque::from([uuid_v4(9)]),
        issued: 0,
    };
    let request = request(
        "revision-op:unknown",
        source_id,
        None,
        0xd4,
        "2026-10-03T13:00:00.000000Z",
        "stable-read:unknown",
    );
    assert_eq!(
        register_source_revision(&request, &mut ids, &mut control, &context())
            .expect_err("unavailable exact readback remains unknown"),
        RegistryError::MutationOutcomeUnknown
    );
    assert_eq!(ids.issued, 1);
    let committed = control
        .operations
        .get(&request.operation_id)
        .expect("atomic test adapter retained operation")
        .revision
        .clone();

    control.fail_reads_after_commit = false;
    let recovered = register_source_revision(&request, &mut ids, &mut control, &context())
        .expect("exact readback recovers the original commit");
    assert_eq!(recovered.revision, committed);
    assert!(recovered.replayed);
    assert_eq!(ids.issued, 1, "recovery must not mint a replacement ID");
}

#[test]
fn expected_head_conflict_is_rejected_before_identity_mint() {
    let source_id = SourceId::from_bytes([0x41; 16]);
    let mut control = TestControl::default();
    let first_revision = SourceRevision {
        revision_id: uuid_v4(0x51),
        source_id,
        occurrence_sequence: 1,
        content_digest: Blake3Digest32::from_bytes([0x52; 32]),
        byte_length: 7,
        observed_at: UtcTimestamp::parse("2026-10-03T14:00:00.000000Z").expect("timestamp"),
        acquisition_kind: AcquisitionKind::Filesystem,
        stability_receipt_ref: ReceiptRef::new("stable-read:head").expect("receipt"),
        object_residency_key_digest: ObjectResidencyKeyDigest::from_bytes([0x53; 32]),
    };
    control.admit(SourceNamespaceId::from_bytes([0x20; 16]), source_id);
    control.revisions.insert(
        (
            SourceNamespaceId::from_bytes([0x20; 16]),
            source_id,
            first_revision.revision_id,
        ),
        first_revision.clone(),
    );
    control.heads.insert(
        (SourceNamespaceId::from_bytes([0x20; 16]), source_id),
        SourceRevisionHead {
            source_namespace_id: SourceNamespaceId::from_bytes([0x20; 16]),
            source_id,
            state: SourceRevisionSourceState::Active,
            revision: Some(first_revision),
        },
    );
    let mut ids = TestIds {
        ids: VecDeque::from([uuid_v4(0x54)]),
        issued: 0,
    };
    let wrong_head = uuid_v4(0x55);
    let request = request(
        "revision-op:stale-head",
        source_id,
        Some(wrong_head),
        0x56,
        "2026-10-03T14:01:00.000000Z",
        "stable-read:stale-head",
    );
    assert_eq!(
        register_source_revision(&request, &mut ids, &mut control, &context())
            .expect_err("expected head is checked against durable source head"),
        RegistryError::SourceRevisionConflict
    );
    assert_eq!(ids.issued, 0);
}

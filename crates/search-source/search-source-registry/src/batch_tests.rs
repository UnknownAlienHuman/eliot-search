//! Component evidence for the actual public batch producer; no disk durability claim.

use super::{
    api, cancel, context, digest, journal, opaque, operation, receipt, register_change,
    source_identity,
};
use crate::error::{
    DEFAULT_REGISTRY_LIMITS, InMemoryRegistryJournal, JournalEntryKind, RegistryControlPort,
    RegistryError, RegistryJournalEntry, RegistryPortError, registry_mutation, registry_port_error,
};
use crate::membership::{MembershipKey, MembershipLifecycle, NewMembership};
use crate::recovery::{
    BatchItemStatus, NamespaceCutover, PreparedBatch, RegistryBatch, RegistryBatchLookup,
    RegistryBatchReadback, RegistryChange, RegistryMutationRecovery, RegistryStateRecord,
    SourceRegistry, decode_batch_record, decode_registry_state,
};
use crate::source::SourceLifecycle;
use search_contracts::{NonZeroRevision, OpaqueId};
use search_ports::{
    FakeCancellation, MutationIdentity, OperationContext, Port, PortErrorKind, PortReceipt,
    PortRetryability,
};

#[derive(Clone, Copy)]
enum Fault {
    None,
    Refuse,
    LostAck,
    Missing,
    Duplicate,
    Foreign,
    ReadError,
    AbsentNewer,
}

struct BatchPort {
    store: InMemoryRegistryJournal<FakeCancellation>,
    fault: Fault,
    writes: usize,
}
impl BatchPort {
    fn new(fault: Fault) -> Self {
        Self {
            store: journal(),
            fault,
            writes: 0,
        }
    }
    fn failure(kind: PortErrorKind, id: &OpaqueId) -> RegistryPortError {
        registry_port_error(
            kind,
            PortRetryability::AfterReadback,
            RegistryError::MutationOutcomeUnknown,
            Some(id.clone()),
        )
    }
}
impl Port for BatchPort {
    type Error = RegistryPortError;
    type Cancellation = FakeCancellation;
}
impl RegistryControlPort for BatchPort {
    fn commit_batch(
        &mut self,
        batch: &PreparedBatch,
        ctx: &OperationContext<FakeCancellation>,
        mutation: &MutationIdentity,
    ) -> Result<RegistryBatchReadback, RegistryPortError> {
        assert_eq!(
            batch.expected_registry_revision(),
            batch.record().entry().before_revision
        );
        assert_eq!(batch.states().len(), batch.record().deltas().len());
        for (encoded, typed) in batch.states().iter().zip(batch.record().deltas()) {
            assert_eq!(encoded.key(), typed.key());
            assert_eq!(
                encoded.key_bytes(),
                typed.key().encode().expect("owner key")
            );
            assert_eq!(
                encoded.after_bytes(),
                typed.after().encode().expect("owner value")
            );
            assert_eq!(
                encoded.before_bytes(),
                typed
                    .before()
                    .map(|v| v.encode().expect("before"))
                    .as_deref()
            );
        }
        self.writes += 1;
        if matches!(self.fault, Fault::Refuse) {
            return Err(Self::failure(
                PortErrorKind::CompareAndSwapRejected,
                &mutation.operation_id,
            ));
        }
        let observed = self.store.commit_batch(batch, ctx, mutation)?;
        if matches!(self.fault, Fault::LostAck) {
            return Err(Self::failure(
                PortErrorKind::OutcomeUnknown,
                &mutation.operation_id,
            ));
        }
        Ok(observed)
    }
    fn load_batch(
        &self,
        id: &OpaqueId,
        ctx: &OperationContext<FakeCancellation>,
    ) -> Result<RegistryBatchLookup, RegistryPortError> {
        if matches!(self.fault, Fault::ReadError) {
            return Err(Self::failure(PortErrorKind::DependencyUnavailable, id));
        }
        if matches!(self.fault, Fault::AbsentNewer) {
            return Ok(RegistryBatchLookup::Absent { revision: 1 });
        }
        let observed = self.store.load_batch(id, ctx)?;
        let RegistryBatchLookup::Committed(readback) = observed else {
            return Ok(observed);
        };
        let states = match self.fault {
            Fault::Missing => vec![],
            Fault::Duplicate => vec![readback.states()[0].clone(); readback.states().len()],
            Fault::Foreign => vec![
                RegistryStateRecord::Generation {
                    corpus: opaque("foreign"),
                    generation: rev(9),
                };
                readback.states().len()
            ],
            _ => return Ok(RegistryBatchLookup::Committed(readback)),
        };
        Ok(RegistryBatchLookup::Committed(
            RegistryBatchReadback::new(
                readback.record().clone(),
                readback.revision(),
                states,
                DEFAULT_REGISTRY_LIMITS,
            )
            .expect("bounded observations"),
        ))
    }
    fn persist_entry(
        &mut self,
        entry: &RegistryJournalEntry,
        ctx: &OperationContext<FakeCancellation>,
        mutation: &MutationIdentity,
    ) -> Result<PortReceipt, RegistryPortError> {
        self.store.persist_entry(entry, ctx, mutation)
    }
    fn load_entry(
        &self,
        id: &OpaqueId,
        ctx: &OperationContext<FakeCancellation>,
    ) -> Result<Option<RegistryJournalEntry>, RegistryPortError> {
        self.store.load_entry(id, ctx)
    }
}
fn rev(value: u64) -> NonZeroRevision {
    NonZeroRevision::new(value).expect("revision")
}
fn member(name: &str) -> MembershipKey {
    MembershipKey {
        corpus_id: opaque("batch-corpus"),
        source_identity: source_identity(name),
    }
}
fn initial() -> RegistryBatch {
    RegistryBatch {
        expected_registry_revision: 0,
        operation: operation("O1", 11),
        changes: vec![
            register_change("one"),
            register_change("two"),
            RegistryChange::AddMembership(NewMembership {
                key: member("one"),
                generation: rev(1),
                receipt: receipt("member:one"),
            }),
            RegistryChange::AddMembership(NewMembership {
                key: member("two"),
                generation: rev(1),
                receipt: receipt("member:two"),
            }),
        ],
    }
}
fn empty() -> SourceRegistry {
    SourceRegistry::new(DEFAULT_REGISTRY_LIMITS).expect("registry")
}

#[test]
fn batch_public_o1_replay_after_o2_keeps_original_receipt_and_newer_state() {
    let mut registry = empty();
    let mut port = BatchPort::new(Fault::None);
    let ctx = context(cancel());
    let first = initial();
    let original = api::apply_admission_batch(
        &mut registry,
        first.clone(),
        &receipt("O1-receipt"),
        &mut port,
        &ctx,
    )
    .expect("first transaction");
    let second = RegistryBatch {
        expected_registry_revision: 1,
        operation: operation("O2", 12),
        changes: vec![RegistryChange::RetireSource {
            identity: source_identity("one"),
            expected_source_revision: rev(1),
            receipt: receipt("retire:one"),
        }],
    };
    api::apply_admission_batch(
        &mut registry,
        second,
        &receipt("O2-receipt"),
        &mut port,
        &ctx,
    )
    .expect("second transaction");
    let before = api::registry_snapshot(&registry);
    let replay = api::apply_admission_batch(
        &mut registry,
        first,
        &receipt("never-replace-O1"),
        &mut port,
        &ctx,
    )
    .expect("historical exact replay");
    let mut expected = original;
    expected.replayed = true;
    assert_eq!(replay, expected);
    assert_eq!((replay.before_revision, replay.after_revision), (0, 1));
    assert_eq!(api::registry_snapshot(&registry), before);
    assert_eq!(
        registry
            .source(&source_identity("one"))
            .expect("source")
            .lifecycle(),
        SourceLifecycle::Retired
    );
    assert_eq!(
        registry
            .membership(&member("one"))
            .expect("member")
            .lifecycle(),
        MembershipLifecycle::Retired
    );
    assert_eq!(
        (registry.revision(), port.writes, port.store.len()),
        (2, 2, 2)
    );
    assert!(
        matches!(api::recover_registry_mutation(&registry, &opaque("registry-operation:O1"),
        digest(11), &port, &ctx).expect("historical read"), RegistryMutationRecovery::Recovered(value)
        if value.before_revision == 0 && value.after_revision == 1)
    );
}

#[test]
fn batch_public_restart_readback_restores_complete_targets_and_derived_reverse_index() {
    let mut registry = empty();
    let mut port = BatchPort::new(Fault::None);
    let ctx = context(cancel());
    api::apply_admission_batch(&mut registry, initial(), &receipt("O1"), &mut port, &ctx)
        .expect("commit");
    let original = api::registry_snapshot(&registry);
    drop(registry);
    let mut restarted = empty();
    let recovered = api::apply_admission_batch(
        &mut restarted,
        initial(),
        &receipt("ignored"),
        &mut port,
        &ctx,
    )
    .expect("restore exact original delta");
    assert!(recovered.replayed);
    assert_eq!(api::registry_snapshot(&restarted), original);
    assert_eq!(
        restarted.active_generation(&opaque("batch-corpus")),
        Some(rev(1))
    );
    for key in [member("one"), member("two")] {
        let id = restarted
            .membership(&key)
            .expect("membership")
            .membership_id();
        assert_eq!(
            restarted.membership_key_by_id(id).expect("derived index"),
            &key
        );
    }
    let RegistryBatchLookup::Committed(observed) = port
        .load_batch(&opaque("registry-operation:O1"), &ctx)
        .expect("canonical operation read")
    else {
        panic!("committed operation")
    };
    let bytes = observed.record().encode().expect("encode");
    let mut previous_profile = bytes.clone();
    let current_profile = b"eliot.registry-batch.v3";
    let offset = previous_profile
        .windows(current_profile.len())
        .position(|value| value == current_profile)
        .expect("profile");
    previous_profile[offset + current_profile.len() - 1] = b'2';
    assert_eq!(
        decode_batch_record(&previous_profile, DEFAULT_REGISTRY_LIMITS)
            .expect("v2 source-family compatibility"),
        *observed.record()
    );
    assert_eq!(
        decode_batch_record(&bytes, DEFAULT_REGISTRY_LIMITS).expect("closed owner decode"),
        *observed.record()
    );
    for delta in observed.record().deltas() {
        assert_eq!(
            decode_registry_state(&delta.after().encode().expect("state encode"))
                .expect("state decode"),
            *delta.after()
        );
        let mut previous_state = delta.after().encode().expect("state bytes");
        let profile = b"eliot.registry-state.v3";
        let offset = previous_state
            .windows(profile.len())
            .position(|value| value == profile)
            .expect("state profile");
        previous_state[offset + profile.len() - 1] = b'2';
        assert_eq!(
            decode_registry_state(&previous_state)
                .expect("v2 source/member/generation compatibility"),
            *delta.after()
        );
    }
    let mut legacy = bytes.clone();
    let old_profile = b"eliot.registry-batch.v3";
    let position = legacy
        .windows(old_profile.len())
        .position(|v| v == old_profile)
        .expect("profile");
    legacy[position + old_profile.len() - 1] = b'1';
    assert_eq!(
        decode_batch_record(&legacy, DEFAULT_REGISTRY_LIMITS),
        Err(RegistryError::AdmissionRestorationUnavailable)
    );
    let mut unknown_version = bytes.clone();
    let profile = b"eliot.registry-batch.v3";
    let offset = unknown_version
        .windows(profile.len())
        .position(|v| v == profile)
        .expect("profile");
    unknown_version[offset + profile.len() - 1] = b'9';
    assert!(decode_batch_record(&unknown_version, DEFAULT_REGISTRY_LIMITS).is_err());
    assert!(decode_batch_record(&bytes[..bytes.len() - 1], DEFAULT_REGISTRY_LIMITS).is_err());
    assert_eq!(port.writes, 1);
}

#[test]
fn batch_unknown_ack_requires_exact_targets_and_never_redispatches_committed_operation() {
    let mut registry = empty();
    let mut port = BatchPort::new(Fault::LostAck);
    let ctx = context(cancel());
    let original = api::registry_snapshot(&registry);
    assert_eq!(
        api::apply_admission_batch(&mut registry, initial(), &receipt("O1"), &mut port, &ctx),
        Err(RegistryError::MutationOutcomeUnknown)
    );
    assert_eq!(api::registry_snapshot(&registry), original);
    assert_eq!(
        port.store.len(),
        1,
        "state and receipt committed despite lost acknowledgment"
    );
    let mut other = initial();
    other.operation = operation("another", 13);
    assert_eq!(
        api::apply_admission_batch(&mut registry, other, &receipt("other"), &mut port, &ctx),
        Err(RegistryError::MutationOutcomeUnknown)
    );
    for fault in [
        Fault::Missing,
        Fault::Duplicate,
        Fault::Foreign,
        Fault::ReadError,
    ] {
        port.fault = fault;
        assert!(
            api::apply_admission_batch(
                &mut registry,
                initial(),
                &receipt("retry"),
                &mut port,
                &ctx
            )
            .is_err()
        );
        assert_eq!(api::registry_snapshot(&registry), original);
        assert_eq!(port.writes, 1);
    }
    port.fault = Fault::None;
    let recovered = api::apply_admission_batch(
        &mut registry,
        initial(),
        &receipt("ignored"),
        &mut port,
        &ctx,
    )
    .expect("exact operation plus state recovered");
    assert!(recovered.replayed);
    assert_eq!(recovered.receipt, receipt("O1"));
    assert_eq!((registry.revision(), port.writes), (1, 1));
}

#[test]
fn batch_atomic_refusal_and_preparation_failure_leave_both_authorities_unchanged() {
    let mut registry = empty();
    let mut port = BatchPort::new(Fault::Refuse);
    let ctx = context(cancel());
    let before = api::registry_snapshot(&registry);
    assert_eq!(
        api::apply_admission_batch(&mut registry, initial(), &receipt("O1"), &mut port, &ctx),
        Err(RegistryError::RegistryRevisionConflict)
    );
    assert_eq!(api::registry_snapshot(&registry), before);
    assert!(port.store.is_empty());
    let mut invalid = initial();
    if let RegistryChange::RegisterSource { assignment, .. } = &mut invalid.changes[1] {
        assignment.readback_verified = false;
    }
    let rejected =
        api::apply_admission_batch(&mut registry, invalid, &receipt("invalid"), &mut port, &ctx)
            .expect("business rejection");
    assert!(
        rejected
            .items
            .iter()
            .all(|v| v.status == BatchItemStatus::Rejected)
    );
    assert_eq!(api::registry_snapshot(&registry), before);
    assert_eq!(port.writes, 1, "invalid semantics refused before dispatch");
    let limits = crate::error::RegistryLimits {
        max_batch_changes: 4,
        ..DEFAULT_REGISTRY_LIMITS
    };
    let mut bounded = SourceRegistry::new(limits).expect("bounded");
    let rejected = api::apply_admission_batch(
        &mut bounded,
        initial(),
        &receipt("bounded"),
        &mut port,
        &ctx,
    )
    .expect("bounded closure rejection");
    assert!(
        rejected
            .items
            .iter()
            .all(|v| v.reason == Some(RegistryError::CapacityExceeded))
    );
    assert_eq!((bounded.revision(), port.writes), (0, 1));
}

#[test]
fn batch_absence_is_revision_guarded_and_read_errors_are_never_retry_permission() {
    let mut registry = empty();
    let mut port = BatchPort::new(Fault::ReadError);
    let ctx = context(cancel());
    let id = opaque("registry-operation:O1");
    assert_eq!(
        api::apply_admission_batch(&mut registry, initial(), &receipt("O1"), &mut port, &ctx),
        Err(RegistryError::MutationOutcomeUnknown)
    );
    assert_eq!(
        api::recover_registry_mutation(&registry, &id, digest(11), &port, &ctx),
        Err(RegistryError::MutationOutcomeUnknown)
    );
    port.fault = Fault::AbsentNewer;
    assert_eq!(
        api::apply_admission_batch(&mut registry, initial(), &receipt("O1"), &mut port, &ctx),
        Err(RegistryError::RegistryRevisionConflict)
    );
    assert_eq!(
        api::recover_registry_mutation(&registry, &id, digest(11), &port, &ctx)
            .expect("coherent foreign revision"),
        RegistryMutationRecovery::Quarantined
    );
    assert_eq!((registry.revision(), port.writes), (0, 0));
}

#[test]
fn batch_same_operation_cannot_reuse_declared_digest_for_changed_actual_payload() {
    let mut registry = empty();
    let mut port = BatchPort::new(Fault::None);
    let ctx = context(cancel());
    api::apply_admission_batch(&mut registry, initial(), &receipt("O1"), &mut port, &ctx)
        .expect("commit");
    let before = api::registry_snapshot(&registry);
    let mut altered = initial();
    if let RegistryChange::RegisterSource { receipt: value, .. } = &mut altered.changes[0] {
        *value = receipt("changed-semantic-input");
    }
    assert_eq!(
        api::apply_admission_batch(&mut registry, altered, &receipt("forged"), &mut port, &ctx),
        Err(RegistryError::OperationConflict)
    );
    assert_eq!(api::registry_snapshot(&registry), before);
    assert_eq!(port.writes, 1);
}

#[test]
fn batch_generation_cutover_captures_complete_implicit_membership_changes() {
    let mut registry = empty();
    let mut port = BatchPort::new(Fault::None);
    let ctx = context(cancel());
    api::register_root(
        &mut registry,
        &super::root_request(),
        &opaque("cutover:root"),
        digest(20),
        &receipt("cutover:root"),
        &mut port,
        &ctx,
    )
    .expect("root transaction");
    let mut first = initial();
    first.expected_registry_revision = 1;
    api::apply_admission_batch(&mut registry, first, &receipt("O1"), &mut port, &ctx)
        .expect("first");
    let second = RegistryBatch {
        expected_registry_revision: 2,
        operation: operation("cutover", 21),
        changes: vec![RegistryChange::CutoverNamespace(NamespaceCutover {
            corpus_id: opaque("batch-corpus"),
            expected_generation: Some(rev(1)),
            next_generation: rev(2),
            frozen_inventory: vec![source_identity("two")],
            inventory_digest: digest(22),
            authorization_verified: true,
            readback_verified: true,
            receipt: receipt("cutover"),
        })],
    };
    port.fault = Fault::LostAck;
    let before = api::registry_snapshot(&registry);
    assert_eq!(
        api::apply_admission_batch(
            &mut registry,
            second.clone(),
            &receipt("O2"),
            &mut port,
            &ctx
        ),
        Err(RegistryError::MutationOutcomeUnknown)
    );
    assert_eq!(api::registry_snapshot(&registry), before);
    port.fault = Fault::None;
    api::apply_admission_batch(&mut registry, second, &receipt("ignored"), &mut port, &ctx)
        .expect("recover cutover");
    assert_eq!(
        registry.active_generation(&opaque("batch-corpus")),
        Some(rev(2))
    );
    assert_eq!(
        registry
            .membership(&member("one"))
            .expect("retained old member")
            .lifecycle(),
        MembershipLifecycle::Retired
    );
    let current = registry
        .membership(&member("two"))
        .expect("new generation member");
    assert_eq!(
        (
            current.generation(),
            current.membership_revision(),
            current.lifecycle()
        ),
        (rev(2), rev(3), MembershipLifecycle::Active)
    );
    assert_eq!(
        registry
            .membership_key_by_id(current.membership_id())
            .expect("derived index"),
        &member("two")
    );
    assert!(
        registry.namespaces().is_empty(),
        "legacy corpus cutover does not fabricate namespace activation"
    );
    let current_id = current.membership_id();
    let expected = api::registry_snapshot(&registry);
    drop(registry);
    let restored = port
        .store
        .restore_observed_registry(
            DEFAULT_REGISTRY_LIMITS,
            crate::recovery::MAX_REGISTRY_RESTORE_BYTES,
        )
        .expect("corpus cutover effects are fully encoded");
    assert_eq!(api::registry_snapshot(restored.registry()), expected);
    assert!(restored.registry().namespaces().is_empty());
    assert_eq!(
        restored.registry().membership_key_by_id(current_id),
        Ok(&member("two"))
    );
    assert_eq!(port.writes, 3);
}

#[test]
fn batch_generic_receipt_path_cannot_mint_atomic_state_evidence() {
    let mut port = journal();
    let ctx = context(cancel());
    let id = opaque("fake-batch");
    let entry = RegistryJournalEntry::new(
        id.clone(),
        digest(30),
        0,
        1,
        JournalEntryKind::BatchApply,
        receipt("fake"),
    );
    assert!(
        port.persist_entry(&entry, &ctx, &registry_mutation(&id))
            .is_err()
    );
    assert!(port.is_empty());
    assert!(matches!(
        port.load_batch(&id, &ctx).expect("coherent absence"),
        RegistryBatchLookup::Absent { revision: 0 }
    ));
    let mut memory_only = empty();
    memory_only.apply(initial()).expect("pure legacy state");
    assert_eq!(
        api::recover_registry_mutation(
            &memory_only,
            &opaque("registry-operation:O1"),
            digest(11),
            &port,
            &ctx
        )
        .expect("memory receipt is insufficient"),
        RegistryMutationRecovery::Quarantined
    );
}

#[test]
fn batch_actual_canonical_key_value_decoder_refuses_family_and_identity_substitution() {
    use crate::recovery::{
        RegistryStateFamily, RegistryStateKey, decode_registry_state_entry,
        decode_registry_state_key,
    };
    let mut registry = empty();
    let mut port = BatchPort::new(Fault::None);
    let ctx = context(cancel());
    api::apply_admission_batch(&mut registry, initial(), &receipt("O1"), &mut port, &ctx)
        .expect("actual public transaction");
    let RegistryBatchLookup::Committed(observed) = port
        .load_batch(&opaque("registry-operation:O1"), &ctx)
        .expect("owner bytes readback")
    else {
        panic!("committed")
    };
    for delta in observed.record().deltas() {
        let key = delta.key().encode().expect("versioned owner key");
        assert_eq!(
            decode_registry_state_key(&key).expect("actual key"),
            *delta.key()
        );
        assert_eq!(
            decode_registry_state_entry(&key, &delta.after().encode().expect("owner value"))
                .expect("matched owner record"),
            *delta.after()
        );
    }
    let source_key = RegistryStateKey::Source(source_identity("one"));
    assert_eq!(source_key.family(), RegistryStateFamily::Source);
    let source_bytes = RegistryStateRecord::Source(
        registry
            .source(&source_identity("one"))
            .expect("source")
            .clone(),
    )
    .encode()
    .expect("owner source bytes");
    let other_key = RegistryStateKey::Source(source_identity("two"))
        .encode()
        .expect("other source key");
    assert!(decode_registry_state_entry(&other_key, &source_bytes).is_err());
    let membership_key = RegistryStateKey::Membership(member("one"));
    assert_eq!(membership_key.family(), RegistryStateFamily::Membership);
    assert!(
        decode_registry_state_entry(&membership_key.encode().expect("member key"), &source_bytes)
            .is_err()
    );
    assert_eq!(
        RegistryStateKey::Generation(opaque("batch-corpus")).family(),
        RegistryStateFamily::Generation
    );
    let mut unknown_family = source_key.encode().expect("source key");
    let offset = unknown_family
        .windows(6)
        .position(|value| value == b"source")
        .expect("source family");
    unknown_family[offset..offset + 6].copy_from_slice(b"froggy");
    assert!(decode_registry_state_key(&unknown_family).is_err());
    assert!(decode_registry_state_entry(&unknown_family, &source_bytes).is_err());
    let mut unknown_version = source_key.encode().expect("source key");
    let profile = b"eliot.registry-state-key.v1";
    let offset = unknown_version
        .windows(profile.len())
        .position(|value| value == profile)
        .expect("profile");
    unknown_version[offset + profile.len() - 1] = b'9';
    assert!(decode_registry_state_key(&unknown_version).is_err());
}

#[test]
fn batch_original_envelope_restores_from_bytes_and_is_bound_by_snapshot_digest() {
    let mut registry = empty();
    let mut port = BatchPort::new(Fault::None);
    let ctx = context(cancel());
    api::apply_admission_batch(
        &mut registry,
        initial(),
        &receipt("original"),
        &mut port,
        &ctx,
    )
    .expect("actual retained-input producer");
    let source = registry
        .source(&source_identity("one"))
        .expect("source")
        .clone();
    let bytes = RegistryStateRecord::Source(source.clone())
        .encode()
        .expect("complete state bytes");
    let mut snapshot_without = api::registry_snapshot(&registry);
    let before_digest = api::snapshot_digest(&snapshot_without).expect("with original inputs");
    snapshot_without.sources.insert(
        source_identity("one"),
        source
            .clone()
            .retaining_original(None)
            .expect("legacy absence"),
    );
    assert_ne!(
        api::snapshot_digest(&snapshot_without).expect("absent inputs"),
        before_digest
    );
    drop(registry);
    let RegistryStateRecord::Source(restored) =
        decode_registry_state(&bytes).expect("decode actual persisted originals")
    else {
        panic!("source")
    };
    assert_eq!(restored, source);
    let original = restored
        .original_admission()
        .expect("retained original inputs");
    assert_eq!(original.receipt(), restored.admission());
    assert_eq!(
        original.verify_current_policy(
            &search_source_admission::baseline_policy(2, 1024),
            search_source_admission::CancelFlag::live()
        ),
        Err(search_source_admission::AdmissionError::ReceiptStale)
    );
    let mut tampered = bytes;
    let profile = b"eliot-admission-original@1";
    let offset = tampered
        .windows(profile.len())
        .position(|value| value == profile)
        .expect("actual envelope bytes");
    tampered[offset + profile.len() - 1] = b'2';
    assert!(decode_registry_state(&tampered).is_err());
    assert_eq!(
        RegistryStateRecord::Source(
            restored
                .retaining_original(None)
                .expect("explicit legacy gap")
        )
        .encode(),
        Err(RegistryError::AdmissionRestorationUnavailable)
    );
}

#[test]
fn batch_original_envelope_missing_or_foreign_refuses_before_atomic_dispatch() {
    let mut registry = empty();
    let mut port = BatchPort::new(Fault::None);
    let ctx = context(cancel());
    for foreign in [false, true] {
        let mut batch = initial();
        if let RegistryChange::RegisterSource {
            original_admission, ..
        } = &mut batch.changes[0]
        {
            *original_admission = foreign.then(|| super::original_admission(2));
        }
        let rejected =
            api::apply_admission_batch(&mut registry, batch, &receipt("refuse"), &mut port, &ctx)
                .expect("closed business refusal");
        let reason = if foreign {
            RegistryError::AdmissionBindingMismatch
        } else {
            RegistryError::AdmissionRestorationUnavailable
        };
        assert!(rejected.items.iter().all(|item| item.status == BatchItemStatus::Rejected && item.reason == Some(reason)));
        assert_eq!((registry.revision(), port.writes), (0, 0));
        assert!(port.store.is_empty());
    }
}

#[test]
fn root_atomic_restart_and_historical_receipts_keep_exact_root_revisions() {
    use crate::root::RootStatus;
    let mut registry = empty();
    let mut port = BatchPort::new(Fault::None);
    let ctx = context(cancel());
    let request = super::root_request();
    let original = api::register_root(
        &mut registry,
        &request,
        &opaque("root:O1"),
        digest(51),
        &receipt("root:registration"),
        &mut port,
        &ctx,
    )
    .expect("atomic root registration");
    let first = api::registry_snapshot(&registry);
    drop(registry);
    let mut registry = empty();
    let replay = api::register_root(
        &mut registry,
        &request,
        &opaque("root:O1"),
        digest(51),
        &receipt("ignored"),
        &mut port,
        &ctx,
    )
    .expect("restore complete root from atomic readback");
    let mut expected = original.clone();
    expected.replayed = true;
    assert_eq!(replay, expected);
    assert_eq!(api::registry_snapshot(&registry), first);
    let policy = api::update_root_policy(
        &mut registry,
        request.root_binding_id,
        rev(1),
        digest(52),
        rev(2),
        &opaque("root:O2"),
        digest(53),
        &receipt("root:policy"),
        &mut port,
        &ctx,
    )
    .expect("atomic policy revision");
    let second = api::registry_snapshot(&registry);
    let before_unbind = registry.clone();
    let unbind = api::unbind_root(
        &mut registry,
        request.root_binding_id,
        rev(2),
        &opaque("root:O3"),
        digest(54),
        &receipt("root:unbind"),
        &mut port,
        &ctx,
    )
    .expect("atomic retained tombstone");
    let final_state = api::registry_snapshot(&registry);
    assert_eq!(
        registry.root(request.root_binding_id).expect("root").status,
        RootStatus::Unbound
    );
    assert_eq!(
        (unbind.record_revision, unbind.registry_revision),
        (rev(3), 3)
    );
    let replay = api::register_root(
        &mut registry,
        &request,
        &opaque("root:O1"),
        digest(51),
        &receipt("must-not-replace"),
        &mut port,
        &ctx,
    )
    .expect("O1 after revisions 2 and 3");
    assert_eq!(replay, expected);
    let replay = api::update_root_policy(
        &mut registry,
        request.root_binding_id,
        rev(1),
        digest(52),
        rev(2),
        &opaque("root:O2"),
        digest(53),
        &receipt("must-not-replace"),
        &mut port,
        &ctx,
    )
    .expect("historical policy receipt");
    let mut expected_policy = policy;
    expected_policy.replayed = true;
    assert_eq!(replay, expected_policy);
    assert_eq!(
        api::registry_snapshot(&registry),
        final_state,
        "no historical state rollback"
    );
    let mut restarted_before_unbind = before_unbind;
    assert_eq!(api::registry_snapshot(&restarted_before_unbind), second);
    api::unbind_root(
        &mut restarted_before_unbind,
        request.root_binding_id,
        rev(2),
        &opaque("root:O3"),
        digest(54),
        &receipt("ignored"),
        &mut port,
        &ctx,
    )
    .expect("recover revision 3 from exact original before-state");
    assert_eq!(
        api::registry_snapshot(&restarted_before_unbind),
        final_state
    );
    api::validate_live_snapshot(&registry).expect("complete metadata snapshot");
    assert_eq!((port.writes, port.store.len()), (3, 3));
}

#[test]
fn root_atomic_lost_ack_and_stale_global_or_record_fences_never_mint_success() {
    let mut registry = empty();
    let mut port = BatchPort::new(Fault::LostAck);
    let ctx = context(cancel());
    let request = super::root_request();
    assert_eq!(
        api::register_root(
            &mut registry,
            &request,
            &opaque("root:unknown"),
            digest(61),
            &receipt("root:unknown"),
            &mut port,
            &ctx
        ),
        Err(RegistryError::MutationOutcomeUnknown)
    );
    assert!(registry.roots().is_empty());
    assert_eq!(registry.revision(), 0);
    port.fault = Fault::Missing;
    assert_eq!(
        api::register_root(
            &mut registry,
            &request,
            &opaque("root:unknown"),
            digest(61),
            &receipt("ignored"),
            &mut port,
            &ctx
        ),
        Err(RegistryError::MutationOutcomeUnknown)
    );
    assert!(registry.roots().is_empty());
    port.fault = Fault::None;
    api::register_root(
        &mut registry,
        &request,
        &opaque("root:unknown"),
        digest(61),
        &receipt("ignored"),
        &mut port,
        &ctx,
    )
    .expect("exact Root target readback recovery");
    let before = api::registry_snapshot(&registry);
    assert_eq!(
        api::update_root_policy(
            &mut registry,
            request.root_binding_id,
            rev(2),
            digest(62),
            rev(2),
            &opaque("root:stale-record"),
            digest(63),
            &receipt("stale"),
            &mut port,
            &ctx
        ),
        Err(RegistryError::RootPolicyGenerationMismatch)
    );
    assert_eq!(api::registry_snapshot(&registry), before);
    let mut stale = empty();
    let mut other = request.clone();
    other.root_binding_id = search_contracts::RootBindingId::from_bytes([77; 16]);
    other.canonical_root_digest = digest(64);
    assert_eq!(
        api::register_root(
            &mut stale,
            &other,
            &opaque("root:stale-global"),
            digest(65),
            &receipt("stale"),
            &mut port,
            &ctx
        ),
        Err(RegistryError::RegistryRevisionConflict)
    );
    assert!(stale.roots().is_empty());
    assert_eq!(port.writes, 1, "only the original commit was dispatched");
}

#[test]
fn root_complete_codec_is_closed_and_generic_receipts_cannot_supply_root_state() {
    use crate::recovery::{RegistryStateFamily, RegistryStateKey};
    let mut registry = empty();
    let mut port = BatchPort::new(Fault::None);
    let ctx = context(cancel());
    let request = super::root_request();
    api::register_root(
        &mut registry,
        &request,
        &opaque("root:codec"),
        digest(71),
        &receipt("root:codec"),
        &mut port,
        &ctx,
    )
    .expect("register");
    let root = registry
        .root(request.root_binding_id)
        .expect("complete root");
    let key = RegistryStateKey::Root(request.root_binding_id);
    assert_eq!(key.family(), RegistryStateFamily::Root);
    let state = RegistryStateRecord::Root(root.clone());
    let bytes = state.encode().expect("canonical complete fields");
    assert_eq!(
        crate::recovery::decode_registry_state_entry(&key.encode().expect("key"), &bytes)
            .expect("decode"),
        state
    );
    let mut unknown_status = bytes.clone();
    let offset = unknown_status
        .windows(6)
        .position(|value| value == b"active")
        .expect("status");
    unknown_status[offset..offset + 6].copy_from_slice(b"forged");
    assert!(decode_registry_state(&unknown_status).is_err());
    let mut old_family = bytes;
    let profile = b"eliot.registry-state.v3";
    let offset = old_family
        .windows(profile.len())
        .position(|value| value == profile)
        .expect("profile");
    old_family[offset + profile.len() - 1] = b'2';
    assert!(
        decode_registry_state(&old_family).is_err(),
        "Root was not a known v2 family"
    );
    for kind in [
        JournalEntryKind::RootRegistration,
        JournalEntryKind::RootPolicyChange,
        JournalEntryKind::RootUnbind,
    ] {
        let id = opaque("root:receipt-only");
        let entry = RegistryJournalEntry::new(
            id.clone(),
            digest(72),
            1,
            2,
            kind,
            receipt("root:receipt-only"),
        );
        assert!(
            port.persist_entry(&entry, &ctx, &registry_mutation(&id))
                .is_err()
        );
        assert!(matches!(
            port.load_batch(&id, &ctx).expect("coherent absence"),
            RegistryBatchLookup::Absent { revision: 1 }
        ));
    }
    let mut changed = request;
    changed.owner_fence_receipt = receipt("changed-original-request");
    assert_eq!(
        api::register_root(
            &mut registry,
            &changed,
            &opaque("root:codec"),
            digest(71),
            &receipt("same-declared-digest"),
            &mut port,
            &ctx
        ),
        Err(RegistryError::OperationConflict)
    );
    assert_eq!(port.writes, 1);
}

#[test]
fn source_revalidation_refuses_receipt_change_before_dispatch_and_later_batch_keeps_provenance() {
    let mut registry = empty();
    let mut port = BatchPort::new(Fault::None);
    let ctx = context(cancel());
    let request = super::root_request();
    api::register_root(
        &mut registry,
        &request,
        &opaque("revalidation:root"),
        digest(81),
        &receipt("revalidation:root"),
        &mut port,
        &ctx,
    )
    .expect("root");
    let mut batch = initial();
    batch.expected_registry_revision = registry.revision();
    api::apply_admission_batch(
        &mut registry,
        batch,
        &receipt("revalidation:sources"),
        &mut port,
        &ctx,
    )
    .expect("complete original admission retained");
    let replacement = super::original_admission(2);
    api::update_root_policy(
        &mut registry,
        request.root_binding_id,
        rev(1),
        search_contracts::Blake3Digest32::from_stored_bytes(
            *replacement.receipt().policy_fingerprint().as_bytes(),
        ),
        rev(2),
        &opaque("revalidation:policy"),
        digest(82),
        &receipt("revalidation:policy"),
        &mut port,
        &ctx,
    )
    .expect("live policy changes separately");
    let before = api::registry_snapshot(&registry);
    let original_bytes = registry
        .source(&source_identity("one"))
        .expect("source")
        .original_admission()
        .expect("original")
        .encoded()
        .to_vec();
    let before_writes = port.writes;
    let before_entries = port.store.len();
    let id = opaque("revalidation:receipt-change");
    assert_eq!(
        api::revalidate_admitted_source(
            &mut registry,
            &source_identity("one"),
            rev(1),
            replacement.receipt(),
            &id,
            digest(83),
            &receipt("must-not-persist"),
            &mut port,
            &ctx
        ),
        Err(RegistryError::AdmissionReceiptMismatch)
    );
    assert_eq!(api::registry_snapshot(&registry), before);
    assert_eq!(
        (port.writes, port.store.len()),
        (before_writes, before_entries)
    );
    assert!(
        port.load_entry(&id, &ctx)
            .expect("journal absence")
            .is_none()
    );
    let revision = registry.revision();
    api::apply_admission_batch(
        &mut registry,
        RegistryBatch {
            expected_registry_revision: revision,
            operation: operation("after-refused-revalidation", 84),
            changes: vec![RegistryChange::RetireSource {
                identity: source_identity("one"),
                expected_source_revision: rev(1),
                receipt: receipt("retire-after-refusal"),
            }],
        },
        &receipt("later-atomic-batch"),
        &mut port,
        &ctx,
    )
    .expect("later bounded state encoding still succeeds");
    let state = RegistryStateRecord::Source(
        registry
            .source(&source_identity("one"))
            .expect("retained source")
            .clone(),
    );
    let decoded = decode_registry_state(&state.encode().expect("complete state bytes"))
        .expect("owner restoration");
    assert_eq!(decoded, state);
    let RegistryStateRecord::Source(source) = decoded else {
        panic!("source")
    };
    assert_eq!(
        source
            .original_admission()
            .expect("old original retained")
            .encoded(),
        original_bytes
    );
    assert_eq!(
        source.original_admission().expect("original").receipt(),
        source.admission()
    );
    assert_eq!(source.lifecycle(), SourceLifecycle::Retired);
    api::validate_live_snapshot(&registry)
        .expect("coherent snapshot")
        .require_original_admission_inputs()
        .expect("explicit original-input availability");
}

#[test]
fn source_legacy_snapshot_has_explicit_unavailable_admission_inputs_despite_valid_digest() {
    let (mut registry, mut port) = super::registry_with_root();
    let ctx = context(cancel());
    let current = super::allow_receipt(1);
    api::admit_source(
        &mut registry,
        &source_identity("legacy"),
        &super::source_binding("legacy"),
        &current,
        &super::assignment("legacy", &current),
        &opaque("legacy:admit"),
        digest(85),
        &receipt("legacy:admit"),
        &mut port,
        &ctx,
    )
    .expect("existing receipt-only path");
    let snapshot = api::registry_snapshot(&registry);
    api::snapshot_digest(&snapshot).expect("technical digest can exist without retained inputs");
    let validated = api::validate_live_snapshot(&registry).expect("coherent view metadata");
    assert_eq!(
        validated.original_admission_availability(),
        api::OriginalAdmissionAvailability::LegacyInputsUnavailable
    );
    assert_eq!(
        validated.require_original_admission_inputs(),
        Err(RegistryError::AdmissionRestorationUnavailable)
    );
    assert_eq!(
        RegistryStateRecord::Source(
            registry
                .source(&source_identity("legacy"))
                .expect("legacy")
                .clone()
        )
        .encode(),
        Err(RegistryError::AdmissionRestorationUnavailable)
    );
}

#[test]
fn startup_restore_factory_rebuilds_real_values_history_and_historical_root_replay() {
    let mut registry = empty();
    let mut port = BatchPort::new(Fault::None);
    let ctx = context(cancel());
    let request = super::root_request();
    let root_receipt = api::register_root(
        &mut registry,
        &request,
        &opaque("startup:root"),
        digest(91),
        &receipt("startup:root"),
        &mut port,
        &ctx,
    )
    .expect("root transaction");
    let mut source_batch = initial();
    source_batch.expected_registry_revision = 1;
    api::apply_admission_batch(
        &mut registry,
        source_batch.clone(),
        &receipt("startup:sources"),
        &mut port,
        &ctx,
    )
    .expect("source/member/generation transaction");
    api::update_root_policy(
        &mut registry,
        request.root_binding_id,
        rev(1),
        digest(92),
        rev(2),
        &opaque("startup:policy"),
        digest(93),
        &receipt("startup:policy"),
        &mut port,
        &ctx,
    )
    .expect("later root revision");
    api::apply_admission_batch(
        &mut registry,
        RegistryBatch {
            expected_registry_revision: 3,
            operation: operation("startup-retire", 96),
            changes: vec![RegistryChange::RetireSource {
                identity: source_identity("one"),
                expected_source_revision: rev(1),
                receipt: receipt("startup:retire"),
            }],
        },
        &receipt("startup:retire"),
        &mut port,
        &ctx,
    )
    .expect("source and implicit membership tombstones");
    let expected = api::registry_snapshot(&registry);
    drop(registry);
    let observed = port
        .store
        .restore_observed_registry(
            DEFAULT_REGISTRY_LIMITS,
            crate::recovery::MAX_REGISTRY_RESTORE_BYTES,
        )
        .expect("bounded complete restore");
    assert_eq!(api::registry_snapshot(observed.registry()), expected);
    let original = observed
        .registry()
        .committed_operation(&opaque("startup:root"))
        .expect("original root operation retained");
    assert_eq!(original.entry().receipt, root_receipt.receipt);
    let RegistryStateRecord::Root(root) = original.deltas()[0].after() else {
        panic!("historical root")
    };
    assert_eq!((root.record_revision, root.registry_revision), (rev(1), 1));
    for key in [member("one"), member("two")] {
        let id = observed
            .registry()
            .membership(&key)
            .expect("member")
            .membership_id();
        assert_eq!(
            observed
                .registry()
                .membership_key_by_id(id)
                .expect("derived index"),
            &key
        );
    }
    let mut restored = observed.into_registry();
    let replay = api::register_root(
        &mut restored,
        &request,
        &opaque("startup:root"),
        digest(91),
        &receipt("ignored"),
        &mut port,
        &ctx,
    )
    .expect("historical receipt after restore");
    let mut original = root_receipt;
    original.replayed = true;
    assert_eq!(replay, original);
    assert_eq!(api::registry_snapshot(&restored), expected);
    assert!(
        restored
            .apply(source_batch.clone())
            .expect("exact cached historical replay")
            .replayed
    );
    if let RegistryChange::RegisterSource { receipt: value, .. } = &mut source_batch.changes[0] {
        *value = receipt("changed-payload-same-id-and-declared-digest");
    }
    assert_eq!(
        restored.apply(source_batch),
        Err(RegistryError::OperationConflict)
    );
    assert_eq!((port.writes, port.store.len()), (4, 4));
}

#[test]
fn startup_restore_factory_refuses_receipt_only_namespace_and_portfolio_revision_gaps() {
    let ctx = context(cancel());
    for namespace_cutover in [true, false] {
        let (mut registry, mut port, namespace) = super::registry_with_namespace();
        let unsupported_id = opaque("startup:receipt-only");
        let kind = if namespace_cutover {
            api::prepare_namespace_cutover(
                &mut registry,
                namespace,
                opaque("system:new"),
                search_contracts::InstallationIncarnationId::from_bytes([2; 16]),
                vec![source_identity("one")],
                Vec::new(),
                digest(97),
                search_contracts::CutoverId::from_bytes([3; 16]),
                receipt("startup:cutover-ref"),
                &unsupported_id,
                digest(98),
                &receipt("startup:prepare"),
                &mut port,
                &ctx,
            )
            .expect("actual receipt-only namespace preparation");
            assert!(registry.namespaces()[&namespace].preparation.is_some());
            JournalEntryKind::CutoverPrepare
        } else {
            api::publish_reference_portfolio(
                &mut registry,
                &crate::portfolio::PublishPortfolioRequest {
                    portfolio_id: search_contracts::ReferencePortfolioId::from_bytes([48; 16]),
                    portfolio_revision: search_contracts::PortfolioRevision::new(1),
                    expected_current: None,
                    membership_precedence: Vec::new(),
                    allow_empty: true,
                },
                &unsupported_id,
                digest(98),
                &receipt("startup:portfolio"),
                &mut port,
                &ctx,
            )
            .expect("actual receipt-only portfolio publication");
            assert_eq!(registry.portfolios().len(), 1);
            JournalEntryKind::PortfolioPublish
        };
        assert_eq!(registry.revision(), 2);
        assert_eq!(
            port.load_entry(&unsupported_id, &ctx)
                .expect("receipt read")
                .expect("original unsupported operation")
                .kind,
            kind
        );
        assert!(matches!(
            port.restore_observed_registry(
                DEFAULT_REGISTRY_LIMITS,
                crate::recovery::MAX_REGISTRY_RESTORE_BYTES
            ),
            Err(RegistryError::MutationOutcomeUnknown)
        ));
        api::update_root_policy(
            &mut registry,
            super::root_request().root_binding_id,
            rev(1),
            digest(99),
            rev(2),
            &opaque("startup:after-gap"),
            digest(100),
            &receipt("startup:after-gap"),
            &mut port,
            &ctx,
        )
        .expect("supported atomic mutation after unsupported revision");
        assert_eq!(registry.revision(), 3);
        assert!(matches!(
            port.restore_observed_registry(
                DEFAULT_REGISTRY_LIMITS,
                crate::recovery::MAX_REGISTRY_RESTORE_BYTES
            ),
            Err(RegistryError::MutationOutcomeUnknown)
        ));
    }
}

#[test]
fn startup_restore_factory_refuses_missing_root_relationship_in_actual_committed_values() {
    let mut registry = empty();
    let mut port = BatchPort::new(Fault::None);
    let ctx = context(cancel());
    api::apply_admission_batch(
        &mut registry,
        initial(),
        &receipt("unrooted-component"),
        &mut port,
        &ctx,
    )
    .expect("existing component batch has no Root record");
    assert!(
        matches!(
            port.store.restore_observed_registry(
                DEFAULT_REGISTRY_LIMITS,
                crate::recovery::MAX_REGISTRY_RESTORE_BYTES
            ),
            Err(RegistryError::RootNotRegistered)
        ),
        "a binding ID/receipt cannot reconstruct the missing Root state"
    );
}

#[test]
fn startup_restore_factory_refuses_missing_history_identity_substitution_duplicates_and_bounds() {
    use crate::recovery::{
        MAX_REGISTRY_RESTORE_BYTES, RegistryRestoreOperation, RegistryRestoreState,
        restore_registry_state,
    };
    let mut registry = empty();
    let mut port = BatchPort::new(Fault::None);
    let ctx = context(cancel());
    let request = super::root_request();
    let id = opaque("startup:single-root");
    api::register_root(
        &mut registry,
        &request,
        &id,
        digest(94),
        &receipt("startup:single-root"),
        &mut port,
        &ctx,
    )
    .expect("actual producer");
    let RegistryBatchLookup::Committed(readback) =
        port.load_batch(&id, &ctx).expect("actual readback")
    else {
        panic!("committed")
    };
    let key = readback.record().deltas()[0].key().encode().expect("key");
    let value = readback.states()[0].encode().expect("actual state");
    let operation = readback.record().encode().expect("operation bytes");
    let states = [RegistryRestoreState {
        key: &key,
        value: &value,
    }];
    let operations = [RegistryRestoreOperation {
        operation_id: &id,
        value: &operation,
    }];
    restore_registry_state(
        1,
        &states,
        &operations,
        DEFAULT_REGISTRY_LIMITS,
        MAX_REGISTRY_RESTORE_BYTES,
    )
    .expect("baseline coherent bytes");
    assert!(matches!(
        restore_registry_state(
            1,
            &states,
            &[],
            DEFAULT_REGISTRY_LIMITS,
            MAX_REGISTRY_RESTORE_BYTES
        ),
        Err(RegistryError::MutationOutcomeUnknown)
    ));
    assert!(matches!(
        restore_registry_state(
            2,
            &states,
            &operations,
            DEFAULT_REGISTRY_LIMITS,
            MAX_REGISTRY_RESTORE_BYTES
        ),
        Err(RegistryError::MutationOutcomeUnknown)
    ));
    assert!(
        restore_registry_state(
            1,
            &[],
            &operations,
            DEFAULT_REGISTRY_LIMITS,
            MAX_REGISTRY_RESTORE_BYTES
        )
        .is_err()
    );
    assert!(
        restore_registry_state(
            1,
            &[states[0], states[0]],
            &operations,
            DEFAULT_REGISTRY_LIMITS,
            MAX_REGISTRY_RESTORE_BYTES
        )
        .is_err()
    );
    let foreign = opaque("wrong-operation-key");
    assert!(matches!(
        restore_registry_state(
            1,
            &states,
            &[RegistryRestoreOperation {
                operation_id: &foreign,
                value: &operation
            }],
            DEFAULT_REGISTRY_LIMITS,
            MAX_REGISTRY_RESTORE_BYTES
        ),
        Err(RegistryError::OperationConflict)
    ));
    assert!(matches!(
        restore_registry_state(1, &states, &operations, DEFAULT_REGISTRY_LIMITS, 1),
        Err(RegistryError::CapacityExceeded)
    ));
    let mut tampered = value.clone();
    let position = tampered
        .windows(6)
        .position(|bytes| bytes == b"active")
        .expect("lifecycle");
    tampered[position..position + 6].copy_from_slice(b"forged");
    assert!(
        restore_registry_state(
            1,
            &[RegistryRestoreState {
                key: &key,
                value: &tampered
            }],
            &operations,
            DEFAULT_REGISTRY_LIMITS,
            MAX_REGISTRY_RESTORE_BYTES
        )
        .is_err()
    );
    assert!(
        restore_registry_state(
            1,
            &states,
            &[RegistryRestoreOperation {
                operation_id: &id,
                value: &operation[..operation.len() - 1]
            }],
            DEFAULT_REGISTRY_LIMITS,
            MAX_REGISTRY_RESTORE_BYTES
        )
        .is_err()
    );
    let mut wrong_key = key;
    let position = wrong_key
        .windows(4)
        .position(|bytes| bytes == b"root")
        .expect("family");
    wrong_key[position..position + 4].copy_from_slice(b"fake");
    assert!(
        restore_registry_state(
            1,
            &[RegistryRestoreState {
                key: &wrong_key,
                value: &value
            }],
            &operations,
            DEFAULT_REGISTRY_LIMITS,
            MAX_REGISTRY_RESTORE_BYTES
        )
        .is_err()
    );
}

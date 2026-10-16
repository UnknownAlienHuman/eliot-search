//! Real disk proof for the closed revision-use reservation.
//!
//! Genesis, material binding, counter accounting and the single opaque permit are
//! proven through the actual checked transaction and original-context seams on the
//! real `redb` journal. Owner metadata bodies stay opaque canonical bytes: this
//! file proves C02 storage/CAS behavior only and claims no Native custody,
//! credential, crypto AAD or Qdrant authority.

use super::*;
use crate::source_retention::{reserved, revision_seal_reservation_key, revision_use_keys,
    CommittedDerivedArtifactSealUse, CommittedRevisionSealUse, DerivedArtifactReservationReadback,
    DerivedArtifactScopeGuardKeysV1, DerivedArtifactUseCommand, DerivedArtifactUseMetadataV1,
    RevisionSealMetadata, RevisionUseCommand, REVISION_USE_STATE_KEY,
    derived_artifact_reservation_keys};
use crate::{ConditionalControlMutation, ControlPortCommand, ControlRecordCondition};
use crate::{RevisionDekCommand, RevisionDekSelectionCommand};
use crate::{REVISION_DEK_APPLICATION_INTENT_KEY, REVISION_DEK_CANDIDATE_KEY,
    REVISION_DEK_CURRENT_KEY, REVISION_DEK_INTENT_KEY};
use search_contracts::{to_canonical_cbor, BoundedList, CanonicalText, CanonicalValue as V,
    MaterializationId, OpaqueId, OpaqueRef, RepresentationId, RequestId, SourceId,
    SourceNamespaceId, SourceRevisionId, SourceRevisionRef};
use search_ports::{CancellationProbe, ControlJournalPort, ControlSnapshotPort, MutationIdentity,
    OperationContext, PackageOpaque};
use std::cell::Cell;
use std::time::{Duration, Instant};

#[derive(Clone, Debug)]
struct Cancel;
impl PackageOpaque for Cancel {
    fn owner_package(&self) -> &'static str { "search-control-redb" }
}
impl CancellationProbe for Cancel {
    fn is_cancelled(&self) -> bool { false }
}

fn context() -> OperationContext<Cancel> {
    OperationContext::new(RequestId::from_bytes([7; 16]), 60_000, Cancel,
        OpaqueRef::new("budget:revision-use-disk-fixture").unwrap()).unwrap()
}
fn deadline() -> Instant { Instant::now() + Duration::from_secs(60) }
fn digest32(byte: u8) -> Blake3Digest32 { Blake3Digest32::from_bytes([byte; 32]) }
fn cbor(value: &V) -> Vec<u8> { to_canonical_cbor(value).unwrap().as_slice().to_vec() }
fn id(byte: u8) -> MutationIdentity { control_mutation_identity(MutationId([byte; 32])).unwrap() }

/// Schema-5 Direct journal on an exclusively created file; no indexed route.
fn create5(scratch: &Scratch) -> PersistentControlJournal {
    let file = OpenOptions::new().read(true).write(true).create_new(true).open(scratch.path()).unwrap();
    scratch.keep(&file);
    let mut journal = PersistentControlJournal::create(file,
        JournalIdentity { schema_version: crate::REGISTRY_JOURNAL_SCHEMA_VERSION, ..identity() }, LIMITS).unwrap();
    journal.initialize_operating_profile(JournalProfileInitialization::Direct,
        MutationId([90; 32]), digest32(91), &context()).unwrap();
    journal
}

/// Opaque structural Core intent/candidate frame; the four semantic fields stay opaque.
fn dek_value(kind: u64) -> ControlValue {
    let value = V::Array(BoundedList::new(vec![
        V::Text(CanonicalText::new("eliot.revision-dek.record.v1").unwrap()),
        V::U64(1), V::U64(kind), V::Null, V::Null, V::Null, V::U64(0)]).unwrap());
    ControlValue::new(ControlRecordClass::State, cbor(&value), LIMITS).unwrap()
}

fn dek_command(byte: u8, generation: u64, candidate: bool) -> RevisionDekCommand {
    let conditions = vec![
        if candidate { ControlRecordCondition::exact(key(REVISION_DEK_INTENT_KEY), dek_value(0)) }
        else { ControlRecordCondition::absent(key(REVISION_DEK_INTENT_KEY)) },
        ControlRecordCondition::absent(key(REVISION_DEK_CANDIDATE_KEY)),
        ControlRecordCondition::absent(key(REVISION_DEK_CURRENT_KEY))];
    RevisionDekCommand::new(ConditionalControlMutation::new(ControlMutation::new(
        MutationId([byte; 32]), digest32(9), generation, vec![ControlWrite {
            key: key(if candidate { REVISION_DEK_CANDIDATE_KEY } else { REVISION_DEK_INTENT_KEY }),
            value: dek_value(u64::from(candidate)) }], vec![]), conditions)).unwrap()
}

fn selection_array(values: Vec<V>) -> V { V::Array(BoundedList::new(values).unwrap()) }

fn selection_value(current: bool) -> ControlValue {
    let intent = selection_array(vec![V::Null; 8]);
    let body = if current { selection_array(vec![intent, V::U64(1), V::U64(1), V::Null]) } else { intent };
    let value = selection_array(vec![
        V::Text(CanonicalText::new("eliot.revision-dek.selection.v1").unwrap()),
        V::U64(1), V::U64(u64::from(current)), body]);
    ControlValue::new(ControlRecordClass::State, cbor(&value), LIMITS).unwrap()
}

fn selection_command(byte: u8, generation: u64, current: bool) -> RevisionDekSelectionCommand {
    RevisionDekSelectionCommand::new(ConditionalControlMutation::new(ControlMutation::new(
        MutationId([byte; 32]), digest32(9), generation, vec![ControlWrite {
            key: key(if current { REVISION_DEK_CURRENT_KEY } else { REVISION_DEK_APPLICATION_INTENT_KEY }),
            value: selection_value(current) }], vec![]), vec![
        ControlRecordCondition::exact(key(REVISION_DEK_INTENT_KEY), dek_value(0)),
        ControlRecordCondition::exact(key(REVISION_DEK_CANDIDATE_KEY), dek_value(1)),
        if current { ControlRecordCondition::exact(key(REVISION_DEK_APPLICATION_INTENT_KEY), selection_value(false)) }
        else { ControlRecordCondition::absent(key(REVISION_DEK_APPLICATION_INTENT_KEY)) },
        ControlRecordCondition::absent(key(REVISION_DEK_CURRENT_KEY))])).unwrap()
}

/// Exact owner metadata captured before sealing; all fields are opaque canonical bytes.
fn seal_metadata(residency: &[u8]) -> RevisionSealMetadata {
    RevisionSealMetadata {
        operation_id: OpaqueId::new("retention-operation:disk-fixture").unwrap(),
        preseal_occurrence_canonical: cbor(&V::U64(7)),
        residency_canonical: residency.to_vec(),
        aad_canonical: cbor(&V::U64(11)),
        plaintext_length: 32,
        content_digest: digest32(42),
    }
}

/// Genesis, Core intent binding, candidate and first Current, in that exact order.
fn use_read(bound: &BoundControlJournal<'_, Cancel>, reservation: MutationId)
    -> crate::ScopedControlRecordsReadback
{
    // Reserve() also conditions on the exact operation reservation key, so the
    // real caller read scope is the four genesis/material keys plus that key.
    let mut keys = revision_use_keys(LIMITS).unwrap().to_vec();
    keys.push(revision_seal_reservation_key(reservation, LIMITS).unwrap());
    let work = |_unit: MigrationBootstrapWork| -> Result<(), &str> { Ok(()) };
    bound.read_control_keys_with_original_context(&keys, deadline(), &work).unwrap()
}

fn install_derived_scope_guards(bound: &mut BoundControlJournal<'_, Cancel>)
    -> DerivedArtifactScopeGuardKeysV1
{
    let guards = DerivedArtifactScopeGuardKeysV1::new(key(b"root/owner"), key(b"source/revision"),
        key(b"namespace/owner"), key(b"residency/policy"), key(b"purge/fence")).unwrap();
    let guard_keys = [guards.root_owner().clone(), guards.source_revision().clone(),
        guards.namespace_owner().clone(), guards.residency_policy().clone(), guards.purge_fence().clone()];
    let work = |_unit: MigrationBootstrapWork| -> Result<(), &str> { Ok(()) };
    let generation = bound.read_control_snapshot_with_original_context(deadline(), &work).unwrap().generation;
    let writes = guard_keys.iter().enumerate().map(|(index, key)| ControlWrite {
        key: key.clone(),
        value: ControlValue::new(ControlRecordClass::State,
            cbor(&V::U64(u64::try_from(index).unwrap() + 1)), LIMITS).unwrap(),
    }).collect();
    let conditions = guard_keys.iter().cloned().map(ControlRecordCondition::absent).collect();
    let command = ControlPortCommand::Records(ConditionalControlMutation::new(ControlMutation::new(
        MutationId([40; 32]), digest32(40), generation, writes, vec![]), conditions));
    bound.transact(&command, &context(), &id(40)).unwrap();
    guards
}

fn derived_use_metadata(current: &[u8], counter: u64) -> DerivedArtifactUseMetadataV1 {
    DerivedArtifactUseMetadataV1 {
        operation_id: OpaqueId::new("derived-operation:disk-fixture").unwrap(),
        source_revision: SourceRevisionRef {
            source_namespace_id: SourceNamespaceId::from_bytes([51; 16]),
            source_id: SourceId::from_bytes([52; 16]),
            revision_id: SourceRevisionId::from_bytes([53; 16]),
            content_digest: digest32(54), byte_length: 32,
        },
        materialization_id: MaterializationId::from_bytes([55; 16]),
        representation_id: RepresentationId::from_bytes([56; 16]),
        // Generic canonical fixtures exercise C02 opacity only; they are not C07 receipts.
        descriptor_canonical: cbor(&V::U64(57)),
        residency_canonical: cbor(&V::U64(3)),
        selected_current_canonical: current.to_vec(),
        aad_canonical: cbor(&V::U64(58 + counter)),
        plaintext_length: 32,
        expected_counter: counter,
    }
}

fn materialize_bound_state(bound: &mut BoundControlJournal<'_, Cancel>, preparation: &[u8]) -> Vec<u8> {
    let work = |_unit: MigrationBootstrapWork| -> Result<(), &str> { Ok(()) };
    let keys = revision_use_keys(LIMITS).unwrap();
    let read = bound.read_control_keys_with_original_context(&keys, deadline(), &work).unwrap();
    let genesis = RevisionUseCommand::genesis(&read, MutationId([1; 32]), digest32(1), preparation, LIMITS).unwrap();
    let (receipt, grant) = bound.transact_revision_use_with_original_context(&genesis, &id(1), deadline(), &work).unwrap();
    assert_eq!((receipt.before_generation, receipt.after_generation), (1, 2));
    assert!(grant.is_none(), "genesis is not a use permit");
    let intent = dek_command(2, 2, false);
    bound.transact_revision_dek_with_original_context(&intent, &id(2), deadline(), &work).unwrap();
    let read = bound.read_control_keys_with_original_context(&keys, deadline(), &work).unwrap();
    let binding = RevisionUseCommand::bind_material(&read, MutationId([3; 32]), digest32(3), preparation, LIMITS).unwrap();
    let (receipt, grant) = bound.transact_revision_use_with_original_context(&binding, &id(3), deadline(), &work).unwrap();
    assert_eq!((receipt.before_generation, receipt.after_generation), (3, 4));
    assert!(grant.is_none(), "material binding is not a use permit");
    let candidate = dek_command(4, 4, true);
    bound.transact_revision_dek_with_original_context(&candidate, &id(4), deadline(), &work).unwrap();
    let application = selection_command(5, 5, false);
    bound.transact_revision_dek_selection_with_original_context(&application, &id(5), deadline(), &work).unwrap();
    let current = selection_command(6, 6, true);
    bound.transact_revision_dek_selection_with_original_context(&current, &id(6), deadline(), &work).unwrap();
    selection_value(true).as_bytes().to_vec()
}

#[test]
fn revision_use_disk_genesis_material_reserve_and_prefix_refusal() {
    let scratch = Scratch::new();
    let mut journal = create5(&scratch);
    let mut publisher = ControlSnapshotPublisher::new();
    let preparation = cbor(&V::U64(1));
    let residency = cbor(&V::U64(3));
    let work = |_unit: MigrationBootstrapWork| -> Result<(), &str> { Ok(()) };
    let keys = revision_use_keys(LIMITS).unwrap();
    {
        let mut bound = journal.bind_control_port::<Cancel>(&mut publisher).unwrap();
        // A missing state row is not a fresh Current and mints no counter.
        let fresh = bound.read_control_keys_with_original_context(&keys, deadline(), &work).unwrap();
        assert_eq!(fresh.missing_keys().len(), 4);
        assert_eq!(RevisionUseCommand::next_counter(&fresh, &preparation, LIMITS), Err(ControlError::InvalidValue));
        // Genesis needs the explicit four-key absent scope; a narrow read refuses.
        let narrow = bound.read_control_keys_with_original_context(&keys[..3], deadline(), &work).unwrap();
        assert_eq!(RevisionUseCommand::genesis(&narrow, MutationId([7; 32]), digest32(7), &preparation, LIMITS),
            Err(ControlError::OperationConflict));
        let current = materialize_bound_state(&mut bound, &preparation);
        let read = use_read(&bound, MutationId([8; 32]));
        assert_eq!(RevisionUseCommand::next_counter(&read, &preparation, LIMITS).unwrap(), 0);
        let first_use = RevisionUseCommand::reserve(&read, MutationId([8; 32]), digest32(8), &preparation, 0,
            seal_metadata(&residency), LIMITS).unwrap();
        let (receipt, grant) = bound.transact_revision_use_with_original_context(&first_use, &id(8), deadline(), &work).unwrap();
        assert_eq!((receipt.before_generation, receipt.after_generation), (7, 8));
        let grant: CommittedRevisionSealUse = grant.expect("a fresh reservation mints the one opaque permit");
        assert_eq!(grant.counter(), 0);
        assert_eq!(grant.selected_current_canonical(), current.as_slice());
        assert_eq!(grant.metadata().operation_id.as_str(), "retention-operation:disk-fixture");
        assert_eq!(grant.authenticated_blocks(), 4, "32-byte body, 1-byte AAD and the length block");
        let reservation = revision_seal_reservation_key(MutationId([8; 32]), LIMITS).unwrap();
        let stored = bound
            .read_control_keys_with_original_context(std::slice::from_ref(&reservation), deadline(), &work)
            .unwrap();
        assert_eq!(stored.get(&reservation).unwrap().class(), ControlRecordClass::State);
        drop(grant);
        // Exact replay returns the historical receipt and never a permit.
        let (replay_receipt, replay_grant) = bound
            .transact_revision_use_with_original_context(&first_use, &id(8), deadline(), &work).unwrap();
        assert!(replay_receipt.replayed);
        assert_eq!(replay_receipt.after_generation, 8);
        assert!(replay_grant.is_none(), "replay never reissues a permit");
        // The burned counter is durable despite the unused dropped permit.
        let read = use_read(&bound, MutationId([9; 32]));
        assert_eq!(RevisionUseCommand::next_counter(&read, &preparation, LIMITS).unwrap(), 1);
        let second_use = RevisionUseCommand::reserve(&read, MutationId([9; 32]), digest32(9), &preparation, 1,
            seal_metadata(&residency), LIMITS).unwrap();
        let (receipt, second_grant) = bound
            .transact_revision_use_with_original_context(&second_use, &id(9), deadline(), &work).unwrap();
        assert_eq!((receipt.before_generation, receipt.after_generation), (8, 9));
        assert_eq!(second_grant.expect("second reservation").counter(), 1);
        // A foreign residency claim and a stale counter both refuse after that reservation.
        let read = use_read(&bound, MutationId([10; 32]));
        for (counter, claimed) in [(2_u64, cbor(&V::U64(99))), (0, residency)] {
            assert_eq!(RevisionUseCommand::reserve(&read, MutationId([10; 32]), digest32(10), &preparation,
                counter, seal_metadata(&claimed), LIMITS), Err(ControlError::OperationConflict));
        }
        assert_eq!(bound.read_control_snapshot_with_original_context(deadline(), &work).unwrap().generation, 9);
    }
    // Generic records may neither write nor delete the reserved revision-use prefix.
    let before = journal.verify().unwrap();
    assert!(reserved(REVISION_USE_STATE_KEY));
    let reserved_keys = [REVISION_USE_STATE_KEY.to_vec(),
        revision_seal_reservation_key(MutationId([8; 32]), LIMITS).unwrap().as_bytes().to_vec(),
        b"secret/revision-use/v2/unknown".to_vec()];
    for target in reserved_keys {
        for deleting in [false, true] {
            let command = ConditionalControlMutation::new(ControlMutation::new(MutationId([11; 32]),
                digest32(11), 9,
                if deleting { vec![] } else { vec![ControlWrite { key: key(&target), value: value(b"opaque") }] },
                if deleting { vec![key(&target)] } else { vec![] }), vec![]);
            assert_eq!(journal.transact_conditionally(command, &context()).unwrap_err().control_error(),
                ControlError::InvalidKey);
        }
    }
    assert_eq!(journal.verify().unwrap(), before);
}

#[test]
fn revision_use_postcommit_refusal_is_unknown_without_a_permit() {
    let scratch = Scratch::new();
    let mut journal = create5(&scratch);
    let mut publisher = ControlSnapshotPublisher::new();
    let preparation = cbor(&V::U64(1));
    let residency = cbor(&V::U64(3));
    let work = |_unit: MigrationBootstrapWork| -> Result<(), &str> { Ok(()) };
    {
        let mut bound = journal.bind_control_port::<Cancel>(&mut publisher).unwrap();
        materialize_bound_state(&mut bound, &preparation);
        let read = use_read(&bound, MutationId([8; 32]));
        let first_use = RevisionUseCommand::reserve(&read, MutationId([8; 32]), digest32(8), &preparation, 0,
            seal_metadata(&residency), LIMITS).unwrap();
        // Refuse the real post-commit readback: the write boundary count is the
        // existing seam, and the first refusal latches without renewing the budget.
        let boundaries = Cell::new(0_usize);
        let refused = Cell::new(false);
        let refusing = |unit: MigrationBootstrapWork| -> Result<(), &str> {
            if matches!(unit, MigrationBootstrapWork::Write(0)) { boundaries.set(boundaries.get() + 1); }
            if !refused.get() && boundaries.get() >= 2 && matches!(unit, MigrationBootstrapWork::Read(_)) {
                refused.set(true);
                return Err("post-commit-readback");
            }
            Ok(())
        };
        let error = bound.transact_revision_use_with_original_context(&first_use, &id(8), deadline(), &refusing)
            .unwrap_err();
        assert_eq!(error.control, ControlError::CommitOutcomeUnknown);
        assert_eq!(error.first_context, Some("post-commit-readback"));
        assert!(bound.current_snapshot().is_err(), "admission stays suspended");
        // Exact recovery classifies the original commit and cannot reissue a permit.
        let decision = bound.recover_revision_use_with_original_context(&first_use, &id(8), deadline(), &work).unwrap();
        assert!(matches!(decision, CommitRecoveryDecision::Committed(receipt) if receipt.replayed));
        assert!(bound.current_snapshot().is_err(), "recovery does not publish");
        assert_eq!(bound.read_control_snapshot_with_original_context(deadline(), &work).unwrap().generation, 8);
    }
    assert!(!journal.requires_recovery());
    assert_eq!(journal.verify().unwrap().generation, 8);
}

#[test]
fn derived_artifact_use_is_fresh_only_and_burns_the_shared_counter() {
    let scratch = Scratch::new();
    let mut journal = create5(&scratch);
    let mut publisher = ControlSnapshotPublisher::new();
    let preparation = cbor(&V::U64(1));
    let work = |_unit: MigrationBootstrapWork| -> Result<(), &str> { Ok(()) };
    {
        let mut bound = journal.bind_control_port::<Cancel>(&mut publisher).unwrap();
        let current = materialize_bound_state(&mut bound, &preparation);
        let guards = install_derived_scope_guards(&mut bound);
        let reservation_id = MutationId([41; 32]);
        let read_keys = derived_artifact_reservation_keys(reservation_id, &guards, LIMITS).unwrap();
        let read = bound.read_control_keys_with_original_context(&read_keys, deadline(), &work).unwrap();
        assert_eq!(RevisionUseCommand::next_counter(&read, &preparation, LIMITS).unwrap(), 0);

        let metadata = derived_use_metadata(&current, 0);
        let command = DerivedArtifactUseCommand::reserve(&read, reservation_id, digest32(41),
            &preparation, metadata.clone(), &guards, LIMITS).unwrap();
        let (receipt, permit) = bound.transact_derived_artifact_use_with_original_context(
            &command, &id(41), deadline(), &work).unwrap();
        assert!(!receipt.replayed);
        let permit: CommittedDerivedArtifactSealUse = permit.expect("fresh ack mints one permit");
        assert_eq!(permit.metadata(), &metadata);
        assert_eq!(permit.selected_current_canonical(), current);
        assert_eq!(permit.aad_canonical(), metadata.aad_canonical);
        assert_eq!(permit.counter(), 0);
        assert_eq!(permit.plaintext_length(), 32);
        assert_eq!(permit.authenticated_blocks(), 4);
        drop(permit);

        let (replay_receipt, replay_permit) = bound.transact_derived_artifact_use_with_original_context(
            &command, &id(41), deadline(), &work).unwrap();
        assert!(replay_receipt.replayed);
        assert!(replay_permit.is_none(), "replay cannot reissue the permit");
        let recovery = bound.recover_derived_artifact_use_with_original_context(
            &command, &id(41), deadline(), &work).unwrap();
        assert!(matches!(recovery, CommitRecoveryDecision::Committed(receipt) if receipt.replayed));

        let read = bound.read_control_keys_with_original_context(&read_keys, deadline(), &work).unwrap();
        let reservation = DerivedArtifactReservationReadback::read(&read, reservation_id, LIMITS).unwrap();
        assert_eq!(reservation.source_revision_ref().content_digest, digest32(54));
        assert_eq!(reservation.descriptor_canonical(), metadata.descriptor_canonical);
        assert_eq!(reservation.aad_canonical(), metadata.aad_canonical);
        assert_eq!(reservation.counter(), 0);
        assert_eq!(reservation.authenticated_blocks(), 4);
        assert_eq!(RevisionUseCommand::next_counter(&read, &preparation, LIMITS).unwrap(), 1);
        let usage = crate::source_retention::RevisionUseUsageReadback::read(&read, &preparation, LIMITS)
            .unwrap();
        assert_eq!(usage.attempts(), 1);
        assert_eq!(usage.blocks(), 4);
        assert_eq!(usage.plaintext_bytes(), 32);
        assert_eq!(usage.residency_canonical(), Some(metadata.residency_canonical.as_slice()));
    }
    assert_eq!(journal.verify().unwrap().generation, 9);
}


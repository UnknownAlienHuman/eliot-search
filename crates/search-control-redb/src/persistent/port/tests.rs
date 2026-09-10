//! Port remainder: closed commands, guard wiring, side-effect-free reads, idempotency.
//! Real files only; no second database, corpus or fabricated receipts.

use super::{ControlPortCommand, control_mutation_identity};
use crate::persistent::PUBLICATION_VISIBILITY_SCHEMA_VERSION;
use crate::persistent::publication::visibility::PublicationVisibilityState;
use crate::{
    ConditionalControlMutation, ControlKey, ControlMutation, ControlRecordClass,
    ControlSnapshotPublisher, ControlValue, ControlWrite, JournalIdentity, JournalLimits,
    MutationId, PublicationIntentUpdate,
};
use crate::{ControlError, PersistentControlJournal};
use search_contracts::{
    Blake3Digest32, CollectionGenerationId, DataRootId, Epoch, InstallationIncarnationId,
    OpaqueRef, OwnerEpoch, PublicationGuards, PublicationIntent, PublicationIntentId,
    PublicationIntentState, ReceiptRef, RequestId,
};
use search_ports::{
    CancellationProbe, ControlJournalPort, ControlSnapshotPort, OperationContext, PackageOpaque,
    PortErrorKind, PortFailure,
};
use std::cell::RefCell;
use std::fs::{self, File, OpenOptions};
use std::io::{Read, Seek, SeekFrom};
use std::path::PathBuf;
use std::sync::atomic::{AtomicU64, Ordering};
use std::time::{SystemTime, UNIX_EPOCH};

const LIMITS: JournalLimits = JournalLimits::BASELINE;
static NEXT: AtomicU64 = AtomicU64::new(0);

#[derive(Debug, Clone, Default)]
struct Cancel(bool);
impl PackageOpaque for Cancel {
    fn owner_package(&self) -> &'static str {
        "search-control-redb"
    }
}
impl CancellationProbe for Cancel {
    fn is_cancelled(&self) -> bool {
        self.0
    }
}
fn context(cancel: bool) -> OperationContext<Cancel> {
    OperationContext::new(
        RequestId::from_bytes([7; 16]),
        60_000,
        Cancel(cancel),
        OpaqueRef::new("budget:port-remainder").unwrap(),
    )
    .unwrap()
}
fn identity3() -> JournalIdentity {
    JournalIdentity {
        installation_incarnation_id: InstallationIncarnationId::from_bytes([1; 16]),
        data_root_id: DataRootId::from_bytes([2; 16]),
        owner_epoch: OwnerEpoch::new(1).unwrap(),
        path_identity_digest: Blake3Digest32::from_bytes([3; 32]),
        schema_family_digest: Blake3Digest32::from_bytes([4; 32]),
        schema_version: PUBLICATION_VISIBILITY_SCHEMA_VERSION,
    }
}
fn vis_state() -> PublicationVisibilityState {
    PublicationVisibilityState {
        collection_generation_id: CollectionGenerationId::from_bytes([5; 16]),
        schema_identity_digest: Blake3Digest32::from_bytes([6; 32]),
        visible_epoch: Epoch::new(0).unwrap(),
        guards: PublicationGuards {
            owner_epoch: OwnerEpoch::new(1).unwrap(),
            source_catalog_generation: 3,
            membership_generation: 5,
            access_generation: 7,
            shadow_generation: 11,
            purge_generation: 13,
            profile_digest: Blake3Digest32::from_bytes([7; 32]),
        },
        last_receipt: None,
    }
}
fn prepared(target: i64) -> PublicationIntent {
    PublicationIntent {
        publication_intent_id: PublicationIntentId::from_bytes([8; 16]),
        target_epoch: Epoch::new(target).unwrap(),
        prepared_manifest_ref: ReceiptRef::new("cas:port-prepared").unwrap(),
        owner_source_membership_access_guards: vis_state().guards,
        state: PublicationIntentState::Prepared,
    }
}
fn digest() -> Blake3Digest32 {
    Blake3Digest32::from_bytes([9; 32])
}

struct Scratch {
    root: PathBuf,
    handle: RefCell<Option<File>>,
}
impl Scratch {
    fn new() -> Self {
        let stamp = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_nanos();
        let root = std::env::temp_dir().join(format!(
            "eliot-port-{}-{stamp}-{}",
            std::process::id(),
            NEXT.fetch_add(1, Ordering::Relaxed)
        ));
        fs::create_dir(&root).unwrap();
        Self {
            root,
            handle: RefCell::new(None),
        }
    }
    fn path(&self) -> PathBuf {
        self.root.join("control.redb")
    }
    fn keep(&self, file: &File) {
        *self.handle.borrow_mut() = Some(file.try_clone().unwrap());
    }
    fn create3(&self) -> PersistentControlJournal {
        let file = OpenOptions::new()
            .read(true)
            .write(true)
            .create_new(true)
            .open(self.path())
            .unwrap();
        self.keep(&file);
        PersistentControlJournal::create(file, identity3(), LIMITS).unwrap()
    }
    fn bytes(&self) -> Vec<u8> {
        let mut guard = self.handle.borrow_mut();
        let file = guard.as_mut().expect("scratch handle");
        file.seek(SeekFrom::Start(0)).unwrap();
        let mut out = Vec::new();
        file.read_to_end(&mut out).unwrap();
        out
    }
}
impl Drop for Scratch {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.root);
    }
}

fn key(bytes: &[u8]) -> ControlKey {
    ControlKey::new(bytes.to_vec(), LIMITS).unwrap()
}
fn value(bytes: &[u8]) -> ControlValue {
    ControlValue::new(ControlRecordClass::State, bytes.to_vec(), LIMITS).unwrap()
}
fn records_command(id: u8, generation: u64) -> ConditionalControlMutation {
    let mutation = ControlMutation::new(
        MutationId([id; 32]),
        digest(),
        generation,
        vec![ControlWrite {
            key: key(b"port-test/alpha"),
            value: value(b"READY"),
        }],
        vec![],
    );
    ConditionalControlMutation::new(mutation, vec![])
}

#[test]
fn port_closed_records_reject_reserved_and_preserve_idempotency() {
    let scratch = Scratch::new();
    let mut journal = scratch.create3();
    // Schema-3 requires explicit visibility before generic technical writes;
    // otherwise verification would observe a missing route as corruption.
    journal
        .initialize_publication_visibility(
            vis_state(),
            MutationId([39; 32]),
            digest(),
            &context(false),
        )
        .unwrap();
    let mut publisher = ControlSnapshotPublisher::new();
    // Reserved publication key cannot pass the closed port; no durable write occurs.
    {
        let mut bound = journal.bind_control_port::<Cancel>(&mut publisher).unwrap();
        let before = scratch.bytes();
        let reserved = ControlMutation::new(
            MutationId([40; 32]),
            digest(),
            1,
            vec![ControlWrite {
                key: key(b"collection_route/visibility/v1"),
                value: value(b"FORGED"),
            }],
            vec![],
        );
        let reserved = ConditionalControlMutation::new(reserved, vec![]);
        let command = ControlPortCommand::Records(reserved);
        let identity = control_mutation_identity(MutationId([40; 32])).unwrap();
        let error = bound
            .transact(&command, &context(false), &identity)
            .unwrap_err();
        assert_eq!(*error.reason(), ControlError::InvalidKey);
        assert_eq!(error.kind(), PortErrorKind::InvalidInput);
        assert_eq!(scratch.bytes(), before);
        // Valid closed command commits once through the real producer.
        let command = ControlPortCommand::Records(records_command(41, 1));
        let identity = control_mutation_identity(MutationId([41; 32])).unwrap();
        let receipt = bound
            .transact(&command, &context(false), &identity)
            .unwrap();
        assert_eq!(receipt.before_generation, 1);
        assert_eq!(receipt.after_generation, 2);
        assert!(!receipt.replayed);
        // Same operation plus same input replays without a new write.
        let writes = bound
            .write_counters(&context(false))
            .unwrap()
            .acknowledged_mutating_calls;
        let replay = bound
            .transact(&command, &context(false), &identity)
            .unwrap();
        assert!(replay.replayed);
        assert_eq!(replay.before_generation, 1);
        assert_eq!(
            bound
                .write_counters(&context(false))
                .unwrap()
                .acknowledged_mutating_calls,
            writes
        );
        // Same operation plus different input conflicts and preserves the ledger.
        let mutation = ControlMutation::new(
            MutationId([41; 32]),
            Blake3Digest32::from_bytes([77; 32]),
            1,
            vec![ControlWrite {
                key: key(b"port-test/alpha"),
                value: value(b"OTHER"),
            }],
            vec![],
        );
        let conflicting =
            ControlPortCommand::Records(ConditionalControlMutation::new(mutation, vec![]));
        let error = bound
            .transact(&conflicting, &context(false), &identity)
            .unwrap_err();
        assert_eq!(*error.reason(), ControlError::OperationConflict);
        assert_eq!(error.kind(), PortErrorKind::Conflict);
        // Exact recovery distinguishes the committed request from the conflict.
        let decision = bound
            .recover_command(&command, &context(false), &identity)
            .unwrap();
        assert!(matches!(
            decision,
            crate::CommitRecoveryDecision::Committed(_)
        ));
        let decision = bound
            .recover_command(&conflicting, &context(false), &identity)
            .unwrap();
        assert_eq!(decision, crate::CommitRecoveryDecision::ConflictingInput);
    }
    assert_eq!(journal.verify().unwrap().generation, 2);
}

#[test]
fn port_publication_rejects_skipped_guards_and_abnormal_finalization() {
    // Invalidation-only and terminal states have no typed intent path.
    let current =
        PublicationIntentUpdate::begin(MutationId([50; 32]), digest(), 1, &prepared(1)).unwrap();
    for state in [
        PublicationIntentState::ControlCommitted,
        PublicationIntentState::InvalidationOnlyCommitted,
        PublicationIntentState::Reclaimable,
    ] {
        assert!(
            PublicationIntentUpdate::advance(
                MutationId([51; 32]),
                digest(),
                2,
                current.intent().clone(),
                state,
            )
            .is_err(),
            "{state:?}"
        );
    }
    // Valid first intent commits through the port with exact guard wiring.
    let scratch = Scratch::new();
    let mut journal = scratch.create3();
    journal
        .initialize_publication_visibility(
            vis_state(),
            MutationId([52; 32]),
            digest(),
            &context(false),
        )
        .unwrap();
    assert_eq!(journal.verify().unwrap().generation, 1);
    let mut publisher = ControlSnapshotPublisher::new();
    {
        let mut bound = journal.bind_control_port::<Cancel>(&mut publisher).unwrap();
        let update =
            PublicationIntentUpdate::begin(MutationId([53; 32]), digest(), 1, &prepared(1))
                .unwrap();
        let command = ControlPortCommand::PublicationIntent(update);
        let identity = control_mutation_identity(MutationId([53; 32])).unwrap();
        let receipt = bound
            .transact(&command, &context(false), &identity)
            .unwrap();
        assert_eq!(receipt.after_generation, 2);
        // Skipped epoch cannot bypass the stored visibility guards.
        let mut skipped = prepared(2);
        skipped.publication_intent_id = PublicationIntentId::from_bytes([55; 16]);
        let skipped =
            PublicationIntentUpdate::begin(MutationId([54; 32]), digest(), 2, &skipped).unwrap();
        let skipped = ControlPortCommand::PublicationIntent(skipped);
        let skipped_id = control_mutation_identity(MutationId([54; 32])).unwrap();
        let before = scratch.bytes();
        let error = bound
            .transact(&skipped, &context(false), &skipped_id)
            .unwrap_err();
        assert!(
            matches!(
                error.reason(),
                ControlError::GenerationMismatch | ControlError::TransactionConflict
            ),
            "{:?}",
            error.reason()
        );
        assert_eq!(scratch.bytes(), before);
        // Mismatched guard axis cannot echo stale producer state.
        let mut stale = prepared(1);
        stale.publication_intent_id = PublicationIntentId::from_bytes([56; 16]);
        stale
            .owner_source_membership_access_guards
            .source_catalog_generation += 1;
        let stale =
            PublicationIntentUpdate::begin(MutationId([57; 32]), digest(), 2, &stale).unwrap();
        let stale = ControlPortCommand::PublicationIntent(stale);
        let stale_id = control_mutation_identity(MutationId([57; 32])).unwrap();
        let error = bound
            .transact(&stale, &context(false), &stale_id)
            .unwrap_err();
        assert_eq!(*error.reason(), ControlError::GenerationMismatch);
        // Exact recovery still resolves the first committed intent.
        let decision = bound
            .recover_command(&command, &context(false), &identity)
            .unwrap();
        assert!(matches!(
            decision,
            crate::CommitRecoveryDecision::Committed(_)
        ));
    }
    assert_eq!(journal.verify().unwrap().generation, 2);
}

#[test]
fn port_inspection_and_cancellation_are_side_effect_free() {
    let scratch = Scratch::new();
    let mut journal = scratch.create3();
    journal
        .initialize_publication_visibility(
            vis_state(),
            MutationId([60; 32]),
            digest(),
            &context(false),
        )
        .unwrap();
    let mut publisher = ControlSnapshotPublisher::new();
    {
        let mut bound = journal.bind_control_port::<Cancel>(&mut publisher).unwrap();
        let command = ControlPortCommand::Records(records_command(61, 1));
        let identity = control_mutation_identity(MutationId([61; 32])).unwrap();
        let receipt = bound
            .transact(&command, &context(false), &identity)
            .unwrap();
        assert_eq!(receipt.after_generation, 2);
        // Publish the exact commit so admission has a current historical view.
        bound
            .publish_committed_snapshot(&receipt, &context(false))
            .unwrap();
        assert_eq!(bound.current_snapshot().unwrap().generation, 2);
        let before = scratch.bytes();
        let writes = bound
            .write_counters(&context(false))
            .unwrap()
            .acknowledged_mutating_calls;
        // Repeated inspection performs no durable write and keeps the ledger.
        for _ in 0..1_000 {
            let snapshot = bound.read_control_snapshot(&context(false)).unwrap();
            assert_eq!(snapshot.generation, 2);
            let counters = bound.write_counters(&context(false)).unwrap();
            assert_eq!(counters.data_generation, 2);
            assert_eq!(counters.operation_records, 2);
            assert_eq!(counters.acknowledged_mutating_calls, writes);
            assert!(
                bound
                    .load_unresolved_publication(&context(false))
                    .unwrap()
                    .is_none()
            );
            assert_eq!(bound.current_snapshot().unwrap().generation, 2);
        }
        assert_eq!(scratch.bytes(), before);
        assert_eq!(
            bound
                .write_counters(&context(false))
                .unwrap()
                .acknowledged_mutating_calls,
            writes
        );
        // Pre-cancelled inspection and mutation are explicit no-effect failures.
        let cancelled = context(true);
        let error = bound.read_control_snapshot(&cancelled).unwrap_err();
        assert_eq!(error.kind(), PortErrorKind::CancelledBeforeSideEffect);
        let command = ControlPortCommand::Records(records_command(62, 2));
        let identity = control_mutation_identity(MutationId([62; 32])).unwrap();
        let error = bound.transact(&command, &cancelled, &identity).unwrap_err();
        assert_eq!(error.kind(), PortErrorKind::CancelledBeforeSideEffect);
        assert_eq!(scratch.bytes(), before);
    }
    assert_eq!(journal.verify().unwrap().generation, 2);
}

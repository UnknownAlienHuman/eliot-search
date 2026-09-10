//! Synthetic journal/snapshot observations for pure coordinator tests only.
//! This helper performs no redb, Qdrant or operating-system I/O.

use search_contracts::{Blake3Digest32, OpaqueId, ReceiptRef};
use search_publication::{
    AbortControlCommitObservation, AbortFinalizationRequest, PublicationCoordinator,
    SnapshotPublishReceipt,
};

pub fn commit(request: AbortFinalizationRequest) -> AbortControlCommitObservation {
    AbortControlCommitObservation {
        collection_generation_id: request.collection_generation_id,
        visible_epoch: request.previous_visible_epoch,
        last_reserved_epoch: request.intent.target_epoch,
        observed_guards: request.current_guards,
        control_generation: request.expected_control_generation.checked_add(1).unwrap(),
        control_state_digest: Blake3Digest32::from_bytes([51; 32]),
        commit_receipt: ReceiptRef::new("fixture:abort-commit-readback").unwrap(),
        request,
    }
}

pub fn snapshot(commit: &AbortControlCommitObservation) -> SnapshotPublishReceipt {
    SnapshotPublishReceipt {
        transaction_id: commit.request.intent.transaction_id.clone(),
        visible_epoch: commit.visible_epoch,
        control_generation: commit.control_generation,
        // Deliberately distinct: snapshot digest is not a control-state digest cast.
        snapshot_digest: Blake3Digest32::from_bytes([52; 32]),
    }
}

pub fn acknowledge(machine: &mut PublicationCoordinator) {
    let active = machine.active().unwrap();
    let current_guards = active.prepared.guards;
    let target = active.target_epoch.get();
    let operation = OpaqueId::new(format!("fixture:abort-finalize-{target}")).unwrap();
    let generation = 100 + u64::try_from(target).unwrap();
    let request = machine
        .prepare_abort_finalization(operation, generation, current_guards)
        .unwrap();
    let observed = commit(request);
    machine.acknowledge_abort_commit(observed.clone()).unwrap();
    machine.publish_abort_snapshot(snapshot(&observed)).unwrap();
}

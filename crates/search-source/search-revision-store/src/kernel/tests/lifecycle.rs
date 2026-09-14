use search_contracts::{NonZeroRevision, OpaqueId, ReceiptRef};

use super::fixtures::*;
use super::super::*;

#[test]
fn purge_tombstone_fences_residency_writes() {
    let mut store = RevisionStore::new(DEFAULT_REVISION_STORE_LIMITS).expect("store");
    let tombstone = PurgeTombstone {
        scope: TombstoneScope::Residency(baseline_residency()),
        generation: NonZeroRevision::new(1).expect("generation"),
        tombstone_receipt: ReceiptRef::new("receipt:tombstone:1").expect("receipt"),
        operation: operation("tombstone", 1),
    };
    let installed = store
        .install_purge_tombstone(tombstone.clone())
        .expect("install");
    assert!(!installed.replayed);
    let replayed = store.install_purge_tombstone(tombstone).expect("reinstall");
    assert!(replayed.replayed);
    let conflicting = PurgeTombstone {
        scope: TombstoneScope::Residency(baseline_residency()),
        generation: NonZeroRevision::new(1).expect("generation"),
        tombstone_receipt: ReceiptRef::new("receipt:tombstone:other").expect("receipt"),
        operation: operation("tombstone-other", 2),
    };
    assert_eq!(
        store.install_purge_tombstone(conflicting),
        Err(RevisionStoreError::RevisionConflict)
    );
    assert_eq!(
        store.prepare_append(intent(1, "fenced", 1)),
        Err(RevisionStoreError::Tombstoned)
    );
    let free = intent_full(
        source("scope-free"),
        residency_variant(0),
        1,
        "scope-free",
        5,
        5,
        5,
        "secret:revision-key",
    );
    store.prepare_append(free).expect("exact fence");
    store
        .install_purge_tombstone(PurgeTombstone {
            scope: TombstoneScope::Scope(baseline_residency().scope),
            generation: NonZeroRevision::new(2).expect("generation"),
            tombstone_receipt: ReceiptRef::new("receipt:tombstone:scope").expect("receipt"),
            operation: operation("tombstone-scope", 3),
        })
        .expect("scope fence");
    let scoped = intent_full(
        source("scoped"),
        residency_variant(1),
        1,
        "scoped",
        6,
        6,
        6,
        "secret:revision-key",
    );
    assert_eq!(
        store.prepare_append(scoped),
        Err(RevisionStoreError::Tombstoned)
    );
    let mut pending = intent_full(
        source("pending-fenced"),
        residency_variant(0),
        1,
        "pending-fenced",
        7,
        7,
        7,
        "secret:revision-key",
    );
    pending.storage_object_id = OpaqueId::new("object:pending-fenced-own").expect("object");
    store.prepare_append(pending.clone()).expect("pending");
    store
        .install_purge_tombstone(PurgeTombstone {
            scope: TombstoneScope::Residency(residency_variant(0)),
            generation: NonZeroRevision::new(3).expect("generation"),
            tombstone_receipt: ReceiptRef::new("receipt:tombstone:pending").expect("receipt"),
            operation: operation("tombstone-pending", 4),
        })
        .expect("pending fence");
    assert_eq!(
        store.confirm_append(&pending.key, &pending.operation, readback(&pending)),
        Err(RevisionStoreError::Tombstoned)
    );
}

#[test]
fn exact_deletion_requires_lifecycle_plan_and_exact_address() {
    let mut store = RevisionStore::new(DEFAULT_REVISION_STORE_LIMITS).expect("store");
    let first = intent(1, "one", 1);
    confirm(&mut store, &first);
    let wrong_address = LifecycleDeletionPlan {
        plan_receipt: ReceiptRef::new("receipt:lifecycle-plan:1").expect("receipt"),
        authority: DeletionAuthorityKind::OrdinarySweep,
        target: first.key.clone(),
        target_storage_object_id: OpaqueId::new("object:wrong").expect("object"),
        operation: operation("delete-one", 1),
    };
    assert_eq!(
        store.apply_exact_object_deletion(wrong_address),
        Err(RevisionStoreError::DeletionNotAuthorized)
    );
    let unknown = LifecycleDeletionPlan {
        plan_receipt: ReceiptRef::new("receipt:lifecycle-plan:1").expect("receipt"),
        authority: DeletionAuthorityKind::OrdinarySweep,
        target: key(2),
        target_storage_object_id: OpaqueId::new("object:unknown").expect("object"),
        operation: operation("delete-unknown", 2),
    };
    assert_eq!(
        store.apply_exact_object_deletion(unknown),
        Err(RevisionStoreError::RevisionNotFound)
    );
    let plan = LifecycleDeletionPlan {
        plan_receipt: ReceiptRef::new("receipt:lifecycle-plan:1").expect("receipt"),
        authority: DeletionAuthorityKind::OrdinarySweep,
        target: first.key.clone(),
        target_storage_object_id: first.storage_object_id.clone(),
        operation: operation("delete-one", 3),
    };
    let receipt = store
        .apply_exact_object_deletion(plan.clone())
        .expect("delete");
    assert!(!receipt.replayed);
    assert_eq!(receipt.authority, DeletionAuthorityKind::OrdinarySweep);
    let replayed = store.apply_exact_object_deletion(plan).expect("replay");
    assert!(replayed.replayed);
    let reused_operation = LifecycleDeletionPlan {
        plan_receipt: ReceiptRef::new("receipt:lifecycle-plan:other").expect("receipt"),
        authority: DeletionAuthorityKind::OrdinarySweep,
        target: first.key.clone(),
        target_storage_object_id: first.storage_object_id.clone(),
        operation: operation("delete-one", 3),
    };
    assert_eq!(
        store.apply_exact_object_deletion(reused_operation),
        Err(RevisionStoreError::OperationConflict)
    );
    assert_eq!(
        store.active_record(&first.key),
        Err(RevisionStoreError::RevisionNotFound)
    );
    confirm(&mut store, &intent(1, "one", 1));
    let second = intent(2, "two", 2);
    confirm(&mut store, &second);
    let purge = LifecycleDeletionPlan {
        plan_receipt: ReceiptRef::new("receipt:lifecycle-plan:purge").expect("receipt"),
        authority: DeletionAuthorityKind::SecurityPurge,
        target: second.key.clone(),
        target_storage_object_id: second.storage_object_id.clone(),
        operation: operation("delete-two", 4),
    };
    let purge_receipt = store.apply_exact_object_deletion(purge).expect("purge");
    assert_eq!(
        purge_receipt.authority,
        DeletionAuthorityKind::SecurityPurge
    );
    store
        .install_purge_tombstone(PurgeTombstone {
            scope: TombstoneScope::Residency(baseline_residency()),
            generation: NonZeroRevision::new(1).expect("generation"),
            tombstone_receipt: ReceiptRef::new("receipt:tombstone:purge").expect("receipt"),
            operation: operation("tombstone-purge", 5),
        })
        .expect("purge fence");
    assert_eq!(
        store.prepare_append(intent(2, "two", 2)),
        Err(RevisionStoreError::Tombstoned)
    );
    let pending = intent_full(
        source("pending-src"),
        residency_variant(0),
        1,
        "pending-del",
        5,
        5,
        5,
        "secret:pending-key",
    );
    store.prepare_append(pending.clone()).expect("pending");
    let pending_plan = LifecycleDeletionPlan {
        plan_receipt: ReceiptRef::new("receipt:lifecycle-plan:pending").expect("receipt"),
        authority: DeletionAuthorityKind::OrdinarySweep,
        target: pending.key.clone(),
        target_storage_object_id: pending.storage_object_id,
        operation: operation("delete-pending", 6),
    };
    assert_eq!(
        store.apply_exact_object_deletion(pending_plan),
        Err(RevisionStoreError::OutcomeUnknown)
    );
}

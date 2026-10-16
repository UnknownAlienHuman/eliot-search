use search_contracts::{NonZeroRevision, OpaqueId, ReceiptRef};

use super::super::*;
use super::fixtures::*;

fn scopes() -> [TombstoneScope; 2] {
    [
        TombstoneScope::Residency(baseline_residency()),
        TombstoneScope::Scope(baseline_residency().scope),
    ]
}

fn install_fence(store: &mut RevisionStore, scope: TombstoneScope) {
    store
        .install_purge_tombstone(PurgeTombstone {
            scope,
            generation: NonZeroRevision::new(1).expect("generation"),
            tombstone_receipt: ReceiptRef::new("receipt:purge-fence").expect("receipt"),
            operation: operation("purge-fence", 91),
        })
        .expect("install fence");
}

#[test]
fn purge_after_active_blocks_reads_and_every_exact_replay_without_changing_history() {
    for scope in scopes() {
        let limits = RevisionStoreLimits {
            max_operations: 4,
            max_revisions: 3,
            ..DEFAULT_REVISION_STORE_LIMITS
        };
        let mut store = RevisionStore::new(limits).expect("store");
        let first = intent(1, "one", 1);
        let free = intent_full(
            source("free"),
            residency_variant(0),
            1,
            "free",
            7,
            7,
            7,
            "secret:free-key",
        );
        let sibling = intent_full(
            source("sibling"),
            residency_variant(1),
            1,
            "sibling",
            9,
            9,
            9,
            "secret:sibling-key",
        );
        confirm(&mut store, &first);
        let free_receipt = confirm(&mut store, &free);
        confirm(&mut store, &sibling);
        let historical = store.state(&first.key).expect("active state").clone();
        install_fence(&mut store, scope);

        for _ in 0..2 {
            assert_eq!(
                store.active_record(&first.key),
                Err(RevisionStoreError::Tombstoned)
            );
            assert_eq!(
                store.prepare_append(first.clone()),
                Err(RevisionStoreError::Tombstoned)
            );
            assert_eq!(
                store.confirm_append(&first.key, &first.operation, readback(&first)),
                Err(RevisionStoreError::Tombstoned)
            );
            for observed in [Some(readback(&first)), None] {
                assert_eq!(
                    store.recover_unknown(&first.key, &first.operation, observed),
                    Err(RevisionStoreError::Tombstoned)
                );
            }
            assert_eq!(store.state(&first.key), Ok(&historical));
            assert_eq!(store.len(), 3);
        }

        // The complete-residency fence leaves another access domain alone;
        // the whole-scope fence also denies that same-scope sibling.
        if matches!(scope, TombstoneScope::Scope(_)) {
            assert_eq!(
                store.active_record(&sibling.key),
                Err(RevisionStoreError::Tombstoned)
            );
            assert_eq!(
                store.prepare_append(sibling),
                Err(RevisionStoreError::Tombstoned)
            );
        } else {
            assert!(store.active_record(&sibling.key).is_ok());
            assert!(matches!(
                store.prepare_append(sibling),
                Ok(PrepareAppendResult::AlreadyStored(_))
            ));
        }

        // A cached exact replay still succeeds outside the fence at full capacity.
        let mut expected = free_receipt;
        expected.replayed = true;
        assert_eq!(
            store.prepare_append(free.clone()),
            Ok(PrepareAppendResult::AlreadyStored(expected.clone()))
        );
        assert_eq!(
            store.confirm_append(&free.key, &free.operation, readback(&free)),
            Ok(expected.clone())
        );
        assert_eq!(
            store.recover_unknown(&free.key, &free.operation, None),
            Ok(RecoveryResult::Applied(expected))
        );
        assert!(store.active_record(&free.key).is_ok());
        let extra = intent_full(
            source("extra"),
            residency_variant(0),
            1,
            "extra",
            8,
            8,
            8,
            "secret:free-key",
        );
        assert_eq!(
            store.prepare_append(extra),
            Err(RevisionStoreError::CapacityExceeded)
        );
    }
}

#[test]
fn purge_after_pending_or_unknown_preserves_unresolved_evidence() {
    for scope in scopes() {
        for unknown in [false, true] {
            let mut store = RevisionStore::new(DEFAULT_REVISION_STORE_LIMITS).expect("store");
            let first = intent(1, "one", 1);
            store.prepare_append(first.clone()).expect("pending");
            if unknown {
                store
                    .mark_outcome_unknown(&first.key, &first.operation)
                    .expect("unknown");
            }
            install_fence(&mut store, scope);
            let historical = store.state(&first.key).expect("retained state").clone();

            assert_eq!(
                store.active_record(&first.key),
                Err(RevisionStoreError::Tombstoned)
            );
            assert_eq!(
                store.prepare_append(first.clone()),
                Err(RevisionStoreError::Tombstoned)
            );
            assert_eq!(
                store.confirm_append(&first.key, &first.operation, readback(&first)),
                Err(RevisionStoreError::Tombstoned)
            );
            let wrong = operation("wrong", 2);
            assert_eq!(
                store.confirm_append(&first.key, &wrong, readback(&first)),
                Err(RevisionStoreError::OperationConflict)
            );
            assert_eq!(
                store.recover_unknown(&first.key, &wrong, None),
                Err(RevisionStoreError::OperationConflict)
            );
            if unknown {
                for _ in 0..2 {
                    for observed in [Some(readback(&first)), None] {
                        assert_eq!(
                            store.recover_unknown(&first.key, &first.operation, observed),
                            Err(RevisionStoreError::Tombstoned)
                        );
                    }
                }
            }
            assert_eq!(store.state(&first.key), Ok(&historical));
            assert_eq!(
                store.apply_exact_object_deletion(LifecycleDeletionPlan {
                    plan_receipt: ReceiptRef::new("receipt:purge-plan").expect("receipt"),
                    authority: DeletionAuthorityKind::SecurityPurge,
                    target: first.key.clone(),
                    target_storage_object_id: first.storage_object_id.clone(),
                    operation: operation("cleanup-pending", 92),
                }),
                Err(RevisionStoreError::OutcomeUnknown)
            );
            assert_eq!(store.state(&first.key), Ok(&historical));
        }
    }
}

#[test]
fn purge_does_not_rebind_conflicting_operation_identities() {
    for scope in scopes() {
        let mut store = RevisionStore::new(DEFAULT_REVISION_STORE_LIMITS).expect("store");
        let first = intent(1, "one", 1);
        confirm(&mut store, &first);
        install_fence(&mut store, scope);
        let historical = store.state(&first.key).expect("historical state").clone();
        let mut conflicting = first.clone();
        conflicting.operation = operation("one", 2);
        assert_eq!(
            store.prepare_append(conflicting),
            Err(RevisionStoreError::OperationConflict)
        );
        let wrong = operation("wrong", 2);
        assert_eq!(
            store.confirm_append(&first.key, &wrong, readback(&first)),
            Err(RevisionStoreError::RevisionConflict)
        );
        assert_eq!(
            store.recover_unknown(&first.key, &wrong, None),
            Err(RevisionStoreError::RevisionConflict)
        );
        assert_eq!(store.state(&first.key), Ok(&historical));
    }
}

#[test]
fn purge_keeps_exact_lifecycle_cleanup_available_without_resurrection() {
    for scope in scopes() {
        let mut store = RevisionStore::new(DEFAULT_REVISION_STORE_LIMITS).expect("store");
        let first = intent(1, "one", 1);
        confirm(&mut store, &first);
        install_fence(&mut store, scope);
        let plan = LifecycleDeletionPlan {
            plan_receipt: ReceiptRef::new("receipt:purge-plan").expect("receipt"),
            authority: DeletionAuthorityKind::SecurityPurge,
            target: first.key.clone(),
            target_storage_object_id: first.storage_object_id.clone(),
            operation: operation("cleanup", 92),
        };
        let mut wrong = plan.clone();
        wrong.target_storage_object_id = OpaqueId::new("object:wrong").expect("object");
        assert_eq!(
            store.apply_exact_object_deletion(wrong),
            Err(RevisionStoreError::DeletionNotAuthorized)
        );
        let receipt = store
            .apply_exact_object_deletion(plan.clone())
            .expect("exact cleanup");
        let mut replay = receipt.clone();
        replay.replayed = true;
        assert_eq!(store.apply_exact_object_deletion(plan), Ok(replay));
        assert_eq!(
            store.state(&first.key),
            Err(RevisionStoreError::RevisionNotFound)
        );
        assert_eq!(
            store.active_record(&first.key),
            Err(RevisionStoreError::Tombstoned)
        );
        assert_eq!(
            store.prepare_append(first),
            Err(RevisionStoreError::Tombstoned)
        );
    }
}

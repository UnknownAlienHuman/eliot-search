//! Synthetic in-memory lifecycle fixtures, not external pin/storage qualification.

use std::collections::BTreeSet;

use super::atomicity_tests::{emitted, insert, snapshot, store, time};
use super::{
    AuthorizationFence, Blake3Digest32, BoundedList, ContinuationCredential,
    ContinuationEffect, ContinuationError, ContinuationLimits, ContinuationPayload,
    ContinuationPermit, ContinuationRecord, ContinuationStore, ContinuityFence,
    CurrencyFence, InvalidationReason, InvalidationScope, LifecycleRecordStatus,
    LiveContinuationState, NonZeroRevision, OpaqueId, ResumePlan, StoredContinuation,
};

fn permit(store: &ContinuationStore, credential: &ContinuationCredential) -> ContinuationPermit {
    let value = &store.records[&credential.continuation_id];
    let live = LiveContinuationState {
        binding_id: value.binding_id(),
        plan_fingerprint: value.plan_fingerprint(),
        result_fence: value.result_fence().clone(),
        authorization: AuthorizationFence {
            grant_active: true,
            security_permits: true,
            purge_clear: true,
        },
        currency: CurrencyFence {
            owner_generation_current: true,
            view_current: true,
            route_current: true,
        },
        continuity: ContinuityFence {
            profile_current: true,
            durable_job_active: true,
            epoch_pin_valid: true,
        },
    };
    match store.resume(credential, &live, &time(0), 3).unwrap() {
        ResumePlan::DurableReplan { permit, .. } => {
            let remaining = (1_u8..=3)
                .filter(|value| {
                    !store.records[&credential.continuation_id]
                        .issued
                        .contains(&Blake3Digest32::from_bytes([*value; 32]))
                })
                .collect::<Vec<_>>();
            store
                .bind_durable_emission(&permit, &live, &time(0), &emitted(&remaining))
                .unwrap()
        }
        ResumePlan::EphemeralWindow { permit, .. } | ResumePlan::Exhausted { permit } => permit,
    }
}

fn issued_once(store: &mut ContinuationStore, credential: &ContinuationCredential) {
    let permit = permit(store, credential);
    let receipt = store.commit_emission(&permit, &emitted(&[1])).unwrap();
    assert!(!receipt.completed);
    assert_eq!(receipt.issued_total, 1);
}

fn assert_released(
    store: &mut ContinuationStore,
    credential: &ContinuationCredential,
    before: &StoredContinuation,
    status: LifecycleRecordStatus,
) {
    let value = store.records.get_mut(&credential.continuation_id).unwrap();
    assert_eq!(value.status(), status);
    assert!(value.issued.is_empty());
    assert_eq!(value.cleanup_effect(), before.cleanup_effect());
    assert_eq!(value.is_ephemeral(), before.is_ephemeral());

    // Inspect the actual retained backing allocation, not a clone of an empty
    // vector (whose capacity could be zero even if the original was retained).
    match (&mut value.payload, &before.payload) {
        (
            ContinuationPayload::Ephemeral { boot_id, candidates },
            ContinuationPayload::Ephemeral { boot_id: original_boot, .. },
        ) => {
            assert_eq!(&*boot_id, original_boot);
            let released = std::mem::take(candidates).into_vec();
            assert!(released.is_empty());
            assert_eq!(released.capacity(), 0);
            *candidates = BoundedList::new(released).unwrap();
        }
        (ContinuationPayload::DurableReplan, ContinuationPayload::DurableReplan) => {}
        _ => panic!("terminal transition changed durability"),
    }
    let mut expected_record = before.record.clone();
    match &mut expected_record {
        ContinuationRecord::EphemeralWindow(record) => record.status = status,
        ContinuationRecord::DurableReplanCheckpoint(record) => record.status = status,
    }
    assert_eq!(value.record, expected_record);
    assert_eq!(value.revision, before.revision + 1);
    assert_eq!(store.token_index.get(&credential.token_digest), Some(&credential.continuation_id));
}

#[test]
fn explicit_completion_releases_local_state_before_compaction() {
    for durable in [false, true] {
        let mut store = store();
        let credential = insert(&mut store, 1, durable, 7);
        issued_once(&mut store, &credential);
        let before = store.records[&credential.continuation_id].clone();
        let permit = permit(&store, &credential);
        assert_eq!(store.complete(&permit), Ok(before.cleanup_effect()));
        assert_released(&mut store, &credential, &before, LifecycleRecordStatus::Revoked);
        assert_eq!(store.len(), 1);
        assert_eq!(store.resolve(&credential), Err(ContinuationError::SnapshotExpired));
    }
}

#[test]
fn final_emission_receipt_keeps_totals_after_issued_storage_is_released() {
    let mut store = store();
    let credential = insert(&mut store, 1, false, 7);
    issued_once(&mut store, &credential);
    let before = store.records[&credential.continuation_id].clone();
    let permit = permit(&store, &credential);
    let receipt = store.commit_emission(&permit, &emitted(&[2, 3])).unwrap();
    assert_eq!((receipt.emitted_count, receipt.issued_total, receipt.completed), (2, 3, true));
    assert_eq!(receipt.cleanup_effect, Some(before.cleanup_effect()));
    assert_released(&mut store, &credential, &before, LifecycleRecordStatus::Revoked);
    let retired = snapshot(&store);
    assert_eq!(
        store.commit_emission(&permit, &emitted(&[2])),
        Err(ContinuationError::StalePermit),
    );
    assert_eq!(snapshot(&store), retired);
}

#[test]
fn partial_emission_keeps_the_original_window_and_issued_suppression() {
    let mut store = store();
    let credential = insert(&mut store, 1, false, 7);
    let before = store.records[&credential.continuation_id].payload.clone();
    let original_pointer = match &store.records[&credential.continuation_id].payload {
        ContinuationPayload::Ephemeral { candidates, .. } => candidates.as_slice().as_ptr(),
        ContinuationPayload::DurableReplan => panic!("expected ephemeral fixture"),
    };
    issued_once(&mut store, &credential);
    let value = &store.records[&credential.continuation_id];
    assert!(value.is_active());
    assert_eq!(value.payload, before);
    assert_eq!(value.issued, emitted(&[1]).iter().copied().collect::<BTreeSet<_>>());
    if let ContinuationPayload::Ephemeral { candidates, .. } = &value.payload {
        assert_eq!(candidates.as_slice().as_ptr(), original_pointer);
    }
    let next = permit(&store, &credential);
    assert_eq!(
        store.commit_emission(&next, &emitted(&[1])),
        Err(ContinuationError::DuplicateCandidate),
    );
    assert!(!store.commit_emission(&next, &emitted(&[2])).unwrap().completed);
}

#[test]
fn every_invalidation_reason_releases_only_its_selected_record() {
    for reason in [
        InvalidationReason::AccessRevoked,
        InvalidationReason::Purged,
        InvalidationReason::OwnerGenerationChanged,
        InvalidationReason::ViewChanged,
        InvalidationReason::RouteChanged,
        InvalidationReason::ProfileChanged,
        InvalidationReason::DurableJobChanged,
        InvalidationReason::Restart,
        InvalidationReason::Completed,
    ] {
        for durable in [false, true] {
            let mut store = store();
            let credential = insert(&mut store, 1, durable, 7);
            let survivor = insert(&mut store, 2, !durable, 7);
            issued_once(&mut store, &credential);
            let before = store.records[&credential.continuation_id].clone();
            let survivor_before = store.records[&survivor.continuation_id].clone();
            let generation = NonZeroRevision::new(2).unwrap();
            let scope = InvalidationScope::Continuation(credential.continuation_id);
            let receipt = store.invalidate(&scope, reason, generation).unwrap();
            assert_eq!(receipt.invalidated.as_slice(), &[credential.continuation_id]);
            assert_eq!(receipt.effects.as_slice(), &[before.cleanup_effect()]);
            assert_released(&mut store, &credential, &before, LifecycleRecordStatus::Revoked);
            let value = &store.records[&credential.continuation_id];
            assert_eq!(value.terminal_reason, Some(reason));
            assert_eq!(value.last_invalidation_generation, Some(generation));
            assert_eq!(store.records[&survivor.continuation_id], survivor_before);
            assert_eq!(store.resolve(&credential), Err(super::terminal_error(Some(reason))));
            let retired = snapshot(&store);
            assert!(store.invalidate(&scope, reason, generation).unwrap().effects.is_empty());
            assert_eq!(snapshot(&store), retired);
        }
    }
}

#[test]
fn bounded_expiry_releases_each_selected_window_without_an_early_sweep() {
    let mut store = store();
    store.limits.max_lifecycle_batch = 1;
    let first = insert(&mut store, 1, false, 7);
    let second = insert(&mut store, 2, true, 7);
    issued_once(&mut store, &first);
    issued_once(&mut store, &second);
    let first_before = store.records[&first.continuation_id].clone();
    let second_before = store.records[&second.continuation_id].clone();
    let receipt = store.expire(&time(1)).unwrap();
    assert_eq!(receipt.expired.as_slice(), &[first.continuation_id]);
    assert!(receipt.more_remaining);
    assert_released(&mut store, &first, &first_before, LifecycleRecordStatus::Expired);
    assert_eq!(store.records[&second.continuation_id], second_before);
    let receipt = store.expire(&time(1)).unwrap();
    assert_eq!(receipt.expired.as_slice(), &[second.continuation_id]);
    assert!(!receipt.more_remaining);
    assert_released(&mut store, &second, &second_before, LifecycleRecordStatus::Expired);
    assert!(store.expire(&time(1)).unwrap().effects.is_empty());
}

#[test]
fn restart_releases_old_boot_windows_but_keeps_current_and_durable_state() {
    let mut store = store();
    let old = insert(&mut store, 1, false, 7);
    let current = insert(&mut store, 2, false, 7);
    let durable = insert(&mut store, 3, true, 7);
    for credential in [&old, &current, &durable] {
        issued_once(&mut store, credential);
    }
    let current_boot = OpaqueId::new("synthetic:current-boot").unwrap();
    if let ContinuationPayload::Ephemeral { boot_id, .. } =
        &mut store.records.get_mut(&current.continuation_id).unwrap().payload
    {
        *boot_id = current_boot.clone();
    }
    let old_before = store.records[&old.continuation_id].clone();
    let current_before = store.records[&current.continuation_id].clone();
    let durable_before = store.records[&durable.continuation_id].clone();
    let receipt = store.invalidate_restart(&current_boot, NonZeroRevision::new(2).unwrap()).unwrap();
    assert_eq!(receipt.invalidated.as_slice(), &[old.continuation_id]);
    assert_released(&mut store, &old, &old_before, LifecycleRecordStatus::Revoked);
    assert_eq!(store.records[&current.continuation_id], current_before);
    assert_eq!(store.records[&durable.continuation_id], durable_before);
}

#[test]
fn restrictive_window_and_ttl_limits_release_selected_state() {
    for ttl_only in [false, true] {
        let mut store = store();
        let ephemeral = insert(&mut store, 1, false, 7);
        let durable = insert(&mut store, 2, true, 7);
        issued_once(&mut store, &ephemeral);
        issued_once(&mut store, &durable);
        let before_ephemeral = store.records[&ephemeral.continuation_id].clone();
        let before_durable = store.records[&durable.continuation_id].clone();
        let limits = if ttl_only {
            ContinuationLimits { max_ttl_millis: 1, ..store.limits() }
        } else {
            ContinuationLimits {
                max_candidate_window: 1,
                max_expansion_items: 1,
                ..store.limits()
            }
        };
        let receipt = store.apply_live_limits(limits).unwrap();
        assert_released(&mut store, &ephemeral, &before_ephemeral, LifecycleRecordStatus::Expired);
        if ttl_only {
            assert_released(&mut store, &durable, &before_durable, LifecycleRecordStatus::Expired);
            assert_eq!(receipt.effects.len(), 2);
        } else {
            assert_eq!(store.records[&durable.continuation_id], before_durable);
            assert_eq!(receipt.effects.len(), 1);
        }
        let retired = snapshot(&store);
        assert!(store.apply_live_limits(limits).unwrap().effects.is_empty());
        assert_eq!(snapshot(&store), retired);
    }
}

#[test]
fn failed_terminal_batch_does_not_free_a_prefix_of_candidate_windows() {
    let mut store = store();
    insert(&mut store, 1, false, 7);
    insert(&mut store, 2, false, u64::MAX);
    let before = snapshot(&store);
    assert_eq!(store.expire(&time(1)), Err(ContinuationError::RevisionExhausted));
    assert_eq!(snapshot(&store), before);
    assert_eq!(
        store.invalidate(&InvalidationScope::All, InvalidationReason::Purged, NonZeroRevision::new(2).unwrap()),
        Err(ContinuationError::RevisionExhausted),
    );
    assert_eq!(snapshot(&store), before);
    let limits = ContinuationLimits { max_ttl_millis: 1, ..store.limits() };
    assert_eq!(store.apply_live_limits(limits), Err(ContinuationError::RevisionExhausted));
    assert_eq!(snapshot(&store), before);
}

#[test]
fn failed_single_record_transition_keeps_issued_set_and_window() {
    for durable in [false, true] {
        let mut store = store();
        let credential = insert(&mut store, 1, durable, 7);
        issued_once(&mut store, &credential);
        store.records.get_mut(&credential.continuation_id).unwrap().revision = u64::MAX;
        let permit = permit(&store, &credential);
        let before = snapshot(&store);
        assert_eq!(store.complete(&permit), Err(ContinuationError::RevisionExhausted));
        assert_eq!(snapshot(&store), before);
        assert_eq!(
            store.commit_emission(&permit, &emitted(&[2, 3])),
            Err(ContinuationError::RevisionExhausted),
        );
        assert_eq!(snapshot(&store), before);
    }
}

#[test]
fn failed_batch_bound_preserves_all_local_memory_and_old_limits() {
    let mut store = store();
    insert(&mut store, 1, false, 7);
    insert(&mut store, 2, false, 7);
    store.limits.max_lifecycle_batch = 1;
    let before = snapshot(&store);
    let limits = ContinuationLimits { max_ttl_millis: 1, ..store.limits() };
    assert_eq!(store.apply_live_limits(limits), Err(ContinuationError::ResourceExhausted));
    assert_eq!(snapshot(&store), before);
}

#[test]
fn freed_windows_keep_tombstones_and_token_quota_until_compaction() {
    let mut store = store();
    store.limits = ContinuationLimits {
        max_records: 1,
        max_ephemeral_per_binding: 1,
        max_durable_per_binding: 1,
        ..store.limits()
    };
    let credential = insert(&mut store, 1, false, 7);
    let before = store.records[&credential.continuation_id].clone();
    let permit = permit(&store, &credential);
    let cleanup = store.complete(&permit).unwrap();
    assert!(matches!(cleanup, ContinuationEffect::ReleaseEpochPin { .. }));
    assert_released(&mut store, &credential, &before, LifecycleRecordStatus::Revoked);
    assert_eq!(
        store.validate_capacity(credential.binding_id, true),
        Err(ContinuationError::ResourceExhausted),
    );
    assert_eq!(store.resolve(&credential), Err(ContinuationError::SnapshotExpired));
    // The fixture models caller-confirmed cleanup; this does not release a real pin.
    assert_eq!(store.compact_terminal(1).unwrap().as_slice(), &[credential.continuation_id]);
    assert!(store.is_empty());
    assert!(store.token_index.is_empty());
    assert_eq!(store.resolve(&credential), Err(ContinuationError::NotAuthorized));
}

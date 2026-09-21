//! Synthetic in-memory fault tests for the canonical owner, not runtime qualification.
//! The private revision injection reaches exhaustion without 2^64 operations.

use std::collections::{BTreeMap, BTreeSet};

use super::{
    AuthorizationFence, BindingId, Blake3Digest32, BoundedList, CandidateWindowItem,
    ContinuationCredential, ContinuationError, ContinuationId, ContinuationLimits,
    ContinuationPayload, ContinuationPermit, ContinuationRecord, ContinuationStore,
    ContinuityFence, CurrencyFence, HandleTokenDigest, InvalidationReason, InvalidationScope,
    LifecycleRecordStatus, LiveContinuationState, MAX_LIST_ITEMS, NonZeroRevision, OpaqueId,
    OpaqueRef, PlanFingerprint, ResultFence, ResumePlan, StoredContinuation, UtcTimestamp, bounded,
};
use search_contracts::{
    AccessPolicyRevision, CatalogRevision, CollectionRouteRevision, DurableReplanCheckpoint,
    EmissionSecurityFence, EphemeralWindowContinuation, InstallationIncarnationId,
    MembershipRevision, ObservationCursorRevision, ObservationFreshness, ObservationFreshnessState,
    OverlayRevision, PurgeFenceRevision, QuerySnapshotFence, QuerySnapshotFingerprint, ReceiptRef,
    ShadowFenceRevision, SourceView, WorkspaceId, WorkspaceViewRevisionId, WorkspaceViewSource,
};

pub(super) fn time(minute: u8) -> UtcTimestamp {
    UtcTimestamp::parse(format!("2026-09-21T00:{minute:02}:00.000000Z")).expect("fixture time")
}

fn opaque(value: &str) -> OpaqueRef {
    OpaqueRef::new(value).expect("fixture reference")
}

fn fence() -> ResultFence {
    let workspace_revision = WorkspaceViewRevisionId::from_bytes([1; 16]);
    ResultFence {
        planned_snapshot: QuerySnapshotFence {
            installation_incarnation_id: InstallationIncarnationId::from_bytes([2; 16]),
            collection_generation_id: None,
            visible_epoch: None,
            collection_route_revision: CollectionRouteRevision::new(1),
            catalog_revision: CatalogRevision::new(1),
            membership_revision: MembershipRevision::new(1),
            reference_portfolio_revision: None,
            access_policy_revision: AccessPolicyRevision::new(1),
            shadow_fence_revision: ShadowFenceRevision::new(1),
            purge_fence_revision: PurgeFenceRevision::new(1),
            overlay_revision: OverlayRevision::new(1),
            observation_cursor_revision: ObservationCursorRevision::new(1),
            observation_freshness: ObservationFreshness {
                state: ObservationFreshnessState::Unknown,
                observation_cursor_revision: ObservationCursorRevision::new(1),
                observed_age_ms: None,
            },
            source_view: SourceView::WorkingTreeCurrent(WorkspaceViewSource {
                workspace_instance_id: WorkspaceId::from_bytes([3; 16]),
                workspace_view_revision_ref: workspace_revision,
            }),
            workspace_view_revision_ref: Some(workspace_revision),
            lexical_profile_ids: BoundedList::empty(),
            snapshot_fingerprint: QuerySnapshotFingerprint::from_bytes([4; 32]),
        },
        emission_source_owner_fences: BoundedList::empty(),
        emission_security_fence: EmissionSecurityFence {
            access_policy_revision: AccessPolicyRevision::new(1),
            live_deny_generation: 0,
            shadow_fence_revision: ShadowFenceRevision::new(1),
            purge_fence_revision: PurgeFenceRevision::new(1),
            checked_at: time(0),
            receipt_ref: ReceiptRef::new("synthetic:security").expect("fixture reference"),
        },
        result_fingerprint: Blake3Digest32::from_bytes([5; 32]),
    }
}

pub(super) fn insert(
    store: &mut ContinuationStore,
    seed: u8,
    durable: bool,
    revision: u64,
) -> ContinuationCredential {
    let id = ContinuationId::from_bytes([seed; 16]);
    let digest = HandleTokenDigest::from_bytes([seed; 32]);
    let binding = BindingId::from_bytes([1; 16]);
    let record = if durable {
        ContinuationRecord::DurableReplanCheckpoint(DurableReplanCheckpoint {
            continuation_id: id,
            token_digest: digest,
            binding_id: binding,
            plan_fingerprint: PlanFingerprint::from_bytes([6; 32]),
            result_fence: fence(),
            durable_job_ref: opaque(&format!("synthetic:job:{seed}")),
            replan_checkpoint_ref: opaque(&format!("synthetic:checkpoint:{seed}")),
            issued_candidate_identity_set_ref: opaque("synthetic:issued"),
            created_at: time(0),
            expires_at: time(1),
            status: LifecycleRecordStatus::Active,
        })
    } else {
        ContinuationRecord::EphemeralWindow(EphemeralWindowContinuation {
            continuation_id: id,
            token_digest: digest,
            binding_id: binding,
            plan_fingerprint: PlanFingerprint::from_bytes([6; 32]),
            result_fence: fence(),
            candidate_window_ref: opaque("synthetic:window"),
            issued_candidate_identity_set_ref: opaque("synthetic:issued"),
            epoch_pin_ref: opaque(&format!("synthetic:pin:{seed}")),
            created_at: time(0),
            expires_at: time(1),
            status: LifecycleRecordStatus::Active,
        })
    };
    let payload = if durable {
        ContinuationPayload::DurableReplan
    } else {
        ContinuationPayload::Ephemeral {
            boot_id: OpaqueId::new("synthetic:old-boot").expect("fixture boot"),
            candidates: bounded(
                (1..=3)
                    .map(|value| CandidateWindowItem {
                        candidate_ref: opaque(&format!("synthetic:candidate:{value}")),
                        fingerprint: Blake3Digest32::from_bytes([value; 32]),
                    })
                    .collect(),
            )
            .expect("fixture candidates"),
        }
    };
    store.records.insert(id, StoredContinuation {
        record,
        payload,
        issued: BTreeSet::new(),
        revision,
        terminal_reason: None,
        last_invalidation_generation: None,
    });
    store.token_index.insert(digest, id);
    ContinuationCredential {
        continuation_id: id,
        expires_at: time(1),
        token_digest: digest,
        binding_id: binding,
    }
}

pub(super) fn store() -> ContinuationStore {
    ContinuationStore::new(ContinuationLimits::BASELINE).expect("fixture limits")
}

pub(super) fn permit(store: &ContinuationStore, credential: &ContinuationCredential) -> ContinuationPermit {
    let value = store.records.get(&credential.continuation_id).expect("fixture record");
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
    match store.resume(credential, &live, &time(0), 3).expect("fixture resume") {
        ResumePlan::DurableReplan { permit, .. } => store
            .bind_durable_emission(&permit, &live, &time(0), &emitted(&[1, 2, 3]))
            .expect("fixture durable selection"),
        ResumePlan::EphemeralWindow { permit, .. } | ResumePlan::Exhausted { permit } => permit,
    }
}

pub(super) fn emitted(values: &[u8]) -> BoundedList<Blake3Digest32, MAX_LIST_ITEMS> {
    bounded(values.iter().map(|value| Blake3Digest32::from_bytes([*value; 32])).collect())
        .expect("fixture emitted set")
}

#[derive(Debug, Eq, PartialEq)]
pub(super) struct Snapshot {
    limits: ContinuationLimits,
    records: BTreeMap<ContinuationId, StoredContinuation>,
    tokens: BTreeMap<HandleTokenDigest, ContinuationId>,
}

pub(super) fn snapshot(store: &ContinuationStore) -> Snapshot {
    Snapshot {
        limits: store.limits,
        records: store.records.clone(),
        tokens: store.token_index.clone(),
    }
}

fn narrow_limits(store: &ContinuationStore) -> ContinuationLimits {
    ContinuationLimits {
        max_candidate_window: 1,
        max_expansion_items: 1,
        ..store.limits()
    }
}

#[test]
fn emission_revision_exhaustion_preserves_the_complete_record() {
    for durable in [false, true] {
        let mut store = store();
        let credential = insert(&mut store, 1, durable, u64::MAX);
        let permit = permit(&store, &credential);
        let before = snapshot(&store);
        assert_eq!(
            store.commit_emission(&permit, &emitted(&[1])),
            Err(ContinuationError::RevisionExhausted),
        );
        assert_eq!(snapshot(&store), before);
        assert!(store.resolve(&credential).is_ok());
    }
}

#[test]
fn completion_revision_exhaustion_does_not_lose_cleanup() {
    for durable in [false, true] {
        let mut store = store();
        let credential = insert(&mut store, 1, durable, u64::MAX);
        let permit = permit(&store, &credential);
        let before = snapshot(&store);
        assert_eq!(store.complete(&permit), Err(ContinuationError::RevisionExhausted));
        assert_eq!(snapshot(&store), before);
    }
}

#[test]
fn expiry_checks_every_selected_revision_before_any_transition() {
    for exhausted_id in 1..=3 {
        let mut store = store();
        for seed in 1..=3 {
            let revision = if seed == exhausted_id { u64::MAX } else { 7 };
            insert(&mut store, seed, seed == 2, revision);
        }
        let before = snapshot(&store);
        assert_eq!(store.expire(&time(1)), Err(ContinuationError::RevisionExhausted));
        assert_eq!(snapshot(&store), before);
    }
}

#[test]
fn invalidation_checks_every_selected_revision_before_any_transition() {
    for reason in [InvalidationReason::AccessRevoked, InvalidationReason::Purged] {
        for exhausted_id in 1..=3 {
            let mut store = store();
            for seed in 1..=3 {
                let revision = if seed == exhausted_id { u64::MAX } else { 7 };
                insert(&mut store, seed, seed == 2, revision);
            }
            let before = snapshot(&store);
            assert_eq!(
                store.invalidate(&InvalidationScope::All, reason, NonZeroRevision::new(2).unwrap()),
                Err(ContinuationError::RevisionExhausted),
            );
            assert_eq!(snapshot(&store), before);
        }
    }
}

#[test]
fn restart_failure_does_not_invalidate_a_prefix_or_touch_durable_records() {
    let mut store = store();
    insert(&mut store, 1, false, 7);
    insert(&mut store, 2, false, u64::MAX);
    insert(&mut store, 3, true, u64::MAX);
    let before = snapshot(&store);
    let boot = OpaqueId::new("synthetic:new-boot").unwrap();
    assert_eq!(
        store.invalidate_restart(&boot, NonZeroRevision::new(2).unwrap()),
        Err(ContinuationError::RevisionExhausted),
    );
    assert_eq!(snapshot(&store), before);
}

#[test]
fn failed_live_limits_preserve_old_limits_and_all_records() {
    let mut store = store();
    insert(&mut store, 1, false, 7);
    insert(&mut store, 2, false, u64::MAX);
    insert(&mut store, 3, true, 7);
    let before = snapshot(&store);
    assert_eq!(
        store.apply_live_limits(narrow_limits(&store)),
        Err(ContinuationError::RevisionExhausted),
    );
    assert_eq!(snapshot(&store), before);
}

#[test]
fn final_representable_revision_returns_exact_cleanup_for_both_durabilities() {
    for durable in [false, true] {
        let mut store = store();
        let credential = insert(&mut store, 1, durable, u64::MAX - 1);
        let permit = permit(&store, &credential);
        let expected = store.records[&credential.continuation_id].cleanup_effect();
        assert_eq!(store.complete(&permit), Ok(expected));
        let value = &store.records[&credential.continuation_id];
        assert_eq!(value.revision, u64::MAX);
        assert_eq!(value.status(), LifecycleRecordStatus::Revoked);
        assert_eq!(value.terminal_reason, Some(InvalidationReason::Completed));
        let before = snapshot(&store);
        assert_eq!(store.complete(&permit), Err(ContinuationError::StalePermit));
        assert_eq!(snapshot(&store), before);
    }
}

#[test]
fn successful_emission_is_single_use_and_completes_only_the_final_window() {
    let mut store = store();
    let credential = insert(&mut store, 1, false, 7);
    let first = permit(&store, &credential);
    let receipt = store.commit_emission(&first, &emitted(&[1])).unwrap();
    assert_eq!((receipt.emitted_count, receipt.issued_total, receipt.completed), (1, 1, false));
    assert_eq!(receipt.cleanup_effect, None);
    let before = snapshot(&store);
    assert_eq!(
        store.commit_emission(&first, &emitted(&[2])),
        Err(ContinuationError::StalePermit),
    );
    assert_eq!(snapshot(&store), before);
    let final_permit = permit(&store, &credential);
    let expected = store.records[&credential.continuation_id].cleanup_effect();
    let receipt = store.commit_emission(&final_permit, &emitted(&[2, 3])).unwrap();
    assert_eq!((receipt.issued_total, receipt.completed), (3, true));
    assert_eq!(receipt.cleanup_effect, Some(expected));
    assert_eq!(store.records[&credential.continuation_id].revision, 9);
}

#[test]
fn emission_can_use_the_last_revision_without_wrapping() {
    let mut store = store();
    let credential = insert(&mut store, 1, false, u64::MAX - 1);
    let permit = permit(&store, &credential);
    let receipt = store.commit_emission(&permit, &emitted(&[1, 2, 3])).unwrap();
    assert!(receipt.completed);
    assert!(receipt.cleanup_effect.is_some());
    assert_eq!(store.records[&credential.continuation_id].revision, u64::MAX);
}

#[test]
fn expiry_preserves_deadline_order_bounded_batches_and_effect_correspondence() {
    let mut store = store();
    store.limits.max_lifecycle_batch = 1;
    let first = insert(&mut store, 1, false, 7);
    let second = insert(&mut store, 2, true, u64::MAX - 1);
    let first_record = &mut store.records.get_mut(&first.continuation_id).unwrap().record;
    if let ContinuationRecord::EphemeralWindow(record) = first_record {
        record.expires_at = time(2);
    }
    let second_effect = store.records[&second.continuation_id].cleanup_effect();
    let receipt = store.expire(&time(3)).unwrap();
    assert_eq!(
        receipt.expired.iter().copied().collect::<Vec<_>>(),
        vec![second.continuation_id],
    );
    assert_eq!(receipt.effects.iter().cloned().collect::<Vec<_>>(), vec![second_effect]);
    assert!(receipt.more_remaining);
    assert!(store.records[&first.continuation_id].is_active());
    let receipt = store.expire(&time(3)).unwrap();
    assert_eq!(
        receipt.expired.iter().copied().collect::<Vec<_>>(),
        vec![first.continuation_id],
    );
    assert!(!receipt.more_remaining);
    assert!(store.expire(&time(3)).unwrap().effects.is_empty());
}

#[test]
fn exhaustion_outside_the_selected_expiry_batch_does_not_block_it() {
    let mut store = store();
    store.limits.max_lifecycle_batch = 1;
    let first = insert(&mut store, 1, false, 7);
    insert(&mut store, 2, true, u64::MAX);
    let receipt = store.expire(&time(1)).unwrap();
    assert_eq!(
        receipt.expired.iter().copied().collect::<Vec<_>>(),
        vec![first.continuation_id],
    );
    assert!(receipt.more_remaining);
    let before = snapshot(&store);
    assert_eq!(store.expire(&time(1)), Err(ContinuationError::RevisionExhausted));
    assert_eq!(snapshot(&store), before);
}

#[test]
fn successful_invalidation_pairs_every_record_and_effect_and_is_idempotent() {
    let mut store = store();
    let first = insert(&mut store, 1, false, u64::MAX - 1);
    let second = insert(&mut store, 2, true, 7);
    let effects = [first.continuation_id, second.continuation_id]
        .map(|id| store.records[&id].cleanup_effect());
    let generation = NonZeroRevision::new(2).unwrap();
    let receipt = store
        .invalidate(&InvalidationScope::All, InvalidationReason::Purged, generation)
        .unwrap();
    assert_eq!(
        receipt.invalidated.iter().copied().collect::<Vec<_>>(),
        vec![first.continuation_id, second.continuation_id],
    );
    assert_eq!(receipt.effects.iter().cloned().collect::<Vec<_>>(), effects.to_vec());
    for value in store.records.values() {
        assert_eq!(value.status(), LifecycleRecordStatus::Revoked);
        assert_eq!(value.terminal_reason, Some(InvalidationReason::Purged));
        assert_eq!(value.last_invalidation_generation, Some(generation));
    }
    assert_eq!(store.resolve(&first), Err(ContinuationError::Purged));
    assert_eq!(store.resolve(&second), Err(ContinuationError::Purged));
    let before = snapshot(&store);
    assert!(
        store
            .invalidate(&InvalidationScope::All, InvalidationReason::Purged, generation)
            .unwrap()
            .effects
            .is_empty()
    );
    assert_eq!(snapshot(&store), before);
}

#[test]
fn successful_live_limits_do_not_touch_unselected_exhausted_records() {
    let mut store = store();
    let first = insert(&mut store, 1, false, u64::MAX - 1);
    let survivor = insert(&mut store, 2, true, u64::MAX);
    let before_survivor = store.records[&survivor.continuation_id].clone();
    let limits = narrow_limits(&store);
    let expected = store.records[&first.continuation_id].cleanup_effect();
    let receipt = store.apply_live_limits(limits).unwrap();
    assert_eq!(store.limits(), limits);
    assert_eq!(
        receipt.expired.iter().copied().collect::<Vec<_>>(),
        vec![first.continuation_id],
    );
    assert_eq!(receipt.effects.iter().cloned().collect::<Vec<_>>(), vec![expected]);
    assert_eq!(store.records[&survivor.continuation_id], before_survivor);
    assert_eq!(store.records[&first.continuation_id].revision, u64::MAX);
}

#[test]
fn generation_conflict_and_batch_limit_fail_without_changes() {
    let mut store = store();
    insert(&mut store, 1, false, 7);
    let second = insert(&mut store, 2, true, 7);
    store.records.get_mut(&second.continuation_id).unwrap().last_invalidation_generation =
        Some(NonZeroRevision::new(3).unwrap());
    let before = snapshot(&store);
    assert_eq!(
        store.invalidate(
            &InvalidationScope::All,
            InvalidationReason::Purged,
            NonZeroRevision::new(2).unwrap(),
        ),
        Err(ContinuationError::OperationConflict),
    );
    assert_eq!(snapshot(&store), before);
    store.limits.max_lifecycle_batch = 1;
    let before = snapshot(&store);
    assert_eq!(
        store.invalidate(
            &InvalidationScope::All,
            InvalidationReason::Purged,
            NonZeroRevision::new(4).unwrap(),
        ),
        Err(ContinuationError::ResourceExhausted),
    );
    assert_eq!(snapshot(&store), before);
}

#[test]
fn failed_live_limit_batch_and_noop_update_preserve_records() {
    let mut store = store();
    insert(&mut store, 1, false, 7);
    insert(&mut store, 2, false, 7);
    store.limits.max_lifecycle_batch = 1;
    let before = snapshot(&store);
    assert_eq!(
        store.apply_live_limits(narrow_limits(&store)),
        Err(ContinuationError::ResourceExhausted),
    );
    assert_eq!(snapshot(&store), before);
    let limits = store.limits();
    assert!(store.apply_live_limits(limits).unwrap().effects.is_empty());
    assert_eq!(snapshot(&store), before);
}

#[test]
fn compaction_removes_only_terminal_records_and_their_token_indices() {
    let mut store = store();
    let first = insert(&mut store, 1, false, 7);
    let survivor = insert(&mut store, 2, true, u64::MAX);
    let permit = permit(&store, &first);
    let _effect_for_caller = store.complete(&permit).unwrap();
    let before = snapshot(&store);
    assert_eq!(store.compact_terminal(0), Err(ContinuationError::InvalidLimits));
    assert_eq!(snapshot(&store), before);
    let removed = store.compact_terminal(1).unwrap();
    assert_eq!(removed.iter().copied().collect::<Vec<_>>(), vec![first.continuation_id]);
    assert_eq!(store.resolve(&first), Err(ContinuationError::NotAuthorized));
    assert!(store.resolve(&survivor).is_ok());
    assert_eq!(store.len(), 1);
    assert_eq!(store.token_index.len(), 1);
}

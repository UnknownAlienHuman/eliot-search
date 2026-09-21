//! Synthetic selection and live-checkpoint regressions; no external execution evidence.

use super::atomicity_tests::{emitted, insert, permit, snapshot, store, time};
use super::{
    AuthorizationFence, Blake3Digest32, ContinuationCredential, ContinuationError,
    ContinuationPermit, ContinuationRecord, ContinuationStore, ContinuityFence, CurrencyFence,
    HandleTokenDigest, InvalidationReason, InvalidationScope, LiveContinuationState,
    NonZeroRevision, ResumePlan,
};

fn live(store: &ContinuationStore, credential: &ContinuationCredential) -> LiveContinuationState {
    let record = &store.records[&credential.continuation_id];
    LiveContinuationState {
        binding_id: record.binding_id(),
        plan_fingerprint: record.plan_fingerprint(),
        result_fence: record.result_fence().clone(),
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
    }
}

fn resume(
    store: &ContinuationStore,
    credential: &ContinuationCredential,
    count: usize,
) -> ContinuationPermit {
    match store.resume(credential, &live(store, credential), &time(0), count).unwrap() {
        ResumePlan::EphemeralWindow { permit, .. }
        | ResumePlan::DurableReplan { permit, .. }
        | ResumePlan::Exhausted { permit } => permit,
    }
}

#[test]
fn ephemeral_permit_accepts_only_subsets_of_the_selected_window() {
    // Enumerate every nonempty subset of a three-candidate retained window.
    for count in 1..=3 {
        for mask in 1_u8..8 {
            let mut store = store();
            let credential = insert(&mut store, 1, false, 7);
            let permit = resume(&store, &credential, count);
            let values = (1_u8..=3)
                .filter(|value| mask & (1 << (value - 1)) != 0)
                .collect::<Vec<_>>();
            let before = snapshot(&store);
            let result = store.commit_emission(&permit, &emitted(&values));
            if values.iter().all(|value| usize::from(*value) <= count) {
                assert_eq!(result.unwrap().emitted_count, values.len());
            } else {
                assert_eq!(result, Err(ContinuationError::StalePermit));
                assert_eq!(snapshot(&store), before);
            }
        }
    }
}

#[test]
fn partial_selected_acknowledgement_keeps_unissued_candidates_available() {
    let mut store = store();
    let credential = insert(&mut store, 1, false, 7);
    let first = resume(&store, &credential, 2);
    assert_eq!(store.commit_emission(&first, &emitted(&[1])).unwrap().issued_total, 1);
    let before = snapshot(&store);
    assert_eq!(
        store.commit_emission(&first.clone(), &emitted(&[2])),
        Err(ContinuationError::StalePermit),
    );
    assert_eq!(snapshot(&store), before);
    let ResumePlan::EphemeralWindow { permit, candidates, .. } = store
        .resume(&credential, &live(&store, &credential), &time(0), 1)
        .unwrap()
    else {
        panic!("expected remaining window");
    };
    assert_eq!(
        candidates.iter().map(|item| item.fingerprint).collect::<Vec<_>>(),
        emitted(&[2]).iter().copied().collect::<Vec<_>>(),
    );
    assert_eq!(
        store.commit_emission(&permit, &emitted(&[3])),
        Err(ContinuationError::StalePermit),
    );
}

#[test]
fn malformed_and_unselected_acknowledgements_are_nonmutating() {
    let mut store = store();
    let credential = insert(&mut store, 1, false, 7);
    let permit = resume(&store, &credential, 2);
    for (values, error) in [
        (vec![], ContinuationError::InvalidLimits),
        (vec![1, 1], ContinuationError::DuplicateCandidate),
        (vec![9], ContinuationError::StalePermit),
        (vec![1, 3], ContinuationError::StalePermit),
    ] {
        let before = snapshot(&store);
        assert_eq!(store.commit_emission(&permit, &emitted(&values)), Err(error));
        assert_eq!(snapshot(&store), before);
    }
}

#[test]
fn exhausted_observation_cannot_acknowledge_candidates() {
    let mut store = store();
    let credential = insert(&mut store, 1, false, 7);
    store.records.get_mut(&credential.continuation_id).unwrap().issued
        .extend(emitted(&[1, 2, 3]).iter().copied());
    let permit = resume(&store, &credential, 1);
    let before = snapshot(&store);
    assert_eq!(
        store.commit_emission(&permit, &emitted(&[4])),
        Err(ContinuationError::StalePermit),
    );
    assert_eq!(snapshot(&store), before);
    assert!(store.complete(&permit).is_ok());
}

#[test]
fn durable_replan_permit_cannot_directly_acknowledge_any_result() {
    let mut store = store();
    let credential = insert(&mut store, 1, true, 7);
    let pending = resume(&store, &credential, 1);
    let before = snapshot(&store);
    assert_eq!(
        store.commit_emission(&pending, &emitted(&[1])),
        Err(ContinuationError::StalePermit),
    );
    assert_eq!(snapshot(&store), before);
}

#[test]
fn durable_binding_preserves_the_original_request_count() {
    let mut store = store();
    let credential = insert(&mut store, 1, true, 7);
    let pending = resume(&store, &credential, 1);
    let before = snapshot(&store);
    assert_eq!(
        store.bind_durable_emission(&pending, &live(&store, &credential), &time(0), &emitted(&[7, 8])),
        Err(ContinuationError::InvalidLimits),
    );
    assert_eq!(snapshot(&store), before);
}

#[test]
fn durable_binding_rechecks_current_limits_after_resume() {
    let mut store = store();
    let credential = insert(&mut store, 1, true, 7);
    let pending = resume(&store, &credential, 3);
    let mut limits = store.limits();
    limits.max_expansion_items = 1;
    store.apply_live_limits(limits).unwrap();
    let before = snapshot(&store);
    assert_eq!(
        store.bind_durable_emission(&pending, &live(&store, &credential), &time(0), &emitted(&[7, 8])),
        Err(ContinuationError::InvalidLimits),
    );
    assert_eq!(snapshot(&store), before);
}

#[test]
fn durable_bound_selection_is_exact_and_single_revision_use() {
    let mut store = store();
    let credential = insert(&mut store, 1, true, 7);
    let pending = resume(&store, &credential, 2);
    let before = snapshot(&store);
    let bound = store
        .bind_durable_emission(&pending, &live(&store, &credential), &time(0), &emitted(&[7, 8]))
        .unwrap();
    assert_eq!(snapshot(&store), before);
    assert_eq!(
        store.commit_emission(&bound, &emitted(&[9])),
        Err(ContinuationError::StalePermit),
    );
    assert_eq!(snapshot(&store), before);
    let receipt = store.commit_emission(&bound, &emitted(&[7])).unwrap();
    assert_eq!((receipt.issued_total, receipt.completed), (1, false));
    assert!(receipt.cleanup_effect.is_none());
    let before = snapshot(&store);
    assert_eq!(
        store.commit_emission(&bound.clone(), &emitted(&[8])),
        Err(ContinuationError::StalePermit),
    );
    assert_eq!(
        store.bind_durable_emission(&pending, &live(&store, &credential), &time(0), &emitted(&[8])),
        Err(ContinuationError::StalePermit),
    );
    assert_eq!(snapshot(&store), before);
}

#[test]
fn durable_binding_rejects_empty_duplicate_and_already_issued_results() {
    let mut store = store();
    let credential = insert(&mut store, 1, true, 7);
    store.records.get_mut(&credential.continuation_id).unwrap().issued
        .insert(Blake3Digest32::from_bytes([7; 32]));
    let pending = resume(&store, &credential, 3);
    for (values, error) in [
        (vec![], ContinuationError::InvalidLimits),
        (vec![8, 8], ContinuationError::DuplicateCandidate),
        (vec![7, 8], ContinuationError::DuplicateCandidate),
    ] {
        let before = snapshot(&store);
        assert_eq!(
            store.bind_durable_emission(&pending, &live(&store, &credential), &time(0), &emitted(&values)),
            Err(error),
        );
        assert_eq!(snapshot(&store), before);
    }
}

#[test]
fn only_pending_durable_permits_can_be_bound() {
    let mut store = store();
    let ephemeral = insert(&mut store, 1, false, 7);
    let ephemeral_permit = resume(&store, &ephemeral, 1);
    assert_eq!(
        store.bind_durable_emission(&ephemeral_permit, &live(&store, &ephemeral), &time(0), &emitted(&[1])),
        Err(ContinuationError::StalePermit),
    );
    let durable = insert(&mut store, 2, true, 7);
    let pending = resume(&store, &durable, 1);
    let bound = store.bind_durable_emission(&pending, &live(&store, &durable), &time(0), &emitted(&[7])).unwrap();
    let before = snapshot(&store);
    assert_eq!(
        store.bind_durable_emission(&bound, &live(&store, &durable), &time(0), &emitted(&[8])),
        Err(ContinuationError::StalePermit),
    );
    assert_eq!(snapshot(&store), before);
}

#[test]
fn durable_binding_cannot_exceed_the_issued_set_quota() {
    let mut store = store();
    let credential = insert(&mut store, 1, true, 7);
    let pending = resume(&store, &credential, 2);
    let mut limits = store.limits();
    limits.max_issued_candidates = 1;
    store.apply_live_limits(limits).unwrap();
    let before = snapshot(&store);
    assert_eq!(
        store.bind_durable_emission(&pending, &live(&store, &credential), &time(0), &emitted(&[7, 8])),
        Err(ContinuationError::ResourceExhausted),
    );
    assert_eq!(snapshot(&store), before);
}

#[test]
fn emission_checkpoint_rechecks_every_live_fence_for_both_durabilities() {
    for durable in [false, true] {
        let mut store = store();
        let credential = insert(&mut store, 1, durable, 7);
        let permit = permit(&store, &credential);
        for case in 0..12 {
            let mut observed = live(&store, &credential);
            let expected = match case {
                0 => { observed.authorization.grant_active = false; ContinuationError::AccessRevoked }
                1 => { observed.authorization.security_permits = false; ContinuationError::AccessRevoked }
                2 => { observed.authorization.purge_clear = false; ContinuationError::Purged }
                3 => { observed.currency.owner_generation_current = false; ContinuationError::SnapshotExpired }
                4 => { observed.currency.view_current = false; ContinuationError::SnapshotExpired }
                5 => { observed.currency.route_current = false; ContinuationError::SnapshotExpired }
                6 => { observed.continuity.profile_current = false; ContinuationError::SnapshotExpired }
                7 => { observed.result_fence.result_fingerprint = Blake3Digest32::from_bytes([99; 32]); ContinuationError::SnapshotExpired }
                8 => { observed.plan_fingerprint = super::PlanFingerprint::from_bytes([99; 32]); ContinuationError::SnapshotExpired }
                9 => { observed.binding_id = super::BindingId::from_bytes([99; 16]); ContinuationError::NotAuthorized }
                10 if !durable => { observed.continuity.epoch_pin_valid = false; ContinuationError::EpochPinUnavailable }
                11 if durable => { observed.continuity.durable_job_active = false; ContinuationError::SnapshotExpired }
                _ => continue,
            };
            let before = snapshot(&store);
            assert_eq!(
                store.revalidate_emission(&permit, &emitted(&[1]), &observed, &time(0)),
                Err(expected),
                "durable={durable} case={case}",
            );
            assert_eq!(snapshot(&store), before);
        }
    }
}

#[test]
fn expiration_at_the_emission_checkpoint_returns_no_accounting_receipt() {
    for durable in [false, true] {
        let mut store = store();
        let credential = insert(&mut store, 1, durable, 7);
        let permit = permit(&store, &credential);
        let before = snapshot(&store);
        assert_eq!(
            store.revalidate_emission(&permit, &emitted(&[1]), &live(&store, &credential), &time(1)),
            Err(ContinuationError::SnapshotExpired),
        );
        assert_eq!(snapshot(&store), before);
    }
}

#[test]
fn durable_binding_rechecks_authority_and_expiration_after_execution() {
    let mut store = store();
    let credential = insert(&mut store, 1, true, 7);
    let pending = resume(&store, &credential, 1);
    let mut denied = live(&store, &credential);
    denied.authorization.purge_clear = false;
    let before = snapshot(&store);
    assert_eq!(
        store.bind_durable_emission(&pending, &denied, &time(0), &emitted(&[7])),
        Err(ContinuationError::Purged),
    );
    assert_eq!(
        store.bind_durable_emission(&pending, &live(&store, &credential), &time(1), &emitted(&[7])),
        Err(ContinuationError::SnapshotExpired),
    );
    assert_eq!(snapshot(&store), before);
}

#[test]
fn successful_checkpoint_does_not_mark_candidates_issued() {
    for durable in [false, true] {
        let mut store = store();
        let credential = insert(&mut store, 1, durable, 7);
        let permit = permit(&store, &credential);
        let before = snapshot(&store);
        assert_eq!(
            store.revalidate_emission(&permit, &emitted(&[1]), &live(&store, &credential), &time(0)),
            Ok(()),
        );
        assert_eq!(snapshot(&store), before);
        // Cancellation here has no acknowledgement to roll back.
        let again = resume(&store, &credential, 1);
        assert_eq!(again.record_revision(), permit.record_revision());
    }
}

#[test]
fn checkpoint_rejects_unselected_fingerprints_and_revision_exhaustion() {
    let mut store = store();
    let credential = insert(&mut store, 1, false, 7);
    let narrow = resume(&store, &credential, 1);
    let before = snapshot(&store);
    assert_eq!(
        store.revalidate_emission(&narrow, &emitted(&[2]), &live(&store, &credential), &time(0)),
        Err(ContinuationError::StalePermit),
    );
    assert_eq!(snapshot(&store), before);
    store.records.get_mut(&credential.continuation_id).unwrap().revision = u64::MAX;
    let exhausted = resume(&store, &credential, 1);
    let before = snapshot(&store);
    assert_eq!(
        store.revalidate_emission(&exhausted, &emitted(&[1]), &live(&store, &credential), &time(0)),
        Err(ContinuationError::RevisionExhausted),
    );
    assert_eq!(snapshot(&store), before);
}

#[test]
fn restrictive_quota_after_selection_blocks_both_checkpoint_and_commit() {
    let mut store = store();
    let credential = insert(&mut store, 1, false, 7);
    let permit = resume(&store, &credential, 2);
    let mut limits = store.limits();
    limits.max_issued_candidates = 1;
    store.apply_live_limits(limits).unwrap();
    let before = snapshot(&store);
    assert_eq!(
        store.revalidate_emission(&permit, &emitted(&[1, 2]), &live(&store, &credential), &time(0)),
        Err(ContinuationError::ResourceExhausted),
    );
    assert_eq!(
        store.commit_emission(&permit, &emitted(&[1, 2])),
        Err(ContinuationError::ResourceExhausted),
    );
    assert_eq!(snapshot(&store), before);
}

#[test]
fn invalidation_between_selection_and_delivery_stales_the_permit() {
    let mut store = store();
    let credential = insert(&mut store, 1, true, 7);
    let permit = permit(&store, &credential);
    let observed = live(&store, &credential);
    store.invalidate(&InvalidationScope::All, InvalidationReason::Purged, NonZeroRevision::new(1).unwrap()).unwrap();
    let before = snapshot(&store);
    assert_eq!(
        store.revalidate_emission(&permit, &emitted(&[1]), &observed, &time(0)),
        Err(ContinuationError::StalePermit),
    );
    assert_eq!(store.commit_emission(&permit, &emitted(&[1])), Err(ContinuationError::StalePermit));
    assert_eq!(snapshot(&store), before);
}

#[test]
fn reused_id_and_revision_do_not_reuse_a_previous_token_incarnation() {
    for durable in [false, true] {
        let mut store = store();
        let credential = insert(&mut store, 1, durable, 7);
        let old = permit(&store, &credential);
        store.complete(&old).unwrap();
        store.compact_terminal(1).unwrap();
        let replaced = insert(&mut store, 1, durable, 7);
        let digest = HandleTokenDigest::from_bytes([99; 32]);
        match &mut store.records.get_mut(&replaced.continuation_id).unwrap().record {
            ContinuationRecord::EphemeralWindow(record) => record.token_digest = digest,
            ContinuationRecord::DurableReplanCheckpoint(record) => record.token_digest = digest,
        }
        store.token_index.remove(&replaced.token_digest);
        store.token_index.insert(digest, replaced.continuation_id);
        let before = snapshot(&store);
        assert_eq!(store.commit_emission(&old, &emitted(&[1])), Err(ContinuationError::StalePermit));
        assert_eq!(store.complete(&old), Err(ContinuationError::StalePermit));
        assert_eq!(snapshot(&store), before);
    }
}

#[test]
fn changed_record_lifetime_cannot_reuse_a_previous_permit() {
    for change_creation in [false, true] {
        let mut store = store();
        let credential = insert(&mut store, 1, false, 7);
        let permit = permit(&store, &credential);
        let ContinuationRecord::EphemeralWindow(record) = &mut store.records
            .get_mut(&credential.continuation_id).unwrap().record else { unreachable!() };
        if change_creation {
            record.created_at = time(1);
            record.expires_at = time(2);
        } else {
            record.expires_at = time(2);
        }
        let before = snapshot(&store);
        assert_eq!(store.commit_emission(&permit, &emitted(&[1])), Err(ContinuationError::StalePermit));
        assert_eq!(snapshot(&store), before);
    }
}

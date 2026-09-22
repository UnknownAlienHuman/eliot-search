//! Synthetic quota regressions. References, times and token bytes are fixtures,
//! not evidence of native cleanup, qualified entropy or daemon integration.

use super::atomicity_tests::{emitted, insert, permit, snapshot, store, time};
use super::{
    AuthorizationFence, BindingId, BoundedList, ContinuationCredential, ContinuationError,
    ContinuationLimits, ContinuationStore, ContinuationTokenMaterial,
    ContinuityFence, CreateContinuationRequest, CurrencyFence, InvalidationReason,
    InvalidationScope, LiveContinuationState, NonZeroRevision, OpaqueHandleToken, ResumePlan,
};

fn retained_limit(owner: &ContinuationStore, maximum: usize) -> ContinuationLimits {
    ContinuationLimits {
        max_records: maximum,
        max_ephemeral_per_binding: maximum,
        max_durable_per_binding: maximum,
        ..owner.limits()
    }
}

fn live(owner: &ContinuationStore, credential: &ContinuationCredential) -> LiveContinuationState {
    let record = &owner.records[&credential.continuation_id];
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

fn create_request(seed: u8, durable: bool) -> CreateContinuationRequest {
    let mut fixture = store();
    let credential = insert(&mut fixture, seed, durable, 1);
    let value = fixture.records.remove(&credential.continuation_id).unwrap();
    CreateContinuationRequest {
        record: value.record,
        payload: value.payload,
        token: ContinuationTokenMaterial {
            continuation_id: credential.continuation_id,
            token_digest: credential.token_digest,
            opaque_token: OpaqueHandleToken::new(&[seed; 32]).unwrap(),
        },
        ttl_millis: 60_000,
        issued_fingerprints: BoundedList::empty(),
    }
}

fn revoke(owner: &mut ContinuationStore, credential: &ContinuationCredential) {
    let receipt = owner
        .invalidate(
            &InvalidationScope::Continuation(credential.continuation_id),
            InvalidationReason::Purged,
            NonZeroRevision::new(1).unwrap(),
        )
        .unwrap();
    assert_eq!(receipt.invalidated.as_slice(), &[credential.continuation_id]);
    assert_eq!(receipt.effects.len(), 1);
}

#[test]
fn record_cap_counts_every_active_and_terminal_record_before_any_change() {
    // Every status placement, including all active and all terminal, for both
    // payload forms. Expiring a record never reduces the retained record count.
    for durable in [false, true] {
        for retired_mask in 0_u8..8 {
            let mut owner = store();
            for seed in 1..=3 {
                let credential = insert(&mut owner, seed, durable, 7);
                if retired_mask & (1 << (seed - 1)) != 0 {
                    revoke(&mut owner, &credential);
                }
            }
            let before = snapshot(&owner);
            assert_eq!(
                owner.apply_live_limits(retained_limit(&owner, 2)),
                Err(ContinuationError::ResourceExhausted),
            );
            assert_eq!(snapshot(&owner), before);
            assert_eq!((owner.len(), owner.token_index.len()), (3, 3));
        }
    }
}

#[test]
fn expired_records_also_block_an_impossible_record_cap() {
    let mut owner = store();
    let first = insert(&mut owner, 1, false, 7);
    let second = insert(&mut owner, 2, true, 7);
    assert_eq!(owner.expire(&time(1)).unwrap().expired.len(), 2);
    let before = snapshot(&owner);
    assert_eq!(
        owner.apply_live_limits(retained_limit(&owner, 1)),
        Err(ContinuationError::ResourceExhausted),
    );
    assert_eq!(snapshot(&owner), before);
    for credential in [&first, &second] {
        assert_eq!(owner.resolve(credential), Err(ContinuationError::SnapshotExpired));
    }
}

#[test]
fn record_cap_refusal_does_not_partially_apply_other_restrictions() {
    let mut owner = store();
    let first = insert(&mut owner, 1, false, u64::MAX);
    insert(&mut owner, 2, true, 7);
    let mut limits = retained_limit(&owner, 1);
    limits.max_ttl_millis = 1;
    limits.max_candidate_window = 1;
    limits.max_expansion_items = 1;
    let before = snapshot(&owner);
    for _ in 0..3 {
        assert_eq!(owner.apply_live_limits(limits), Err(ContinuationError::ResourceExhausted));
        assert_eq!(snapshot(&owner), before);
    }
    assert!(owner.resolve(&first).is_ok());
}

#[test]
fn exact_retained_cap_is_valid_but_leaves_no_room_for_admission() {
    let mut owner = store();
    let first = insert(&mut owner, 1, false, 7);
    let second = insert(&mut owner, 2, true, 7);
    revoke(&mut owner, &first);
    let records = owner.records.clone();
    let tokens = owner.token_index.clone();
    let receipt = owner.apply_live_limits(retained_limit(&owner, 2)).unwrap();
    assert!(receipt.expired.is_empty());
    assert!(receipt.effects.is_empty());
    assert_eq!(owner.records, records);
    assert_eq!(owner.token_index, tokens);
    let before = snapshot(&owner);
    assert_eq!(owner.create(create_request(3, false)), Err(ContinuationError::ResourceExhausted));
    assert_eq!(snapshot(&owner), before);
    assert!(owner.resolve(&second).is_ok());
    assert_eq!(owner.resolve(&first), Err(ContinuationError::Purged));
}

#[test]
fn explicit_compaction_allows_a_previously_refused_record_cap() {
    let mut owner = store();
    let first = insert(&mut owner, 1, false, 7);
    let second = insert(&mut owner, 2, true, 7);
    let survivor = insert(&mut owner, 3, false, 7);
    revoke(&mut owner, &first);
    revoke(&mut owner, &second);
    let limits = retained_limit(&owner, 2);
    assert_eq!(owner.apply_live_limits(limits), Err(ContinuationError::ResourceExhausted));
    // Synthetic caller crosses the existing compaction seam. This does not
    // claim any real external pin release or checkpoint deletion occurred.
    assert_eq!(owner.compact_terminal(1).unwrap().as_slice(), &[first.continuation_id]);
    assert!(owner.apply_live_limits(limits).unwrap().effects.is_empty());
    assert_eq!(owner.resolve(&first), Err(ContinuationError::NotAuthorized));
    assert_eq!(owner.resolve(&second), Err(ContinuationError::Purged));
    assert!(owner.resolve(&survivor).is_ok());
    assert_eq!(owner.create(create_request(4, false)), Err(ContinuationError::ResourceExhausted));
    assert_eq!(owner.compact_terminal(1).unwrap().as_slice(), &[second.continuation_id]);
    assert!(owner.create(create_request(4, false)).is_ok());
    assert_eq!(owner.len(), owner.limits().max_records);
    assert_eq!(owner.token_index.len(), owner.len());
}

#[test]
fn per_binding_restrictions_preserve_terminal_records_and_newer_survivors() {
    let mut owner = store();
    let first = insert(&mut owner, 1, false, 7);
    let second = insert(&mut owner, 2, false, 7);
    let third = insert(&mut owner, 3, true, 7);
    let fourth = insert(&mut owner, 4, true, 7);
    let retired = insert(&mut owner, 5, false, 7);
    revoke(&mut owner, &retired);
    let before_retired = owner.records[&retired.continuation_id].clone();
    let expected_effects = [first.continuation_id, third.continuation_id]
        .map(|id| owner.records[&id].cleanup_effect());
    let limits = ContinuationLimits {
        max_ephemeral_per_binding: 1,
        max_durable_per_binding: 1,
        ..retained_limit(&owner, 5)
    };
    let receipt = owner.apply_live_limits(limits).unwrap();
    assert_eq!(receipt.expired.as_slice(), &[first.continuation_id, third.continuation_id]);
    assert_eq!(receipt.effects.as_slice(), &expected_effects);
    assert_eq!(owner.records[&retired.continuation_id], before_retired);
    assert!(owner.resolve(&second).is_ok());
    assert!(owner.resolve(&fourth).is_ok());
    assert_eq!(owner.len(), 5);
    let before = snapshot(&owner);
    assert!(owner.apply_live_limits(limits).unwrap().effects.is_empty());
    assert_eq!(snapshot(&owner), before);
}

#[test]
fn invalid_limits_remain_distinct_from_a_retained_capacity_refusal() {
    let mut owner = store();
    insert(&mut owner, 1, false, 7);
    let before = snapshot(&owner);
    assert_eq!(
        owner.apply_live_limits(retained_limit(&owner, 0)),
        Err(ContinuationError::InvalidLimits),
    );
    assert_eq!(snapshot(&owner), before);
}

#[test]
fn ephemeral_resume_uses_remaining_issued_capacity_and_recovers_after_limit_increase() {
    let mut owner = store();
    let credential = insert(&mut owner, 1, false, 7);
    let initial = permit(&owner, &credential);
    owner.commit_emission(&initial, &emitted(&[1])).unwrap();
    let limits = ContinuationLimits { max_issued_candidates: 2, ..owner.limits() };
    owner.apply_live_limits(limits).unwrap();
    let before = snapshot(&owner);
    let ResumePlan::EphemeralWindow { permit, candidates, .. } =
        owner.resume(&credential, &live(&owner, &credential), &time(0), 3).unwrap()
    else {
        panic!("expected one remaining issuable item");
    };
    assert_eq!(permit.max_emission_items(), 1);
    assert_eq!(candidates.len(), 1);
    assert_eq!(candidates.as_slice()[0].fingerprint, emitted(&[2]).as_slice()[0]);
    assert_eq!(snapshot(&owner), before);
    owner
        .revalidate_emission(&permit, &emitted(&[2]), &live(&owner, &credential), &time(0))
        .unwrap();
    assert!(!owner.commit_emission(&permit, &emitted(&[2])).unwrap().completed);
    let full = snapshot(&owner);
    assert_eq!(
        owner.resume(&credential, &live(&owner, &credential), &time(0), 3),
        Err(ContinuationError::ResourceExhausted),
    );
    assert_eq!(snapshot(&owner), full);
    owner.apply_live_limits(ContinuationLimits { max_issued_candidates: 3, ..limits }).unwrap();
    let ResumePlan::EphemeralWindow { permit, candidates, .. } =
        owner.resume(&credential, &live(&owner, &credential), &time(0), 3).unwrap()
    else {
        panic!("expected the original final candidate");
    };
    assert_eq!(candidates.len(), 1);
    assert_eq!(candidates.as_slice()[0].fingerprint, emitted(&[3]).as_slice()[0]);
    let final_page = owner.commit_emission(&permit, &emitted(&[3])).unwrap();
    assert!(final_page.completed);
    assert_eq!(final_page.issued_total, 3);
    assert!(final_page.cleanup_effect.is_some());
}

#[test]
fn a_fully_issued_window_is_exhausted_not_resource_exhausted() {
    let mut owner = store();
    let credential = insert(&mut owner, 1, false, 7);
    // Creation/restoration may supply an already-issued set. Preserve the
    // explicit exhaustion/completion path when no candidate remains.
    owner.records.get_mut(&credential.continuation_id).unwrap().issued
        .extend(emitted(&[1, 2, 3]).iter().copied());
    owner
        .apply_live_limits(ContinuationLimits {
            max_issued_candidates: 3,
            ..owner.limits()
        })
        .unwrap();
    let before = snapshot(&owner);
    let ResumePlan::Exhausted { permit } =
        owner.resume(&credential, &live(&owner, &credential), &time(0), 3).unwrap()
    else {
        panic!("expected exhaustion without pin renewal");
    };
    assert_eq!(permit.max_emission_items(), 0);
    assert_eq!(snapshot(&owner), before);
    assert!(owner.complete(&permit).is_ok());
}

#[test]
fn durable_resume_binds_the_remaining_budget_and_does_not_reset_issued_history() {
    let mut owner = store();
    let credential = insert(&mut owner, 1, true, 7);
    let initial = permit(&owner, &credential);
    owner.commit_emission(&initial, &emitted(&[1])).unwrap();
    let limits = ContinuationLimits { max_issued_candidates: 2, ..owner.limits() };
    owner.apply_live_limits(limits).unwrap();
    let before = snapshot(&owner);
    let ResumePlan::DurableReplan { permit, issued_fingerprints, .. } =
        owner.resume(&credential, &live(&owner, &credential), &time(0), 3).unwrap()
    else {
        panic!("expected bounded durable replan");
    };
    assert_eq!(permit.max_emission_items(), 1);
    assert_eq!(issued_fingerprints, emitted(&[1]));
    assert_eq!(
        owner.bind_durable_emission(
            &permit,
            &live(&owner, &credential),
            &time(0),
            &emitted(&[2, 3]),
        ),
        Err(ContinuationError::InvalidLimits),
    );
    assert_eq!(snapshot(&owner), before);
    let bound = owner
        .bind_durable_emission(&permit, &live(&owner, &credential), &time(0), &emitted(&[2]))
        .unwrap();
    assert_eq!(bound.max_emission_items(), 1);
    assert_eq!(owner.commit_emission(&bound, &emitted(&[2])).unwrap().issued_total, 2);
    let full = snapshot(&owner);
    assert_eq!(
        owner.resume(&credential, &live(&owner, &credential), &time(0), 3),
        Err(ContinuationError::ResourceExhausted),
    );
    assert_eq!(snapshot(&owner), full);
    assert!(owner.resolve(&credential).is_ok());
}

#[test]
fn increasing_limits_does_not_widen_an_existing_durable_permit() {
    let mut owner = store();
    let credential = insert(&mut owner, 1, true, 7);
    owner
        .apply_live_limits(ContinuationLimits {
            max_issued_candidates: 1,
            ..owner.limits()
        })
        .unwrap();
    let ResumePlan::DurableReplan { permit, .. } =
        owner.resume(&credential, &live(&owner, &credential), &time(0), 3).unwrap()
    else {
        panic!("expected bounded durable replan");
    };
    owner
        .apply_live_limits(ContinuationLimits {
            max_issued_candidates: 3,
            ..owner.limits()
        })
        .unwrap();
    let before = snapshot(&owner);
    assert_eq!(
        owner.bind_durable_emission(
            &permit,
            &live(&owner, &credential),
            &time(0),
            &emitted(&[1, 2]),
        ),
        Err(ContinuationError::InvalidLimits),
    );
    assert_eq!(snapshot(&owner), before);
    assert!(
        owner
            .bind_durable_emission(&permit, &live(&owner, &credential), &time(0), &emitted(&[1]))
            .is_ok()
    );
}

#[test]
fn issued_capacity_is_per_record_not_shared_with_other_continuations() {
    let mut owner = store();
    let full = insert(&mut owner, 1, true, 7);
    let other = insert(&mut owner, 2, false, 7);
    let initial = permit(&owner, &full);
    owner.commit_emission(&initial, &emitted(&[1])).unwrap();
    owner
        .apply_live_limits(ContinuationLimits {
            max_issued_candidates: 1,
            ..owner.limits()
        })
        .unwrap();
    assert_eq!(
        owner.resume(&full, &live(&owner, &full), &time(0), 1),
        Err(ContinuationError::ResourceExhausted),
    );
    let ResumePlan::EphemeralWindow { candidates, .. } =
        owner.resume(&other, &live(&owner, &other), &time(0), 3).unwrap()
    else {
        panic!("another record retains its own quota");
    };
    assert_eq!(candidates.len(), 1);
}

#[test]
fn quota_checks_do_not_mask_authorization_lifetime_or_request_failures() {
    for durable in [false, true] {
        let mut owner = store();
        let credential = insert(&mut owner, 1, durable, 7);
        let initial = permit(&owner, &credential);
        owner.commit_emission(&initial, &emitted(&[1])).unwrap();
        owner
            .apply_live_limits(ContinuationLimits {
                max_issued_candidates: 1,
                ..owner.limits()
            })
            .unwrap();
        let before = snapshot(&owner);
        let mut foreign = credential.clone();
        foreign.binding_id = BindingId::from_bytes([99; 16]);
        assert_eq!(
            owner.resume(&foreign, &live(&owner, &credential), &time(0), 1),
            Err(ContinuationError::NotAuthorized),
        );
        let mut revoked = live(&owner, &credential);
        revoked.authorization.grant_active = false;
        assert_eq!(
            owner.resume(&credential, &revoked, &time(0), 1),
            Err(ContinuationError::AccessRevoked),
        );
        let mut purged = live(&owner, &credential);
        purged.authorization.purge_clear = false;
        assert_eq!(owner.resume(&credential, &purged, &time(0), 1), Err(ContinuationError::Purged));
        assert_eq!(
            owner.resume(&credential, &live(&owner, &credential), &time(1), 1),
            Err(ContinuationError::SnapshotExpired),
        );
        assert_eq!(
            owner.resume(&credential, &live(&owner, &credential), &time(0), 0),
            Err(ContinuationError::InvalidLimits),
        );
        assert_eq!(snapshot(&owner), before);
    }
}

#[test]
fn every_small_window_request_respects_issued_and_request_limits() {
    for issued in 0_u8..=3 {
        for ceiling in 1_usize..=4 {
            for requested in 1_usize..=3 {
                let mut owner = store();
                let credential = insert(&mut owner, 1, false, 7);
                let issued_values = (1..=issued).collect::<Vec<_>>();
                owner.records.get_mut(&credential.continuation_id).unwrap().issued
                    .extend(emitted(&issued_values).iter().copied());
                owner.limits.max_issued_candidates = ceiling;
                let before = snapshot(&owner);
                let result =
                    owner.resume(&credential, &live(&owner, &credential), &time(0), requested);
                let used = usize::from(issued);
                if used > ceiling || (used == ceiling && issued < 3) {
                    assert_eq!(result, Err(ContinuationError::ResourceExhausted));
                } else if issued == 3 {
                    assert!(matches!(result, Ok(ResumePlan::Exhausted { .. })));
                } else {
                    let ResumePlan::EphemeralWindow { permit, candidates, .. } = result.unwrap()
                    else {
                        panic!("expected bounded window");
                    };
                    let count = requested.min(ceiling - used).min(3 - used);
                    assert_eq!(permit.max_emission_items(), count);
                    assert_eq!(candidates.len(), count);
                    let proposed =
                        BoundedList::new(candidates.iter().map(|item| item.fingerprint).collect())
                            .unwrap();
                    owner
                        .revalidate_emission(&permit, &proposed, &live(&owner, &credential), &time(0))
                        .unwrap();
                }
                assert_eq!(snapshot(&owner), before);
            }
        }
    }
}

#[test]
fn exhausted_issuance_quota_does_not_block_explicit_invalidation_and_cleanup() {
    for durable in [false, true] {
        let mut owner = store();
        let credential = insert(&mut owner, 1, durable, 7);
        let initial = permit(&owner, &credential);
        owner.commit_emission(&initial, &emitted(&[1])).unwrap();
        owner
            .apply_live_limits(ContinuationLimits {
                max_issued_candidates: 1,
                ..owner.limits()
            })
            .unwrap();
        assert_eq!(
            owner.resume(&credential, &live(&owner, &credential), &time(0), 1),
            Err(ContinuationError::ResourceExhausted),
        );
        let expected = owner.records[&credential.continuation_id].cleanup_effect();
        let receipt = owner.invalidate(
            &InvalidationScope::Continuation(credential.continuation_id),
            InvalidationReason::Completed,
            NonZeroRevision::new(1).unwrap(),
        ).unwrap();
        assert_eq!(receipt.effects.as_slice(), &[expected]);
        assert_eq!(owner.resolve(&credential), Err(ContinuationError::SnapshotExpired));
        assert_eq!(owner.compact_terminal(1).unwrap().as_slice(), &[credential.continuation_id]);
        assert!(owner.is_empty());
        assert!(owner.token_index.is_empty());
    }
}

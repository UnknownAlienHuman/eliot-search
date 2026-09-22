//! Synthetic live-snapshot continuity tests through the public access API.
//! No persistence, source retrieval, external effects or provider qualification.

use std::collections::BTreeSet;

use search_access::{
    AccessError, ActiveRequestDecision, BaseEligibilityPlan, ContaminationDecision,
    IndexedRouteFence, LegSecurityPopulation, LiveSecurityState, MembershipAccessBinding,
    classify_active_request_contamination, classify_contaminated_legs, compile_base_eligibility,
    retain_eligible_plans,
};
use search_contracts::{
    Blake3Digest32, CollectionGenerationId, Epoch, OpaqueId, OwnerEpoch, SourceMembershipId,
};

const fn member(seed: u8) -> SourceMembershipId {
    SourceMembershipId::from_bytes([seed; 16])
}

const fn digest(seed: u8) -> Blake3Digest32 {
    Blake3Digest32::from_bytes([seed; 32])
}

fn live(generation: u64) -> LiveSecurityState {
    LiveSecurityState {
        generation,
        denied_memberships: BTreeSet::new(),
        purged_memberships: BTreeSet::new(),
        fail_closed: false,
        snapshot_digest: digest(7),
    }
}

fn execution() -> [LegSecurityPopulation; 2] {
    [
        LegSecurityPopulation {
            leg_id: 10, memberships: BTreeSet::from([member(1)]),
            security_generation: 7, idf_population_digest: Some(digest(1)),
        },
        LegSecurityPopulation {
            leg_id: 20, memberships: BTreeSet::from([member(2), member(3)]),
            security_generation: 7, idf_population_digest: Some(digest(2)),
        },
    ]
}

fn plan(seed: u8, generation: u64) -> BaseEligibilityPlan {
    compile_base_eligibility(
        &MembershipAccessBinding {
            membership_id: member(seed), access_partition_digest: digest(1),
            scoring_partition_digest: digest(2),
            projection_membership_id: OpaqueId::new("synthetic:projection").unwrap(), active: true,
        },
        IndexedRouteFence {
            collection_generation_id: CollectionGenerationId::from_bytes([4; 16]),
            visible_epoch: Epoch::new(12).unwrap(), route_generation: 2,
            owner_epoch: OwnerEpoch::new(1).unwrap(),
        },
        generation, 6, 6,
    ).unwrap()
}

#[test]
fn regressed_live_generation_never_certifies_work_as_clean() {
    for generation in [0, 1, 6] {
        let previous = live(7);
        let current = live(generation);
        let before = (previous.clone(), current.clone());
        assert_eq!(classify_contaminated_legs(&execution(), &previous, &current),
            ContaminationDecision::DiscardLegs(BTreeSet::from([10, 20])));
        for pending in [false, true] {
            assert_eq!(classify_active_request_contamination(
                &BTreeSet::from([member(1)]), &previous, &current, pending,
            ), ActiveRequestDecision::CancelAndGap);
        }
        assert_eq!((previous, current), before);
    }
}

#[test]
fn one_generation_cannot_name_conflicting_snapshot_contents() {
    for change in 0..4 {
        let previous = live(7);
        let mut current = previous.clone();
        match change {
            0 => { current.denied_memberships.insert(member(3)); }
            1 => { current.purged_memberships.insert(member(3)); }
            2 => { current.snapshot_digest = digest(99); }
            _ => { current.fail_closed = true; }
        }
        assert_eq!(classify_contaminated_legs(&execution(), &previous, &current),
            ContaminationDecision::DiscardLegs(BTreeSet::from([10, 20])));
        // Even unrelated or content-free requests cannot trust inconsistent
        // generations. This is not merely filtering the changed membership.
        for pending in [false, true] {
            assert_eq!(classify_active_request_contamination(
                &BTreeSet::from([member(1)]), &previous, &current, pending,
            ), ActiveRequestDecision::CancelAndGap);
        }
    }
}

#[test]
fn identical_snapshots_preserve_clean_and_content_free_results() {
    let previous = live(7);
    let current = previous.clone();
    assert_eq!(classify_contaminated_legs(&execution(), &previous, &current), ContaminationDecision::Clean);
    for (pending, expected) in [
        (true, ActiveRequestDecision::ContinueUnaffected),
        (false, ActiveRequestDecision::CompleteNonContentReceiptOnly),
    ] {
        assert_eq!(classify_active_request_contamination(
            &BTreeSet::from([member(1)]), &previous, &current, pending,
        ), expected);
    }
}

#[test]
fn valid_generation_increase_discards_the_whole_affected_leg_only() {
    for purged in [false, true] {
        let previous = live(7);
        let mut current = live(8);
        current.snapshot_digest = digest(8);
        if purged {
            current.purged_memberships.insert(member(2));
        } else {
            current.denied_memberships.insert(member(2));
        }
        assert_eq!(classify_contaminated_legs(&execution(), &previous, &current),
            ContaminationDecision::DiscardLegs(BTreeSet::from([20])));
        assert_eq!(classify_active_request_contamination(
            &BTreeSet::from([member(2)]), &previous, &current, true,
        ), if purged { ActiveRequestDecision::Deny } else { ActiveRequestDecision::DiscardAndReplan });
        assert_eq!(classify_active_request_contamination(
            &BTreeSet::from([member(1)]), &previous, &current, true,
        ), ActiveRequestDecision::ContinueUnaffected);
    }
}

#[test]
fn newer_unrelated_or_permissive_snapshots_are_not_globally_rejected() {
    for permissive in [false, true] {
        let mut previous = live(7);
        let mut current = live(8);
        current.snapshot_digest = digest(8);
        if permissive {
            previous.denied_memberships.insert(member(9));
        } else {
            current.denied_memberships.insert(member(9));
        }
        assert_eq!(classify_contaminated_legs(&execution(), &previous, &current), ContaminationDecision::Clean);
        assert_eq!(classify_active_request_contamination(
            &BTreeSet::from([member(1)]), &previous, &current, true,
        ), ActiveRequestDecision::ContinueUnaffected);
    }
}

#[test]
fn current_snapshot_cannot_certify_a_leg_from_its_future() {
    let state = live(7);
    for future in [8, u64::MAX] {
        let mut legs = execution();
        legs[1].security_generation = future;
        let before = legs.clone();
        assert_eq!(classify_contaminated_legs(&legs, &state, &state),
            ContaminationDecision::DiscardLegs(BTreeSet::from([20])));
        assert_eq!(legs, before);
    }
    let maximum = live(u64::MAX);
    let mut legs = execution();
    for leg in &mut legs { leg.security_generation = u64::MAX; }
    assert_eq!(classify_contaminated_legs(&legs, &maximum, &maximum), ContaminationDecision::Clean);
}

#[test]
fn predicate_retention_rejects_one_stale_observation_without_a_partial_result() {
    for future_position in 0..3 {
        let mut plans = vec![plan(1, 7), plan(2, 7), plan(3, 7)];
        plans[future_position].live_security_generation = 8;
        let before = plans.clone();
        assert_eq!(retain_eligible_plans(&plans, &live(7)), Err(AccessError::SecurityFenceStale));
        assert_eq!(plans, before);
        let retained = retain_eligible_plans(&plans, &live(8)).unwrap();
        assert_eq!(retained.len(), plans.len());
        for (observed, original) in retained.iter().zip(&plans) {
            assert!(std::ptr::eq(*observed, original));
        }
    }
    let maximum = [plan(1, u64::MAX)];
    assert!(retain_eligible_plans(&maximum, &live(u64::MAX)).is_ok());
}

#[test]
fn immediate_security_refusals_keep_priority_over_stale_predicates() {
    let plans = [plan(1, 8)];
    let mut state = live(7);
    state.fail_closed = true;
    state.denied_memberships.insert(member(1));
    state.purged_memberships.insert(member(1));
    assert_eq!(retain_eligible_plans(&plans, &state), Err(AccessError::SecurityFailClosed));
    state.fail_closed = false;
    assert_eq!(retain_eligible_plans(&plans, &state), Err(AccessError::LivePurge));
    state.purged_memberships.clear();
    assert_eq!(retain_eligible_plans(&plans, &state), Err(AccessError::LiveRevocation));
    state.denied_memberships.clear();
    assert_eq!(retain_eligible_plans(&plans, &state), Err(AccessError::SecurityFenceStale));
}

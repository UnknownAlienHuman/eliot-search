//! Synthetic time/limit regressions for the canonical in-memory owner.
//! No clock source, token entropy, external pin or durable backend is qualified here.

use super::atomicity_tests::{emitted, insert, permit, snapshot, store, time};
use super::lifetime::{duration_micros, fits_ttl};
use super::{
    AuthorizationFence, BoundedList, ContinuationCredential, ContinuationError,
    ContinuationLimits, ContinuationRecord, ContinuationStore, ContinuationTokenMaterial,
    ContinuityFence, CreateContinuationRequest, CurrencyFence, LifecycleRecordStatus,
    LiveContinuationState, OpaqueHandleToken, UtcTimestamp,
};

fn utc(value: &str) -> UtcTimestamp {
    UtcTimestamp::parse(value).expect("synthetic timestamp")
}

fn set_lifetime(record: &mut ContinuationRecord, start: &str, end: &str) {
    let (created_at, expires_at) = match record {
        ContinuationRecord::EphemeralWindow(value) => (&mut value.created_at, &mut value.expires_at),
        ContinuationRecord::DurableReplanCheckpoint(value) => {
            (&mut value.created_at, &mut value.expires_at)
        }
    };
    *created_at = utc(start);
    *expires_at = utc(end);
}

fn request(durable: bool, start: &str, end: &str, ttl_millis: u64) -> CreateContinuationRequest {
    let mut fixture = store();
    let credential = insert(&mut fixture, 1, durable, 1);
    let mut value = fixture.records.remove(&credential.continuation_id).unwrap();
    set_lifetime(&mut value.record, start, end);
    CreateContinuationRequest {
        record: value.record,
        payload: value.payload,
        token: ContinuationTokenMaterial {
            continuation_id: credential.continuation_id,
            opaque_token: OpaqueHandleToken::new(&[1; 32]).expect("synthetic token bytes"),
            token_digest: credential.token_digest,
        },
        ttl_millis,
        issued_fingerprints: BoundedList::empty(),
    }
}

fn live(store: &ContinuationStore, credential: &ContinuationCredential) -> LiveContinuationState {
    let value = &store.records[&credential.continuation_id];
    LiveContinuationState {
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
    }
}

const START: &str = "2026-09-21T00:00:00.000000Z";
const MINUTE: &str = "2026-09-21T00:01:00.000000Z";

#[test]
fn creation_rejects_a_false_short_ttl_without_inserting_any_state() {
    for durable in [false, true] {
        let mut owner = store();
        insert(&mut owner, 99, false, 7);
        let before = snapshot(&owner);
        assert_eq!(
            owner.create(request(durable, START, MINUTE, 1_000)),
            Err(ContinuationError::InvalidTtl),
        );
        assert_eq!(snapshot(&owner), before);
    }
}

#[test]
fn creation_accepts_exact_and_shorter_lifetimes_without_rewriting_expiry() {
    for durable in [false, true] {
        for end in [MINUTE, "2026-09-21T00:00:00.000001Z"] {
            let mut owner = store();
            let created = owner.create(request(durable, START, end, 60_000)).unwrap();
            assert_eq!(created.handle.expires_at, utc(end));
            assert_eq!(owner.len(), 1);
            assert_eq!(owner.token_index.len(), 1);
        }
    }
}

#[test]
fn one_microsecond_over_the_limit_is_not_rounded_down() {
    for durable in [false, true] {
        let mut owner = store();
        let before = snapshot(&owner);
        assert_eq!(
            owner.create(request(durable, START, "2026-09-21T00:01:00.000001Z", 60_000)),
            Err(ContinuationError::InvalidTtl),
        );
        assert_eq!(snapshot(&owner), before);
        assert!(owner.create(request(durable, START, MINUTE, 60_000)).is_ok());
    }
}

#[test]
fn invalid_declared_or_actual_lifetimes_preserve_the_store() {
    for durable in [false, true] {
        for (start, end, ttl) in [
            (START, MINUTE, 0),
            (START, MINUTE, 900_001),
            (START, START, 60_000),
            (MINUTE, START, 60_000),
            (START, "2027-09-21T00:00:00.000000Z", 900_000),
        ] {
            let mut owner = store();
            let before = snapshot(&owner);
            assert_eq!(
                owner.create(request(durable, start, end, ttl)),
                Err(ContinuationError::InvalidTtl),
            );
            assert_eq!(snapshot(&owner), before);
        }
    }
}

#[test]
fn calendar_rollovers_preserve_microsecond_precision() {
    for (start, end) in [
        ("2026-09-21T00:00:59.999500Z", "2026-09-21T00:01:00.000500Z"),
        ("2026-09-21T00:59:59.999500Z", "2026-09-21T01:00:00.000500Z"),
        ("2026-09-30T23:59:59.999500Z", "2026-10-01T00:00:00.000500Z"),
        ("2026-12-31T23:59:59.999500Z", "2027-01-01T00:00:00.000500Z"),
        ("2000-02-28T23:59:59.999500Z", "2000-02-29T00:00:00.000500Z"),
        ("2000-02-29T23:59:59.999500Z", "2000-03-01T00:00:00.000500Z"),
        ("1900-02-28T23:59:59.999500Z", "1900-03-01T00:00:00.000500Z"),
        ("2100-02-28T23:59:59.999500Z", "2100-03-01T00:00:00.000500Z"),
    ] {
        assert_eq!(duration_micros(&utc(start), &utc(end)), Some(1_000));
        assert!(fits_ttl(&utc(start), &utc(end), 1));
        for durable in [false, true] {
            assert!(store().create(request(durable, start, end, 1)).is_ok());
        }
    }
}

#[test]
fn the_entire_timestamp_range_and_u64_ttl_do_not_overflow() {
    let first = utc("0001-01-01T00:00:00.000000Z");
    let last = utc("9999-12-31T23:59:59.999999Z");
    assert_eq!(duration_micros(&first, &last), Some(315_537_897_599_999_999));
    assert!(fits_ttl(&first, &last, u64::MAX));
    assert!(!fits_ttl(&first, &last, 315_537_897_599_999));
    assert!(fits_ttl(&first, &last, 315_537_897_600_000));
    assert_eq!(duration_micros(&last, &first), None);
    assert!(!fits_ttl(&first, &first, u64::MAX));
    for durable in [false, true] {
        let mut owner = ContinuationStore::new(ContinuationLimits {
            max_ttl_millis: u64::MAX,
            ..ContinuationLimits::BASELINE
        }).unwrap();
        assert!(owner.create(request(durable, first.as_str(), last.as_str(), u64::MAX)).is_ok());
    }
}

#[test]
fn every_date_in_a_gregorian_cycle_has_the_expected_ordinal() {
    let first = utc("2000-01-01T00:00:00.000000Z");
    let mut days = 0_u64;
    for year in 2000..2400 {
        for month in 1..=12 {
            for day in 1..=31 {
                let Ok(current) = UtcTimestamp::parse(format!(
                    "{year:04}-{month:02}-{day:02}T00:00:00.000000Z"
                )) else {
                    continue;
                };
                assert_eq!(duration_micros(&first, &current), Some(days * 86_400_000_000));
                days += 1;
            }
        }
    }
    assert_eq!(days, 146_097);
}

#[test]
fn restrictive_ttl_expires_only_incompatible_records_with_exact_cleanup() {
    for durable in [false, true] {
        let mut owner = store();
        let overlong = insert(&mut owner, 1, durable, 7);
        let survivor = insert(&mut owner, 2, !durable, 7);
        set_lifetime(
            &mut owner.records.get_mut(&overlong.continuation_id).unwrap().record,
            START,
            "2026-09-21T00:01:00.000001Z",
        );
        let old_survivor = owner.records[&survivor.continuation_id].clone();
        let old_deadline = owner.records[&overlong.continuation_id].expires_at().clone();
        let expected = owner.records[&overlong.continuation_id].cleanup_effect();
        let limits = ContinuationLimits { max_ttl_millis: 60_000, ..owner.limits() };
        let receipt = owner.apply_live_limits(limits).unwrap();
        assert_eq!(receipt.expired.iter().copied().collect::<Vec<_>>(), vec![overlong.continuation_id]);
        assert_eq!(receipt.effects.iter().cloned().collect::<Vec<_>>(), vec![expected]);
        let value = &owner.records[&overlong.continuation_id];
        assert_eq!(value.status(), LifecycleRecordStatus::Expired);
        assert_eq!(value.revision, 8);
        assert_eq!(value.expires_at(), &old_deadline);
        assert_eq!(owner.records[&survivor.continuation_id], old_survivor);
        assert_eq!(owner.limits(), limits);
    }
}

#[test]
fn ttl_restriction_preflights_every_revision_before_applying_any_record() {
    for exhausted_id in 1..=3 {
        let mut owner = store();
        for seed in 1..=3 {
            insert(&mut owner, seed, seed == 2, if seed == exhausted_id { u64::MAX } else { 7 });
        }
        let limits = ContinuationLimits { max_ttl_millis: 30_000, ..owner.limits() };
        let before = snapshot(&owner);
        assert_eq!(owner.apply_live_limits(limits), Err(ContinuationError::RevisionExhausted));
        assert_eq!(snapshot(&owner), before);
    }
}

#[test]
fn a_ttl_batch_over_the_limit_does_not_publish_configuration() {
    let mut owner = store();
    insert(&mut owner, 1, false, 7);
    insert(&mut owner, 2, true, 7);
    owner.limits.max_lifecycle_batch = 1;
    let limits = ContinuationLimits { max_ttl_millis: 30_000, ..owner.limits() };
    let before = snapshot(&owner);
    assert_eq!(owner.apply_live_limits(limits), Err(ContinuationError::ResourceExhausted));
    assert_eq!(snapshot(&owner), before);
}

#[test]
fn applying_equal_or_larger_ttl_never_renews_or_revives_a_record() {
    let mut owner = store();
    let credential = insert(&mut owner, 1, false, 7);
    let limits = ContinuationLimits { max_ttl_millis: 30_000, ..owner.limits() };
    assert_eq!(owner.apply_live_limits(limits).unwrap().expired.len(), 1);
    let before = snapshot(&owner);
    assert!(owner.apply_live_limits(limits).unwrap().effects.is_empty());
    assert_eq!(snapshot(&owner), before);
    let value = owner.records[&credential.continuation_id].clone();
    assert!(owner.apply_live_limits(ContinuationLimits::BASELINE).unwrap().effects.is_empty());
    assert_eq!(owner.records[&credential.continuation_id], value);
    assert_eq!(owner.resolve(&credential), Err(ContinuationError::SnapshotExpired));
}

#[test]
fn ttl_revocation_invalidates_previously_selected_permits() {
    for durable in [false, true] {
        let mut owner = store();
        let credential = insert(&mut owner, 1, durable, 7);
        let selected = permit(&owner, &credential);
        let limits = ContinuationLimits { max_ttl_millis: 30_000, ..owner.limits() };
        owner.apply_live_limits(limits).unwrap();
        let before = snapshot(&owner);
        assert_eq!(
            owner.commit_emission(&selected, &emitted(&[1])),
            Err(ContinuationError::StalePermit),
        );
        assert_eq!(snapshot(&owner), before);
    }
}

#[test]
fn future_creation_is_rejected_at_resume_and_all_predelivery_checkpoints() {
    let before_creation = utc("2026-09-20T23:59:59.999999Z");
    for durable in [false, true] {
        let mut owner = store();
        let credential = insert(&mut owner, 1, durable, 7);
        let observed = live(&owner, &credential);
        let selected = permit(&owner, &credential);
        let before = snapshot(&owner);
        assert_eq!(
            owner.resume(&credential, &observed, &before_creation, 1),
            Err(ContinuationError::SnapshotExpired),
        );
        assert_eq!(
            owner.revalidate_emission(&selected, &emitted(&[1]), &observed, &before_creation),
            Err(ContinuationError::SnapshotExpired),
        );
        if durable {
            let super::ResumePlan::DurableReplan { permit: pending, .. } =
                owner.resume(&credential, &observed, &time(0), 1).unwrap()
            else {
                panic!("durable fixture");
            };
            assert_eq!(
                owner.bind_durable_emission(&pending, &observed, &before_creation, &emitted(&[1])),
                Err(ContinuationError::SnapshotExpired),
            );
        }
        assert_eq!(snapshot(&owner), before);
    }
}

#[test]
fn creation_time_is_inclusive_and_expiry_is_exclusive() {
    let just_before_expiry = utc("2026-09-21T00:00:59.999999Z");
    for durable in [false, true] {
        let mut owner = store();
        let credential = insert(&mut owner, 1, durable, 7);
        let observed = live(&owner, &credential);
        assert!(owner.resume(&credential, &observed, &time(0), 1).is_ok());
        assert!(owner.resume(&credential, &observed, &just_before_expiry, 1).is_ok());
        assert_eq!(
            owner.resume(&credential, &observed, &time(1), 1),
            Err(ContinuationError::SnapshotExpired),
        );
    }
}

#[test]
fn new_creations_obey_the_successfully_restricted_limit() {
    for durable in [false, true] {
        let mut owner = store();
        let limits = ContinuationLimits { max_ttl_millis: 30_000, ..owner.limits() };
        assert!(owner.apply_live_limits(limits).unwrap().effects.is_empty());
        let before = snapshot(&owner);
        assert_eq!(
            owner.create(request(durable, START, MINUTE, 30_000)),
            Err(ContinuationError::InvalidTtl),
        );
        assert_eq!(snapshot(&owner), before);
    }
}

#[test]
fn combined_ttl_and_window_restrictions_return_cleanup_only_once() {
    let mut owner = store();
    let credential = insert(&mut owner, 1, false, 7);
    let limits = ContinuationLimits {
        max_ttl_millis: 30_000,
        max_candidate_window: 1,
        max_expansion_items: 1,
        ..owner.limits()
    };
    let receipt = owner.apply_live_limits(limits).unwrap();
    assert_eq!(receipt.expired.iter().copied().collect::<Vec<_>>(), vec![credential.continuation_id]);
    assert_eq!(receipt.effects.len(), 1);
    assert_eq!(owner.records[&credential.continuation_id].revision, 8);
}

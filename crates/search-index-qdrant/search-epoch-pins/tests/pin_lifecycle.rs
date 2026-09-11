//! T29 pin-lifecycle discriminating tests: pins protect visible snapshots and
//! old routes, release exactly once on every terminal path, and never survive
//! the process.

#![forbid(unsafe_code)]

use search_contracts::{CollectionGenerationId, CollectionRouteRevision, Epoch, OpaqueId};
use search_epoch_pins::{
    EpochPinPurpose, PinLimits, PinRegistry, RetiredVisibilityFence, RouteIdentity, can_reclaim,
    compute_reclamation_watermark,
};

const fn route(generation: u8, rev: u64) -> RouteIdentity {
    RouteIdentity {
        collection_generation_id: CollectionGenerationId::from_bytes([generation; 16]),
        route_revision: CollectionRouteRevision::new(rev),
    }
}

fn epoch(value: i64) -> Epoch {
    Epoch::new(value).expect("fixture epoch is valid")
}

fn owner(tag: &str) -> OpaqueId {
    OpaqueId::new(tag).expect("fixture owner is valid")
}

fn registry() -> PinRegistry {
    PinRegistry::new(route(0xA1, 3), epoch(7), PinLimits::BASELINE).expect("fixture registry")
}

fn fence(generation: u8, rev: u64, retired_at: i64) -> RetiredVisibilityFence {
    RetiredVisibilityFence {
        route: route(generation, rev),
        retirement_epoch_exclusive: epoch(retired_at),
    }
}

#[test]
fn pinned_visible_points_cannot_be_reclaimed() {
    let registry = registry();
    let retired = fence(0xA1, 3, 8);
    let guard = registry
        .acquire_epoch_pin(
            route(0xA1, 3),
            epoch(7),
            owner("t29:query-a"),
            EpochPinPurpose::Query,
            1_000,
        )
        .expect("visible epoch pins");
    let snapshot = registry.snapshot().expect("snapshot reads");
    assert!(!can_reclaim(retired, &snapshot));
    let watermark = compute_reclamation_watermark(retired, &snapshot);
    assert_eq!(watermark.blocking_epoch_pins, 1);
    assert_eq!(watermark.blocking_route_pins, 0);
    drop(guard);
    let snapshot = registry.snapshot().expect("snapshot reads");
    assert!(can_reclaim(retired, &snapshot));
}

#[test]
fn guard_releases_on_drop_and_owner_release_is_idempotent() {
    let registry = registry();
    let guard = registry
        .acquire_epoch_pin(
            route(0xA1, 3),
            epoch(7),
            owner("t29:query-a"),
            EpochPinPurpose::Query,
            1_000,
        )
        .expect("visible epoch pins");
    drop(guard);
    let receipt = registry
        .release_owner_pins(&owner("t29:query-a"))
        .expect("release reads");
    assert_eq!(receipt.released_pins, 0);
    let snapshot = registry.snapshot().expect("snapshot reads");
    assert_eq!(snapshot.total_pins, 0);
}

#[test]
fn cancellation_releases_only_the_cancelled_owner() {
    let registry = registry();
    let guard_a = registry
        .acquire_epoch_pin(
            route(0xA1, 3),
            epoch(7),
            owner("t29:query-a"),
            EpochPinPurpose::Query,
            1_000,
        )
        .expect("owner A pins");
    let _guard_b = registry
        .acquire_epoch_pin(
            route(0xA1, 3),
            epoch(7),
            owner("t29:query-b"),
            EpochPinPurpose::Query,
            1_000,
        )
        .expect("owner B pins");
    let receipt = registry
        .release_owner_pins(&owner("t29:query-a"))
        .expect("cancel releases");
    assert_eq!(receipt.released_pins, 1);
    drop(guard_a);
    // Owner B pinned the same epoch: the retired state is still observable.
    let snapshot = registry.snapshot().expect("snapshot reads");
    assert!(!can_reclaim(fence(0xA1, 3, 8), &snapshot));
    assert_eq!(snapshot.total_pins, 1);
}

#[test]
fn continuation_ttl_bounds_pin_lifetime_without_leak() {
    let registry = registry();
    let guard = registry
        .acquire_epoch_pin(
            route(0xA1, 3),
            epoch(6),
            owner("t29:continuation-a"),
            EpochPinPurpose::Continuation {
                expires_at_ms: 2_000,
            },
            1_000,
        )
        .expect("continuation pins within TTL");
    let early = registry
        .expire_continuation_pins(1_500, 64)
        .expect("bounded sweep reads");
    assert_eq!(early.expired_pins, 0);
    assert!(!early.more_expired);
    assert!(!can_reclaim(
        fence(0xA1, 3, 7),
        &registry.snapshot().expect("snapshot reads")
    ));
    let late = registry
        .expire_continuation_pins(2_000, 64)
        .expect("bounded sweep reads");
    // The sweep expires the still-held continuation pin exactly once; the
    // later guard drop is an idempotent no-op, so nothing leaks.
    assert_eq!(late.expired_pins, 1);
    assert!(!late.more_expired);
    drop(guard);
    let snapshot = registry.snapshot().expect("snapshot reads");
    assert_eq!(snapshot.total_pins, 0);
    assert!(snapshot.earliest_continuation_expiry_ms.is_none());
    assert!(can_reclaim(fence(0xA1, 3, 7), &snapshot));
}

#[test]
fn expired_unowned_continuation_pins_do_not_leak() {
    let registry = registry();
    for index in 0..4 {
        let _guard = registry
            .acquire_epoch_pin(
                route(0xA1, 3),
                epoch(6),
                owner(&format!("t29:continuation-{index}")),
                EpochPinPurpose::Continuation {
                    expires_at_ms: 2_000,
                },
                1_000,
            )
            .expect("continuation pins within TTL");
        // Guards drop at iteration end, exactly like a disconnect path.
    }
    let receipt = registry
        .expire_continuation_pins(5_000, 64)
        .expect("bounded sweep reads");
    assert_eq!(receipt.expired_pins, 0);
    let snapshot = registry.snapshot().expect("snapshot reads");
    assert_eq!(snapshot.total_pins, 0);
    assert!(snapshot.earliest_continuation_expiry_ms.is_none());
}

#[test]
fn old_route_drains_only_after_final_pin_release() {
    let registry = registry();
    let old_route_pin = registry
        .acquire_route_pin(route(0xA1, 3), owner("t29:drain"))
        .expect("old route pins during migration");
    registry
        .publish_active_route(route(0xB2, 4), epoch(9))
        .expect("cutover publishes the new route");
    // The retired old route stays protected while its drain pin is held, even
    // though it is no longer the active route.
    let snapshot = registry.snapshot().expect("snapshot reads");
    assert!(!can_reclaim(fence(0xA1, 3, 8), &snapshot));
    drop(old_route_pin);
    let snapshot = registry.snapshot().expect("snapshot reads");
    assert!(can_reclaim(fence(0xA1, 3, 8), &snapshot));
}

#[test]
fn query_pins_are_not_renewable_and_stale_routes_are_denied() {
    let registry = registry();
    let mut guard = registry
        .acquire_epoch_pin(
            route(0xA1, 3),
            epoch(7),
            owner("t29:query-a"),
            EpochPinPurpose::Query,
            1_000,
        )
        .expect("visible epoch pins");
    let renewal = guard.renew_continuation_pin(1_100, 1_200);
    assert_eq!(renewal, Err(search_epoch_pins::PinError::PinNotRenewable));
    registry
        .publish_active_route(route(0xB2, 4), epoch(9))
        .expect("cutover publishes the new route");
    let stale = registry
        .acquire_epoch_pin(
            route(0xA1, 3),
            epoch(7),
            owner("t29:stale-owner"),
            EpochPinPurpose::Query,
            1_100,
        )
        .map(|_| ());
    assert_eq!(stale, Err(search_epoch_pins::PinError::RouteNotActive));
}

#[test]
fn crash_requires_no_durable_lease_cleanup() {
    // Pins are intentionally process-local: a fresh registry (the post-crash
    // process) starts empty and never replays old pins.
    let snapshot = registry().snapshot().expect("snapshot reads");
    assert_eq!(snapshot.total_pins, 0);
    assert!(snapshot.route_counts.is_empty());
    assert!(snapshot.epoch_counts.is_empty());
}

#[test]
fn finite_limits_fail_closed() {
    let bad = PinLimits {
        max_total_pins: 0,
        max_pins_per_owner: 1,
        max_continuation_ttl_ms: 1,
    };
    assert_eq!(
        PinRegistry::new(route(0xA1, 3), epoch(7), bad).map(|_| ()),
        Err(search_epoch_pins::PinError::InvalidLimits)
    );
    let registry = PinRegistry::new(
        route(0xA1, 3),
        epoch(7),
        PinLimits {
            max_total_pins: 64,
            max_pins_per_owner: 1,
            max_continuation_ttl_ms: 60_000,
        },
    )
    .expect("fixture registry");
    let _first = registry
        .acquire_epoch_pin(
            route(0xA1, 3),
            epoch(7),
            owner("t29:capped"),
            EpochPinPurpose::Query,
            1_000,
        )
        .expect("first pin fits the owner ceiling");
    let second = registry
        .acquire_epoch_pin(
            route(0xA1, 3),
            epoch(7),
            owner("t29:capped"),
            EpochPinPurpose::Query,
            1_000,
        )
        .map(|_| ());
    assert_eq!(
        second,
        Err(search_epoch_pins::PinError::OwnerCapacityExceeded)
    );
}

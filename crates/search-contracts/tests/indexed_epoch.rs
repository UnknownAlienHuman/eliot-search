//! Focused #258 collection-epoch boundary fixtures.
//!
//! The Qdrant-compatible logical epoch domain is closed and inclusive:
//! `0..=MAX_QDRANT_EPOCH`, where the maximum is `2^53`. Zero is the empty
//! initial generation and never a published point. The maximum stays
//! readable as the last published epoch but can never reserve another one.

use search_contracts::{ContractErrorKind, Epoch, MAX_QDRANT_EPOCH, MIN_QDRANT_EPOCH};

/// The frozen constant equals `2^53`, the exact upper bound of the closed
/// domain.
#[test]
fn maximum_epoch_is_the_two_pow_fifty_three_boundary() {
    assert_eq!(MAX_QDRANT_EPOCH, 9_007_199_254_740_992);
    assert_eq!(MIN_QDRANT_EPOCH, 0);
    assert_eq!(
        i64::try_from(1_i128 << 53).expect("2^53 fits i64"),
        MAX_QDRANT_EPOCH
    );
}

/// Boundary one: the minimum is accepted and reported unchanged.
#[test]
fn minimum_epoch_is_the_empty_initial_generation() {
    let epoch = Epoch::new(MIN_QDRANT_EPOCH).expect("zero is in the domain");
    assert_eq!(epoch.get(), 0);
    assert_eq!(
        epoch.checked_next().expect("zero reserves one").get(),
        1,
        "the first publication normally uses epoch 1"
    );
}

/// Boundary two and three: the closed domain rejects both sides of its
/// immediate neighbours, so nothing outside the domain is constructible.
#[test]
fn domain_edges_reject_their_neighbours() {
    let below = Epoch::new(MIN_QDRANT_EPOCH - 1).expect_err("negative is refused");
    assert_eq!(below.kind(), ContractErrorKind::EpochOutOfRange);

    let above = Epoch::new(MAX_QDRANT_EPOCH + 1).expect_err("one past the maximum");
    assert_eq!(above.kind(), ContractErrorKind::EpochOutOfRange);

    // The former sentinel pair is now fully outside the closed domain.
    assert!(Epoch::new(i64::MAX).is_err());
    assert!(Epoch::new(i64::MAX - 1).is_err());
}

/// Boundary four: the maximum itself is a valid, readable epoch.
#[test]
fn maximum_epoch_is_a_valid_readable_epoch() {
    let epoch = Epoch::new(MAX_QDRANT_EPOCH).expect("the maximum is in the domain");
    assert_eq!(epoch.get(), MAX_QDRANT_EPOCH);
}

/// Boundary five: `checked_next` increments normally below the maximum.
#[test]
fn stepping_below_the_maximum_is_allowed() {
    let start = Epoch::new(MAX_QDRANT_EPOCH - 1).expect("penultimate is in the domain");
    let advanced = start.checked_next().expect("penultimate can reserve one");
    assert_eq!(advanced.get(), MAX_QDRANT_EPOCH);
}

/// Boundary six: `checked_next` refuses at the maximum with the exhaustion
/// reason, so the next publication must move to a fresh collection
/// generation rather than a wider epoch.
#[test]
fn checked_next_refuses_at_the_maximum() {
    let epoch = Epoch::new(MAX_QDRANT_EPOCH).expect("the maximum is in the domain");
    let exhausted = epoch
        .checked_next()
        .expect_err("no epoch exists above the maximum");
    assert_eq!(exhausted.kind(), ContractErrorKind::EpochExhausted);
    assert_eq!(exhausted.field(), "epoch");
}

/// Boundary seven: the epoch is still stored as signed 64-bit bytes, so
/// handling of in-range bytes is unchanged. Values above the closed domain
/// now fail closed deliberately; this fixture makes no claim about
/// canonical wire bytes or the readability of out-of-range values.
#[test]
fn representation_is_unchanged_signed_sixty_four_bit() {
    let epoch = Epoch::new(MAX_QDRANT_EPOCH).expect("the maximum is in the domain");
    let raw = epoch.get().to_be_bytes();
    assert_eq!(raw.len(), 8, "the signed 64-bit representation is retained");
    assert_eq!(i64::from_be_bytes(raw), MAX_QDRANT_EPOCH);
    assert_eq!(
        Epoch::new(i64::from_be_bytes(raw)).expect("round trip"),
        epoch
    );
}

/// `OwnerEpoch` guards owner incarnation, not collection epochs, and keeps
/// its own non-zero `u64` domain. This fixture fails if the two types
/// collapse into one constraint.
#[test]
fn owner_epoch_domain_is_independent() {
    assert!(search_contracts::OwnerEpoch::new(0).is_err());
    assert_eq!(
        search_contracts::OwnerEpoch::new(1)
            .expect("owner epoch one")
            .get(),
        1
    );
}

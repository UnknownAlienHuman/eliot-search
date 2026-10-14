//! Focused proofs for immutable portfolio history and guarded pointers.

use search_contracts::{NonZeroRevision, OpaqueId, PortfolioRevision, ReferencePortfolioId,
    SourceMembershipId};

use crate::error::RegistryError;
use crate::portfolio_history::{
    ImmutablePortfolioRevision, PortfolioHistory, PortfolioRevisionKey,
};

fn portfolio_id(seed: u8) -> ReferencePortfolioId {
    ReferencePortfolioId::from_bytes([seed; 16])
}

fn revision(value: u64) -> PortfolioRevision {
    PortfolioRevision::new(value)
}

fn key(seed: u8, value: u64) -> PortfolioRevisionKey {
    PortfolioRevisionKey::new(portfolio_id(seed), revision(value))
}

fn membership(seed: u8) -> SourceMembershipId {
    SourceMembershipId::from_bytes([seed; 16])
}

fn content(seeds: &[u8]) -> ImmutablePortfolioRevision {
    ImmutablePortfolioRevision::new(seeds.iter().copied().map(membership).collect())
}

fn operation(seed: u8) -> OpaqueId {
    OpaqueId::new(format!("op-{seed}")).expect("operation identity")
}

fn pointer_revision(value: u64) -> NonZeroRevision {
    NonZeroRevision::new(value).expect("non-zero pointer revision")
}

fn receipt(value: &str) -> search_contracts::ReceiptRef {
    search_contracts::ReceiptRef::new(value.to_owned()).expect("receipt reference")
}

#[test]
fn three_revisions_stay_resolvable_after_pointer_moves() {
    let mut history = PortfolioHistory::new();
    let first = key(1, 1);
    let second = key(1, 2);
    let third = key(1, 3);
    history
        .publish_revision(first, content(&[7]), 1, &receipt("r1"), 16)
        .expect("first revision");
    history
        .publish_revision(second, content(&[7, 8]), 2, &receipt("r2"), 16)
        .expect("second revision");
    history
        .publish_revision(third, content(&[7, 9]), 3, &receipt("r3"), 16)
        .expect("third revision");
    assert_eq!(history.len(), 3, "no revision is replaced by a later one");
    history
        .move_current(second, pointer_revision(1), None, &operation(1), &receipt("p1"))
        .expect("first pointer move");
    history
        .move_current(
            third,
            pointer_revision(2),
            Some(second),
            &operation(2),
            &receipt("p2"),
        )
        .expect("second pointer move");
    assert_eq!(history.current_key(), Some(third), "pointer names newest");
    assert_eq!(
        history.revision(&first).map(|record| record.revision.membership_precedence()),
        Some(&[membership(7)][..]),
        "oldest revision keeps its exact interpretation",
    );
    assert_eq!(
        history.revision(&second).expect("second retained")
            .revision
            .membership_precedence(),
        &[membership(7), membership(8)],
    );
    let current = history.current_revision().expect("current");
    let retained = history.revision(&third).expect("third retained");
    assert_eq!(current, retained, "current resolves the newest revision");
}

#[test]
fn same_key_same_content_replays_and_different_content_conflicts() {
    let mut history = PortfolioHistory::new();
    let target = key(2, 1);
    let replayed = history
        .publish_revision(target, content(&[3]), 1, &receipt("r1"), 16)
        .expect("first publication");
    let again = history
        .publish_revision(target, content(&[3]), 1, &receipt("r1"), 16)
        .expect("identical replay");
    assert_eq!(replayed, again, "same key and content replay the exact record");
    assert_eq!(history.len(), 1, "replay retains one record, not two");
    let conflict = history.publish_revision(target, content(&[4]), 2, &receipt("r2"), 16);
    assert_eq!(
        conflict.err(),
        Some(RegistryError::ReferencePortfolioInvalid),
        "same key with different content refuses to rewrite history",
    );
    assert_eq!(
        history.revision(&target).expect("retained").revision,
        content(&[3]),
        "the conflicting content is not stored",
    );
}

#[test]
fn membership_order_is_part_of_revision_identity() {
    let mut history = PortfolioHistory::new();
    let target = key(3, 1);
    history
        .publish_revision(target, content(&[1, 2]), 1, &receipt("r1"), 16)
        .expect("ordered publication");
    let reordered = history
        .publish_revision(target, content(&[2, 1]), 1, &receipt("r1"), 16);
    assert_eq!(
        reordered.err(),
        Some(RegistryError::ReferencePortfolioInvalid),
        "a permutation is a different revision rather than a relabelling",
    );
}

#[test]
fn pointer_move_conflicts_are_fail_closed_and_retain_history() {
    let mut history = PortfolioHistory::new();
    let first = key(4, 1);
    let second = key(4, 2);
    history
        .publish_revision(first, content(&[1]), 1, &receipt("r1"), 16)
        .expect("first");
    history
        .publish_revision(second, content(&[2]), 2, &receipt("r2"), 16)
        .expect("second");
    history
        .move_current(first, pointer_revision(1), None, &operation(1), &receipt("p1"))
        .expect("initial move");
    assert_eq!(
        history
            .move_current(
                second,
                pointer_revision(2),
                Some(key(4, 9)),
                &operation(2),
                &receipt("p2"),
            )
            .err(),
        Some(RegistryError::ReferenceScopeEmpty),
        "a stale expected-current guard refuses the move",
    );
    assert_eq!(
        history
            .move_current(second, pointer_revision(1), Some(first), &operation(3), &receipt("p3"))
            .err(),
        Some(RegistryError::MembershipGenerationMismatch),
        "a skipped or regressed pointer revision refuses the move",
    );
    assert_eq!(
        history
            .move_current(second, pointer_revision(2), Some(first), &operation(1), &receipt("p4"))
            .err(),
        Some(RegistryError::OperationConflict),
        "one operation identity cannot move the pointer to two targets",
    );
    assert_eq!(
        history
            .move_current(key(4, 9), pointer_revision(2), Some(first), &operation(4), &receipt("p5"))
            .err(),
        Some(RegistryError::ReferencePortfolioInvalid),
        "a pointer cannot name an unpublished revision",
    );
    assert_eq!(history.current_key(), Some(first), "pointer never moved");
    assert_eq!(history.len(), 2, "every conflict retains history intact");
}

#[test]
fn aggregate_history_ceiling_bounds_total_records_not_latest_per_identity() {
    let mut history = PortfolioHistory::new();
    for index in 1..=3_u64 {
        history
            .publish_revision(
                key(5, index),
                content(&[u8::try_from(index).expect("bounded seed")]),
                index,
                &receipt("r"),
                3,
            )
            .unwrap_or_else(|error| panic!("revision {index} accepted: {error}"));
    }
    let overflow = history.publish_revision(
        key(5, 4),
        content(&[4]),
        4,
        &receipt("r"),
        3,
    );
    assert_eq!(
        overflow.err(),
        Some(RegistryError::CapacityExceeded),
        "the aggregate record ceiling is enforced",
    );
    assert_eq!(history.total_records, 3);
    assert_eq!(history.len(), 3, "all three revisions of one identity survive");
}


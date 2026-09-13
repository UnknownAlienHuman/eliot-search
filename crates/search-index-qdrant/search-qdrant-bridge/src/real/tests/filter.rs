use super::*;

#[test]
fn empty_filter_and_inexact_epoch_rejected_before_dispatch() {
    let mut allowed = BTreeSet::new();
    allowed.insert(OpaqueId::new("member").expect("member"));
    let filter = EligibilityFilter {
        access_partition_digest: search_contracts::Blake3Digest32::from_bytes([
            0x01;
            32
        ]),
        allowed_source_memberships: allowed,
        visible_epoch: search_contracts::Epoch::new(42).expect("epoch"),
    };
    assert!(base_filter(&filter).is_ok());
    let mut empty = filter;
    empty.allowed_source_memberships.clear();
    assert_eq!(
        base_filter(&empty).expect_err("empty"),
        BridgeError::InvalidFilter
    );
    // 2^53 + 1 is not exactly representable as f64: the filter must fail,
    // never silently widen.
    assert_eq!(
        epoch_bound(9_007_199_254_740_993).expect_err("inexact"),
        BridgeError::InvalidFilter
    );
    assert!(epoch_bound(42).is_ok());
}

#[test]
fn indexed_field_constants_match_filter_translation() {
    assert_eq!(
        EligibilityFilter::INDEXED_FIELDS,
        [
            "access_partition_digest",
            "source_membership_id",
            "valid_from_epoch",
            "valid_until_epoch_exclusive"
        ]
    );
}

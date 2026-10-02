use super::*;

fn typed_filter() -> EligibilityFilter {
    EligibilityFilter {
        installation_incarnation_id:
            search_contracts::InstallationIncarnationId::from_bytes([0x01; 16]),
        collection_generation_id:
            search_contracts::CollectionGenerationId::from_bytes([0x02; 16]),
        allowed_projection_memberships: BTreeSet::from([
            search_contracts::ProjectionMembershipId::from_bytes([0x03; 16]),
        ]),
        access_partition_id: search_contracts::AccessPartitionId::from_bytes([0x04; 16]),
        scoring_partition_id: search_contracts::ScoringPartitionId::from_bytes([0x05; 16]),
        projection_profile_set_id:
            search_contracts::ProjectionProfileSetId::new("filter-unit-v1")
                .expect("profile set"),
        visible_epoch: search_contracts::Epoch::new(42).expect("epoch"),
    }
}

#[test]
fn empty_filter_and_inexact_epoch_rejected_before_dispatch() {
    let filter = typed_filter();
    assert!(base_filter(&filter).is_ok());
    let mut empty = filter;
    empty.allowed_projection_memberships.clear();
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
fn indexed_field_constants_match_s10_3_filter_translation() {
    assert_eq!(
        EligibilityFilter::INDEXED_FIELDS,
        [
            PointPayload::INSTALLATION_INCARNATION_FIELD,
            PointPayload::COLLECTION_GENERATION_FIELD,
            PointPayload::PROJECTION_MEMBERSHIP_FIELD,
            PointPayload::ACCESS_PARTITION_FIELD,
            PointPayload::SCORING_PARTITION_FIELD,
            PointPayload::PROJECTION_PROFILE_SET_FIELD,
            PointPayload::VALID_FROM_FIELD,
            PointPayload::VALID_UNTIL_FIELD,
        ]
    );
}

use super::support::*;

#[test]
fn public_query_matches_full_sort_with_ties_and_negative_scores() {
    let (mut bridge, route) = bridge(1);
    let points: Vec<_> = (1..=65_u8)
        .map(|byte| point(byte, f32::from(i16::from(byte % 13) - 6)))
        .collect();
    let mut expected: Vec<_> = points
        .iter()
        .map(|point| CandidateNomination {
            point_id: point.point_id,
            score: point.vectors[VECTOR].values[0].1,
            payload_digest: point.payload.payload_digest,
            identity_digest: point.payload.identity_digest,
        })
        .collect();
    expected.sort_by(|left, right| {
        right
            .score
            .partial_cmp(&left.score)
            .expect("fixture scores are finite")
            .then_with(|| left.point_id.cmp(&right.point_id))
    });
    bridge
        .upsert_exact(&route, points, mutation("population", 1))
        .expect("population");
    for limit in [1, 2, 7, 31, 64, 65, 70] {
        let actual = bridge
            .query_filtered(&route, &filter(), VECTOR, &[(0, 1.0)], limit)
            .expect("bounded query");
        assert_eq!(actual, expected[..limit.min(expected.len())]);
    }
    for limit in [0, BridgeLimits::BASELINE.max_query_candidates + 1] {
        assert_eq!(
            bridge.query_filtered(
                &route,
                &filter(),
                VECTOR,
                &[(0, 1.0)],
                limit,
            ),
            Err(BridgeError::QueryBudgetExceeded)
        );
    }
}

#[test]
fn access_and_epoch_exclusions_happen_before_scoring() {
    let (mut bridge, route) = bridge(1);
    // Every excluded point would overflow if scoring happened before filtering.
    let mut partition = point(2, f32::MAX);
    partition.payload.access_partition_digest = digest(99);
    let mut membership = point(3, f32::MAX);
    membership.payload.source_membership_id = opaque("denied");
    let mut future = point(4, f32::MAX);
    future.payload.valid_from_epoch = epoch(43);
    let mut expired = point(5, f32::MAX);
    expired.payload.valid_until_epoch_exclusive = Some(epoch(42));
    let mut active = point(6, 0.5);
    active.payload.valid_until_epoch_exclusive = Some(epoch(43));
    bridge
        .upsert_exact(
            &route,
            vec![
                point(1, 1.0),
                partition,
                membership,
                future,
                expired,
                active,
            ],
            mutation("population", 1),
        )
        .expect("population");
    let actual = bridge
        .query_filtered(&route, &filter(), VECTOR, &[(0, 2.0)], 2)
        .expect("filtered query");
    let ids: Vec<_> = actual
        .iter()
        .map(|candidate| candidate.point_id)
        .collect();
    assert_eq!(ids, vec![id(1), id(6)]);
    assert_eq!(
        bridge
            .count_exact(&route, &filter())
            .expect("exact count")
            .count,
        2
    );
}

#[test]
fn eligible_score_overflow_is_not_hidden_by_a_full_top_k() {
    let (mut bridge, route) = bridge(1);
    bridge
        .upsert_exact(
            &route,
            vec![point(1, 1.0), point(2, f32::MAX)],
            mutation("population", 1),
        )
        .expect("population");
    assert_eq!(
        bridge.query_filtered(&route, &filter(), VECTOR, &[(0, 2.0)], 1),
        Err(BridgeError::InvalidScore)
    );
}

#[test]
fn shared_query_validation_rejects_malformed_vectors() {
    let (bridge, route) = seeded(1);
    let malformed = [
        vec![],
        vec![(0, f32::NAN)],
        vec![(0, f32::INFINITY)],
        vec![(0, f32::NEG_INFINITY)],
        vec![(0, 1.0), (0, 2.0)],
        vec![(1, 1.0), (0, 2.0)],
        vec![(8, 1.0)],
    ];
    for query in malformed {
        assert_eq!(
            bridge.query_filtered(&route, &filter(), VECTOR, &query, 1),
            Err(BridgeError::VectorDimensionMismatch)
        );
    }
}

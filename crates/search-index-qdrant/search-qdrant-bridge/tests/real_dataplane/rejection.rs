use super::support::*;

#[tokio::test]
async fn t24_real_wrong_route_filter_and_bounds_rejected() {
    let outcome = tokio::time::timeout(Duration::from_secs(240), async {
        let (_server, mut plane) = live_plane().await;
        let context = ctx();
        let route = make_route("t24_reject", 0x31);
        plane
            .create_collection(&route, &schema(), &context)
            .await
            .expect("create");

        // Wrong namespace and wrong generation never leak data.
        let filter = permitted_filter();
        assert_wrong_route_rejected(
            &plane,
            &make_route("t24_no_such", 0x31),
            &filter,
            &context,
        )
        .await;
        assert_wrong_route_rejected(
            &plane,
            &make_route("t24_reject", 0x32),
            &filter,
            &context,
        )
        .await;

        // Closed filter language: an empty membership set is invalid before
        // any network use.
        let mut empty = permitted_filter();
        empty.allowed_source_memberships.clear();
        assert_eq!(
            plane
                .count_exact(&route, &empty, &context)
                .await
                .expect_err("empty filter"),
            BridgeError::InvalidFilter
        );

        // Finite pagination floors fail pre-dispatch.
        assert_eq!(
            plane
                .query_filtered(
                    &route,
                    &permitted_filter(),
                    VECTOR_NAME,
                    &[(0, 1.0)],
                    0,
                    IdfScope::ScopedToRetrieval,
                    &context,
                )
                .await
                .expect_err("zero limit"),
            BridgeError::QueryBudgetExceeded
        );
        assert_eq!(
            plane
                .query_filtered(
                    &route,
                    &permitted_filter(),
                    VECTOR_NAME,
                    &[(0, 1.0)],
                    limits().max_query_candidates + 1,
                    IdfScope::ScopedToRetrieval,
                    &context,
                )
                .await
                .expect_err("over-max limit"),
            BridgeError::QueryBudgetExceeded
        );
        assert_eq!(
            plane
                .scroll_exact(&route, &permitted_filter(), None, 0, &context)
                .await
                .expect_err("zero scroll"),
            BridgeError::QueryBudgetExceeded
        );

        // Duplicate IDs fail whole-batch validation and commit nothing.
        let duplicate = vec![
            point(
                11,
                0xA1,
                "t24-member-a",
                10,
                None,
                vec![(0, 1.0)],
            ),
            point(
                11,
                0xA1,
                "t24-member-a",
                10,
                None,
                vec![(0, 1.0)],
            ),
        ];
        assert_eq!(
            plane
                .upsert_exact(
                    &route,
                    duplicate,
                    mutation("t24-reject-dup", 0x41),
                    &context,
                )
                .await
                .expect_err("duplicate batch"),
            BridgeError::DuplicatePointId
        );
        assert_eq!(
            plane
                .count_exact(&route, &permitted_filter(), &context)
                .await
                .expect("count")
                .count,
            0
        );

        assert_eq!(
            plane
                .upsert_exact(
                    &route,
                    oversize_batch(),
                    mutation("t24-reject-oversize", 0x42),
                    &context,
                )
                .await
                .expect_err("oversize batch"),
            BridgeError::MutationTooLarge
        );
    })
    .await;
    outcome.expect("rejection suite finishes before the 240s budget");
}

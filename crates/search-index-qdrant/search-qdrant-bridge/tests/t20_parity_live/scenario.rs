use super::support::*;

#[tokio::test]
async fn t24_live_t20_parity_denied_cannot_move_permitted() {
    let outcome = tokio::time::timeout(Duration::from_secs(240), async {
        let (_server, mut plane) = live_plane().await;
        let context = ctx();
        let route = route();
        plane
            .create_collection(&route, &schema(), &context)
            .await
            .expect("create collection");

        // Distinct term-0 weights avoid relying on Qdrant's unspecified order
        // among equal-score candidates while preserving IDF test power.
        let permitted = vec![
            permitted_point(1, vec![(0, 2.0), (1, 1.0)]),
            permitted_point(2, vec![(0, 1.0)]),
            permitted_point(3, vec![(2, 1.0)]),
        ];
        plane
            .upsert_exact(
                &route,
                permitted,
                mutation("t24-t20-permitted", 0xE1),
                &context,
            )
            .await
            .expect("permitted ingest");

        let permitted_filter = permitted_filter();
        let (
            base_count,
            scoped_t0_before,
            scoped_t1_before,
            global_t0_before,
        ) = permitted_snapshot(&plane, &route, &permitted_filter, &context).await;
        assert_eq!(base_count, 3);
        assert_eq!(scoped_t0_before.len(), 2);
        assert!(scoped_t0_before.iter().all(|hit| hit.score.is_finite()));
        assert_eq!(
            scoped_t0_before
                .iter()
                .map(|hit| hit.point_id)
                .collect::<Vec<_>>(),
            vec![point_id(1), point_id(2)],
            "distinct weights establish deterministic ranking power"
        );

        // Six denied documents share term 0 but differ by both partition and
        // membership. A permitted contract cannot name them.
        let denied: Vec<PointRecord> = (10..=15).map(denied_point).collect();
        plane
            .upsert_exact(
                &route,
                denied,
                mutation("t24-t20-denied", 0xE2),
                &context,
            )
            .await
            .expect("denied ingest");

        let denied_count = plane
            .count_exact(&route, &denied_filter(), &context)
            .await
            .expect("denied count");
        assert_eq!(denied_count.count, 6);

        let (
            after_count,
            scoped_t0_after,
            scoped_t1_after,
            global_t0_after,
        ) = permitted_snapshot(&plane, &route, &permitted_filter, &context).await;
        assert_eq!(
            after_count, base_count,
            "denied docs cannot move permitted counts"
        );
        assert_eq!(
            scoped_t0_after, scoped_t0_before,
            "denied docs cannot move permitted scores/order/IDF"
        );
        assert_eq!(
            scoped_t1_after, scoped_t1_before,
            "denied docs cannot move the rare-term ranking either"
        );
        let denied_ids = denied_ids();
        for hit in scoped_t0_after.iter().chain(&scoped_t1_after) {
            assert!(
                !denied_ids.contains(&hit.point_id),
                "denied identity leaked into permitted nominations: {:?}",
                hit.point_id
            );
        }

        // Without corpus scope, denied population must move global IDF. This
        // proves the fixture has discrimination power and scoped IDF provides
        // the isolation.
        assert_ne!(
            global_t0_after, global_t0_before,
            "global IDF must observe the denied population (test power)"
        );
        assert_ne!(
            global_t0_after, scoped_t0_after,
            "scoped corpus must differ from contaminated global IDF"
        );
    })
    .await;
    outcome.expect("T20 parity suite finishes before the 240s budget");
}

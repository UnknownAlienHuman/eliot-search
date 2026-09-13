use super::support::*;

#[tokio::test]
async fn t24_real_unknown_write_recovery_and_replay() {
    let outcome = tokio::time::timeout(Duration::from_secs(240), async {
        let (_server, mut plane) = live_plane().await;
        let context = ctx();
        let route = make_route("t24_recovery", 0x51);
        plane
            .create_collection(&route, &schema(), &context)
            .await
            .expect("create");

        // Pre-dispatch cancellation is definite and commits nothing.
        let flag = Arc::new(AtomicBool::new(true));
        let cancelled =
            OpContext::with_cancel(Duration::from_secs(20), Arc::clone(&flag));
        assert_eq!(
            plane
                .upsert_exact(
                    &route,
                    vec![point(
                        21,
                        0xA1,
                        "t24-member-a",
                        10,
                        None,
                        vec![(0, 1.0)],
                    )],
                    mutation("t24-recovery-cancel", 0x61),
                    &cancelled,
                )
                .await
                .expect_err("cancelled upsert"),
            BridgeError::Cancelled
        );
        assert_eq!(
            plane
                .count_exact(&route, &permitted_filter(), &context)
                .await
                .expect("count")
                .count,
            0
        );

        // A zero deadline forces an ambiguous timeout. Exact replay with the
        // same identity converges to one effective write.
        let squeezed = OpContext::new(Duration::ZERO);
        let mutation_id = mutation("t24-recovery-unknown", 0x62);
        let batch = vec![point(
            22,
            0xA1,
            "t24-member-a",
            10,
            None,
            vec![(0, 1.0)],
        )];
        assert_eq!(
            plane
                .upsert_exact(
                    &route,
                    batch.clone(),
                    mutation_id.clone(),
                    &squeezed,
                )
                .await
                .expect_err("timeout is unknown"),
            BridgeError::MutationOutcomeUnknown
        );
        let replay = plane
            .upsert_exact(&route, batch, mutation_id, &context)
            .await
            .expect("replay resolves");
        assert!(replay.affected_ids.contains(&point_id(22)));
        assert_eq!(
            plane
                .count_exact(&route, &permitted_filter(), &context)
                .await
                .expect("count")
                .count,
            1
        );

        let again = plane
            .upsert_exact(
                &route,
                vec![point(
                    22,
                    0xA1,
                    "t24-member-a",
                    10,
                    None,
                    vec![(0, 1.0)],
                )],
                mutation("t24-recovery-unknown", 0x62),
                &context,
            )
            .await
            .expect("recorded replay");
        assert!(again.replayed);
        assert_eq!(
            plane
                .count_exact(&route, &permitted_filter(), &context)
                .await
                .expect("count")
                .count,
            1
        );

        // Same operation identity with different input is a conflict.
        assert_eq!(
            plane
                .upsert_exact(
                    &route,
                    vec![point(
                        23,
                        0xA1,
                        "t24-member-a",
                        10,
                        None,
                        vec![(1, 1.0)],
                    )],
                    mutation("t24-recovery-unknown", 0x99),
                    &context,
                )
                .await
                .expect_err("conflict"),
            BridgeError::OperationConflict
        );

        assert_partial_batch_rejected(&mut plane, &route, &context).await;
        assert_explicit_missing(&plane, &route, &context).await;
    })
    .await;
    outcome.expect("recovery suite finishes before the 240s budget");
}

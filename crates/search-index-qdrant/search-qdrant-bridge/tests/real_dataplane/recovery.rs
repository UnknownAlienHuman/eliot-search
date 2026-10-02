use super::support::*;

#[tokio::test]
async fn t24_real_pre_dispatch_deadline_and_replay() {
    let outcome = tokio::time::timeout(Duration::from_secs(240), async {
        let (_server, mut plane) = live_plane().await;
        let context = ctx();
        let route = make_route("t24_recovery", 0x51);
        let filter = permitted_filter(&route);
        plane
            .create_collection(&route, &schema(), &context)
            .await
            .expect("create");

        // Pre-dispatch cancellation is definite and commits nothing.
        let flag = Arc::new(AtomicBool::new(true));
        let cancelled = OpContext::with_cancel(Duration::from_secs(20), Arc::clone(&flag));
        assert_eq!(
            plane
                .upsert_exact(
                    &route,
                    vec![point(
                        &route,
                        21,
                        0xA1,
                        0xB1,
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
                .count_exact(&route, &filter, &context)
                .await
                .expect("count")
                .count,
            0
        );

        // A zero total budget expires before preflight or mutation dispatch,
        // so the result is definite and no point may exist afterward.
        let squeezed = OpContext::new(Duration::ZERO);
        let mutation_id = mutation("t24-recovery-unknown", 0x62);
        let batch = vec![point(
            &route,
            22,
            0xA1,
            0xB1,
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
                .expect_err("zero budget expires before dispatch"),
            BridgeError::DeadlineExceeded
        );
        assert_eq!(
            plane
                .count_exact(&route, &filter, &context)
                .await
                .expect("count after definite timeout")
                .count,
            0
        );

        let first = plane
            .upsert_exact(&route, batch, mutation_id, &context)
            .await
            .expect("first admitted write");
        assert!(!first.replayed);
        assert!(first.affected_ids.contains(&point_id(22)));
        assert_eq!(
            plane
                .count_exact(&route, &filter, &context)
                .await
                .expect("count")
                .count,
            1
        );

        let again = plane
            .upsert_exact(
                &route,
                vec![point(
                    &route,
                    22,
                    0xA1,
                    0xB1,
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
                .count_exact(&route, &filter, &context)
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
                        &route,
                        23,
                        0xA1,
                        0xB1,
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

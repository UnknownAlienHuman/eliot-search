use super::support::*;

#[tokio::test]
async fn t24_real_pagination_cancellation_and_error_redaction() {
    let outcome = tokio::time::timeout(Duration::from_secs(240), async {
        let (_server, mut plane) = live_plane().await;
        let context = ctx();
        let route = make_route("t24_pages", 0x71);
        plane
            .create_collection(&route, &schema(), &context)
            .await
            .expect("create");
        let mut batch = Vec::new();
        for number in 31..=35 {
            batch.push(point(
                number,
                0xA1,
                "t24-member-a",
                10,
                None,
                vec![(0, 1.0)],
            ));
        }
        plane
            .upsert_exact(
                &route,
                batch,
                mutation("t24-pages-upsert", 0x81),
                &context,
            )
            .await
            .expect("upsert five");

        let seen = scroll_all_ids(
            &plane,
            &route,
            &permitted_filter(),
            &context,
            2,
        )
        .await;
        assert_eq!(
            seen,
            vec![
                point_id(31),
                point_id(32),
                point_id(33),
                point_id(34),
                point_id(35),
            ]
        );

        let flag = Arc::new(AtomicBool::new(true));
        flag.store(true, Ordering::SeqCst);
        let cancelled =
            OpContext::with_cancel(Duration::from_secs(20), Arc::clone(&flag));
        assert_reads_cancelled(&plane, &route, &cancelled).await;

        // Typed errors carry stable redacted codes only.
        let forbidden = [
            "http://",
            "127.0.0.1",
            "localhost",
            "Bearer",
            "api-key",
            "t24-member-a",
        ];
        let samples = [
            plane
                .count_exact(
                    &make_route("t24_pages_missing", 0x71),
                    &permitted_filter(),
                    &context,
                )
                .await
                .expect_err("sample"),
            BridgeError::InvalidFilter,
            BridgeError::QueryBudgetExceeded,
            BridgeError::Cancelled,
            BridgeError::MutationOutcomeUnknown,
            BridgeError::TransportFailed,
            BridgeError::MalformedResponse,
        ];
        for sample in samples {
            let rendered = sample.to_string();
            assert_eq!(rendered, sample.code(), "Display is the stable code");
            for needle in forbidden {
                assert!(
                    !rendered.contains(needle),
                    "redacted error {rendered:?} must not contain {needle:?}"
                );
            }
        }
    })
    .await;
    outcome.expect("pagination suite finishes before the 240s budget");
}

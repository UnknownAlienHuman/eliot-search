use super::support::*;

#[tokio::test]
async fn t24_real_crud_query_parity_with_oracle() {
    let outcome = tokio::time::timeout(Duration::from_secs(240), async {
        let (_server, mut plane) = live_plane().await;
        let context = ctx();
        let route = make_route("t24_parity", 0x11);
        let schema = schema();
        let filter = permitted_filter();

        let mut oracle = oracle();
        oracle
            .create_candidate_collection(route.clone(), schema.clone())
            .expect("oracle create");
        plane
            .create_collection(&route, &schema, &context)
            .await
            .expect("real create");
        plane
            .verify_schema(&route, &schema, &context)
            .await
            .expect("real schema verifies");

        let batch = parity_points();
        let create_mutation = mutation("t24-parity-upsert-1", 0x21);
        let oracle_receipt = oracle
            .upsert_exact(&route, batch.clone(), create_mutation.clone())
            .expect("oracle upsert");
        assert!(!oracle_receipt.replayed);
        let real_receipt = plane
            .upsert_exact(&route, batch, create_mutation, &context)
            .await
            .expect("real upsert");
        assert!(!real_receipt.replayed);
        assert_eq!(real_receipt.affected_ids, oracle_receipt.affected_ids);

        let oracle_count =
            oracle.count_exact(&route, &filter).expect("oracle count");
        let real_count = plane
            .count_exact(&route, &filter, &context)
            .await
            .expect("real count");
        assert_eq!(real_count, oracle_count);
        assert_eq!(real_count.count, 4);

        let ids: Vec<QdrantPointId> = (1..=4).map(point_id).collect();
        let oracle_readback = oracle
            .readback_exact(&route, ids.clone())
            .expect("oracle readback");
        let real_readback = plane
            .readback_exact(&route, ids, &context)
            .await
            .expect("real readback");
        assert_readback_parity(&oracle_readback, &real_readback);

        // Single-term queries keep IDF-monotone order: positively matching IDs
        // and ranking equal the oracle TF order. Zero-match oracle documents
        // are server-pruned nominations without signal.
        let oracle_hits = oracle
            .query_filtered(&route, &filter, VECTOR_NAME, &[(0, 1.0)], 10)
            .expect("oracle query");
        let real_hits = plane
            .query_filtered(
                &route,
                &filter,
                VECTOR_NAME,
                &[(0, 1.0)],
                10,
                IdfScope::ScopedToRetrieval,
                &context,
            )
            .await
            .expect("real query");
        let oracle_ids: Vec<QdrantPointId> = oracle_hits
            .iter()
            .filter(|hit| hit.score > 0.0)
            .map(|hit| hit.point_id)
            .collect();
        let real_ids: Vec<QdrantPointId> =
            real_hits.iter().map(|hit| hit.point_id).collect();
        assert_eq!(real_ids, oracle_ids, "single-term ranking parity");
        for hit in &real_hits {
            assert!(hit.score.is_finite(), "finite scores only");
            assert!(
                hit.score > 0.0,
                "server returns positively-matching nominations"
            );
        }

        let scrolled =
            scroll_all_ids(&plane, &route, &filter, &context, 2).await;
        assert_eq!(
            scrolled,
            vec![point_id(1), point_id(2), point_id(3), point_id(4)]
        );

        parity_close_delete_tail(
            &mut plane,
            &mut oracle,
            &route,
            &filter,
            &context,
        )
        .await;
    })
    .await;
    outcome.expect("parity suite finishes before the 240s budget");
}

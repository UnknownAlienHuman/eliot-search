use super::super::*;

/// `close_exact` and `delete_exact` parity tail: both planes narrow the
/// exact-proof denominator identically.
pub(crate) async fn parity_close_delete_tail(
    plane: &mut RealDataPlane,
    oracle: &mut QdrantBridge,
    route: &CollectionRoute,
    filter: &EligibilityFilter,
    context: &OpContext,
) {
    let close_mutation = mutation("t24-parity-close-1", 0x22);
    let close_epoch = Epoch::new(42).expect("close epoch");
    oracle
        .close_exact(
            route,
            vec![point_id(4)],
            close_epoch,
            close_mutation.clone(),
        )
        .expect("oracle close");
    plane
        .close_exact(
            route,
            vec![point_id(4)],
            close_epoch,
            close_mutation,
            context,
        )
        .await
        .expect("real close");
    assert_eq!(
        plane
            .count_exact(route, filter, context)
            .await
            .expect("count")
            .count,
        oracle
            .count_exact(route, filter)
            .expect("oracle count")
            .count
    );

    let delete_mutation = mutation("t24-parity-delete-1", 0x23);
    oracle
        .delete_exact(route, vec![point_id(3)], delete_mutation.clone())
        .expect("oracle delete");
    plane
        .delete_exact(route, vec![point_id(3)], delete_mutation, context)
        .await
        .expect("real delete");
    assert_eq!(
        plane
            .count_exact(route, filter, context)
            .await
            .expect("count")
            .count,
        2
    );
    assert_eq!(
        oracle
            .count_exact(route, filter)
            .expect("oracle count")
            .count,
        2
    );
}

/// A poisoned batch fails whole-batch validation before dispatch and commits
/// nothing.
pub(crate) async fn assert_partial_batch_rejected(
    plane: &mut RealDataPlane,
    route: &CollectionRoute,
    context: &OpContext,
) {
    let partial = vec![
        point(
            24,
            0xA1,
            "t24-member-a",
            10,
            None,
            vec![(0, 1.0)],
        ),
        point(
            25,
            0xA1,
            "t24-member-a",
            10,
            None,
            vec![(1, 1.0), (0, 1.0)],
        ),
    ];
    assert_eq!(
        plane
            .upsert_exact(
                route,
                partial,
                mutation("t24-recovery-partial", 0x63),
                context,
            )
            .await
            .expect_err("partial batch"),
        BridgeError::VectorDimensionMismatch
    );
    let missing = plane
        .readback_exact(route, vec![point_id(24), point_id(25)], context)
        .await
        .expect("readback");
    assert!(missing.points.is_empty());
    assert_eq!(missing.missing_ids.len(), 2);
}

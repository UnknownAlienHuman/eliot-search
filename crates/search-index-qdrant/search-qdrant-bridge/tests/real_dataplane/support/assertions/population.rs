use super::super::*;

/// Walks the whole eligible set in bounded pages and returns sorted IDs.
pub(crate) async fn scroll_all_ids(
    plane: &RealDataPlane,
    route: &CollectionRoute,
    filter: &EligibilityFilter,
    context: &OpContext,
    page_size: usize,
) -> Vec<QdrantPointId> {
    let mut seen = Vec::new();
    let mut offset: Option<QdrantPointId> = None;
    for _ in 0..32 {
        let page = plane
            .scroll_exact(route, filter, offset, page_size, context)
            .await
            .expect("bounded page");
        assert!(page.points.len() <= page_size, "bounded page");
        if page.points.is_empty() {
            break;
        }
        seen.extend(page.points.iter().map(|point| point.point_id));
        offset = page.next_offset;
        if offset.is_none() {
            break;
        }
    }
    seen.sort();
    seen
}

/// A wrong namespace or generation never leaks data on any read path.
pub(crate) async fn assert_wrong_route_rejected(
    plane: &RealDataPlane,
    wrong: &CollectionRoute,
    filter: &EligibilityFilter,
    context: &OpContext,
) {
    assert_eq!(
        plane
            .count_exact(wrong, filter, context)
            .await
            .expect_err("wrong route count"),
        BridgeError::CollectionNotFound
    );
    assert_eq!(
        plane
            .readback_exact(wrong, vec![point_id(1)], context)
            .await
            .expect_err("wrong route readback"),
        BridgeError::CollectionNotFound
    );
    assert_eq!(
        plane
            .query_filtered(
                wrong,
                filter,
                VECTOR_NAME,
                &[(0, 1.0)],
                10,
                IdfScope::ScopedToRetrieval,
                context,
            )
            .await
            .expect_err("wrong route query"),
        BridgeError::CollectionNotFound
    );
    assert_eq!(
        plane
            .scroll_exact(wrong, filter, None, 10, context)
            .await
            .expect_err("wrong route scroll"),
        BridgeError::CollectionNotFound
    );
}

/// Every read path checks cancellation before dispatch.
pub(crate) async fn assert_reads_cancelled(
    plane: &RealDataPlane,
    route: &CollectionRoute,
    cancelled: &OpContext,
) {
    assert_eq!(
        plane
            .count_exact(route, &permitted_filter(), cancelled)
            .await
            .expect_err("cancelled count"),
        BridgeError::Cancelled
    );
    assert_eq!(
        plane
            .query_filtered(
                route,
                &permitted_filter(),
                VECTOR_NAME,
                &[(0, 1.0)],
                10,
                IdfScope::ScopedToRetrieval,
                cancelled,
            )
            .await
            .expect_err("cancelled query"),
        BridgeError::Cancelled
    );
    assert_eq!(
        plane
            .scroll_exact(route, &permitted_filter(), None, 2, cancelled)
            .await
            .expect_err("cancelled scroll"),
        BridgeError::Cancelled
    );
    assert_eq!(
        plane
            .readback_exact(route, vec![point_id(31)], cancelled)
            .await
            .expect_err("cancelled readback"),
        BridgeError::Cancelled
    );
}

/// Unknown point IDs surface as explicit missing entries, never errors.
pub(crate) async fn assert_explicit_missing(
    plane: &RealDataPlane,
    route: &CollectionRoute,
    context: &OpContext,
) {
    let unknown = plane
        .readback_exact(route, vec![point_id(22), point_id(77)], context)
        .await
        .expect("readback with unknown");
    assert_eq!(unknown.points.len(), 1);
    assert_eq!(unknown.missing_ids, vec![point_id(77)]);
    assert!(unknown.unexpected_ids.is_empty());
}

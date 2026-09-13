use super::*;

/// Permitted-contract snapshot: exact count plus one global and two scoped
/// queries, exercising retrieval and IDF filters separately and jointly.
pub(crate) async fn permitted_snapshot(
    plane: &RealDataPlane,
    route: &CollectionRoute,
    filter: &EligibilityFilter,
    context: &OpContext,
) -> (
    usize,
    Vec<CandidateNomination>,
    Vec<CandidateNomination>,
    Vec<CandidateNomination>,
) {
    let count = plane
        .count_exact(route, filter, context)
        .await
        .expect("exact count")
        .count;
    let scoped_t0 = plane
        .query_filtered(
            route,
            filter,
            VECTOR_NAME,
            &[(0, 1.0)],
            10,
            IdfScope::ScopedToRetrieval,
            context,
        )
        .await
        .expect("scoped t0");
    let scoped_t1 = plane
        .query_filtered(
            route,
            filter,
            VECTOR_NAME,
            &[(1, 1.0)],
            10,
            IdfScope::ScopedToRetrieval,
            context,
        )
        .await
        .expect("scoped t1");
    let global_t0 = plane
        .query_filtered(
            route,
            filter,
            VECTOR_NAME,
            &[(0, 1.0)],
            10,
            IdfScope::Global,
            context,
        )
        .await
        .expect("global t0");
    (count, scoped_t0, scoped_t1, global_t0)
}

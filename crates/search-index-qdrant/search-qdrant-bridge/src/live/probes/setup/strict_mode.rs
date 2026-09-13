//! Strict-mode negative probes for unindexed retrieval and update filters.

use qdrant_client::qdrant::{
    DeletePoints, Filter, PointsSelector, Query, QueryPoints, VectorInput,
    points_selector,
};

use super::super::super::fixtures::{
    FIELD_UNINDEXED, QUALIFICATION_COLLECTION, VECTOR_CODE,
    keyword_condition, strong_ordering,
};
use super::super::super::suite::Suite;
use super::super::super::LiveError;

pub(super) async fn probe_strict_negatives(
    suite: &mut Suite,
) -> Result<(), LiveError> {
    let unindexed = Filter {
        must: vec![keyword_condition(FIELD_UNINDEXED, "code_unit")],
        ..Default::default()
    };
    let retrieve_rejected = suite
        .client
        .query(QueryPoints {
            collection_name: QUALIFICATION_COLLECTION.to_owned(),
            query: Some(Query::new_nearest(VectorInput::new_sparse(
                vec![0_u32],
                vec![1.0_f32],
            ))),
            using: Some(VECTOR_CODE.to_owned()),
            filter: Some(unindexed.clone()),
            limit: Some(10),
            ..Default::default()
        })
        .await
        .is_err();
    suite.record(
        "strict_unindexed_retrieve_rejected",
        retrieve_rejected,
        "filter on never-indexed unit_kind".to_owned(),
    );
    let update_rejected = suite
        .client
        .delete_points(DeletePoints {
            collection_name: QUALIFICATION_COLLECTION.to_owned(),
            wait: Some(true),
            points: Some(PointsSelector {
                points_selector_one_of: Some(
                    points_selector::PointsSelectorOneOf::Filter(unindexed),
                ),
            }),
            ordering: Some(strong_ordering()),
            ..Default::default()
        })
        .await
        .is_err();
    suite.record(
        "strict_unindexed_update_rejected",
        update_rejected,
        "delete-by-filter on never-indexed unit_kind".to_owned(),
    );
    Ok(())
}

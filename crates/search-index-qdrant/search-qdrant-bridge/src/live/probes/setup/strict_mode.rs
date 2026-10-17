//! Strict-mode negative probes for unindexed retrieval and update filters.

use crate::BridgeError;
use qdrant_client::qdrant::{
    DeletePoints, Filter, PointsSelector, Query, QueryPoints, VectorInput, points_selector,
};

use super::super::super::LiveError;
use super::super::super::fixtures::{
    FIELD_UNINDEXED, VECTOR_CODE, keyword_condition, strong_ordering,
};
use super::super::super::suite::Suite;

const fn expected_unindexed_rejection<T>(result: &Result<T, BridgeError>) -> bool {
    matches!(result, Err(BridgeError::UnindexedFilter))
}
pub(in crate::live) async fn probe_strict_negatives(
    suite: &mut Suite<'_>,
) -> Result<(), LiveError> {
    let unindexed = Filter {
        must: vec![keyword_condition(FIELD_UNINDEXED, "code_unit")],
        ..Default::default()
    };
    let retrieve_result = suite
        .client
        .query(QueryPoints {
            collection_name: suite.client.collection_name().to_owned(),
            query: Some(Query::new_nearest(VectorInput::new_sparse(
                vec![0_u32],
                vec![1.0_f32],
            ))),
            using: Some(VECTOR_CODE.to_owned()),
            filter: Some(unindexed.clone()),
            limit: Some(10),
            ..Default::default()
        })
        .await;
    let retrieve_rejected = expected_unindexed_rejection(&retrieve_result);
    suite.record(
        "strict_unindexed_retrieve_rejected",
        retrieve_rejected,
        "filter on never-indexed unit_kind".to_owned(),
    );
    let update_result = suite
        .client
        .delete_points(DeletePoints {
            collection_name: suite.client.collection_name().to_owned(),
            wait: Some(true),
            points: Some(PointsSelector {
                points_selector_one_of: Some(points_selector::PointsSelectorOneOf::Filter(
                    unindexed,
                )),
            }),
            ordering: Some(strong_ordering()),
            ..Default::default()
        })
        .await;
    let update_rejected = expected_unindexed_rejection(&update_result);
    suite.record(
        "strict_unindexed_update_rejected",
        update_rejected,
        "delete-by-filter on never-indexed unit_kind".to_owned(),
    );
    Ok(())
}
#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn only_the_typed_unindexed_rejection_counts_as_strict_enforcement() {
        assert!(expected_unindexed_rejection::<()>(&Err(
            BridgeError::UnindexedFilter
        )));
        for error in [
            BridgeError::AuthenticationInvalid,
            BridgeError::TransportFailed,
            BridgeError::MutationOutcomeUnknown,
        ] {
            assert!(!expected_unindexed_rejection::<()>(&Err(error)));
        }
        assert!(!expected_unindexed_rejection(&Ok::<(), BridgeError>(())));
    }
}

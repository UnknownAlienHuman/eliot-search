//! Exact tenant-A sparse query capture and order-independent score comparison.

use qdrant_client::qdrant::{
    IdfParams, Query, QueryPoints, SearchParams, VectorInput,
};

use super::super::super::fixtures::{
    QUALIFICATION_COLLECTION, VECTOR_CODE, base_filter, snapshot_id,
};
use super::super::super::suite::Suite;
use super::super::super::LiveError;

#[derive(Clone, Debug, PartialEq)]
pub(super) struct ScoredSnapshot {
    pub(super) id: String,
    pub(super) score: f32,
}

pub(super) async fn query_tenant_a(
    suite: &Suite,
    term: u32,
    with_idf_corpus: bool,
) -> Result<Vec<ScoredSnapshot>, LiveError> {
    let params = SearchParams {
        exact: Some(true),
        idf: with_idf_corpus
            .then(base_filter)
            .transpose()?
            .map(|corpus| IdfParams {
                corpus: Some(corpus),
            }),
        ..Default::default()
    };
    let response = suite
        .client
        .query(QueryPoints {
            collection_name: QUALIFICATION_COLLECTION.to_owned(),
            query: Some(Query::new_nearest(VectorInput::new_sparse(
                vec![term],
                vec![1.0_f32],
            ))),
            using: Some(VECTOR_CODE.to_owned()),
            filter: Some(base_filter()?),
            params: Some(params),
            limit: Some(10),
            ..Default::default()
        })
        .await
        .map_err(|_| LiveError::TransportFailed)?;
    Ok(response
        .result
        .into_iter()
        .map(|scored| ScoredSnapshot {
            id: snapshot_id(scored.id.as_ref()),
            score: scored.score,
        })
        .collect())
}

/// Compares the exact ID/score population without depending on Qdrant's
/// unspecified ordering among equal-score candidates.
pub(super) fn same_scores(
    left: &[ScoredSnapshot],
    right: &[ScoredSnapshot],
) -> bool {
    if left.len() != right.len() {
        return false;
    }
    let mut left = left.to_vec();
    let mut right = right.to_vec();
    left.sort_unstable_by(|a, b| a.id.cmp(&b.id));
    right.sort_unstable_by(|a, b| a.id.cmp(&b.id));
    left == right
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn score_population_comparison_ignores_tie_order_only() {
        let first = vec![
            ScoredSnapshot {
                id: "b".to_owned(),
                score: 1.0,
            },
            ScoredSnapshot {
                id: "a".to_owned(),
                score: 1.0,
            },
        ];
        let reversed = vec![first[1].clone(), first[0].clone()];
        assert!(same_scores(&first, &reversed));

        let mut changed = reversed;
        changed[0].score = 2.0;
        assert!(!same_scores(&first, &changed));
    }
}

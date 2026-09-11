use qdrant_client::qdrant::{
    CountPoints, Filter, IdfParams, PointStruct, Query, QueryPoints,
    SearchParams, UpsertPoints, VectorInput,
};

use crate::qualified::{IndependentIdfProfile, admit_independent_idf};

use super::super::fixtures::{
    FIELD_TENANT, FIELD_UNTIL, QUALIFICATION_COLLECTION, TENANT_A, TENANT_B,
    UUID_POINT, VECTOR_CODE, VISIBLE_EPOCH_I64, base_eligibility, base_filter,
    exact_f64, keyword_condition, point, range_condition, snapshot_id,
    strong_ordering, update_completed,
};
use super::super::suite::Suite;
use super::super::LiveError;

#[derive(Clone, Debug, PartialEq)]
struct ScoredSnapshot {
    id: String,
    score: f32,
}

async fn query_tenant_a(
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

pub(super) async fn probe_independent_idf(
    suite: &mut Suite,
) -> Result<(), LiveError> {
    let plan = base_eligibility();
    let gate_ok = admit_independent_idf(&IndependentIdfProfile {
        vector_name: VECTOR_CODE.to_owned(),
        local_idf_factor: 1.0,
        local_statistics_present: false,
        qdrant_modifier_idf: true,
        retrieval_eligibility: plan.clone(),
        idf_corpus_eligibility: plan,
    })
    .is_ok();
    let global_before = query_tenant_a(suite, 0, false).await?;
    let scoped_before = query_tenant_a(suite, 0, true).await?;
    let same_population = global_before == scoped_before;
    suite.log.push(format!(
        "IDF pre-insert global={global_before:?} scoped={scoped_before:?} gate={gate_ok}"
    ));

    let forbidden: Vec<PointStruct> = (3..=8)
        .map(|id| {
            point(
                id,
                TENANT_B,
                10,
                None,
                vec![(0, 1.0)],
                vec![(100, 1.0)],
            )
        })
        .collect();
    let inserted = suite
        .client
        .upsert_points(UpsertPoints {
            collection_name: QUALIFICATION_COLLECTION.to_owned(),
            wait: Some(true),
            ordering: Some(strong_ordering()),
            points: forbidden,
            ..Default::default()
        })
        .await
        .map_err(|_| LiveError::TransportFailed)?;
    if !inserted
        .result
        .as_ref()
        .is_some_and(|result| update_completed(result.status))
    {
        suite.record(
            "independent_idf_population_filter",
            false,
            "forbidden-population insert not acknowledged".to_owned(),
        );
        return Ok(());
    }
    let global_after = query_tenant_a(suite, 0, false).await?;
    let scoped_after = query_tenant_a(suite, 0, true).await?;
    let noninterference = scoped_before == scoped_after
        && scoped_after
            .iter()
            .all(|snapshot| snapshot.score.is_finite());
    let discrimination =
        global_before != global_after && global_after != scoped_after;
    suite.record(
        "independent_idf_population_filter",
        gate_ok && same_population && noninterference && discrimination,
        format!(
            "scoped_stable={noninterference} global_moved={discrimination} \
             global_after={global_after:?} scoped_after={scoped_after:?}"
        ),
    );
    Ok(())
}

pub(super) async fn probe_sparse_modifier(
    suite: &mut Suite,
) -> Result<(), LiveError> {
    let rare = query_tenant_a(suite, 1, true).await?;
    let common = query_tenant_a(suite, 0, true).await?;
    let rare_top = rare.first().map(|snapshot| snapshot.id.clone());
    let rare_score = rare
        .iter()
        .find(|snapshot| snapshot.id == "1")
        .map(|snapshot| snapshot.score);
    let common_score = common
        .iter()
        .find(|snapshot| snapshot.id == "1")
        .map(|snapshot| snapshot.score);
    let rarer_scores_higher = match (rare_score, common_score) {
        (Some(rare_score), Some(common_score)) => {
            rare_score.is_finite()
                && common_score.is_finite()
                && rare_score > common_score
        }
        _ => false,
    };
    let passed = matches!(rare_top.as_deref(), Some("1" | UUID_POINT))
        && rarer_scores_higher;
    suite.record(
        "sparse_idf_modifier",
        passed,
        format!(
            "t1 top={rare_top:?} score_p1(t1)={rare_score:?} score_p1(t0)={common_score:?}"
        ),
    );
    Ok(())
}

pub(super) async fn probe_missing_upper_bound(
    suite: &mut Suite,
) -> Result<(), LiveError> {
    let open = suite
        .client
        .count(CountPoints {
            collection_name: QUALIFICATION_COLLECTION.to_owned(),
            filter: Some(base_filter()?),
            exact: Some(true),
            ..Default::default()
        })
        .await
        .map_err(|_| LiveError::TransportFailed)?;
    let closed = suite
        .client
        .count(CountPoints {
            collection_name: QUALIFICATION_COLLECTION.to_owned(),
            filter: Some(Filter {
                must: vec![
                    keyword_condition(FIELD_TENANT, TENANT_A),
                    range_condition(
                        FIELD_UNTIL,
                        None,
                        Some(exact_f64(VISIBLE_EPOCH_I64)?),
                    ),
                ],
                ..Default::default()
            }),
            exact: Some(true),
            ..Default::default()
        })
        .await
        .map_err(|_| LiveError::TransportFailed)?;
    let open_count = open.result.as_ref().map(|result| result.count);
    let closed_count = closed.result.as_ref().map(|result| result.count);
    suite.record(
        "missing_valid_until_open_end",
        open_count == Some(4) && closed_count == Some(0),
        format!(
            "must_not(until<=42)={open_count:?} must(until<=42)={closed_count:?}"
        ),
    );
    Ok(())
}

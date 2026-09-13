//! Independent IDF-corpus noninterference qualification probe.

use qdrant_client::qdrant::{PointStruct, UpsertPoints};

use crate::qualified::{IndependentIdfProfile, admit_independent_idf};

use super::query::{query_tenant_a, same_scores};
use super::super::super::fixtures::{
    QUALIFICATION_COLLECTION, TENANT_B, VECTOR_CODE, base_eligibility, point,
    strong_ordering, update_completed,
};
use super::super::super::suite::Suite;
use super::super::super::LiveError;

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
    let same_population = same_scores(&global_before, &scoped_before);
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
    let noninterference = same_scores(&scoped_before, &scoped_after)
        && scoped_after
            .iter()
            .all(|snapshot| snapshot.score.is_finite());
    let discrimination = !same_scores(&global_before, &global_after)
        && !same_scores(&global_after, &scoped_after);
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

//! Sparse-IDF modifier ranking probe.

use super::query::query_tenant_a;
use super::super::super::fixtures::UUID_POINT;
use super::super::super::suite::Suite;
use super::super::super::LiveError;

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

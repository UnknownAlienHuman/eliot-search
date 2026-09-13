//! Signed integer range-transport qualification probe.

use qdrant_client::qdrant::{CountPoints, Filter};

use super::super::super::fixtures::{
    EPOCH_MAX, EPOCH_MIN, FIELD_FROM, FIELD_UNTIL, QUALIFICATION_COLLECTION,
    exact_f64, range_condition,
};
use super::super::super::suite::Suite;
use super::super::super::LiveError;

pub(super) async fn probe_signed_range(
    suite: &mut Suite,
) -> Result<(), LiveError> {
    let lower = suite
        .client
        .count(CountPoints {
            collection_name: QUALIFICATION_COLLECTION.to_owned(),
            filter: Some(Filter {
                must: vec![range_condition(
                    FIELD_FROM,
                    None,
                    Some(exact_f64(EPOCH_MIN)?),
                )],
                ..Default::default()
            }),
            exact: Some(true),
            ..Default::default()
        })
        .await
        .map_err(|_| LiveError::TransportFailed)?;
    let upper = suite
        .client
        .count(CountPoints {
            collection_name: QUALIFICATION_COLLECTION.to_owned(),
            filter: Some(Filter {
                must: vec![range_condition(
                    FIELD_UNTIL,
                    Some(exact_f64(EPOCH_MAX)?),
                    None,
                )],
                ..Default::default()
            }),
            exact: Some(true),
            ..Default::default()
        })
        .await
        .map_err(|_| LiveError::TransportFailed)?;
    let lower_count = lower.result.as_ref().map(|result| result.count);
    let upper_count = upper.result.as_ref().map(|result| result.count);
    suite.record(
        "signed_i64_epoch_range",
        lower_count == Some(1) && upper_count == Some(1),
        format!(
            "from<=MIN count={lower_count:?} until>=MAX count={upper_count:?}"
        ),
    );
    Ok(())
}

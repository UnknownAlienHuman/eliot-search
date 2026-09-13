//! Missing valid-until field open-end semantics probe.

use qdrant_client::qdrant::{CountPoints, Filter};

use super::super::super::fixtures::{
    FIELD_TENANT, FIELD_UNTIL, QUALIFICATION_COLLECTION, TENANT_A,
    VISIBLE_EPOCH_I64, base_filter, exact_f64, keyword_condition,
    range_condition,
};
use super::super::super::suite::Suite;
use super::super::super::LiveError;

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

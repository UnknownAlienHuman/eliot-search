//! Canonical retrieval and IDF eligibility fixture filters.

use qdrant_client::qdrant::{
    Condition, FieldCondition, Filter, Match, Range, condition, r#match,
};

use crate::qualified::BaseEligibility;

use super::super::LiveError;
use super::spec::{
    ACCESS_A, FIELD_ACCESS, FIELD_FROM, FIELD_TENANT, FIELD_UNTIL, TENANT_A,
    VISIBLE_EPOCH, VISIBLE_EPOCH_I64,
};

pub(super) fn base_eligibility() -> BaseEligibility {
    BaseEligibility {
        access_partition: ACCESS_A.to_owned(),
        tenant: TENANT_A.to_owned(),
        visible_epoch: VISIBLE_EPOCH,
    }
}

pub(super) const fn exact_f64(value: i64) -> Result<f64, LiveError> {
    #[allow(clippy::cast_precision_loss)]
    let as_float = value as f64;
    #[allow(clippy::cast_possible_truncation)]
    if as_float as i64 != value {
        return Err(LiveError::FixtureNotRepresentable);
    }
    Ok(as_float)
}

pub(super) fn keyword_condition(key: &str, value: &str) -> Condition {
    Condition {
        condition_one_of: Some(condition::ConditionOneOf::Field(FieldCondition {
            key: key.to_owned(),
            r#match: Some(Match {
                match_value: Some(r#match::MatchValue::Keyword(value.to_owned())),
            }),
            ..Default::default()
        })),
    }
}

pub(super) fn range_condition(
    key: &str,
    gte: Option<f64>,
    lte: Option<f64>,
) -> Condition {
    Condition {
        condition_one_of: Some(condition::ConditionOneOf::Field(FieldCondition {
            key: key.to_owned(),
            range: Some(Range {
                gte,
                lte,
                ..Default::default()
            }),
            ..Default::default()
        })),
    }
}

/// Canonical base eligibility plan, shared verbatim by retrieval and the IDF
/// corpus: tenant A, access partition A, `valid_from <= 42`, with the
/// open-ended upper bound expressed as `must_not(valid_until <= 42)`.
pub(super) fn base_filter() -> Result<Filter, LiveError> {
    Ok(Filter {
        must: vec![
            keyword_condition(FIELD_TENANT, TENANT_A),
            keyword_condition(FIELD_ACCESS, ACCESS_A),
            range_condition(
                FIELD_FROM,
                None,
                Some(exact_f64(VISIBLE_EPOCH_I64)?),
            ),
        ],
        must_not: vec![range_condition(
            FIELD_UNTIL,
            None,
            Some(exact_f64(VISIBLE_EPOCH_I64)?),
        )],
        ..Default::default()
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn signed_epoch_fixture_is_exactly_representable() {
        assert_eq!(exact_f64(VISIBLE_EPOCH_I64), Ok(42.0));
        assert_eq!(exact_f64(9_007_199_254_740_001), Ok(9_007_199_254_740_001.0));
        assert_eq!(
            exact_f64(9_007_199_254_740_993),
            Err(LiveError::FixtureNotRepresentable)
        );
    }
}

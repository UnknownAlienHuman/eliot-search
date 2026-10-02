use crate::query::validate_filter;

pub(super) fn keyword_condition(key: &str, text: String) -> Condition {
    Condition {
        condition_one_of: Some(condition::ConditionOneOf::Field(FieldCondition {
            key: key.to_owned(),
            r#match: Some(Match {
                match_value: Some(r#match::MatchValue::Keyword(text)),
            }),
            ..Default::default()
        })),
    }
}

pub(super) fn keywords_condition(key: &str, texts: Vec<String>) -> Condition {
    Condition {
        condition_one_of: Some(condition::ConditionOneOf::Field(FieldCondition {
            key: key.to_owned(),
            r#match: Some(Match {
                match_value: Some(r#match::MatchValue::Keywords(RepeatedStrings {
                    strings: texts,
                })),
            }),
            ..Default::default()
        })),
    }
}

pub(super) fn range_condition(key: &str, gte: Option<f64>, lte: Option<f64>) -> Condition {
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

/// Range-bound epoch as an exactly representable `f64`.
///
/// Qdrant `Range` bounds travel as doubles; only exactly representable
/// integers are admitted so a large epoch can never silently truncate into a
/// wider filter.
pub(super) const fn epoch_bound(number: i64) -> Result<f64, BridgeError> {
    #[allow(clippy::cast_precision_loss)]
    let as_float = number as f64;
    #[allow(clippy::cast_possible_truncation)]
    if as_float as i64 != number {
        return Err(BridgeError::InvalidFilter);
    }
    Ok(as_float)
}

/// Canonical S10.3 base eligibility plan: the one closed filter value shared
/// verbatim by retrieval and the IDF corpus.
pub(super) fn base_filter(filter: &EligibilityFilter) -> Result<Filter, BridgeError> {
    validate_filter(filter)?;
    let projection_memberships = filter
        .allowed_projection_memberships
        .iter()
        .map(ToString::to_string)
        .collect();
    let visible = epoch_bound(filter.visible_epoch.get())?;
    Ok(Filter {
        must: vec![
            keyword_condition(
                PointPayload::INSTALLATION_INCARNATION_FIELD,
                filter.installation_incarnation_id.to_string(),
            ),
            keyword_condition(
                PointPayload::COLLECTION_GENERATION_FIELD,
                filter.collection_generation_id.to_string(),
            ),
            keywords_condition(
                PointPayload::PROJECTION_MEMBERSHIP_FIELD,
                projection_memberships,
            ),
            keyword_condition(
                PointPayload::ACCESS_PARTITION_FIELD,
                filter.access_partition_id.to_string(),
            ),
            keyword_condition(
                PointPayload::SCORING_PARTITION_FIELD,
                filter.scoring_partition_id.to_string(),
            ),
            keyword_condition(
                PointPayload::PROJECTION_PROFILE_SET_FIELD,
                filter.projection_profile_set_id.as_str().to_owned(),
            ),
            range_condition(PointPayload::VALID_FROM_FIELD, None, Some(visible)),
        ],
        must_not: vec![range_condition(
            PointPayload::VALID_UNTIL_FIELD,
            None,
            Some(visible),
        )],
        ..Default::default()
    })
}

pub(super) fn validate_point(
    point: &PointRecord,
    schema: &CollectionSchema,
    limits: BridgeLimits,
) -> Result<(), BridgeError> {
    crate::mutation::validate_point(point, schema, limits)?;
    // Vendor payload encoding has additional representability constraints.
    encode_payload(point)?;
    Ok(())
}

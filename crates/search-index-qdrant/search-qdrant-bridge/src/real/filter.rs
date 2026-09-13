use crate::mutation::validate_exact_ids;
use crate::query::validate_query_vector;

fn keyword_condition(key: &str, text: String) -> Condition {
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

fn keywords_condition(key: &str, texts: Vec<String>) -> Condition {
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

fn range_condition(key: &str, gte: Option<f64>, lte: Option<f64>) -> Condition {
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
const fn epoch_bound(number: i64) -> Result<f64, BridgeError> {
    #[allow(clippy::cast_precision_loss)]
    let as_float = number as f64;
    #[allow(clippy::cast_possible_truncation)]
    if as_float as i64 != number {
        return Err(BridgeError::InvalidFilter);
    }
    Ok(as_float)
}

/// Canonical base eligibility plan: the one closed filter value shared
/// verbatim by retrieval and the IDF corpus.
fn base_filter(filter: &EligibilityFilter) -> Result<Filter, BridgeError> {
    if filter.allowed_source_memberships.is_empty() {
        return Err(BridgeError::InvalidFilter);
    }
    let members: Vec<String> = filter
        .allowed_source_memberships
        .iter()
        .map(|member| member.as_str().to_owned())
        .collect();
    let from = epoch_bound(filter.visible_epoch.get())?;
    let until = epoch_bound(filter.visible_epoch.get())?;
    Ok(Filter {
        must: vec![
            keyword_condition(
                EligibilityFilter::INDEXED_FIELDS[0],
                hex_from_32(filter.access_partition_digest.as_bytes()),
            ),
            keywords_condition(EligibilityFilter::INDEXED_FIELDS[1], members),
            range_condition(EligibilityFilter::INDEXED_FIELDS[2], None, Some(from)),
        ],
        must_not: vec![range_condition(
            EligibilityFilter::INDEXED_FIELDS[3],
            None,
            Some(until),
        )],
        ..Default::default()
    })
}

fn validate_point(
    point: &PointRecord,
    schema: &CollectionSchema,
    limits: BridgeLimits,
) -> Result<(), BridgeError> {
    crate::mutation::validate_point(point, schema, limits)?;
    // Vendor payload encoding has additional representability constraints.
    encode_payload(point)?;
    Ok(())
}

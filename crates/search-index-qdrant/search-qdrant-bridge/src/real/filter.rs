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
    if point.vectors.len() != schema.named_vectors.len() {
        return Err(BridgeError::NamedVectorMissing);
    }
    let mut stored_values = 0_usize;
    for (name, vector_schema) in &schema.named_vectors {
        let vector = point
            .vectors
            .get(name)
            .ok_or(BridgeError::NamedVectorMissing)?;
        if vector.dimensions != vector_schema.dimensions || vector.sparse != vector_schema.sparse {
            return Err(BridgeError::VectorDimensionMismatch);
        }
        if vector.values.is_empty()
            || vector.values.iter().any(|(_, score)| !score.is_finite())
            || vector.values.windows(2).any(|pair| pair[0].0 >= pair[1].0)
            || vector
                .values
                .last()
                .is_some_and(|(index, _)| *index >= vector_schema.dimensions)
        {
            return Err(BridgeError::VectorDimensionMismatch);
        }
        stored_values = stored_values
            .checked_add(vector.values.len())
            .ok_or(BridgeError::MutationTooLarge)?;
    }
    if stored_values > limits.max_vector_values_per_point {
        return Err(BridgeError::MutationTooLarge);
    }
    encode_payload(point)?;
    Ok(())
}

fn validate_exact_ids(
    ids: Vec<QdrantPointId>,
    limit: usize,
) -> Result<Vec<QdrantPointId>, BridgeError> {
    if ids.is_empty() || ids.len() > limit {
        return Err(BridgeError::MutationTooLarge);
    }
    let mut sorted = ids;
    sorted.sort();
    if sorted.windows(2).any(|pair| pair[0] == pair[1]) {
        return Err(BridgeError::DuplicatePointId);
    }
    Ok(sorted)
}

fn validate_query_vector(query: &[(u32, f32)], dimensions: u32) -> Result<(), BridgeError> {
    if query.is_empty()
        || query.iter().any(|(_, score)| !score.is_finite())
        || query.windows(2).any(|pair| pair[0].0 >= pair[1].0)
        || query.last().is_some_and(|(index, _)| *index >= dimensions)
    {
        return Err(BridgeError::VectorDimensionMismatch);
    }
    Ok(())
}

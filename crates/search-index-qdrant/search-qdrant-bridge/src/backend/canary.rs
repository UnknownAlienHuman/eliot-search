//! Saved-point-scoped readiness, independent of optional user queries.

use std::sync::atomic::AtomicBool;
use std::time::Instant;

use qdrant_client::qdrant::{
    Condition, Filter, IdfParams, Query, QueryPoints, ScoredPoint, SearchParams, TextQueryBuilder,
};

use super::{Backend, check};
use crate::codec::{decode_id, point_id, source_id};
use crate::model::collection_name;
use crate::{BackendError, MAX_POINT_TEXT_BYTES, PointInput, schema};

impl Backend {
    /// Proves scoring/index wiring for one exact real saved point and source word.
    ///
    /// The caller deterministically selects a complete bounded ASCII-alphabetic
    /// identifier from captured unit text, independently of the optional user query.
    /// The real text query has a singleton point-ID candidate filter, limit one,
    /// whole-generation IDF and routing-only/no-vector response selection.
    ///
    /// Success requires exactly the expected ID/source and a finite positive
    /// server score. Empty/nonpositive results do not establish a tokenless corpus.
    /// True zero-point generation acceptance belongs to the complete inventory/
    /// evidence/schema/exact-count owner and does not call this method.
    ///
    /// # Errors
    ///
    /// Returns input/budget/source-word failures, actual read/execution errors,
    /// malformed/misrouted responses or `ReadinessNotProven`. No failure permits
    /// publication acceptance or substitution of an arbitrary user query.
    pub fn verify_canary(
        &self,
        name: &str,
        expected: PointInput<'_>,
        query: &str,
        deadline: Instant,
        cancelled: &AtomicBool,
    ) -> Result<(), BackendError> {
        check(deadline, cancelled)?;
        collection_name(name)?;
        if expected.retrieval_text.is_empty()
            || expected.retrieval_text.len() > MAX_POINT_TEXT_BYTES
            || query.is_empty()
            || !query.bytes().all(|byte| byte.is_ascii_alphabetic())
        {
            return Err(BackendError::InvalidInput);
        }
        if query.len() > self.limits.max_query_bytes {
            return Err(BackendError::RequestTooLarge);
        }
        if !saved_word(expected.retrieval_text, query, deadline, cancelled)? {
            return Err(BackendError::InvalidInput);
        }
        let request = self.request(query_message(name, expected, query), deadline, cancelled)?;
        let mut client = self.points();
        let reply = self.call(client.query(request), deadline, cancelled, false)?;
        check(deadline, cancelled)?;
        verify_hit(&reply.result, expected)
    }
}

fn query_message(name: &str, expected: PointInput<'_>, query: &str) -> QueryPoints {
    QueryPoints {
        collection_name: name.to_owned(),
        query: Some(Query::new_text(
            TextQueryBuilder::new(query).k(1.2).b(0.75).build(),
        )),
        using: Some(schema::TEXT_FIELD.to_owned()),
        filter: Some(Filter::must([Condition::has_id([point_id(expected.id)])])),
        params: Some(SearchParams {
            idf: Some(IdfParams { corpus: None }),
            ..Default::default()
        }),
        limit: Some(1),
        with_vectors: Some(false.into()),
        with_payload: Some(vec!["source_id"].into()),
        ..Default::default()
    }
}

fn verify_hit(points: &[ScoredPoint], expected: PointInput<'_>) -> Result<(), BackendError> {
    let [point] = points else {
        return Err(if points.is_empty() {
            BackendError::ReadinessNotProven
        } else {
            BackendError::ResponseTooLarge
        });
    };
    if !point.score.is_finite() || point.vectors.is_some() || point.payload.len() != 1 {
        return Err(BackendError::MalformedResponse);
    }
    if decode_id(point.id.as_ref())? != expected.id
        || source_id(&point.payload)? != expected.source_id
    {
        return Err(BackendError::PointMismatch);
    }
    if point.score <= 0.0 {
        return Err(BackendError::ReadinessNotProven);
    }
    Ok(())
}

// Validate only the selected source word; do not produce or emulate a token stream.
// Official CodeTokenizer identifies maximal alphanumeric/_ runs. With the selected
// no-stemmer/no-stopword/no-length-clipping profile, a full ASCII-alphabetic run
// shares tokens between document and ranking query, including joined case splits.
fn saved_word(
    text: &str,
    query: &str,
    deadline: Instant,
    cancelled: &AtomicBool,
) -> Result<bool, BackendError> {
    for (start, _) in text.match_indices(query) {
        check(deadline, cancelled)?;
        let end = start + query.len();
        let before = text[..start].chars().next_back();
        let after = text[end..].chars().next();
        if before.is_none_or(separator) && after.is_none_or(separator) {
            return Ok(true);
        }
    }
    check(deadline, cancelled)?;
    Ok(false)
}

fn separator(character: char) -> bool {
    !character.is_alphanumeric() && character != '_'
}

#[cfg(test)]
mod tests {
    use std::collections::HashMap;
    use std::time::Duration;

    use qdrant_client::qdrant::{condition, point_id, value};
    use search_contracts::SourceId;

    use super::*;

    fn expected() -> PointInput<'static> {
        PointInput {
            id: [1; 16],
            source_id: SourceId::from_bytes([2; 16]),
            retrieval_text: "before AlphaWord after",
        }
    }

    #[test]
    fn query_metadata_is_singleton_global_idf_and_not_user_query() {
        let point = expected();
        let request = query_message("generation", point, "AlphaWord");
        assert_eq!(request.limit, Some(1));
        assert!(request.params.unwrap().idf.unwrap().corpus.is_none());
        let filter = request.filter.unwrap();
        let Some(condition::ConditionOneOf::HasId(ids)) = filter.must[0].condition_one_of.as_ref()
        else {
            panic!("singleton HasId absent");
        };
        assert_eq!(ids.has_id.len(), 1);
        assert_eq!(decode_id(ids.has_id.first()).unwrap(), point.id);
        assert!(matches!(
            request.with_vectors.unwrap().selector_options,
            Some(qdrant_client::qdrant::with_vectors_selector::SelectorOptions::Enable(false))
        ));
        let Some(qdrant_client::qdrant::with_payload_selector::SelectorOptions::Include(payload)) =
            request.with_payload.unwrap().selector_options
        else {
            panic!("source routing absent")
        };
        assert_eq!(payload.fields, ["source_id"]);
    }

    #[test]
    fn word_binding_rejects_identifier_substrings_without_declaring_tokenless() {
        let deadline = Instant::now() + Duration::from_secs(2);
        let cancel = AtomicBool::new(false);
        assert!(saved_word("before AlphaWord after", "AlphaWord", deadline, &cancel).unwrap());
        assert!(!saved_word("concatenate", "cat", deadline, &cancel).unwrap());
        assert!(!saved_word("alpha_suffix", "alpha", deadline, &cancel).unwrap());
    }

    #[test]
    fn empty_zero_score_and_wrong_routing_never_establish_readiness() {
        let expected = expected();
        assert_eq!(
            verify_hit(&[], expected),
            Err(BackendError::ReadinessNotProven)
        );
        let mut point = ScoredPoint {
            id: Some(crate::codec::point_id(expected.id)),
            payload: HashMap::from([(
                "source_id".to_owned(),
                qdrant_client::qdrant::Value {
                    kind: Some(value::Kind::StringValue(expected.source_id.to_string())),
                },
            )]),
            score: 1.0,
            ..Default::default()
        };
        assert_eq!(verify_hit(&[point.clone()], expected), Ok(()));
        point.score = 0.0;
        assert_eq!(
            verify_hit(&[point.clone()], expected),
            Err(BackendError::ReadinessNotProven)
        );
        point.score = f32::NAN;
        assert_eq!(
            verify_hit(&[point.clone()], expected),
            Err(BackendError::MalformedResponse)
        );
        point.score = 1.0;
        point.id = Some(qdrant_client::qdrant::PointId {
            point_id_options: Some(point_id::PointIdOptions::Uuid(
                "03030303-0303-0303-0303-030303030303".to_owned(),
            )),
        });
        assert_eq!(
            verify_hit(&[point], expected),
            Err(BackendError::PointMismatch)
        );
    }
}

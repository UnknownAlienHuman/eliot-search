//! Server-side Code/BM25 nomination, with routing-only responses.

use std::collections::BTreeSet;
use std::sync::atomic::AtomicBool;
use std::time::Instant;

use qdrant_client::qdrant::{
    BatchResult, Condition, Filter, IdfParams, Query, QueryBatchPoints, QueryPoints, SearchParams,
    TextQueryBuilder,
};

use super::{Backend, check};
use crate::codec::{decode_id, payload_string, source_id};
use crate::model::collection_name;
use crate::{BackendError, Hit, SourceClass, schema};

impl Backend {
    /// Ranks with server BM25 and a bounded set of indexed source-class candidates.
    ///
    /// The request selects `retrieval_text`, explicit k=1.2/b=0.75 and global
    /// IDF(corpus=None). A classified generation asks for top-k in each disjoint
    /// class. A positive constant multiplier preserves the order within each
    /// class, so the weighted top-k is contained in the candidate union.
    ///
    /// # Errors
    ///
    /// Returns invalid/oversized query/limit, read/response, cancellation or
    /// deadline failures. Results are nominations; CONTROL evidence supplies text.
    pub fn query_text(
        &self,
        name: &str,
        text: &str,
        limit: usize,
        source_classes: bool,
        deadline: Instant,
        cancelled: &AtomicBool,
    ) -> Result<Vec<Hit>, BackendError> {
        check(deadline, cancelled)?;
        collection_name(name)?;
        if text.is_empty() || limit == 0 || limit > self.limits.max_hits {
            return Err(BackendError::InvalidInput);
        }
        if text.len() > self.limits.max_query_bytes {
            return Err(BackendError::RequestTooLarge);
        }
        let wire_limit = u64::try_from(limit).map_err(|_| BackendError::InvalidInput)?;
        let classes = SourceClass::ALL.map(Some);
        let lanes: &[Option<SourceClass>] = if source_classes { &classes } else { &[None] };
        let mut client = self.points();
        let batches = if source_classes {
            let message = QueryBatchPoints {
                collection_name: name.to_owned(),
                query_points: lanes
                    .iter()
                    .map(|&class| query(name, text, wire_limit, class))
                    .collect(),
                ..Default::default()
            };
            let request = self.request(message, deadline, cancelled)?;
            self.call(client.query_batch(request), deadline, cancelled, false)?
                .result
        } else {
            let request = self.request(query(name, text, wire_limit, None), deadline, cancelled)?;
            let reply = self.call(client.query(request), deadline, cancelled, false)?;
            vec![BatchResult {
                result: reply.result,
            }]
        };
        if batches.len() != lanes.len() {
            return Err(BackendError::MalformedResponse);
        }
        let mut hits = Vec::new();
        hits.try_reserve_exact(lanes.len() * limit)
            .map_err(|_| BackendError::AllocationFailed)?;
        let mut seen = BTreeSet::new();
        for (batch, &class) in batches.into_iter().zip(lanes) {
            if batch.result.len() > limit {
                return Err(BackendError::ResponseTooLarge);
            }
            for point in batch.result {
                check(deadline, cancelled)?;
                let id = decode_id(point.id.as_ref())?;
                if !seen.insert(id)
                    || !point.score.is_finite()
                    || point.vectors.is_some()
                    || point.payload.len() != 1 + usize::from(class.is_some())
                {
                    return Err(BackendError::MalformedResponse);
                }
                if let Some(class) = class
                    && payload_string(&point.payload, schema::CLASS_FIELD)? != class.keyword()
                {
                    return Err(BackendError::MalformedResponse);
                }
                hits.push(Hit {
                    id,
                    source_id: source_id(&point.payload)?,
                    score: point.score * class.map_or(1.0, SourceClass::weight),
                    source_class: class,
                });
            }
        }
        if source_classes {
            hits.sort_unstable_by(|left, right| {
                right
                    .score
                    .total_cmp(&left.score)
                    .then_with(|| left.id.cmp(&right.id))
            });
            hits.truncate(limit);
        }
        Ok(hits)
    }
}

fn query(name: &str, text: &str, limit: u64, class: Option<SourceClass>) -> QueryPoints {
    let mut payload_fields = vec!["source_id"];
    if class.is_some() {
        payload_fields.push(schema::CLASS_FIELD);
    }
    QueryPoints {
        collection_name: name.to_owned(),
        query: Some(Query::new_text(
            TextQueryBuilder::new(text).k(1.2).b(0.75).build(),
        )),
        using: Some(schema::TEXT_FIELD.to_owned()),
        filter: class.map(|class| {
            Filter::must([Condition::matches(
                schema::CLASS_FIELD,
                class.keyword().to_owned(),
            )])
        }),
        params: Some(SearchParams {
            idf: Some(IdfParams { corpus: None }),
            ..Default::default()
        }),
        limit: Some(limit),
        with_vectors: Some(false.into()),
        with_payload: Some(payload_fields.into()),
        ..Default::default()
    }
}

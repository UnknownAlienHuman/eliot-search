//! Server-side Code/BM25 nomination, with routing-only responses.

use std::collections::BTreeSet;
use std::sync::atomic::AtomicBool;
use std::time::Instant;

use qdrant_client::qdrant::{IdfParams, Query, QueryPoints, SearchParams, TextQueryBuilder};

use super::{Backend, check};
use crate::codec::{decode_id, source_id};
use crate::model::collection_name;
use crate::{BackendError, Hit, schema};

impl Backend {
    /// Ranks using the fixed server BM25 query over the entire physical generation.
    ///
    /// The request selects `retrieval_text`, explicit k=1.2/b=0.75 and global
    /// IDF(corpus=None). It has no score threshold, sparse/model inference or
    /// local aliases. It returns only UUID/source routing and finite score.
    /// A known committed empty population may be handled without RPC by its owner.
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
        let message = QueryPoints {
            collection_name: name.to_owned(),
            query: Some(Query::new_text(
                TextQueryBuilder::new(text).k(1.2).b(0.75).build(),
            )),
            using: Some(schema::TEXT_FIELD.to_owned()),
            params: Some(SearchParams {
                idf: Some(IdfParams { corpus: None }),
                ..Default::default()
            }),
            limit: Some(u64::try_from(limit).map_err(|_| BackendError::InvalidInput)?),
            with_vectors: Some(false.into()),
            with_payload: Some(vec!["source_id"].into()),
            ..Default::default()
        };
        let request = self.request(message, deadline, cancelled)?;
        let mut client = self.points();
        let reply = self.call(client.query(request), deadline, cancelled, false)?;
        if reply.result.len() > limit {
            return Err(BackendError::ResponseTooLarge);
        }
        let mut hits = Vec::new();
        hits.try_reserve_exact(reply.result.len())
            .map_err(|_| BackendError::AllocationFailed)?;
        let mut seen = BTreeSet::new();
        for point in reply.result {
            check(deadline, cancelled)?;
            let id = decode_id(point.id.as_ref())?;
            if !seen.insert(id)
                || !point.score.is_finite()
                || point.vectors.is_some()
                || point.payload.len() != 1
            {
                return Err(BackendError::MalformedResponse);
            }
            hits.push(Hit {
                id,
                source_id: source_id(&point.payload)?,
                score: point.score,
            });
        }
        Ok(hits)
    }
}

//! Server-side Code/BM25 nomination, with routing-only responses.

use std::collections::{BTreeMap, BTreeSet};
use std::sync::atomic::AtomicBool;
use std::time::Instant;

use qdrant_client::qdrant::{
    BatchResult, Condition, Filter, IdfParams, Query, QueryBatchPoints, QueryPointGroups,
    QueryPoints, ScoredPoint, SearchParams, TextQueryBuilder, group_id,
};
use search_contracts::SourceId;

use super::{Backend, check};
use crate::codec::{decode_id, payload_string, source_id};
use crate::model::collection_name;
use crate::{BackendError, Hit, SourceClass, schema};

const MAX_HITS_PER_SOURCE: usize = 2;

impl Backend {
    /// Ranks with server BM25 and a bounded set of indexed source-class candidates.
    ///
    /// The request selects `retrieval_text`, explicit k=1.2/b=0.75 and global
    /// IDF(corpus=None). A classified generation asks for top-k in each disjoint
    /// class. A positive constant multiplier preserves the order within each
    /// class. When the first prefix exceeds two units per source, grouped
    /// queries nominate other sources in classes that need more candidates.
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
                let hit = decode_hit(&point, class)?;
                if !seen.insert(hit.id) {
                    return Err(BackendError::MalformedResponse);
                }
                hits.push(hit);
            }
        }
        if source_classes {
            sort_hits(&mut hits);
        }
        if exceeds_source_cap(hits.iter().take(limit)) {
            self.group_repeated_sources(
                name,
                text,
                limit,
                lanes,
                &mut hits,
                (deadline, cancelled),
            )?;
            sort_hits(&mut hits);
        }
        hits.truncate(limit);
        if exceeds_source_cap(hits.iter()) {
            return Err(BackendError::MalformedResponse);
        }
        Ok(hits)
    }

    fn group_repeated_sources(
        &self,
        name: &str,
        text: &str,
        limit: usize,
        lanes: &[Option<SourceClass>],
        hits: &mut Vec<Hit>,
        budget: (Instant, &AtomicBool),
    ) -> Result<(), BackendError> {
        let (deadline, cancelled) = budget;
        let repeated: Vec<_> = lanes
            .iter()
            .copied()
            .filter(|&class| {
                exceeds_source_cap(hits.iter().filter(|hit| hit.source_class == class))
            })
            .collect();
        hits.retain(|hit| !repeated.contains(&hit.source_class));
        let mut point_ids: BTreeSet<_> = hits.iter().map(|hit| hit.id).collect();
        let mut client = self.points();
        for class in repeated {
            let limit_u64 = u64::try_from(limit).map_err(|_| BackendError::InvalidInput)?;
            let request = self.request(
                grouped_query(name, text, limit_u64, class),
                deadline,
                cancelled,
            )?;
            let reply = self.call(client.query_groups(request), deadline, cancelled, false)?;
            let groups = reply.result.ok_or(BackendError::MalformedResponse)?.groups;
            if groups.len() > limit {
                return Err(BackendError::ResponseTooLarge);
            }
            hits.try_reserve_exact(groups.len() * MAX_HITS_PER_SOURCE)
                .map_err(|_| BackendError::AllocationFailed)?;
            let mut sources = BTreeSet::new();
            for group in groups {
                check(deadline, cancelled)?;
                let Some(group_id::Kind::StringValue(group_id)) = group.id.and_then(|id| id.kind)
                else {
                    return Err(BackendError::MalformedResponse);
                };
                let source =
                    SourceId::parse(&group_id).map_err(|_| BackendError::MalformedResponse)?;
                if !sources.insert(source)
                    || group.hits.is_empty()
                    || group.hits.len() > MAX_HITS_PER_SOURCE
                    || group.lookup.is_some()
                {
                    return Err(BackendError::MalformedResponse);
                }
                for point in group.hits {
                    let hit = decode_hit(&point, class)?;
                    if hit.source_id != source || !point_ids.insert(hit.id) {
                        return Err(BackendError::MalformedResponse);
                    }
                    hits.push(hit);
                }
            }
        }
        Ok(())
    }
}

fn decode_hit(point: &ScoredPoint, class: Option<SourceClass>) -> Result<Hit, BackendError> {
    if !point.score.is_finite()
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
    Ok(Hit {
        id: decode_id(point.id.as_ref())?,
        source_id: source_id(&point.payload)?,
        score: point.score * class.map_or(1.0, SourceClass::weight),
        source_class: class,
    })
}

fn exceeds_source_cap<'a>(hits: impl Iterator<Item = &'a Hit>) -> bool {
    let mut counts = BTreeMap::new();
    let mut hits = hits;
    hits.any(|hit| {
        let count = counts.entry(hit.source_id).or_insert(0);
        *count += 1;
        *count > MAX_HITS_PER_SOURCE
    })
}

fn sort_hits(hits: &mut [Hit]) {
    hits.sort_unstable_by(|left, right| {
        right
            .score
            .total_cmp(&left.score)
            .then_with(|| left.id.cmp(&right.id))
    });
}

fn grouped_query(
    name: &str,
    text: &str,
    limit: u64,
    class: Option<SourceClass>,
) -> QueryPointGroups {
    let point = query(name, text, limit, class);
    QueryPointGroups {
        collection_name: point.collection_name,
        query: point.query,
        using: point.using,
        filter: point.filter,
        params: point.params,
        limit: point.limit,
        group_size: Some(MAX_HITS_PER_SOURCE as u64),
        group_by: "source_id".to_owned(),
        with_payload: point.with_payload,
        with_vectors: point.with_vectors,
        ..Default::default()
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

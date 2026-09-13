//! Reference scoring and retrieval; no vendor transport or IDF implementation.

use core::cmp::Ordering;

use super::ranking::TopCandidates;
use super::{
    CandidateNomination, EligibilityFilter, ensure_filter_indexes, validate_filter,
    validate_query_vector,
};
use crate::{BridgeError, CollectionRoute, QdrantBridge};

impl QdrantBridge {
    /// Returns bounded filtered nominations. These are not evidence until exact
    /// candidate readback and access revalidation occur outside the bridge.
    /// The reference scan retains at most `limit` nominations while still
    /// checking every eligible score. It is not the production query engine.
    pub fn query_filtered(
        &self,
        route: &CollectionRoute,
        filter: &EligibilityFilter,
        vector_name: &str,
        query: &[(u32, f32)],
        limit: usize,
    ) -> Result<Vec<CandidateNomination>, BridgeError> {
        validate_filter(filter)?;
        if limit == 0 || limit > self.limits.max_query_candidates {
            return Err(BridgeError::QueryBudgetExceeded);
        }
        let collection = self
            .collections
            .get(route)
            .ok_or(BridgeError::CollectionNotFound)?;
        ensure_filter_indexes(&collection.schema)?;
        let vector_schema = collection
            .schema
            .named_vectors
            .get(vector_name)
            .ok_or(BridgeError::NamedVectorMissing)?;
        validate_query_vector(query, vector_schema.dimensions)?;

        let mut candidates = TopCandidates::new(limit);
        for point in collection.points.values() {
            if !filter.matches(&point.payload) {
                continue;
            }
            let vector = point
                .vectors
                .get(vector_name)
                .ok_or(BridgeError::NamedVectorMissing)?;
            let score = dot_sparse(query, &vector.values);
            candidates.consider(CandidateNomination {
                point_id: point.point_id,
                score,
                payload_digest: point.payload.payload_digest,
                identity_digest: point.payload.identity_digest,
            })?;
        }
        Ok(candidates.finish())
    }
}

fn dot_sparse(left: &[(u32, f32)], right: &[(u32, f32)]) -> f32 {
    let mut left_index = 0;
    let mut right_index = 0;
    let mut score = 0.0_f32;
    while left_index < left.len() && right_index < right.len() {
        match left[left_index].0.cmp(&right[right_index].0) {
            Ordering::Less => left_index += 1,
            Ordering::Greater => right_index += 1,
            Ordering::Equal => {
                // Single-rounding FMA may differ in the last ulp from a
                // separate multiply-then-add; ranking stays deterministic via
                // the point_id tiebreak in `ranking`.
                score = left[left_index].1.mul_add(right[right_index].1, score);
                left_index += 1;
                right_index += 1;
            }
        }
    }
    score
}

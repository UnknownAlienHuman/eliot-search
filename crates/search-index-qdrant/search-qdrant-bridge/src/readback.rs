use crate::mutation::validate_exact_ids;
use crate::query::{ensure_filter_indexes, validate_filter};
use crate::{
    BridgeError, CollectionRoute, EligibilityFilter, PointRecord, QdrantBridge,
    QdrantPointId,
};

/// Exact point readback with explicit missing/unexpected IDs.
#[derive(Clone, Debug, PartialEq)]
pub struct BoundedPointReadback {
    pub points: Vec<PointRecord>,
    pub missing_ids: Vec<QdrantPointId>,
    pub unexpected_ids: Vec<QdrantPointId>,
}

/// Exact-count result for the same closed filter language.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct ExactCount {
    pub count: usize,
}

impl QdrantBridge {
    /// Reads back exactly the requested identifiers.
    pub fn readback_exact(
        &self,
        route: &CollectionRoute,
        ids: Vec<QdrantPointId>,
    ) -> Result<BoundedPointReadback, BridgeError> {
        let ids = validate_exact_ids(ids, self.limits.max_points_per_mutation)?;
        let collection = self
            .collections
            .get(route)
            .ok_or(BridgeError::CollectionNotFound)?;
        let mut points = Vec::new();
        let mut missing_ids = Vec::new();
        for id in ids {
            match collection.points.get(&id) {
                Some(point) => points.push(point.clone()),
                None => missing_ids.push(id),
            }
        }
        Ok(BoundedPointReadback {
            points,
            missing_ids,
            unexpected_ids: Vec::new(),
        })
    }

    /// Counts points matching the closed indexed filter.
    pub fn count_exact(
        &self,
        route: &CollectionRoute,
        filter: &EligibilityFilter,
    ) -> Result<ExactCount, BridgeError> {
        validate_filter(filter)?;
        let collection = self
            .collections
            .get(route)
            .ok_or(BridgeError::CollectionNotFound)?;
        ensure_filter_indexes(&collection.schema)?;
        Ok(ExactCount {
            count: collection
                .points
                .values()
                .filter(|point| filter.matches(&point.payload))
                .count(),
        })
    }
}

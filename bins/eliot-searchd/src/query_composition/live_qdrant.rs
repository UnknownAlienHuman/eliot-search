//! Qualified real-Qdrant adapter for one admitted indexed retrieval leg.
//!
//! The adapter binds the synchronous executor port to the bridge-owned
//! [`BlockingRealQueryPlane`]. One rendered [`EligibilityFilter`] is reused for
//! retrieval, scoped IDF and exact counting. Scored payload fields remain hints:
//! membership and digest coordinates are accepted only after exact point
//! readback, and no Qdrant bytes become source evidence.

#![allow(dead_code)]

use std::collections::{BTreeMap, BTreeSet};

use search_contracts::{OpaqueId, ReceiptRef, SourceMembershipId};
use search_qdrant_bridge::live::LiveEndpoint;
use search_qdrant_bridge::qualified::QualifiedGate;
use search_qdrant_bridge::real::{BlockingRealQueryPlane, IdfScope, OpContext};
use search_qdrant_bridge::{
    BridgeError, BridgeLimits, CollectionRoute, CollectionSchema, EligibilityFilter, QdrantPointId,
};
use search_retrieval_executor::{
    LegTicket,
    indexed::{IndexedNomination, IndexedPortError, IndexedRetrievalPort, MAX_INDEXED_LIMIT},
};

use super::{map_bridge_error, membership_opaque_id, parse_membership};

/// Long-lived qualified Qdrant query owner for synchronous daemon composition.
///
/// It owns one bridge runtime/client pair. Route/schema admission is explicit
/// and must succeed after connect or restart before a leg can use that route.
pub(crate) struct LiveQdrantQueryPlane {
    plane: BlockingRealQueryPlane,
}

impl LiveQdrantQueryPlane {
    /// Connect to the exact gate-admitted real Qdrant server.
    pub(crate) fn connect(
        endpoint: &LiveEndpoint,
        gate: QualifiedGate,
        limits: BridgeLimits,
    ) -> Result<Self, BridgeError> {
        BlockingRealQueryPlane::connect(endpoint, gate, limits).map(|plane| Self { plane })
    }

    /// Verify and admit one exact committed physical route/schema.
    pub(crate) fn verify_route(
        &mut self,
        route: &CollectionRoute,
        schema: &CollectionSchema,
        context: &OpContext,
    ) -> Result<ReceiptRef, BridgeError> {
        self.plane.verify_schema(route, schema, context)
    }

    /// Bind one request-local indexed port to a single route/filter/context.
    #[must_use]
    pub(crate) fn bind_leg<'plane>(
        &'plane self,
        route: CollectionRoute,
        filter: EligibilityFilter,
        allowed: BTreeMap<OpaqueId, SourceMembershipId>,
        context: OpContext,
    ) -> LiveQdrantIndexedPort<'plane> {
        LiveQdrantIndexedPort {
            plane: &self.plane,
            route,
            filter,
            allowed,
            context,
        }
    }
}

/// Request-local vendor-neutral indexed port over the live qualified plane.
pub(crate) struct LiveQdrantIndexedPort<'plane> {
    plane: &'plane BlockingRealQueryPlane,
    route: CollectionRoute,
    filter: EligibilityFilter,
    allowed: BTreeMap<OpaqueId, SourceMembershipId>,
    context: OpContext,
}

impl LiveQdrantIndexedPort<'_> {
    fn validate_ticket(&self, ticket: &LegTicket) -> Result<(), IndexedPortError> {
        let safe_leg = ticket
            .leg
            .safe_index_leg
            .as_ref()
            .ok_or(IndexedPortError::Failure)?;
        let [plan] = safe_leg.eligibility_plans.as_slice() else {
            return Err(IndexedPortError::Failure);
        };
        if safe_leg.route.collection_generation_id != self.route.generation
            || safe_leg.route.visible_epoch != self.filter.visible_epoch
            || plan.collection_generation_id != self.route.generation
            || plan.visible_epoch != self.filter.visible_epoch
            || plan.access_partition_digest != self.filter.access_partition_digest
            || ticket.leg.memberships != safe_leg.memberships
            || ticket.leg.memberships.is_empty()
        {
            return Err(IndexedPortError::Failure);
        }

        let mut expected_names = BTreeSet::new();
        for membership in &ticket.leg.memberships {
            let name = membership_opaque_id(*membership)
                .map_err(|_| IndexedPortError::Failure)?;
            if self.allowed.get(&name) != Some(membership) {
                return Err(IndexedPortError::Failure);
            }
            expected_names.insert(name);
        }
        if expected_names != self.filter.allowed_source_memberships
            || self.allowed.len() != expected_names.len()
        {
            return Err(IndexedPortError::Failure);
        }
        Ok(())
    }
}

impl IndexedRetrievalPort for LiveQdrantIndexedPort<'_> {
    fn query_filtered(
        &mut self,
        ticket: &LegTicket,
        _predicates: &search_access::EligibilityPredicates,
        vector_name: &str,
        query_vector: &[(u32, f32)],
        limit: usize,
        idf_scoped_to_retrieval: bool,
    ) -> Result<Vec<IndexedNomination>, IndexedPortError> {
        self.validate_ticket(ticket)?;
        if !idf_scoped_to_retrieval || limit == 0 || limit > MAX_INDEXED_LIMIT {
            return Err(IndexedPortError::Failure);
        }
        let scored = self
            .plane
            .query_filtered(
                &self.route,
                &self.filter,
                vector_name,
                query_vector,
                limit,
                IdfScope::ScopedToRetrieval,
                &self.context,
            )
            .map_err(map_bridge_error)?;
        if scored.is_empty() {
            return Ok(Vec::new());
        }

        let ids: Vec<QdrantPointId> = scored.iter().map(|hit| hit.point_id).collect();
        let readback = self
            .plane
            .readback_exact(&self.route, ids, &self.context)
            .map_err(map_bridge_error)?;
        if !readback.unexpected_ids.is_empty() {
            return Err(IndexedPortError::Failure);
        }

        let mut by_id = BTreeMap::new();
        for point in &readback.points {
            let membership = parse_membership(&point.payload.source_membership_id)
                .map_err(|_| IndexedPortError::Failure)?;
            if !ticket.leg.memberships.contains(&membership)
                || self.allowed.get(&point.payload.source_membership_id) != Some(&membership)
                || by_id.insert(point.point_id, (point, membership)).is_some()
            {
                return Err(IndexedPortError::Failure);
            }
        }

        let mut nominations = Vec::with_capacity(scored.len());
        for hit in &scored {
            let Some((point, membership)) = by_id.get(&hit.point_id) else {
                // Missing exact readback means the nomination became stale. It
                // carries no evidence and is dropped before candidate validation.
                continue;
            };
            if point.payload.payload_digest != hit.payload_digest
                || point.payload.identity_digest != hit.identity_digest
            {
                continue;
            }
            nominations.push(IndexedNomination {
                point_id: hit.point_id.0,
                source_membership_id: *membership,
                identity_digest: point.payload.identity_digest,
                payload_digest: point.payload.payload_digest,
                raw_score: hit.score,
            });
        }
        Ok(nominations)
    }

    fn count_exact(
        &mut self,
        ticket: &LegTicket,
        _predicates: &search_access::EligibilityPredicates,
    ) -> Result<usize, IndexedPortError> {
        self.validate_ticket(ticket)?;
        self.plane
            .count_exact(&self.route, &self.filter, &self.context)
            .map(|count| count.count)
            .map_err(map_bridge_error)
    }
}

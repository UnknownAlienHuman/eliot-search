//! T28 indexed-retrieval composition over one pinned route/epoch.
//!
//! Daemon composition over the accepted `search-retrieval-executor::indexed`
//! kernel. One indexed leg is executed with retrieval and IDF rendered from
//! a single accepted [`BaseEligibilityPlan`] contract, nominations are
//! resolved through exact point readback, and the exact filter-population
//! denominator travels with the output so top-k saturation can never narrow
//! an exact proof.
//!
//! The production adapter in [`live_qdrant`] binds the synchronous executor
//! port to `search-qdrant-bridge::real::BlockingRealQueryPlane`. That bridge
//! facade owns the pinned async vendor transport and one finite runtime; daemon
//! composition sees only Eliot-owned route/filter/readback types. Presence of
//! the adapter does not publish indexed capability: startup must still supply an
//! executed `QualifiedGate`, verify the exact committed route/schema and inject
//! the port into a live registered recipe handler.
//!
//! Fixed production operation order:
//!
//! 1. `query_filtered` with one [`EligibilityFilter`] and scoped IDF (the
//!    retrieval filter is the IDF corpus, so denied documents never move
//!    permitted denominators);
//! 2. `readback_exact` over nominated point IDs (scored payload fields are
//!    hints; membership and digests are accepted only after exact readback);
//! 3. `count_exact` over the same single filter (the exact denominator).
//!
//! [`ProcessTestIndexedPort`] remains an explicit process-test double over the
//! synchronous in-memory oracle. It performs the same query/readback/count
//! sequence but never claims live-server proof. There is no runtime fallback
//! from [`live_qdrant::LiveQdrantIndexedPort`] to this oracle.
//!
//! What this module never does:
//!
//! - No Qdrant payload is treated as source evidence. Evidence comes only
//!   from exact revision readback in `search-candidate-validator`.
//! - No silent fallback: [`QueryCompositionError`] and the executor's
//!   `BackendUnavailable`/`ContaminatedLeg` stay typed.
//! - No second search database: the only indexed store is Qdrant.

#![forbid(unsafe_code)]

pub(crate) mod live_qdrant;
pub(crate) mod registry;
pub(crate) mod service;

use std::collections::{BTreeMap, BTreeSet};

use search_access::{BaseEligibilityPlan, EligibilityPredicates};
use search_contracts::{OpaqueId, SourceMembershipId};
use search_qdrant_bridge::{
    BridgeError, CollectionRoute, EligibilityFilter, QdrantBridge, QdrantPointId,
};
use search_retrieval_executor::{
    ExecuteError, LegTicket,
    indexed::{IndexedNomination, IndexedPortError, IndexedRetrievalPort, MAX_INDEXED_LIMIT},
};

/// Closed query-composition failure.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum QueryCompositionError {
    /// No memberships were supplied for filter rendering.
    EmptyMemberships,
    /// A membership identifier cannot be rendered as a bridge member name.
    MembershipEncoding,
    /// A bridge membership name cannot be parsed back to a membership.
    MembershipDecoding,
}

impl QueryCompositionError {
    /// Stable machine-readable reason code.
    #[must_use]
    pub const fn code(self) -> &'static str {
        match self {
            Self::EmptyMemberships => "QUERY_COMPOSITION_MEMBERSHIPS_EMPTY",
            Self::MembershipEncoding => "QUERY_COMPOSITION_MEMBERSHIP_ENCODING",
            Self::MembershipDecoding => "QUERY_COMPOSITION_MEMBERSHIP_DECODING",
        }
    }
}

/// Renders one membership identifier as a bridge member name.
///
/// The bridge carries memberships as opaque text; the UUID display string is
/// the canonical rendering. [`parse_membership`] inverts exactly this
/// rendering.
pub fn membership_opaque_id(
    membership: SourceMembershipId,
) -> Result<OpaqueId, QueryCompositionError> {
    OpaqueId::new(membership.to_string()).map_err(|_| QueryCompositionError::MembershipEncoding)
}

/// Parses a bridge member name produced by [`membership_opaque_id`].
pub fn parse_membership(name: &OpaqueId) -> Result<SourceMembershipId, QueryCompositionError> {
    SourceMembershipId::parse(name.as_str()).map_err(|_| QueryCompositionError::MembershipDecoding)
}

/// Renders the single closed filter from one accepted eligibility plan.
///
/// The same returned value feeds retrieval, the IDF corpus and the exact
/// count: a diverged second filter is unrepresentable by construction.
/// Every membership in `memberships` must be covered by the plan contract;
/// forgeable memberships outside the ticket are rejected by the executor.
pub fn render_eligibility_filter(
    plan: &BaseEligibilityPlan,
    memberships: &BTreeSet<SourceMembershipId>,
) -> Result<EligibilityFilter, QueryCompositionError> {
    if memberships.is_empty() {
        return Err(QueryCompositionError::EmptyMemberships);
    }
    let mut allowed = BTreeSet::new();
    for membership in memberships {
        allowed.insert(membership_opaque_id(*membership)?);
    }
    Ok(EligibilityFilter {
        access_partition_digest: plan.access_partition_digest,
        allowed_source_memberships: allowed,
        visible_epoch: plan.visible_epoch,
    })
}

/// Maps a bridge failure to the executor port error.
///
/// A missing route/collection/schema is indexed-unavailable (the pinned
/// route is not being served); every other bridge failure is a leg
/// failure. Nothing here becomes DIRECT success.
pub const fn map_bridge_error(error: BridgeError) -> IndexedPortError {
    match error {
        BridgeError::CollectionNotFound
        | BridgeError::CollectionAlreadyExists
        | BridgeError::CollectionSchemaMismatch
        | BridgeError::NamedVectorMissing
        | BridgeError::PayloadIndexMissing
        | BridgeError::UnindexedFilter
        | BridgeError::StrictModeRequired => IndexedPortError::Unavailable,
        _ => IndexedPortError::Failure,
    }
}

/// Process-test double over the synchronous in-memory oracle.
///
/// Binds one [`CollectionRoute`], one single-contract [`EligibilityFilter`]
/// and the authorized membership set. `query_filtered` runs the bridge
/// filtered query and then resolves every nominated point through
/// `readback_exact`: points missing from readback (retired between query
/// and readback) or whose digests moved (mutated between query and
/// readback) are dropped as stale; points outside the authorized set or
/// answers for unrequested IDs fail the leg closed. `count_exact` counts
/// the same single filter value.
pub struct ProcessTestIndexedPort<'bridge> {
    bridge: &'bridge QdrantBridge,
    route: CollectionRoute,
    filter: EligibilityFilter,
    allowed: BTreeMap<OpaqueId, SourceMembershipId>,
}

impl<'bridge> ProcessTestIndexedPort<'bridge> {
    /// Binds the port to one route and one rendered single-contract filter.
    #[must_use]
    pub const fn new(
        bridge: &'bridge QdrantBridge,
        route: CollectionRoute,
        filter: EligibilityFilter,
        allowed: BTreeMap<OpaqueId, SourceMembershipId>,
    ) -> Self {
        Self {
            bridge,
            route,
            filter,
            allowed,
        }
    }

    /// The single filter value shared by retrieval and counting.
    #[must_use]
    pub const fn filter(&self) -> &EligibilityFilter {
        &self.filter
    }

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

impl IndexedRetrievalPort for ProcessTestIndexedPort<'_> {
    fn query_filtered(
        &mut self,
        ticket: &LegTicket,
        _predicates: &EligibilityPredicates,
        vector_name: &str,
        query_vector: &[(u32, f32)],
        limit: usize,
        idf_scoped_to_retrieval: bool,
    ) -> Result<Vec<IndexedNomination>, IndexedPortError> {
        self.validate_ticket(ticket)?;
        if !idf_scoped_to_retrieval {
            return Err(IndexedPortError::Failure);
        }
        if limit == 0 || limit > MAX_INDEXED_LIMIT {
            return Err(IndexedPortError::Failure);
        }
        let scored = self
            .bridge
            .query_filtered(&self.route, &self.filter, vector_name, query_vector, limit)
            .map_err(map_bridge_error)?;
        if scored.is_empty() {
            return Ok(Vec::new());
        }
        let ids: Vec<QdrantPointId> = scored.iter().map(|hit| hit.point_id).collect();
        let readback = self
            .bridge
            .readback_exact(&self.route, ids)
            .map_err(map_bridge_error)?;
        if !readback.unexpected_ids.is_empty() {
            return Err(IndexedPortError::Failure);
        }
        let mut by_id = BTreeMap::new();
        for point in &readback.points {
            let membership = parse_membership(&point.payload.source_membership_id)
                .map_err(|_| IndexedPortError::Failure)?;
            if !ticket.leg.memberships.contains(&membership)
                || self.allowed.get(&point.payload.source_membership_id)
                    != Some(&membership)
                || by_id.insert(point.point_id, (point, membership)).is_some()
            {
                return Err(IndexedPortError::Failure);
            }
        }
        let mut nominations = Vec::new();
        for hit in &scored {
            let Some((point, membership)) = by_id.get(&hit.point_id) else {
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
        _predicates: &EligibilityPredicates,
    ) -> Result<usize, IndexedPortError> {
        self.validate_ticket(ticket)?;
        self.bridge
            .count_exact(&self.route, &self.filter)
            .map(|count| count.count)
            .map_err(map_bridge_error)
    }
}

/// Executes one pinned indexed leg through any accepted indexed port.
///
/// The helper owns no vendor/filter state. The supplied port may be the live
/// qualified Qdrant adapter or the explicit process-test oracle. Both receive
/// the same ticket, predicates, query vector and live security fences; backend
/// unavailability remains typed and never becomes DIRECT completion.
#[allow(clippy::too_many_arguments)]
pub fn execute_pinned_leg<P: IndexedRetrievalPort>(
    ticket: &LegTicket,
    pins: &search_retrieval_executor::LegPinSet,
    predicates: &EligibilityPredicates,
    vector_name: &str,
    query_vector: &[(u32, f32)],
    limit: usize,
    request_security: &search_access::RequestSecurityFence,
    live_before: &search_access::LiveSecurityState,
    live_after: &search_access::LiveSecurityState,
    port: &mut P,
) -> Result<search_retrieval_executor::indexed::IndexedLegOutput, ExecuteError> {
    search_retrieval_executor::indexed::execute_indexed_leg(
        ticket,
        pins,
        predicates,
        vector_name,
        query_vector,
        limit,
        true,
        request_security,
        live_before,
        live_after,
        port,
    )
}

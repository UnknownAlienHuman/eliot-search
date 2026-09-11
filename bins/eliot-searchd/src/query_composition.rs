//! T28 indexed-retrieval composition over one pinned route/epoch.
//!
//! Daemon composition over the accepted `search-retrieval-executor::indexed`
//! kernel. One indexed leg is executed with retrieval and IDF rendered from
//! a single accepted [`BaseEligibilityPlan`] contract, nominations are
//! resolved through exact point readback, and the exact filter-population
//! denominator travels with the output so top-k saturation can never narrow
//! an exact proof.
//!
//! Production data plane (async, qualified gate, live server) lives in
//! `search-qdrant-bridge::real::RealDataPlane` and is intentionally not
//! called here: it needs an async runtime plus an executed T22
//! qualification gate that this composition does not own. The production
//! call sequence mirrors [`ProcessTestIndexedPort`] one-to-one:
//!
//! 1. `query_filtered` with one [`EligibilityFilter`] and
//!    `IdfScope::ScopedToRetrieval` (the retrieval filter cloned as the IDF
//!    corpus, so denied documents never move permitted denominators);
//! 2. `readback_exact` over the nominated point IDs (payload bytes are
//!    hints only; membership and digests are authoritative only after
//!    readback);
//! 3. `count_exact` over the same single filter (the exact denominator).
//!
//! [`ProcessTestIndexedPort`] below is an explicit process-test double over
//! the synchronous in-memory oracle. It performs the same
//! query-then-readback-then-count steps with the same single filter value,
//! but it never claims live-server proof: the oracle scores plain sparse
//! dot products with no IDF weighting, so live scoring/IDF/count parity
//! remains the T24 acceptance obligation (the same deferral as
//! `access_composition::QDRANT_LIVE_PARITY_DEFERRED_TO`). Ranking parity
//! with a live server is not asserted by any test in this file.
//!
//! Wiring (integration owner): add to `entry.rs`
//!
//! ```text
//! #[cfg(feature = "wave4-query")]
//! mod query_composition;
//! ```
//!
//! and call [`execute_pinned_leg`] from the provider `query` path after the
//! T20 pre-retrieval gate. Without `wave4-query` the module is absent and
//! recipe queries stay explicitly unavailable through the existing
//! `PROVIDER_*_UNAVAILABLE` capability gating. The DIRECT path is untouched:
//! indexed-unavailable surfaces as `BackendUnavailable`, never as DIRECT
//! success.
//!
//! What this module never does:
//!
//! - No Qdrant payload is treated as source evidence. Evidence comes only
//!   from exact revision readback in `search-candidate-validator`.
//! - No silent fallback: [`QueryCompositionError`] and the executor's
//!   `BackendUnavailable`/`ContaminatedLeg` stay typed.
//! - No second search database: the only indexed store is Qdrant.

#![forbid(unsafe_code)]

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
        if !idf_scoped_to_retrieval {
            return Err(IndexedPortError::Failure);
        }
        if limit == 0 || limit > MAX_INDEXED_LIMIT {
            return Err(IndexedPortError::Failure);
        }
        for membership in &ticket.leg.memberships {
            let name = membership_opaque_id(*membership).map_err(|_| IndexedPortError::Failure)?;
            if !self.allowed.contains_key(&name) {
                return Err(IndexedPortError::Failure);
            }
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
            if !ticket.leg.memberships.contains(&membership) {
                return Err(IndexedPortError::Failure);
            }
            by_id.insert(point.point_id, (point, membership));
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
        _ticket: &LegTicket,
        _predicates: &EligibilityPredicates,
    ) -> Result<usize, IndexedPortError> {
        self.bridge
            .count_exact(&self.route, &self.filter)
            .map(|count| count.count)
            .map_err(map_bridge_error)
    }
}

/// Executes one pinned indexed leg through the process-test port.
///
/// Thin helper used by the retrieval e2e test: it renders nothing by
/// itself and owns no filter text. Callers supply the ticket, pins, single
/// predicates, query vector and both live fences; the port supplies the
/// single-contract bridge reads. Returns the executor output with its exact
/// denominator, or the typed [`ExecuteError`].
#[allow(clippy::too_many_arguments)]
pub fn execute_pinned_leg(
    ticket: &LegTicket,
    pins: &search_retrieval_executor::LegPinSet,
    predicates: &EligibilityPredicates,
    vector_name: &str,
    query_vector: &[(u32, f32)],
    limit: usize,
    request_security: &search_access::RequestSecurityFence,
    live_before: &search_access::LiveSecurityState,
    live_after: &search_access::LiveSecurityState,
    port: &mut ProcessTestIndexedPort<'_>,
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

//! Real daemon adapter for canonical S9.5/S10.3 indexed retrieval.
//!
//! This module is intentionally a leaf under the existing query-composition
//! owner. The caller passes the immutable leg ticket, its exact route/epoch
//! pin, and the `EligibilityPopulation` borrowed from the owner-compiled C18
//! result. One bridge request context and that same population are retained
//! across filtered retrieval/IDF, exact point readback, and denominator count.
//!
//! Point identities come from a bounded resolver over the current committed
//! projection manifest. A missing or mismatched identity fails the leg; no
//! identity is inferred from a point address or digest alias. Returned point
//! payloads remain nominations and still require the existing exact source
//! validator before evidence is emitted.

use std::collections::BTreeMap;
use std::sync::Arc;
use std::time::Instant;

use search_access::{LiveSecurityState, RequestSecurityFence};
use search_contracts::indexed::{EligibilityPopulation, PointPayload, VectorName, VectorValue};
use search_point_identity::s11::{PointId128, PointIdentity};
use search_ports::CancellationProbe;
use search_qdrant_bridge::real::{
    BlockingRealQueryPlane, CanonicalPointReadback, CanonicalRequestContext,
};
use search_qdrant_bridge::{BridgeError, CollectionRoute};
use search_retrieval_executor::canonical::{
    CanonicalIndexedLegOutput, CanonicalIndexedPortError, CanonicalIndexedReadback,
    CanonicalIndexedReadbackBatch, CanonicalIndexedRetrievalPort, CanonicalMembershipBinding,
    CanonicalPointAddress, execute_canonical_indexed_leg,
};
use search_retrieval_executor::{ExecuteError, LegPinSet, LegTicket};

/// Exact typed identity lookup backed by the current committed projection
/// manifest. Implementations perform only bounded lookups for query hits.
pub trait CanonicalPointIdentityResolver {
    fn resolve(&mut self, address: PointId128) -> Result<PointIdentity, CanonicalIndexedPortError>;
}

/// Exact point data paired with the S11 identity that authorized its readback.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct VerifiedCanonicalPoint {
    pub identity: PointIdentity,
    pub payload: PointPayload,
}

/// Synchronous canonical query adapter over the bridge's retained runtime.
///
/// This adapter owns no plan, filter, Qdrant request, source read, or durable
/// query state. Its population and request context are single borrowed/shared
/// values for the lifetime of one indexed leg.
pub struct RealCanonicalIndexedPort<'plane, 'population, 'resolver> {
    plane: &'plane BlockingRealQueryPlane,
    route: CollectionRoute,
    population: &'population EligibilityPopulation,
    identities: &'resolver mut dyn CanonicalPointIdentityResolver,
    context: CanonicalRequestContext,
}

impl<'plane, 'population, 'resolver> RealCanonicalIndexedPort<'plane, 'population, 'resolver> {
    pub(crate) fn new(
        plane: &'plane BlockingRealQueryPlane,
        route: CollectionRoute,
        population: &'population EligibilityPopulation,
        identities: &'resolver mut dyn CanonicalPointIdentityResolver,
        deadline: Instant,
        cancellation: Arc<dyn CancellationProbe + Send + Sync>,
    ) -> Result<Self, CanonicalIndexedPortError> {
        if route.generation != population.collection_generation_id() {
            return Err(CanonicalIndexedPortError::Failure);
        }
        let context = CanonicalRequestContext::with_deadline(deadline, cancellation)
            .map_err(map_context_error)?;
        context.check().map_err(map_context_error)?;
        Ok(Self {
            plane,
            route,
            population,
            identities,
            context,
        })
    }
}

impl CanonicalIndexedRetrievalPort for RealCanonicalIndexedPort<'_, '_, '_> {
    type Candidate = VerifiedCanonicalPoint;

    fn population(&self) -> &EligibilityPopulation {
        self.population
    }

    fn query_and_readback(
        &mut self,
        vector_name: &VectorName,
        query: &VectorValue,
        limit: usize,
    ) -> Result<CanonicalIndexedReadbackBatch<Self::Candidate>, CanonicalIndexedPortError> {
        self.context.check().map_err(map_context_error)?;
        let scored = self
            .plane
            .query_indexed(
                &self.route,
                self.population,
                vector_name,
                query,
                limit,
                &self.context,
            )
            .map_err(map_bridge_error)?;
        if scored.len() > limit {
            return Err(CanonicalIndexedPortError::Failure);
        }
        let nominated_addresses = scored
            .iter()
            .map(|candidate| point_address(candidate.address))
            .collect::<Vec<_>>();
        if scored.is_empty() {
            return Ok(CanonicalIndexedReadbackBatch {
                nominated_addresses,
                readback_missing_addresses: Vec::new(),
                nominations: Vec::new(),
            });
        }

        let mut expected_by_address = BTreeMap::new();
        let mut expected = Vec::with_capacity(scored.len());
        for candidate in &scored {
            let identity = self.identities.resolve(candidate.address)?;
            if identity.point_id() != candidate.address
                || expected_by_address
                    .insert(candidate.address, (candidate.score, identity.clone()))
                    .is_some()
            {
                return Err(CanonicalIndexedPortError::Failure);
            }
            expected.push(identity);
        }

        self.context.check().map_err(map_context_error)?;
        let readback = self
            .plane
            .readback_indexed_exact(&self.route, self.population, &expected, &self.context)
            .map_err(map_bridge_error)?;
        let nominations = join_exact_readback(readback.points, &mut expected_by_address)?;
        let mut readback_missing_addresses = readback
            .missing_ids
            .into_iter()
            .map(point_address)
            .collect::<Vec<_>>();
        readback_missing_addresses.sort_unstable();
        let expected_missing = expected_by_address
            .keys()
            .copied()
            .map(point_address)
            .collect::<std::collections::BTreeSet<_>>();
        if readback_missing_addresses
            .iter()
            .copied()
            .collect::<std::collections::BTreeSet<_>>()
            != expected_missing
        {
            return Err(CanonicalIndexedPortError::Failure);
        }
        self.context.check().map_err(map_context_error)?;
        Ok(CanonicalIndexedReadbackBatch {
            nominated_addresses,
            readback_missing_addresses,
            nominations,
        })
    }

    fn count_exact(&mut self) -> Result<u64, CanonicalIndexedPortError> {
        self.context.check().map_err(map_context_error)?;
        let count = self
            .plane
            .count_indexed(&self.route, self.population, &self.context)
            .map_err(map_bridge_error)?;
        self.context.check().map_err(map_context_error)?;
        Ok(count)
    }

    fn check_request(&self) -> Result<(), CanonicalIndexedPortError> {
        self.context.check().map_err(map_context_error)
    }
}

fn join_exact_readback(
    points: Vec<CanonicalPointReadback>,
    expected_by_address: &mut BTreeMap<PointId128, (f32, PointIdentity)>,
) -> Result<Vec<CanonicalIndexedReadback<VerifiedCanonicalPoint>>, CanonicalIndexedPortError> {
    let mut nominations = Vec::with_capacity(points.len());
    for point in points {
        let (score, identity) = expected_by_address
            .remove(&point.address)
            .ok_or(CanonicalIndexedPortError::Failure)?;
        if !score.is_finite()
            || identity.point_id() != point.address
            || identity.key().projection_membership_id != point.payload.projection_membership_id
        {
            return Err(CanonicalIndexedPortError::Failure);
        }
        nominations.push(CanonicalIndexedReadback {
            point_address: point_address(point.address),
            projection_membership_id: point.payload.projection_membership_id,
            raw_score: score,
            candidate: VerifiedCanonicalPoint {
                identity,
                payload: point.payload,
            },
        });
    }
    Ok(nominations)
}

const fn point_address(address: PointId128) -> CanonicalPointAddress {
    CanonicalPointAddress::from_bytes(*address.as_bytes())
}

const fn map_context_error(error: BridgeError) -> CanonicalIndexedPortError {
    match error {
        BridgeError::Cancelled => CanonicalIndexedPortError::Cancelled,
        BridgeError::TransportFailed => CanonicalIndexedPortError::DeadlineExceeded,
        _ => CanonicalIndexedPortError::Failure,
    }
}

const fn map_bridge_error(error: BridgeError) -> CanonicalIndexedPortError {
    match error {
        BridgeError::CollectionNotFound
        | BridgeError::CollectionAlreadyExists
        | BridgeError::CollectionSchemaMismatch
        | BridgeError::NamedVectorMissing
        | BridgeError::PayloadIndexMissing
        | BridgeError::UnindexedFilter
        | BridgeError::StrictModeRequired => CanonicalIndexedPortError::Unavailable,
        BridgeError::Cancelled => CanonicalIndexedPortError::Cancelled,
        _ => CanonicalIndexedPortError::Failure,
    }
}

/// Execute one exact pinned leg through real canonical Qdrant operations.
#[allow(clippy::too_many_arguments)]
pub fn execute_pinned_leg_canonical<'population>(
    plane: &BlockingRealQueryPlane,
    route: CollectionRoute,
    ticket: &LegTicket,
    pins: &LegPinSet,
    population: &'population EligibilityPopulation,
    membership_bindings: &[CanonicalMembershipBinding],
    identities: &mut dyn CanonicalPointIdentityResolver,
    vector_name: &VectorName,
    query: &VectorValue,
    limit: usize,
    deadline: Instant,
    cancellation: Arc<dyn CancellationProbe + Send + Sync>,
    request_security: &RequestSecurityFence,
    live_before: &LiveSecurityState,
    live_after: &LiveSecurityState,
) -> Result<CanonicalIndexedLegOutput<'population, VerifiedCanonicalPoint>, ExecuteError> {
    let mut port =
        RealCanonicalIndexedPort::new(plane, route, population, identities, deadline, cancellation)
            .map_err(map_executor_port_error)?;
    execute_canonical_indexed_leg(
        ticket,
        pins,
        population,
        membership_bindings,
        vector_name,
        query,
        limit,
        request_security,
        live_before,
        live_after,
        &mut port,
    )
}

const fn map_executor_port_error(error: CanonicalIndexedPortError) -> ExecuteError {
    match error {
        CanonicalIndexedPortError::Unavailable => ExecuteError::BackendUnavailable,
        CanonicalIndexedPortError::Cancelled => ExecuteError::Cancelled,
        CanonicalIndexedPortError::Failure => ExecuteError::BackendFailure,
        CanonicalIndexedPortError::DeadlineExceeded => ExecuteError::DeadlineExceeded,
    }
}

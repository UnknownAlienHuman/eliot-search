//! Real daemon adapter for canonical S9.5/S10.3 indexed retrieval.
//!
//! This module is intentionally a leaf under the existing query-composition
//! owner. The caller passes the immutable leg ticket, its exact route/epoch
//! pin, and the opaque owner-compiled C18 result. Its single
//! `EligibilityPopulation` and owner-verified source/projection links are
//! retained by reference. One bridge request context and that same population
//! are retained across filtered retrieval/IDF, exact point readback, and count.
//!
//! Point identities come from a bounded resolver over the current committed
//! projection manifest. A missing or mismatched identity fails the leg; no
//! identity is inferred from a point address or digest alias. Returned point
//! payloads remain nominations and still require the existing exact source
//! validator before evidence is emitted.

use std::collections::BTreeMap;
use std::sync::Arc;
use std::time::Instant;

use search_access::{CompiledEligibilityPopulation, LiveSecurityState, RequestSecurityFence};
use search_contracts::ProjectionMembershipId;
use search_contracts::indexed::{PointPayload, VectorName, VectorValue};
use search_point_identity::s11::{PointId128, PointIdentity};
use search_ports::CancellationProbe;
use search_qdrant_bridge::real::{
    BlockingRealQueryPlane, CanonicalPointReadback, CanonicalRequestContext,
};
use search_qdrant_bridge::{BridgeError, CollectionRoute};
use search_retrieval_executor::canonical::{
    CanonicalIndexedLegOutput, CanonicalIndexedPortError, CanonicalIndexedReadback,
    CanonicalIndexedReadbackBatch, CanonicalIndexedRetrievalPort, CanonicalPointAddress,
    execute_canonical_indexed_leg,
};
use search_retrieval_executor::{ExecuteError, LegPinSet, LegTicket};

/// Projection membership observed in the canonical query payload.
///
/// This is an untrusted locator hint only. It can select an owner lookup after
/// the adapter checks it against the opaque C18 population; it cannot authorize
/// source I/O or supply any part of the expected S11 identity.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct UntrustedProjectionMembershipHint {
    projection_membership_id: ProjectionMembershipId,
}

impl UntrustedProjectionMembershipHint {
    fn from_payload_observation(payload: &PointPayload) -> Self {
        Self {
            projection_membership_id: payload.projection_membership_id,
        }
    }

    /// The observed typed membership ID; callers must still validate scope.
    #[must_use]
    pub const fn projection_membership_id(self) -> ProjectionMembershipId {
        self.projection_membership_id
    }
}

/// Exact typed identity lookup backed by the current committed projection
/// manifest. Implementations perform only bounded lookups for query hits.
///
/// Both inputs are untrusted. Reject a foreign projection hint before reading
/// any C03/C02/C07 owner data, then use the in-scope projection hint only to
/// select the current manifest. Do not use the address to select a manifest or
/// source. Decode only the nominated manifest, derive the expected S11 identity
/// from trusted manifest metadata, and compare its derived address with the
/// query address. Never derive source identity or the expected key solely from
/// the query payload observation.
pub trait CanonicalPointIdentityResolver {
    fn resolve(
        &mut self,
        address: PointId128,
        hint: UntrustedProjectionMembershipHint,
    ) -> Result<PointIdentity, CanonicalIndexedPortError>;
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
pub struct RealCanonicalIndexedPort<'plane, 'compiled, 'resolver> {
    plane: &'plane BlockingRealQueryPlane,
    route: CollectionRoute,
    compiled_population: &'compiled CompiledEligibilityPopulation,
    allowed_projection_memberships: Vec<ProjectionMembershipId>,
    identities: &'resolver mut dyn CanonicalPointIdentityResolver,
    context: CanonicalRequestContext,
}

impl<'plane, 'compiled, 'resolver> RealCanonicalIndexedPort<'plane, 'compiled, 'resolver> {
    pub(crate) fn new(
        plane: &'plane BlockingRealQueryPlane,
        route: CollectionRoute,
        compiled_population: &'compiled CompiledEligibilityPopulation,
        identities: &'resolver mut dyn CanonicalPointIdentityResolver,
        deadline: Instant,
        cancellation: Arc<dyn CancellationProbe + Send + Sync>,
    ) -> Result<Self, CanonicalIndexedPortError> {
        let population = compiled_population.population();
        if route.generation != population.collection_generation_id() {
            return Err(CanonicalIndexedPortError::Failure);
        }
        let context = CanonicalRequestContext::with_deadline(deadline, cancellation)
            .map_err(map_context_error)?;
        context.check().map_err(map_context_error)?;
        let allowed_projection_memberships = population.allowed_projection_memberships();
        context.check().map_err(map_context_error)?;
        Ok(Self {
            plane,
            route,
            compiled_population,
            allowed_projection_memberships,
            identities,
            context,
        })
    }
}

impl CanonicalIndexedRetrievalPort for RealCanonicalIndexedPort<'_, '_, '_> {
    type Candidate = VerifiedCanonicalPoint;

    fn compiled_population(&self) -> &CompiledEligibilityPopulation {
        self.compiled_population
    }

    fn query_and_readback(
        &mut self,
        vector_name: &VectorName,
        query: &VectorValue,
        limit: usize,
    ) -> Result<CanonicalIndexedReadbackBatch<Self::Candidate>, CanonicalIndexedPortError> {
        self.context.check().map_err(map_context_error)?;
        let population = self.compiled_population.population();
        let scored = self
            .plane
            .query_indexed(
                &self.route,
                population,
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
            self.context.check().map_err(map_context_error)?;
            let hint = UntrustedProjectionMembershipHint::from_payload_observation(
                &candidate.payload_observation,
            );
            let identity = resolve_candidate_identity(
                self.identities,
                &self.allowed_projection_memberships,
                candidate.address,
                hint,
            )?;
            self.context.check().map_err(map_context_error)?;
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
            .readback_indexed_exact(
                &self.route,
                self.compiled_population.population(),
                &expected,
                &self.context,
            )
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
        let population = self.compiled_population.population();
        let count = self
            .plane
            .count_indexed(&self.route, population, &self.context)
            .map_err(map_bridge_error)?;
        self.context.check().map_err(map_context_error)?;
        Ok(count)
    }

    fn check_request(&self) -> Result<(), CanonicalIndexedPortError> {
        self.context.check().map_err(map_context_error)
    }
}

fn resolve_candidate_identity(
    identities: &mut dyn CanonicalPointIdentityResolver,
    allowed_projection_memberships: &[ProjectionMembershipId],
    address: PointId128,
    hint: UntrustedProjectionMembershipHint,
) -> Result<PointIdentity, CanonicalIndexedPortError> {
    if allowed_projection_memberships
        .binary_search(&hint.projection_membership_id())
        .is_err()
    {
        return Err(CanonicalIndexedPortError::Failure);
    }
    identities.resolve(address, hint)
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

#[cfg(test)]
mod tests {
    use super::{
        CanonicalPointIdentityResolver, UntrustedProjectionMembershipHint,
        resolve_candidate_identity,
    };
    use search_contracts::ProjectionMembershipId;
    use search_point_identity::s11::{PointId128, PointIdentity};
    use search_retrieval_executor::canonical::CanonicalIndexedPortError;
    use std::cell::Cell;

    struct ResolverProbe {
        calls: Cell<usize>,
    }

    impl CanonicalPointIdentityResolver for ResolverProbe {
        fn resolve(
            &mut self,
            _address: PointId128,
            _hint: UntrustedProjectionMembershipHint,
        ) -> Result<PointIdentity, CanonicalIndexedPortError> {
            self.calls.set(self.calls.get().saturating_add(1));
            Err(CanonicalIndexedPortError::Failure)
        }
    }

    #[test]
    fn foreign_projection_hint_is_rejected_before_resolver_owner_reads() {
        let in_scope = ProjectionMembershipId::from_bytes([0x11; 16]);
        let foreign = ProjectionMembershipId::from_bytes([0x22; 16]);
        let allowed = [in_scope];
        let hint = UntrustedProjectionMembershipHint {
            projection_membership_id: foreign,
        };
        let mut resolver = ResolverProbe {
            calls: Cell::new(0),
        };

        assert!(matches!(
            resolve_candidate_identity(
                &mut resolver,
                &allowed,
                PointId128::from_bytes([0x33; 16]),
                hint,
            ),
            Err(CanonicalIndexedPortError::Failure)
        ));
        assert_eq!(resolver.calls.get(), 0);
    }
}

/// Execute one exact pinned leg through real canonical Qdrant operations.
#[allow(clippy::too_many_arguments)]
pub fn execute_pinned_leg_canonical<'compiled>(
    plane: &BlockingRealQueryPlane,
    route: CollectionRoute,
    ticket: &LegTicket,
    pins: &LegPinSet,
    compiled_population: &'compiled CompiledEligibilityPopulation,
    identities: &mut dyn CanonicalPointIdentityResolver,
    vector_name: &VectorName,
    query: &VectorValue,
    limit: usize,
    deadline: Instant,
    cancellation: Arc<dyn CancellationProbe + Send + Sync>,
    request_security: &RequestSecurityFence,
    live_before: &LiveSecurityState,
    live_after: &LiveSecurityState,
) -> Result<CanonicalIndexedLegOutput<'compiled, VerifiedCanonicalPoint>, ExecuteError> {
    let mut port = RealCanonicalIndexedPort::new(
        plane,
        route,
        compiled_population,
        identities,
        deadline,
        cancellation,
    )
    .map_err(map_executor_port_error)?;
    execute_canonical_indexed_leg(
        ticket,
        pins,
        compiled_population,
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

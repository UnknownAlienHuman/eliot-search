//! Synchronous query facade over the qualified async real data plane.
//!
//! The daemon provider loop is synchronous, while the pinned `qdrant-client`
//! transport is async-only. This owner keeps one finite Tokio runtime alive for
//! one [`RealDataPlane`] and exposes the synchronous canonical schema, query,
//! readback and exact mutation operations needed by daemon composition. It never
//! falls back to the in-memory oracle, creates a second protocol stack, or treats
//! Qdrant payloads as source evidence.

use std::fmt;

use tokio::runtime::{Builder, Handle, Runtime};

use search_contracts::Epoch;
use search_contracts::indexed::{
    CollectionSchema as IndexedCollectionSchema, EligibilityPopulation, VectorName, VectorValue,
};
use search_point_identity::s11::{PointId128, PointIdentity};

use crate::live::{LiveEndpoint, LiveSuiteReport, run_live_probe_suite};
use crate::qualified::QualifiedGate;
use crate::{
    BoundedPointReadback, BridgeError, BridgeLimits, CandidateNomination, CollectionRoute,
    CollectionSchema, EligibilityFilter, ExactCount, QdrantApiKeyLease, QdrantConnectionBinding,
    QdrantPointId,
};

use super::{
    CanonicalCandidate, CanonicalDeleteReadback, CanonicalMutationReadback, CanonicalPointInput,
    CanonicalReadback, CanonicalRequestContext, CanonicalScrollPage,
    CanonicalValidityRestoreOutcome, IdfScope, LiveProbeClient, OpContext, OperationalProbeFailure,
    RealDataPlane,
};

/// One real Qdrant query plane for synchronous composition.
///
/// Field order is intentional: the Qdrant client is dropped before its owning
/// runtime. The runtime has one worker thread and is reused for every operation
/// instead of being recreated for each request or leg.
/// The explicit canonical `*_indexed` calls below use this same plane/runtime.
/// Its existing `QualifiedGate` input is transport admission only; this facade
/// does not establish artifact, profile, process or product qualification.
pub struct BlockingRealQueryPlane {
    plane: RealDataPlane,
    runtime: Runtime,
}

impl BlockingRealQueryPlane {
    /// Establishes an operational plane only after the bridge has executed and
    /// read back all mandatory probes on the already-supervised endpoint.
    ///
    /// The probe attempt checks protected collection-list access, then uses one fresh
    /// collection name, the original bounded request context and the supplied
    /// purpose-bound lease. Existing names,
    /// unknown mutation outcomes, failed probes, and failed exact cleanup all
    /// refuse construction. The report proves this probe execution only; it is
    /// not release qualification.
    pub fn connect_and_probe(
        endpoint: &LiveEndpoint,
        supervisor: QdrantConnectionBinding,
        auth_lease: QdrantApiKeyLease,
        context: CanonicalRequestContext,
        limits: BridgeLimits,
    ) -> Result<(Self, LiveSuiteReport), OperationalProbeFailure> {
        ensure_synchronous_context().map_err(OperationalProbeFailure::before_attempt)?;
        let runtime = Builder::new_multi_thread()
            .worker_threads(1)
            .thread_name("eliot-qdrant-query")
            .enable_all()
            .build()
            .map_err(|_| OperationalProbeFailure::before_attempt(BridgeError::TransportFailed))?;
        let client = LiveProbeClient::connect(endpoint, supervisor, auth_lease, context, limits)?;
        let completed = runtime.block_on(run_live_probe_suite(&client))?;
        let Ok(gate) = QualifiedGate::admit_executed(&completed.executed) else {
            return Err(client.failure(BridgeError::CapabilityReceiptMismatch));
        };
        let plane = client.into_data_plane(gate)?;
        Ok((Self { plane, runtime }, completed.report))
    }

    /// Connect through the existing transport gate and supervisor/auth binding.
    ///
    /// Construction is valid only from synchronous composition. Calling this
    /// facade from inside another Tokio runtime fails closed with
    /// [`BridgeError::OperationConflict`] instead of nesting `block_on` and
    /// panicking. Runtime construction failure is a definite pre-dispatch
    /// [`BridgeError::TransportFailed`]. The caller-supplied `QualifiedGate` is
    /// not execution proof or product qualification.
    pub fn connect(
        endpoint: &LiveEndpoint,
        supervisor: QdrantConnectionBinding,
        auth_lease: QdrantApiKeyLease,
        gate: QualifiedGate,
        limits: BridgeLimits,
    ) -> Result<Self, BridgeError> {
        ensure_synchronous_context()?;
        let runtime = Builder::new_multi_thread()
            .worker_threads(1)
            .thread_name("eliot-qdrant-query")
            .enable_all()
            .build()
            .map_err(|_| BridgeError::TransportFailed)?;
        let plane = runtime.block_on(RealDataPlane::connect(
            endpoint, supervisor, auth_lease, gate, limits,
        ))?;
        Ok(Self { plane, runtime })
    }

    /// Recheck and admit one exact committed collection schema after connect or
    /// restart. No route is usable until this succeeds.
    pub fn verify_schema(
        &mut self,
        route: &CollectionRoute,
        expected: &CollectionSchema,
        context: &OpContext,
    ) -> Result<search_contracts::ReceiptRef, BridgeError> {
        ensure_synchronous_context()?;
        let Self { plane, runtime } = self;
        runtime.block_on(plane.verify_schema(route, expected, context))
    }

    /// Creates a fresh canonical collection generation through this plane's retained runtime.
    pub fn create_indexed_generation(
        &mut self,
        route: &CollectionRoute,
        schema: &IndexedCollectionSchema,
        context: &CanonicalRequestContext,
    ) -> Result<(), BridgeError> {
        ensure_synchronous_context()?;
        let Self { plane, runtime } = self;
        runtime.block_on(plane.create_indexed_generation(route, schema, context))
    }

    /// Admit the exact canonical schema and full route after live metadata readback.
    pub fn verify_indexed_generation(
        &mut self,
        route: &CollectionRoute,
        expected: &IndexedCollectionSchema,
        context: &CanonicalRequestContext,
    ) -> Result<(), BridgeError> {
        ensure_synchronous_context()?;
        let Self { plane, runtime } = self;
        runtime.block_on(plane.verify_indexed_generation(route, expected, context))
    }

    /// Synchronous canonical sparse query over one shared eligibility population.
    pub fn query_indexed(
        &self,
        route: &CollectionRoute,
        population: &EligibilityPopulation,
        vector_name: &VectorName,
        query: &VectorValue,
        limit: usize,
        context: &CanonicalRequestContext,
    ) -> Result<Vec<CanonicalCandidate>, BridgeError> {
        ensure_synchronous_context()?;
        self.runtime.block_on(self.plane.query_indexed(
            route,
            population,
            vector_name,
            query,
            limit,
            context,
        ))
    }

    /// Count the exact canonical eligibility population.
    pub fn count_indexed(
        &self,
        route: &CollectionRoute,
        population: &EligibilityPopulation,
        context: &CanonicalRequestContext,
    ) -> Result<u64, BridgeError> {
        ensure_synchronous_context()?;
        self.runtime
            .block_on(self.plane.count_indexed(route, population, context))
    }

    /// Read a bounded page of canonical S11 addresses from one population.
    pub fn scroll_indexed(
        &self,
        route: &CollectionRoute,
        population: &EligibilityPopulation,
        offset: Option<PointId128>,
        limit: usize,
        context: &CanonicalRequestContext,
    ) -> Result<CanonicalScrollPage, BridgeError> {
        ensure_synchronous_context()?;
        self.runtime.block_on(
            self.plane
                .scroll_indexed(route, population, offset, limit, context),
        )
    }

    /// Read exact canonical S11 identities, including full keys and digests.
    pub fn readback_indexed_exact(
        &self,
        route: &CollectionRoute,
        population: &EligibilityPopulation,
        expected: &[PointIdentity],
        context: &CanonicalRequestContext,
    ) -> Result<CanonicalReadback, BridgeError> {
        ensure_synchronous_context()?;
        self.runtime.block_on(
            self.plane
                .readback_indexed_exact(route, population, expected, context),
        )
    }

    /// Full payload/vector readback for exact point expectations, independent
    /// of the current visible eligibility population.
    pub fn readback_indexed_points_exact(
        &self,
        route: &CollectionRoute,
        expected: &[CanonicalPointInput],
        context: &CanonicalRequestContext,
    ) -> Result<CanonicalMutationReadback, BridgeError> {
        ensure_synchronous_context()?;
        self.runtime.block_on(
            self.plane
                .readback_indexed_points_exact(route, expected, context),
        )
    }

    /// Upsert one exact canonical point batch and return actual full readback.
    pub fn upsert_indexed_exact(
        &mut self,
        route: &CollectionRoute,
        points: &[CanonicalPointInput],
        context: &CanonicalRequestContext,
    ) -> Result<CanonicalMutationReadback, BridgeError> {
        ensure_synchronous_context()?;
        let Self { plane, runtime } = self;
        runtime.block_on(plane.upsert_indexed_exact(route, points, context))
    }

    /// Close exact canonical point IDs at one immutable epoch.
    pub fn close_indexed_exact(
        &mut self,
        route: &CollectionRoute,
        expected: &[CanonicalPointInput],
        valid_until_epoch_exclusive: Epoch,
        context: &CanonicalRequestContext,
    ) -> Result<CanonicalMutationReadback, BridgeError> {
        ensure_synchronous_context()?;
        let Self { plane, runtime } = self;
        runtime.block_on(plane.close_indexed_exact(
            route,
            expected,
            valid_until_epoch_exclusive,
            context,
        ))
    }

    /// Restore the exact original validity bounds through the retained runtime.
    /// The original canonical request context and the async bridge's
    /// acknowledged/unknown outcome classification pass through unchanged.
    pub fn restore_indexed_validity_exact(
        &mut self,
        route: &CollectionRoute,
        original: &[CanonicalPointInput],
        close_attempt_until: Epoch,
        context: &CanonicalRequestContext,
    ) -> Result<CanonicalValidityRestoreOutcome, BridgeError> {
        ensure_synchronous_context()?;
        let Self { plane, runtime } = self;
        runtime.block_on(plane.restore_indexed_validity_exact(
            route,
            original,
            close_attempt_until,
            context,
        ))
    }

    /// Delete exact canonical point IDs and return the observed absent IDs.
    pub fn delete_indexed_exact(
        &mut self,
        route: &CollectionRoute,
        expected: &[CanonicalPointInput],
        context: &CanonicalRequestContext,
    ) -> Result<CanonicalDeleteReadback, BridgeError> {
        ensure_synchronous_context()?;
        let Self { plane, runtime } = self;
        runtime.block_on(plane.delete_indexed_exact(route, expected, context))
    }

    /// Execute one bounded filtered sparse query. Retrieval and IDF population
    /// remain coupled by the bridge-owned [`IdfScope`] contract.
    pub fn query_filtered(
        &self,
        route: &CollectionRoute,
        filter: &EligibilityFilter,
        vector_name: &str,
        query: &[(u32, f32)],
        limit: usize,
        idf: IdfScope,
        context: &OpContext,
    ) -> Result<Vec<CandidateNomination>, BridgeError> {
        ensure_synchronous_context()?;
        self.runtime.block_on(self.plane.query_filtered(
            route,
            filter,
            vector_name,
            query,
            limit,
            idf,
            context,
        ))
    }

    /// Read back exactly the nominated point identities and typed metadata.
    pub fn readback_exact(
        &self,
        route: &CollectionRoute,
        ids: Vec<QdrantPointId>,
        context: &OpContext,
    ) -> Result<BoundedPointReadback, BridgeError> {
        ensure_synchronous_context()?;
        self.runtime
            .block_on(self.plane.readback_exact(route, ids, context))
    }

    /// Count the exact population selected by the same eligibility filter.
    pub fn count_exact(
        &self,
        route: &CollectionRoute,
        filter: &EligibilityFilter,
        context: &OpContext,
    ) -> Result<ExactCount, BridgeError> {
        ensure_synchronous_context()?;
        self.runtime
            .block_on(self.plane.count_exact(route, filter, context))
    }

    /// Executed qualification gate retained by the real data plane.
    #[must_use]
    pub const fn gate(&self) -> &QualifiedGate {
        self.plane.gate()
    }
}

impl fmt::Debug for BlockingRealQueryPlane {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("BlockingRealQueryPlane")
            .field("gate", self.plane.gate())
            .finish_non_exhaustive()
    }
}

fn ensure_synchronous_context() -> Result<(), BridgeError> {
    if Handle::try_current().is_ok() {
        Err(BridgeError::OperationConflict)
    } else {
        Ok(())
    }
}

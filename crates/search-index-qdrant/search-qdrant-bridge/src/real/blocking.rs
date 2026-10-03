//! Synchronous query facade over the qualified async real data plane.
//!
//! The daemon provider loop is synchronous, while the pinned `qdrant-client`
//! transport is async-only. This owner keeps one finite Tokio runtime alive for
//! one [`RealDataPlane`] and exposes only the read/query operations needed by the
//! vendor-neutral indexed retrieval port. It never falls back to the in-memory
//! oracle, creates a second protocol stack, or treats Qdrant payloads as source
//! evidence.

use std::fmt;

use tokio::runtime::{Builder, Handle, Runtime};

use crate::live::LiveEndpoint;
use crate::qualified::QualifiedGate;
use crate::{
    BoundedPointReadback, BridgeError, BridgeLimits, CandidateNomination, CollectionRoute,
    CollectionSchema, EligibilityFilter, ExactCount, QdrantApiKeyLease, QdrantConnectionBinding,
    QdrantPointId,
};

use super::{IdfScope, OpContext, RealDataPlane};

/// One qualified real Qdrant query plane for synchronous composition.
///
/// Field order is intentional: the Qdrant client is dropped before its owning
/// runtime. The runtime has one worker thread and is reused for every operation
/// instead of being recreated for each request or leg.
pub struct BlockingRealQueryPlane {
    plane: RealDataPlane,
    runtime: Runtime,
}

impl BlockingRealQueryPlane {
    /// Connect through the exact executed qualification gate.
    ///
    /// Construction is valid only from synchronous composition. Calling this
    /// facade from inside another Tokio runtime fails closed with
    /// [`BridgeError::OperationConflict`] instead of nesting `block_on` and
    /// panicking. Runtime construction failure is a definite pre-dispatch
    /// [`BridgeError::TransportFailed`].
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

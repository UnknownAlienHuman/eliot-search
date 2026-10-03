//! Real T24 data-plane adapter over the pinned `qdrant-client` 1.19.0 transport.
//!
//! The in-memory [`QdrantBridge`](crate::QdrantBridge) stays as the behavioral
//! test oracle only. Every production data-plane operation here executes
//! against a real Qdrant server through the exact qualified client and
//! returns verified typed results, never model success.
//!
//! Admission: [`RealDataPlane::connect`] requires an executed
//! [`QualifiedGate`](crate::qualified::QualifiedGate) (all T22 mandatory live
//! probes passed) and rechecks the live server identity on connect. Collection
//! names, generation identities, filters and batches are validated before
//! dispatch.
//!
//! Single-contract retrieval + IDF (invariant 5): [`RealDataPlane::query_filtered`]
//! takes one [`EligibilityFilter`](crate::EligibilityFilter) and renders both
//! the retrieval `filter` and the `idf.corpus` population filter from that
//! exact value. [`IdfScope::ScopedToRetrieval`] is the sole production mode;
//! collection-wide IDF is available only to the separate qualification probe,
//! never to an admitted query call. A diverged population is unrepresentable.
//!
//! Pre-dispatch versus possible-write failures: validation, cancellation and
//! connect-time failures are definite typed errors (no commit was possible).
//! Any mutation dispatch that may have reached the server — transport loss,
//! deadline expiry, ambiguous acknowledgement — reports
//! [`BridgeError::MutationOutcomeUnknown`](crate::BridgeError::MutationOutcomeUnknown)
//! and must be resolved through [`RealDataPlane::readback_exact`] with the
//! same mutation identity. Reads never report unknown outcomes: they commit
//! nothing, so loss maps to [`BridgeError::TransportFailed`](crate::BridgeError).
//!
//! Vendor (`qdrant_client`) types never appear in public signatures and vendor
//! status text never reaches errors: every failure maps to a stable
//! [`BridgeError`](crate::BridgeError) code.
//!
//! The T24 scope is sparse vectors only: a schema carrying a dense vector is
//! rejected pre-dispatch with
//! [`BridgeError::CollectionSchemaMismatch`](crate::BridgeError::CollectionSchemaMismatch)
//! because dense layouts need a distinct accepted scoring profile.

use std::collections::{BTreeMap, BTreeSet, HashMap};
use std::sync::{
    Arc,
    atomic::{AtomicBool, Ordering},
};
use std::time::{Duration, Instant};

use qdrant_client::Qdrant;
use qdrant_client::qdrant::{
    CollectionInfo, Condition, CountPoints, CreateCollection,
    CreateFieldIndexCollection, DeletePoints, FieldCondition, FieldType,
    Filter, GetPoints, IdfParams, Match, Modifier, PayloadSchemaType, PointId,
    PointStruct, PointsIdsList, PointsSelector, Query, QueryPoints, Range,
    RepeatedStrings, ScrollPoints, SearchParams, SetPayloadPoints,
    SparseVectorConfig, SparseVectorParams, StrictModeConfig, UpdateCollection,
    UpdateStatus, UpsertPoints, Value, Vector, VectorInput, Vectors,
    WriteOrdering, WriteOrderingType, condition, point_id, points_selector,
    r#match, value, vector_output, vectors, vectors_output,
};
use search_contracts::{CollectionGenerationId, OpaqueId, ReceiptRef};

use crate::live::LiveEndpoint;
use crate::mutation::{same_point_identity, validate_close_epoch};
use crate::qualified::{
    QUALIFIED_SERVER_BUILD, QUALIFIED_SERVER_VERSION, QualifiedGate,
};
use crate::{
    BoundedPointReadback, BridgeError, BridgeLimits, BridgeMutation,
    CandidateNomination, CollectionRoute, CollectionSchema, EligibilityFilter,
    ExactCount, MutationReceipt, PointPayload, PointRecord, QdrantPointId,
    StoredVector,
};

const CODE_INVALID_ARGUMENT: i32 = 3;
const CODE_NOT_FOUND: i32 = 5;
const CODE_ALREADY_EXISTS: i32 = 6;
const CODE_PERMISSION_DENIED: i32 = 7;
const CODE_RESOURCE_EXHAUSTED: i32 = 8;
const CODE_FAILED_PRECONDITION: i32 = 9;
const CODE_OUT_OF_RANGE: i32 = 11;
const CODE_UNAUTHENTICATED: i32 = 16;

#[derive(Clone, Debug)]
pub struct OpContext {
    deadline: Duration,
    cancelled: Option<Arc<AtomicBool>>,
}

impl OpContext {
    #[must_use]
    pub const fn new(deadline: Duration) -> Self {
        Self {
            deadline,
            cancelled: None,
        }
    }

    #[must_use]
    pub const fn with_cancel(deadline: Duration, cancelled: Arc<AtomicBool>) -> Self {
        Self {
            deadline,
            cancelled: Some(cancelled),
        }
    }

    #[must_use]
    pub const fn deadline(&self) -> Duration {
        self.deadline
    }

    pub fn check(&self) -> Result<(), BridgeError> {
        if self
            .cancelled
            .as_ref()
            .is_some_and(|flag| flag.load(Ordering::SeqCst))
        {
            return Err(BridgeError::Cancelled);
        }
        Ok(())
    }
}

impl Default for OpContext {
    fn default() -> Self {
        Self::new(Duration::from_secs(10))
    }
}

#[derive(Clone, Debug)]
struct OperationBudget {
    started: Instant,
    total: Duration,
}

impl OperationBudget {
    fn begin(context: &OpContext) -> Result<Self, BridgeError> {
        context.check()?;
        Ok(Self {
            started: Instant::now(),
            total: context.deadline(),
        })
    }

    fn remaining(&self, context: &OpContext) -> Result<Duration, BridgeError> {
        context.check()?;
        self.total
            .checked_sub(self.started.elapsed())
            .filter(|remaining| !remaining.is_zero())
            .ok_or(BridgeError::DeadlineExceeded)
    }

    fn remaining_after_dispatch(&self, context: &OpContext) -> Result<Duration, BridgeError> {
        self.remaining(context)
            .map_err(|_| BridgeError::MutationOutcomeUnknown)
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum IdfScope {
    ScopedToRetrieval,
}

#[derive(Clone, Debug, PartialEq)]
pub struct ScrollPage {
    pub points: Vec<PointRecord>,
    pub next_offset: Option<QdrantPointId>,
}

include!("real/identity.rs");
include!("real/codec.rs");
include!("real/filter.rs");
include!("real/errors_schema.rs");

pub struct RealDataPlane {
    client: Qdrant,
    gate: QualifiedGate,
    limits: BridgeLimits,
    schemas: BTreeMap<String, CollectionSchema>,
    generations: BTreeMap<String, CollectionGenerationId>,
    operations: BTreeMap<OpaqueId, MutationReceipt>,
}

impl RealDataPlane {
    /// Resolves only the exact process-locally admitted physical route.
    ///
    /// A physical name is not sufficient authority: a collection generation
    /// is part of scoring/currentness identity. Reusing the same name with a
    /// different generation behaves as an unavailable route before any Qdrant
    /// read or mutation.
    fn admitted_schema(
        &self,
        route: &CollectionRoute,
    ) -> Result<(String, &CollectionSchema), BridgeError> {
        let name = collection_name(route)?;
        if self.generations.get(&name) != Some(&route.generation) {
            return Err(BridgeError::CollectionNotFound);
        }
        let schema = self
            .schemas
            .get(&name)
            .ok_or(BridgeError::CollectionNotFound)?;
        Ok((name, schema))
    }
}

include!("real/connect_schema.rs");
mod mutations;
include!("real/queries.rs");
include!("real/ledger.rs");

mod blocking;
pub use blocking::BlockingRealQueryPlane;

#[cfg(test)]
#[path = "real/tests.rs"]
mod tests;

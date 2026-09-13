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
//! names, filters and batches are validated before dispatch.
//!
//! Single-contract retrieval + IDF (invariant 5): [`RealDataPlane::query_filtered`]
//! takes one [`EligibilityFilter`](crate::EligibilityFilter) and renders both
//! the retrieval `filter` and the `idf.corpus` population filter from that
//! same value. [`IdfScope::Global`] omits the corpus (collection-wide IDF);
//! [`IdfScope::ScopedToRetrieval`] clones the retrieval filter as the corpus.
//! A diverged corpus is unrepresentable: there is no second filter argument.
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
use std::time::Duration;

use qdrant_client::Qdrant;
use qdrant_client::qdrant::{
    CollectionInfo, Condition, CountPoints, CreateCollection, CreateFieldIndexCollection,
    DeletePoints, FieldCondition, FieldType, Filter, GetPoints, IdfParams, Match, Modifier,
    PointId, PointStruct, PointsIdsList, PointsSelector, Query, QueryPoints, Range,
    RepeatedStrings, ScrollPoints, SearchParams, SetPayloadPoints, SparseVectorConfig,
    SparseVectorParams, StrictModeConfig, UpdateStatus, UpsertPoints, Value, Vector, VectorInput,
    Vectors, WriteOrdering, WriteOrderingType, condition, r#match, point_id, points_selector,
    value, vector_output, vectors, vectors_output,
};
use search_contracts::{OpaqueId, ReceiptRef};

use crate::live::LiveEndpoint;
use crate::qualified::{QUALIFIED_SERVER_BUILD, QUALIFIED_SERVER_VERSION, QualifiedGate};
use crate::{
    BoundedPointReadback, BridgeError, BridgeLimits, BridgeMutation, CandidateNomination,
    CollectionRoute, CollectionSchema, EligibilityFilter, ExactCount, MutationReceipt,
    PointPayload, PointRecord, QdrantPointId, StoredVector,
};

/// gRPC canonical status numbers (`google.rpc.Code`), matched without a
/// `tonic` dependency: only the stable numeric values travel across the
/// crate boundary, never vendor status text.
const CODE_INVALID_ARGUMENT: i32 = 3;
const CODE_NOT_FOUND: i32 = 5;
const CODE_ALREADY_EXISTS: i32 = 6;
const CODE_PERMISSION_DENIED: i32 = 7;
const CODE_RESOURCE_EXHAUSTED: i32 = 8;
const CODE_FAILED_PRECONDITION: i32 = 9;
const CODE_OUT_OF_RANGE: i32 = 11;
const CODE_UNAUTHENTICATED: i32 = 16;

/// Bounded per-operation context.
///
/// A finite deadline plus an optional cancellation flag. Cancellation is
/// checked before dispatch and between bounded pages/batches; expiry after a
/// mutation dispatch reports `MutationOutcomeUnknown` because the write may
/// have committed.
#[derive(Clone, Debug)]
pub struct OpContext {
    deadline: Duration,
    cancelled: Option<Arc<AtomicBool>>,
}

impl OpContext {
    /// Builds a context with a finite deadline and no cancellation flag.
    #[must_use]
    pub const fn new(deadline: Duration) -> Self {
        Self {
            deadline,
            cancelled: None,
        }
    }

    /// Builds a context with a finite deadline and a shared cancellation flag.
    #[must_use]
    pub const fn with_cancel(deadline: Duration, cancelled: Arc<AtomicBool>) -> Self {
        Self {
            deadline,
            cancelled: Some(cancelled),
        }
    }

    /// Finite per-operation deadline.
    #[must_use]
    pub const fn deadline(&self) -> Duration {
        self.deadline
    }

    /// Fails with [`BridgeError::Cancelled`] when the flag is set.
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

/// Which IDF population a filtered query scores with.
///
/// `Global` omits `idf.corpus` (collection-wide denominators).
/// `ScopedToRetrieval` sets `idf.corpus` to the exact retrieval filter built
/// from the same single contract, so denied documents can never move
/// permitted denominators.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum IdfScope {
    Global,
    ScopedToRetrieval,
}

/// One bounded scroll page with an opaque continuation offset.
#[derive(Clone, Debug, PartialEq)]
pub struct ScrollPage {
    pub points: Vec<PointRecord>,
    pub next_offset: Option<QdrantPointId>,
}

// Keep vendor translation private and physically separated by responsibility.
// `include!` preserves the original module namespace and public paths while
// this no-behavior-change split awaits the dedicated test pass.
include!("real/identity.rs");
include!("real/codec.rs");
include!("real/filter.rs");
include!("real/errors_schema.rs");

/// Real Qdrant data plane over the pinned client transport.
///
/// Owns the vendor client opaquely (the type never appears in public
/// signatures), the schemas it created, and a bounded mutation-identity
/// ledger for exact replay. There is no fallback to the in-memory oracle.
pub struct RealDataPlane {
    client: Qdrant,
    gate: QualifiedGate,
    limits: BridgeLimits,
    schemas: BTreeMap<String, CollectionSchema>,
    operations: BTreeMap<OpaqueId, MutationReceipt>,
}

include!("real/connect_schema.rs");
mod mutations;
include!("real/queries.rs");
include!("real/ledger.rs");
include!("real/tests.rs");

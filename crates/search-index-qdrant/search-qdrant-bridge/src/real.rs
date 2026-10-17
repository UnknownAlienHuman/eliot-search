//! Real T24 data-plane adapter over the pinned `qdrant-client` 1.19.0 transport.
//!
//! The in-memory [`QdrantBridge`](crate::QdrantBridge) stays as the behavioral
//! test oracle only. Operations invoked on [`RealDataPlane`] execute against a
//! Qdrant server through the pinned client; this module does not assert product
//! qualification or select production composition.
//!
//! Transport admission: [`RealDataPlane::connect`] accepts the existing
//! [`QualifiedGate`](crate::qualified::QualifiedGate) and rechecks the live
//! endpoint/server identity. Its caller-supplied probe receipt is not execution
//! proof or product qualification. Exact artifact, client/profile and process
//! admission remain separate and are not established by this bridge slice.
//! Collection names, filters and batches are validated before dispatch.
//!
//! The API key is borrowed only inside the lease-provider callback while the
//! SDK client is built. Qdrant client 1.19.0 retains a `String` copy in its
//! interceptor for the lifetime of that client; the bridge does not claim to
//! zeroize SDK-owned copies. The provider revalidates the current process,
//! owner, endpoint and lease before every RPC. SDK calls have a 10-second
//! transport timeout, with operation contexts imposing their own deadlines.
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
//!
//! The unprefixed methods below retain the earlier bridge profile for explicit
//! compatibility. The canonical S9.5/S10.3 path is exposed separately through
//! `create_indexed_generation`, `verify_indexed_generation`, `query_indexed`,
//! `count_indexed`, `scroll_indexed` and `readback_indexed_exact`. It consumes
//! the shared contracts and performs its own exact collection readback. The
//! qualified transport gate alone is not a canonical schema or product
//! qualification receipt; this path is not selected by product composition in
//! this crate slice. Canonical point mutation is not implemented here yet:
//! accepted S9.5 payloads must not go through the older compatibility
//! upsert/close/delete methods.

use std::collections::{BTreeMap, BTreeSet, HashMap};
use std::fmt;
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
use search_ports::CancellationProbe;

use crate::live::LiveEndpoint;
use crate::qualified::{QUALIFIED_SERVER_BUILD, QUALIFIED_SERVER_VERSION, QualifiedGate};
use crate::{
    BoundedPointReadback, BridgeError, BridgeLimits, BridgeMutation, CandidateNomination,
    CollectionRoute, CollectionSchema, EligibilityFilter, ExactCount, MutationReceipt,
    PointPayload, PointRecord, QdrantApiKeyLease, QdrantConnectionBinding, QdrantPointId,
    StoredVector,
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
/// A finite deadline plus an optional shared cancellation observer. The
/// observer is queried before dispatch and between bounded pages/batches;
/// expiry after a mutation dispatch reports `MutationOutcomeUnknown` because
/// the write may have committed.
#[derive(Clone)]
pub struct OpContext {
    deadline: Duration,
    cancellation: Option<CancellationObserver>,
}

#[derive(Clone)]
enum CancellationObserver {
    Flag(Arc<AtomicBool>),
    Probe(Arc<dyn CancellationProbe + Send + Sync>),
}

impl CancellationObserver {
    fn is_cancelled(&self) -> bool {
        match self {
            Self::Flag(flag) => flag.load(Ordering::SeqCst),
            Self::Probe(probe) => probe.is_cancelled(),
        }
    }
}

impl fmt::Debug for OpContext {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("OpContext")
            .field("deadline", &self.deadline)
            .finish_non_exhaustive()
    }
}

impl OpContext {
    /// Builds a context with a finite deadline and no cancellation observer.
    #[must_use]
    pub const fn new(deadline: Duration) -> Self {
        Self {
            deadline,
            cancellation: None,
        }
    }

    /// Builds a context with a finite deadline and a shared cancellation flag.
    ///
    /// Retained for callers that already use an atomic flag. New callers with
    /// an Eliot cancellation capability should use [`Self::with_cancellation_probe`].
    #[must_use]
    pub const fn with_cancel(deadline: Duration, cancelled: Arc<AtomicBool>) -> Self {
        Self {
            deadline,
            cancellation: Some(CancellationObserver::Flag(cancelled)),
        }
    }

    /// Builds a context retaining the caller's live, read-only cancellation observer.
    #[must_use]
    pub fn with_cancellation_probe(
        deadline: Duration,
        probe: Arc<dyn CancellationProbe + Send + Sync>,
    ) -> Self {
        Self {
            deadline,
            cancellation: Some(CancellationObserver::Probe(probe)),
        }
    }

    /// Finite per-operation deadline.
    #[must_use]
    pub const fn deadline(&self) -> Duration {
        self.deadline
    }

    /// Fails with [`BridgeError::Cancelled`] when its shared observer reports cancellation.
    pub fn check(&self) -> Result<(), BridgeError> {
        if self
            .cancellation
            .as_ref()
            .is_some_and(CancellationObserver::is_cancelled)
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

mod canonical;
pub use canonical::{
    CanonicalCandidate, CanonicalPointReadback, CanonicalReadback, CanonicalRequestContext,
    CanonicalScrollPage,
};
mod canonical_mutation;
pub use canonical_mutation::{
    CanonicalDeleteReadback, CanonicalMutationPointReadback, CanonicalMutationReadback,
    CanonicalPointInput, CanonicalValidityRestoreOutcome,
};

/// Real Qdrant data plane over the pinned client transport.
///
/// Owns the vendor client opaquely (the type never appears in public
/// signatures), the schemas it created, and a bounded mutation-identity
/// ledger for exact replay. There is no fallback to the in-memory oracle.
pub struct RealDataPlane {
    client: Qdrant,
    single_attempt: SingleAttemptTransport,
    auth_lease: QdrantApiKeyLease,
    gate: QualifiedGate,
    limits: BridgeLimits,
    schemas: BTreeMap<String, CollectionSchema>,
    operations: BTreeMap<OpaqueId, MutationReceipt>,
    canonical: canonical::CanonicalState,
}

include!("real/connect_schema.rs");
include!("real/single_attempt.rs");
mod mutations;
include!("real/queries.rs");
include!("real/ledger.rs");

mod blocking;
pub use blocking::BlockingRealQueryPlane;

#[cfg(test)]
#[path = "real/tests.rs"]
mod tests;

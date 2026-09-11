//! Vendor-neutral exact Qdrant data-plane semantics.
//!
//! This package does not discover or start Qdrant. A process supervisor supplies
//! an authenticated endpoint and exact process receipt. The in-memory model here
//! defines capability admission, collection schema, exact point mutation,
//! readback, count, and filtered nomination semantics for a concrete adapter.

#![forbid(unsafe_code)]
#![allow(
    clippy::large_enum_variant,
    clippy::missing_errors_doc,
    clippy::module_name_repetitions,
    clippy::too_many_arguments,
    clippy::too_many_lines
)]

mod admin;
mod api;
mod capability;
mod config;
mod error;
mod mutation;
mod query;
mod readback;
mod schema;

pub use api::QdrantBridge;
pub use capability::{
    AuthLeaseEvidence, BridgeEndpoint, CapabilityProbeResults, ConsistencyGates,
    FilterGates, IndexGates, QdrantCapabilityReceipt, SupervisorReceipt,
    TopologyGates, probe_capabilities,
};
pub use config::BridgeLimits;
pub use error::BridgeError;
pub use mutation::{
    BridgeMutation, MutationReceipt, PointPayload, PointRecord, QdrantPointId,
    StoredVector,
};
pub use query::{CandidateNomination, EligibilityFilter};
pub use readback::{BoundedPointReadback, ExactCount};
pub use schema::{
    CollectionRoute, CollectionSchema, StrictnessFloors, VectorSchema,
};

/// Live T22 qualification path beside the in-memory oracle.
pub mod live;
/// Exact T22 artifact/client/IDF qualification gate.
pub mod qualified;
/// Real T24 data-plane adapter over the pinned `qdrant-client` transport.
///
/// The in-memory [`QdrantBridge`] stays as the behavioral test oracle only.
/// Production data-plane work goes through [`real::RealDataPlane`], which
/// admits only an executed [`qualified::QualifiedGate`] and never falls back
/// to the oracle.
pub mod real;

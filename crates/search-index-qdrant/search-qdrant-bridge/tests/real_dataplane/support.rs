mod assertions;
mod live;
mod model;

pub(crate) use std::collections::{BTreeMap, BTreeSet};
pub(crate) use std::sync::{
    Arc,
    atomic::{AtomicBool, Ordering},
};
pub(crate) use std::time::Duration;

pub(crate) use search_contracts::{
    Blake3Digest32, CollectionGenerationId, Epoch, OpaqueId, OwnerEpoch,
};
pub(crate) use search_qdrant_bridge::live::{
    NATIVE_EXE_PATH, free_loopback_ports, run_qualification_suite,
    spawn_disposable_server,
};
pub(crate) use search_qdrant_bridge::qualified::QualifiedGate;
pub(crate) use search_qdrant_bridge::real::{
    IdfScope, OpContext, RealDataPlane, validate_collection_name,
};
pub(crate) use search_qdrant_bridge::{
    AuthLeaseEvidence, BoundedPointReadback, BridgeEndpoint, BridgeError,
    BridgeLimits, BridgeMutation, CapabilityProbeResults, CollectionRoute,
    CollectionSchema, ConsistencyGates, EligibilityFilter, FilterGates,
    IndexGates, PointPayload, PointRecord, QdrantBridge, QdrantPointId,
    StoredVector, StrictnessFloors, SupervisorReceipt, TopologyGates,
    VectorSchema, probe_capabilities,
};

pub(crate) use assertions::*;
pub(crate) use live::*;
pub(crate) use model::*;

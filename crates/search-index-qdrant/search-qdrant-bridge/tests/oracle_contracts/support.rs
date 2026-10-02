mod bridge;
mod model;

pub(crate) use std::collections::{BTreeMap, BTreeSet};

pub(crate) use search_contracts::{
    AccessPartitionId, Blake3Digest32, CollectionGenerationId, Epoch,
    InstallationIncarnationId, Modality, OpaqueId, OwnerEpoch, ProfileId,
    ProjectionMembershipId, ProjectionProfileSetId, RepresentationId,
    ScoringDocumentId, ScoringPartitionId, SourceId, SourceRevisionId, UnitId,
    UnitKind,
};
pub(crate) use search_qdrant_bridge::{
    AuthLeaseEvidence, BridgeEndpoint, BridgeError, BridgeLimits,
    BridgeMutation, CandidateNomination, CapabilityProbeResults,
    CollectionRoute, CollectionSchema, ConsistencyGates, EligibilityFilter,
    FilterGates, IndexGates, MutationReceipt, PointPayload, PointRecord,
    QdrantBridge, QdrantPointId, StoredVector, StrictnessFloors,
    SupervisorReceipt, TopologyGates, VectorSchema, probe_capabilities,
};

pub(crate) use bridge::*;
pub(crate) use model::*;

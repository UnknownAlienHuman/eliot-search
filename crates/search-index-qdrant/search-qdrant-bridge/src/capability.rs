use search_contracts::{Blake3Digest32, OwnerEpoch};

use crate::BridgeError;

/// Content-minimized authenticated endpoint identity.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct BridgeEndpoint {
    pub endpoint_digest: Blake3Digest32,
    pub loopback_only: bool,
}

/// Exact process/supervisor proof required before connecting.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct SupervisorReceipt {
    pub owner_epoch: OwnerEpoch,
    pub process_identity_digest: Blake3Digest32,
    pub artifact_digest: Blake3Digest32,
    pub endpoint_digest: Blake3Digest32,
}

/// Purpose-bound authentication-lease proof.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct AuthLeaseEvidence {
    pub reference_digest: Blake3Digest32,
    pub purpose_digest: Blake3Digest32,
    pub valid: bool,
}

/// Executed capability probe results, grouped by purpose so each gate struct
/// stays within the boolean-count lint without changing admission semantics.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct TopologyGates {
    pub authenticated_health: bool,
    pub single_shard: bool,
    pub signed_i64_ranges: bool,
}

/// Filter-semantics capability gates.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct FilterGates {
    pub missing_upper_bound_must_not: bool,
    pub sparse_idf: bool,
    pub independent_idf_corpus: bool,
}

/// Index and mutation-durability capability gates.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct IndexGates {
    pub strict_mode: bool,
    pub payload_indexes: bool,
    pub wait_for_mutations: bool,
}

/// Readback and ordering capability gates.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct ConsistencyGates {
    pub strong_ordering: bool,
    pub exact_count_and_readback: bool,
    pub named_sparse_vectors: bool,
}

/// Executed capability probe results.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct CapabilityProbeResults {
    pub topology: TopologyGates,
    pub filters: FilterGates,
    pub indexes: IndexGates,
    pub consistency: ConsistencyGates,
}

impl CapabilityProbeResults {
    #[must_use]
    pub const fn all_required(self) -> bool {
        self.topology.authenticated_health
            && self.topology.single_shard
            && self.topology.signed_i64_ranges
            && self.filters.missing_upper_bound_must_not
            && self.filters.sparse_idf
            && self.filters.independent_idf_corpus
            && self.indexes.strict_mode
            && self.indexes.payload_indexes
            && self.indexes.wait_for_mutations
            && self.consistency.strong_ordering
            && self.consistency.exact_count_and_readback
            && self.consistency.named_sparse_vectors
    }
}

/// Exact capability admission receipt.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct QdrantCapabilityReceipt {
    pub process_identity_digest: Blake3Digest32,
    pub artifact_digest: Blake3Digest32,
    pub probe_manifest_digest: Blake3Digest32,
    pub results: CapabilityProbeResults,
}

/// Verifies every mandatory Qdrant capability probe.
pub const fn probe_capabilities(
    supervisor: SupervisorReceipt,
    probe_manifest_digest: Blake3Digest32,
    results: CapabilityProbeResults,
) -> Result<QdrantCapabilityReceipt, BridgeError> {
    if !results.all_required() {
        return Err(BridgeError::CapabilityProbeFailed);
    }
    Ok(QdrantCapabilityReceipt {
        process_identity_digest: supervisor.process_identity_digest,
        artifact_digest: supervisor.artifact_digest,
        probe_manifest_digest,
        results,
    })
}

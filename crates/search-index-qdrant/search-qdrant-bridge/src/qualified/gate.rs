//! Caller-supplied live-probe declarations and compatibility transport gate.

use super::{
    QUALIFIED_CLIENT_VERSION, QUALIFIED_SERVER_BUILD, QUALIFIED_SERVER_VERSION, QualificationError,
};

/// Bridge-owned mandatory live probes for the qualified profile.
pub const MANDATORY_LIVE_PROBES: [&str; 13] = [
    "live_server_identity",
    "one_shard_topology",
    "payload_index_completeness",
    "strict_unindexed_retrieve_rejected",
    "strict_unindexed_update_rejected",
    "signed_i64_epoch_range",
    "missing_valid_until_open_end",
    "sparse_idf_modifier",
    "independent_idf_population_filter",
    "wait_true_mutation_ack",
    "strong_write_ordering",
    "exact_count_and_readback",
    "schema_digest_equality",
];

/// Caller-declared outcome for one live probe; this record does not prove execution.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct LiveProbeOutcome {
    /// Stable probe identifier.
    pub probe_id: String,
    /// Whether the probe passed.
    pub passed: bool,
    /// Bounded diagnostic detail.
    pub detail: String,
}

/// Caller-supplied live-probe declarations, not a trusted execution artifact.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct LiveProbeReceipt {
    /// Observed server version.
    pub server_version: String,
    /// Observed server build identity.
    pub server_build: String,
    /// Observed client version.
    pub client_version: String,
    /// Collection used by every mandatory probe.
    pub collection: String,
    /// Executed probe outcomes.
    pub outcomes: Vec<LiveProbeOutcome>,
}

/// Compatibility transport token for the live Qdrant path.
///
/// This token only validates identity strings and probe outcomes stated in a
/// caller-supplied [`LiveProbeReceipt`]. It is caller-mintable and is not proof
/// that probes ran, nor artifact, client/profile, process or product
/// qualification.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct QualifiedGate {
    server_version: String,
    collection: String,
}

impl QualifiedGate {
    /// Validates the caller-supplied declarations for the compatibility transport path.
    ///
    /// # Errors
    ///
    /// Returns [`QualificationError::LiveIdentityMismatch`] for identity drift
    /// or [`QualificationError::MandatoryProbeFailed`] for missing/failed probes.
    pub fn admit(receipt: &LiveProbeReceipt) -> Result<Self, QualificationError> {
        if receipt.server_version != QUALIFIED_SERVER_VERSION
            || receipt.server_build != QUALIFIED_SERVER_BUILD
            || receipt.client_version != QUALIFIED_CLIENT_VERSION
        {
            return Err(QualificationError::LiveIdentityMismatch);
        }
        for required in MANDATORY_LIVE_PROBES {
            let passed = receipt
                .outcomes
                .iter()
                .any(|outcome| outcome.probe_id == required && outcome.passed);
            if !passed {
                return Err(QualificationError::MandatoryProbeFailed);
            }
        }
        Ok(Self {
            server_version: receipt.server_version.clone(),
            collection: receipt.collection.clone(),
        })
    }

    /// Collection the gate was executed against.
    #[must_use]
    pub fn collection(&self) -> &str {
        &self.collection
    }

    /// Admitted server version.
    #[must_use]
    pub fn server_version(&self) -> &str {
        &self.server_version
    }
}

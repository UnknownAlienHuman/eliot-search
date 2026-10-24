//! Live-probe outcomes used by focused behavior tests.

use super::{
    QUALIFIED_CLIENT_VERSION, QUALIFIED_SERVER_BUILD, QUALIFIED_SERVER_VERSION,
    QualificationError,
};

/// Bridge-owned live cases for the selected profile.
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

/// One executed live-probe outcome.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct LiveProbeOutcome {
    /// Stable probe identifier.
    pub probe_id: String,
    /// Whether the probe passed.
    pub passed: bool,
    /// Bounded diagnostic detail.
    pub detail: String,
}

/// Executed live-probe report.
///
/// This is test or diagnostic output. It is not a runtime credential and is
/// never required by `RealDataPlane::connect`.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct LiveProbeReceipt {
    /// Observed server version.
    pub server_version: String,
    /// Observed server build identity.
    pub server_build: String,
    /// Observed client version.
    pub client_version: String,
    /// Collection used by every live case.
    pub collection: String,
    /// Executed probe outcomes.
    pub outcomes: Vec<LiveProbeOutcome>,
}

/// Validates one executed live report for tests or diagnostics.
///
/// The return value is deliberately `()`: callers cannot retain it as an
/// activation token or use it to make an unrelated runtime ready.
///
/// # Errors
///
/// Returns [`QualificationError::LiveIdentityMismatch`] for identity drift or
/// [`QualificationError::MandatoryProbeFailed`] for missing/failed cases.
pub fn validate_live_probe_receipt(
    receipt: &LiveProbeReceipt,
) -> Result<(), QualificationError> {
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
    Ok(())
}

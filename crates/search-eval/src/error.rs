//! Closed evaluation failures.

use core::fmt;

/// Failure returned by deterministic evaluation, audit, or acceptance logic.
#[derive(Clone, Copy, Debug, Eq, Ord, PartialEq, PartialOrd)]
pub enum EvalError {
    /// A finite limit is zero or internally contradictory.
    InvalidLimits,
    /// The control corpus manifest is malformed.
    CorpusInvalid,
    /// Mandatory case families or topology fixtures are absent.
    CorpusIncomplete,
    /// A case identity appears more than once.
    DuplicateCase,
    /// Fewer than eight independent reference lineages are present.
    InsufficientLineages,
    /// Oracle-private information entered candidate-visible state.
    OracleContamination,
    /// The metric registry is malformed.
    MetricRegistryInvalid,
    /// A metric identity appears more than once.
    DuplicateMetric,
    /// The acceptance policy is malformed or refers to unknown metrics.
    AcceptancePolicyInvalid,
    /// Acceptance criteria were registered after candidate evidence existed.
    PolicyNotPreregistered,
    /// The accepting reviewer is absent, unapproved, or not independent.
    IndependentReviewRequired,
    /// The frozen run manifest is incomplete or internally inconsistent.
    FrozenRunInvalid,
    /// A baseline lacks an exact qualified artifact, driver, or scope binding.
    BaselineUnqualified,
    /// A case block is empty, over budget, or not deterministically bound.
    CaseBlockInvalid,
    /// Evidence does not match the frozen run, case, baseline, or scope.
    EvidenceBindingMismatch,
    /// Evidence timing, terminal state, or raw-output state is contradictory.
    EvidenceStatusInvalid,
    /// One attempt identity was reused with another payload.
    AttemptConflict,
    /// Required raw immutable evidence is missing.
    RawEvidenceMissing,
    /// A metric cannot be computed under its registered missing-value policy.
    MetricUnavailable,
    /// Aggregated evidence mixes incompatible runs, baselines, or lanes.
    AggregateIdentityMismatch,
    /// A finite evidence, metric, sample, or report ceiling was exceeded.
    BudgetExceeded,
    /// A hard source, query, secret, token, or path canary was detected.
    LeakageDetected,
    /// Source-admission audit did not prove the required deny behavior.
    AdmissionAuditFailed,
    /// One or more mandatory fault-recovery cells are absent or unresolved.
    FaultMatrixIncomplete,
    /// Protocol stress violated framing, replay, flow-control, or cleanup rules.
    ProtocolStressFailed,
    /// Resource samples are discontinuous or cannot support the requested claim.
    ResourceReportIncomplete,
    /// Mandatory Product Pulse inputs are missing or mutually inconsistent.
    ProductReportIncomplete,
    /// A zero-tolerance correctness, safety, or reproducibility blocker exists.
    HardBlockerPresent,
    /// A producer attempted to accept its own evidence or report.
    SelfAcceptanceForbidden,
    /// A receipt does not bind the exact report, policy, review, or evidence set.
    ReceiptMismatch,
    /// A shared counter, revision, or deterministic index cannot advance.
    ContractExhausted,
}

impl EvalError {
    /// Stable machine-readable reason code.
    #[must_use]
    pub const fn code(self) -> &'static str {
        match self {
            Self::InvalidLimits => "EVALUATION_LIMITS_INVALID",
            Self::CorpusInvalid => "EVALUATION_FIXTURE_INVALID",
            Self::CorpusIncomplete => "EVALUATION_CORPUS_INCOMPLETE",
            Self::DuplicateCase => "EVALUATION_CASE_DUPLICATE",
            Self::InsufficientLineages => "EVALUATION_REFERENCE_LINEAGES_INSUFFICIENT",
            Self::OracleContamination => "EVALUATION_ORACLE_CONTAMINATION",
            Self::MetricRegistryInvalid => "EVALUATION_METRIC_REGISTRY_INVALID",
            Self::DuplicateMetric => "EVALUATION_METRIC_DUPLICATE",
            Self::AcceptancePolicyInvalid => "EVALUATION_ACCEPTANCE_POLICY_INVALID",
            Self::PolicyNotPreregistered => "EVALUATION_POLICY_NOT_PREREGISTERED",
            Self::IndependentReviewRequired => "EVALUATION_INDEPENDENT_REVIEW_REQUIRED",
            Self::FrozenRunInvalid => "EVALUATION_RUN_MANIFEST_INVALID",
            Self::BaselineUnqualified => "EVALUATION_BASELINE_UNQUALIFIED",
            Self::CaseBlockInvalid => "EVALUATION_CASE_BLOCK_INVALID",
            Self::EvidenceBindingMismatch => "EVALUATION_SCOPE_MISMATCH",
            Self::EvidenceStatusInvalid => "EVALUATION_EVIDENCE_STATUS_INVALID",
            Self::AttemptConflict => "EVALUATION_ATTEMPT_CONFLICT",
            Self::RawEvidenceMissing => "EVALUATION_RAW_EVIDENCE_MISSING",
            Self::MetricUnavailable => "EVALUATION_METRIC_UNAVAILABLE",
            Self::AggregateIdentityMismatch => "EVALUATION_AGGREGATE_IDENTITY_MISMATCH",
            Self::BudgetExceeded => "EVALUATION_BUDGET_EXHAUSTED",
            Self::LeakageDetected => "LEAKAGE_DETECTED",
            Self::AdmissionAuditFailed => "SOURCE_ADMISSION_AUDIT_FAILED",
            Self::FaultMatrixIncomplete => "FAULT_MATRIX_INCOMPLETE",
            Self::ProtocolStressFailed => "PROTOCOL_STRESS_FAILED",
            Self::ResourceReportIncomplete => "RESOURCE_REPORT_INCOMPLETE",
            Self::ProductReportIncomplete => "EVALUATION_INCOMPLETE",
            Self::HardBlockerPresent => "PRODUCT_HARD_BLOCKER_PRESENT",
            Self::SelfAcceptanceForbidden => "SELF_ACCEPTANCE_FORBIDDEN",
            Self::ReceiptMismatch => "EVALUATION_RECEIPT_MISMATCH",
            Self::ContractExhausted => "EVALUATION_CONTRACT_EXHAUSTED",
        }
    }
}

impl fmt::Display for EvalError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(self.code())
    }
}

impl std::error::Error for EvalError {}

//! Product Pulse assembly inputs and complete report model.

use search_contracts::{Blake3Digest32, OpaqueId, ReceiptRef};

use crate::{
    AdmissionAudit, BaselineComparison, BaselineMetricReport, FaultMatrixReport,
    HardBlocker, LeakageAudit, ProtocolStressReport, ReproducibilityReport,
    ResourceReport, SloReport, ValidatedProbeEvidence,
};

use super::coverage::CaseCoverageReport;

/// All already-validated inputs required for one Product Pulse report.
#[derive(Clone, Debug, PartialEq)]
pub struct ProductPulseInputs {
    /// A/B/C metric reports.
    pub metric_reports: Vec<BaselineMetricReport>,
    /// Exact candidate C identity.
    pub candidate_id: OpaqueId,
    /// Preregistered A/B/C comparison.
    pub comparison: BaselineComparison,
    /// Candidate C SLO report.
    pub candidate_slos: SloReport,
    /// Candidate C resource report.
    pub candidate_resources: ResourceReport,
    /// Frozen case coverage.
    pub case_coverage: CaseCoverageReport,
    /// Zero-tolerance leakage audit.
    pub leakage: LeakageAudit,
    /// Unsafe-source admission audit.
    pub admission: AdmissionAudit,
    /// Mandatory fault-recovery matrix.
    pub faults: FaultMatrixReport,
    /// Protocol stress report.
    pub protocol: ProtocolStressReport,
    /// Repeated-run reproducibility report.
    pub reproducibility: ReproducibilityReport,
    /// Additional mandatory/optional external probes.
    pub probes: Vec<ValidatedProbeEvidence>,
    /// Content-free assembly receipt.
    pub assembly_receipt: ReceiptRef,
}

/// Complete Product Pulse report before independent acceptance.
#[derive(Clone, Debug, PartialEq)]
pub struct ProductPulseReport {
    /// Exact frozen run digest.
    pub run_digest: Blake3Digest32,
    /// Exact candidate C identity.
    pub candidate_id: OpaqueId,
    /// A/B/C metric reports.
    pub metric_reports: Vec<BaselineMetricReport>,
    /// A/B/C relationship.
    pub comparison: BaselineComparison,
    /// Candidate SLOs.
    pub candidate_slos: SloReport,
    /// Candidate resources.
    pub candidate_resources: ResourceReport,
    /// Frozen case coverage.
    pub case_coverage: CaseCoverageReport,
    /// Leakage audit.
    pub leakage: LeakageAudit,
    /// Source-admission audit.
    pub admission: AdmissionAudit,
    /// Fault matrix.
    pub faults: FaultMatrixReport,
    /// Protocol stress.
    pub protocol: ProtocolStressReport,
    /// Reproducibility.
    pub reproducibility: ReproducibilityReport,
    /// Additional external probes.
    pub probes: Vec<ValidatedProbeEvidence>,
    /// Every zero-tolerance blocker, never averaged into a score.
    pub hard_blockers: Vec<HardBlocker>,
    /// Whether every mandatory evidence section is complete.
    pub complete: bool,
    /// Deterministic report digest.
    pub report_digest: Blake3Digest32,
    /// Content-free assembly receipt.
    pub assembly_receipt: ReceiptRef,
}

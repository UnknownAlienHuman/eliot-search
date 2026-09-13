//! Immutable external fault, security, and protocol probe evidence.

use search_contracts::{Blake3Digest32, OpaqueId, ReceiptRef};

use crate::{EvalError, FrozenRunManifest};

/// Terminal external probe state.
#[derive(Clone, Copy, Debug, Eq, Ord, PartialEq, PartialOrd)]
pub enum ProbeStatus {
    /// Probe passed with complete immutable evidence.
    Pass,
    /// Probe failed.
    Fail,
    /// Probe could not run or produce usable evidence.
    Unavailable,
}

/// Immutable external fault/security/protocol probe.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ProbeEvidence {
    /// Stable probe identity.
    pub probe_id: OpaqueId,
    /// Exact frozen run digest.
    pub run_digest: Blake3Digest32,
    /// Terminal probe status.
    pub status: ProbeStatus,
    /// Whether the probe is mandatory for acceptance.
    pub mandatory: bool,
    /// Whether raw evidence is immutable.
    pub immutable: bool,
    /// Producer identity.
    pub producer_id: OpaqueId,
    /// Independent reviewer identity when reviewed.
    pub reviewer_id: Option<OpaqueId>,
    /// Immutable raw evidence reference.
    pub raw_evidence_ref: Option<ReceiptRef>,
    /// Digest of exact probe output.
    pub output_digest: Option<Blake3Digest32>,
}

/// Probe that passed identity, evidence, and independent-review checks.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ValidatedProbeEvidence(ProbeEvidence);

impl ValidatedProbeEvidence {
    /// Exact accepted probe evidence.
    #[must_use]
    pub const fn evidence(&self) -> &ProbeEvidence {
        &self.0
    }
}

/// Ingests one external probe without relabeling FAIL/UNAVAILABLE as PASS.
pub fn ingest_external_probe(
    probe: ProbeEvidence,
    run: &FrozenRunManifest,
) -> Result<ValidatedProbeEvidence, EvalError> {
    if probe.run_digest != run.run_digest() {
        return Err(EvalError::EvidenceBindingMismatch);
    }
    if probe.status == ProbeStatus::Pass {
        let reviewer = probe
            .reviewer_id
            .as_ref()
            .ok_or(EvalError::IndependentReviewRequired)?;
        if !probe.immutable
            || reviewer == &probe.producer_id
            || probe.raw_evidence_ref.is_none()
            || probe.output_digest.is_none()
        {
            return Err(if reviewer == &probe.producer_id {
                EvalError::SelfAcceptanceForbidden
            } else {
                EvalError::RawEvidenceMissing
            });
        }
    }
    Ok(ValidatedProbeEvidence(probe))
}

//! Repeated frozen-input reproducibility audit.

use std::collections::{BTreeMap, BTreeSet};

use search_contracts::{Blake3Digest32, OpaqueId, ReceiptRef};

use crate::{EvalError, EvalLimits, FingerprintBuilder, FrozenRunManifest};

use super::common::{HardBlocker, HardBlockerClass, bounded_reason};

/// Repeated-run digest observation.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ReproducibilityObservation {
    /// Exact baseline/candidate identity.
    pub baseline_id: OpaqueId,
    /// Repetition identity.
    pub repetition_id: OpaqueId,
    /// Frozen input digest.
    pub input_digest: Blake3Digest32,
    /// Deterministic output digest.
    pub output_digest: Blake3Digest32,
    /// Whether nondeterminism was explicitly expected by the registered profile.
    pub nondeterminism_expected: bool,
    /// Immutable raw evidence.
    pub evidence_ref: ReceiptRef,
}

/// Reproducibility report.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ReproducibilityReport {
    /// Exact frozen run digest.
    pub run_digest: Blake3Digest32,
    /// Canonical observations.
    pub observations: Vec<ReproducibilityObservation>,
    /// Hard blockers for unexpected output drift.
    pub blockers: Vec<HardBlocker>,
    /// Whether every deterministic profile reproduced exactly.
    pub passed: bool,
    /// Deterministic report digest.
    pub report_digest: Blake3Digest32,
}

/// Audits repeated deterministic runs without hiding divergent output digests.
pub fn audit_reproducibility(
    run: &FrozenRunManifest,
    mut observations: Vec<ReproducibilityObservation>,
    limits: EvalLimits,
) -> Result<ReproducibilityReport, EvalError> {
    let limits = limits.validate()?;
    if observations.is_empty() || observations.len() > limits.max_audit_items {
        return Err(EvalError::ProductReportIncomplete);
    }
    observations.sort_by(|left, right| {
        (&left.baseline_id, &left.repetition_id).cmp(&(
            &right.baseline_id,
            &right.repetition_id,
        ))
    });
    let mut seen = BTreeSet::new();
    let mut expected: BTreeMap<(OpaqueId, Blake3Digest32), Blake3Digest32> = BTreeMap::new();
    let mut blockers = Vec::new();
    for observation in &observations {
        if observation.evidence_ref.as_str().is_empty()
            || !seen.insert((
                observation.baseline_id.clone(),
                observation.repetition_id.clone(),
            ))
        {
            return Err(EvalError::EvidenceBindingMismatch);
        }
        let key = (observation.baseline_id.clone(), observation.input_digest);
        match expected.get(&key) {
            Some(first)
                if first != &observation.output_digest
                    && !observation.nondeterminism_expected =>
            {
                blockers.push(HardBlocker {
                    class: HardBlockerClass::Reproducibility,
                    check_id: observation.repetition_id.clone(),
                    reason: bounded_reason("UNEXPECTED_OUTPUT_DRIFT")?,
                    evidence_ref: observation.evidence_ref.clone(),
                });
            }
            Some(_) => {}
            None => {
                expected.insert(key, observation.output_digest);
            }
        }
    }
    let mut fingerprint = FingerprintBuilder::new(b"eliot-search/eval/reproducibility/v1");
    fingerprint.push_digest(run.run_digest());
    for observation in &observations {
        fingerprint.push_text(observation.baseline_id.as_str());
        fingerprint.push_text(observation.repetition_id.as_str());
        fingerprint.push_digest(observation.input_digest);
        fingerprint.push_digest(observation.output_digest);
        fingerprint.push_bool(observation.nondeterminism_expected);
    }
    Ok(ReproducibilityReport {
        run_digest: run.run_digest(),
        observations,
        passed: blockers.is_empty(),
        blockers,
        report_digest: fingerprint.finish(),
    })
}

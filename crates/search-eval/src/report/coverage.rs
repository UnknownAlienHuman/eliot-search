//! Exact measured case coverage over the frozen A/B/C denominator.

use std::collections::{BTreeMap, BTreeSet};

use search_contracts::{Blake3Digest32, OpaqueId};

use crate::{
    EvalError, FingerprintBuilder, FrozenRunManifest, ValidatedCaseEvidence,
    ValidatedControlCorpus,
};

/// Exact measured case coverage for all three A/B/C identities.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct CaseCoverageReport {
    /// Exact frozen run digest.
    pub run_digest: Blake3Digest32,
    /// Number of cases in the frozen denominator.
    pub expected_cases: usize,
    /// Measured distinct cases by baseline identity.
    pub measured_cases: BTreeMap<OpaqueId, usize>,
    /// Missing case identities by baseline identity.
    pub missing_cases: BTreeMap<OpaqueId, BTreeSet<OpaqueId>>,
    /// Whether every expected case has at least one measured terminal attempt for A/B/C.
    pub complete: bool,
    /// Deterministic coverage digest.
    pub coverage_digest: Blake3Digest32,
}

/// Audits measured case coverage against the frozen control-corpus denominator.
pub fn audit_case_coverage(
    run: &FrozenRunManifest,
    corpus: &ValidatedControlCorpus,
    baseline_ids: &BTreeSet<OpaqueId>,
    evidence: &[ValidatedCaseEvidence],
) -> Result<CaseCoverageReport, EvalError> {
    if baseline_ids.len() != 3 {
        return Err(EvalError::ProductReportIncomplete);
    }
    let expected = corpus
        .manifest()
        .cases
        .iter()
        .map(|case| case.case_id.clone())
        .collect::<BTreeSet<_>>();
    let mut observed: BTreeMap<OpaqueId, BTreeSet<OpaqueId>> = baseline_ids
        .iter()
        .map(|baseline| (baseline.clone(), BTreeSet::new()))
        .collect();
    for item in evidence {
        let attempt = item.evidence();
        if attempt.warmup {
            continue;
        }
        if attempt.run_digest != run.run_digest()
            || !baseline_ids.contains(&attempt.baseline_id)
            || !expected.contains(&attempt.case_id)
        {
            return Err(EvalError::EvidenceBindingMismatch);
        }
        if attempt.terminal_events != 1 {
            return Err(EvalError::EvidenceStatusInvalid);
        }
        observed
            .get_mut(&attempt.baseline_id)
            .ok_or(EvalError::EvidenceBindingMismatch)?
            .insert(attempt.case_id.clone());
    }
    let measured_cases = observed
        .iter()
        .map(|(baseline, cases)| (baseline.clone(), cases.len()))
        .collect::<BTreeMap<_, _>>();
    let missing_cases = observed
        .iter()
        .map(|(baseline, cases)| {
            (
                baseline.clone(),
                expected.difference(cases).cloned().collect::<BTreeSet<_>>(),
            )
        })
        .collect::<BTreeMap<_, _>>();
    let complete = missing_cases.values().all(BTreeSet::is_empty);
    let mut fingerprint = FingerprintBuilder::new(b"eliot-search/eval/case-coverage/v1");
    fingerprint.push_digest(run.run_digest());
    fingerprint.push_u64(u64::try_from(expected.len()).unwrap_or(u64::MAX));
    for (baseline, missing) in &missing_cases {
        fingerprint.push_text(baseline.as_str());
        for case in missing {
            fingerprint.push_text(case.as_str());
        }
    }
    Ok(CaseCoverageReport {
        run_digest: run.run_digest(),
        expected_cases: expected.len(),
        measured_cases,
        missing_cases,
        complete,
        coverage_digest: fingerprint.finish(),
    })
}

//! Warm-up/measured resource-lane aggregation.

use std::collections::BTreeSet;

use search_contracts::{Blake3Digest32, OpaqueId, ReceiptRef};

use crate::{
    AttemptStatus, EvalError, EvalLimits, FingerprintBuilder, FrozenRunManifest,
    ResourceSample, ValidatedCaseEvidence,
};

use super::support::{canonicalize_receipts, resource_lane_tag};

/// Resource lane kept separate during aggregation.
#[derive(Clone, Copy, Debug, Eq, Ord, PartialEq, PartialOrd)]
pub enum ResourceLane {
    /// Excluded warm-up attempts.
    Warmup,
    /// Measured attempts.
    Measured,
}

/// Deterministic resource report for one baseline and lane.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ResourceReport {
    /// Frozen run digest.
    pub run_digest: Blake3Digest32,
    /// Baseline/candidate identity.
    pub baseline_id: OpaqueId,
    /// Warm-up or measured lane.
    pub lane: ResourceLane,
    /// Number of represented attempts.
    pub attempt_count: usize,
    /// Number of resource samples.
    pub sample_count: usize,
    /// Sum of per-attempt CPU deltas.
    pub cpu_millis: u64,
    /// Maximum observed resident-memory bytes.
    pub peak_memory_bytes: u64,
    /// Sum of per-attempt read-byte deltas.
    pub read_bytes: u64,
    /// Sum of per-attempt write-byte deltas.
    pub write_bytes: u64,
    /// Number of attempts without a usable sample series.
    pub missing_sample_attempts: usize,
    /// Whether every selected attempt succeeded with a coherent sample series.
    pub complete: bool,
    /// Immutable attempt evidence references.
    pub evidence_refs: Vec<ReceiptRef>,
    /// Deterministic report digest.
    pub report_digest: Blake3Digest32,
}

/// Computes the measured-lane resource report.
pub fn compute_resource_report(
    run: &FrozenRunManifest,
    baseline_id: &OpaqueId,
    evidence: &[ValidatedCaseEvidence],
    limits: EvalLimits,
) -> Result<ResourceReport, EvalError> {
    compute_resource_report_for_lane(run, baseline_id, evidence, ResourceLane::Measured, limits)
}

/// Computes one exact warm-up or measured resource lane.
pub fn compute_resource_report_for_lane(
    run: &FrozenRunManifest,
    baseline_id: &OpaqueId,
    evidence: &[ValidatedCaseEvidence],
    lane: ResourceLane,
    limits: EvalLimits,
) -> Result<ResourceReport, EvalError> {
    let limits = limits.validate()?;
    if evidence.len() > limits.max_cases.saturating_mul(3) {
        return Err(EvalError::BudgetExceeded);
    }
    let selected = evidence
        .iter()
        .filter(|item| {
            item.evidence().warmup == (lane == ResourceLane::Warmup)
                && &item.evidence().baseline_id == baseline_id
        })
        .collect::<Vec<_>>();
    if selected.is_empty() {
        return Err(EvalError::ResourceReportIncomplete);
    }

    let mut sample_count = 0_usize;
    let mut cpu_millis = 0_u64;
    let mut peak_memory_bytes = 0_u64;
    let mut read_bytes = 0_u64;
    let mut write_bytes = 0_u64;
    let mut missing_sample_attempts = 0_usize;
    let mut complete = true;
    let mut attempt_ids = BTreeSet::new();
    let mut evidence_refs = Vec::new();

    for item in &selected {
        let attempt = item.evidence();
        if attempt.run_digest != run.run_digest()
            || &attempt.baseline_id != baseline_id
            || !attempt_ids.insert(attempt.attempt_digest)
        {
            return Err(EvalError::AggregateIdentityMismatch);
        }
        evidence_refs.push(attempt.invocation_receipt.clone());
        complete &= attempt.status == AttemptStatus::Success;
        if attempt.resource_samples.is_empty() {
            missing_sample_attempts = missing_sample_attempts.saturating_add(1);
            complete = false;
            continue;
        }
        sample_count = sample_count
            .checked_add(attempt.resource_samples.len())
            .ok_or(EvalError::BudgetExceeded)?;
        if sample_count > limits.max_resource_samples {
            return Err(EvalError::BudgetExceeded);
        }
        let summary = summarize_samples(&attempt.resource_samples)?;
        cpu_millis = cpu_millis
            .checked_add(summary.cpu_millis)
            .ok_or(EvalError::ContractExhausted)?;
        read_bytes = read_bytes
            .checked_add(summary.read_bytes)
            .ok_or(EvalError::ContractExhausted)?;
        write_bytes = write_bytes
            .checked_add(summary.write_bytes)
            .ok_or(EvalError::ContractExhausted)?;
        peak_memory_bytes = peak_memory_bytes.max(summary.peak_memory_bytes);
    }
    canonicalize_receipts(&mut evidence_refs, limits.max_receipts)?;

    let mut fingerprint = FingerprintBuilder::new(b"eliot-search/eval/resource-report/v1");
    fingerprint.push_digest(run.run_digest());
    fingerprint.push_text(baseline_id.as_str());
    fingerprint.push_u64(resource_lane_tag(lane));
    fingerprint.push_u64(u64::try_from(selected.len()).map_err(|_| EvalError::ContractExhausted)?);
    fingerprint.push_u64(u64::try_from(sample_count).map_err(|_| EvalError::ContractExhausted)?);
    fingerprint.push_u64(cpu_millis);
    fingerprint.push_u64(peak_memory_bytes);
    fingerprint.push_u64(read_bytes);
    fingerprint.push_u64(write_bytes);
    fingerprint.push_u64(
        u64::try_from(missing_sample_attempts).map_err(|_| EvalError::ContractExhausted)?,
    );
    for reference in &evidence_refs {
        fingerprint.push_text(reference.as_str());
    }
    fingerprint.push_bool(complete);

    Ok(ResourceReport {
        run_digest: run.run_digest(),
        baseline_id: baseline_id.clone(),
        lane,
        attempt_count: selected.len(),
        sample_count,
        cpu_millis,
        peak_memory_bytes,
        read_bytes,
        write_bytes,
        missing_sample_attempts,
        complete,
        evidence_refs,
        report_digest: fingerprint.finish(),
    })
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
struct ResourceSummary {
    cpu_millis: u64,
    peak_memory_bytes: u64,
    read_bytes: u64,
    write_bytes: u64,
}

fn summarize_samples(samples: &[ResourceSample]) -> Result<ResourceSummary, EvalError> {
    let first = samples.first().ok_or(EvalError::ResourceReportIncomplete)?;
    let last = samples.last().ok_or(EvalError::ResourceReportIncomplete)?;
    if samples.windows(2).any(|pair| {
        pair[0].tick >= pair[1].tick
            || pair[0].cpu_millis > pair[1].cpu_millis
            || pair[0].read_bytes > pair[1].read_bytes
            || pair[0].write_bytes > pair[1].write_bytes
    }) {
        return Err(EvalError::ResourceReportIncomplete);
    }
    Ok(ResourceSummary {
        cpu_millis: last
            .cpu_millis
            .checked_sub(first.cpu_millis)
            .ok_or(EvalError::ResourceReportIncomplete)?,
        peak_memory_bytes: samples
            .iter()
            .map(|sample| sample.memory_bytes)
            .max()
            .unwrap_or(0),
        read_bytes: last
            .read_bytes
            .checked_sub(first.read_bytes)
            .ok_or(EvalError::ResourceReportIncomplete)?,
        write_bytes: last
            .write_bytes
            .checked_sub(first.write_bytes)
            .ok_or(EvalError::ResourceReportIncomplete)?,
    })
}

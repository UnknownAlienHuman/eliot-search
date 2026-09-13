//! Immutable case-attempt evidence and exact frozen-binding validation.

use search_contracts::{Blake3Digest32, OpaqueId, ReceiptRef};

use crate::{
    ControlCase, EvalError, EvalLimits, FingerprintBuilder, FrozenRunManifest,
    ValidatedBaseline,
};

use super::schedule::ScheduledAttempt;

/// Terminal state of one attempt.
#[derive(Clone, Copy, Debug, Eq, Ord, PartialEq, PartialOrd)]
pub enum AttemptStatus {
    /// Declared operation completed successfully.
    Success,
    /// Operation produced explicit partial coverage.
    Partial,
    /// Operation failed before a verified result.
    Failed,
    /// Operation was cancelled.
    Cancelled,
    /// Finite deadline elapsed.
    TimedOut,
    /// Baseline/capability was not available for the declared scope.
    Unavailable,
}

/// One monotone process resource sample.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct ResourceSample {
    /// Process-monotonic sample tick.
    pub tick: u64,
    /// Cumulative CPU milliseconds.
    pub cpu_millis: u64,
    /// Resident memory bytes at the sample.
    pub memory_bytes: u64,
    /// Cumulative bytes read.
    pub read_bytes: u64,
    /// Cumulative bytes written.
    pub write_bytes: u64,
}

/// Immutable raw execution evidence for one scheduled attempt.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct CaseExecutionEvidence {
    /// Exact attempt identity from the case block.
    pub attempt_digest: Blake3Digest32,
    /// Exact frozen run digest.
    pub run_digest: Blake3Digest32,
    /// Exact case identity.
    pub case_id: OpaqueId,
    /// Exact baseline identity.
    pub baseline_id: OpaqueId,
    /// Attempt ordinal.
    pub attempt_ordinal: u32,
    /// Warm-up/measured lane.
    pub warmup: bool,
    /// Terminal attempt state.
    pub status: AttemptStatus,
    /// Process-monotonic invocation start tick.
    pub started_tick: u64,
    /// Process-monotonic terminal observation tick.
    pub ended_tick: u64,
    /// Exact declared scope digest.
    pub scope_digest: Blake3Digest32,
    /// Exact source/view digest.
    pub source_view_digest: Blake3Digest32,
    /// Digest of exact raw output when output exists.
    pub output_digest: Option<Blake3Digest32>,
    /// Immutable raw-output object receipt/reference.
    pub raw_output_ref: Option<ReceiptRef>,
    /// Bounded resource samples in monotone tick order.
    pub resource_samples: Vec<ResourceSample>,
    /// Digest of the disclosure classification and sanitization decision.
    pub disclosure_digest: Blake3Digest32,
    /// Number of terminal protocol events observed.
    pub terminal_events: u32,
    /// Content-free invocation receipt.
    pub invocation_receipt: ReceiptRef,
}

/// Attempt evidence that passed exact frozen bindings and status checks.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ValidatedCaseEvidence {
    evidence: CaseExecutionEvidence,
    validation_digest: Blake3Digest32,
}

impl ValidatedCaseEvidence {
    /// Exact accepted evidence.
    #[must_use]
    pub const fn evidence(&self) -> &CaseExecutionEvidence {
        &self.evidence
    }

    /// Deterministic evidence-validation digest.
    #[must_use]
    pub const fn validation_digest(&self) -> Blake3Digest32 {
        self.validation_digest
    }
}

/// Validates exact run/case/baseline/attempt identity and immutable raw evidence.
pub fn validate_case_evidence(
    evidence: CaseExecutionEvidence,
    scheduled: &ScheduledAttempt,
    case: &ControlCase,
    baseline: &ValidatedBaseline,
    run: &FrozenRunManifest,
    limits: EvalLimits,
) -> Result<ValidatedCaseEvidence, EvalError> {
    let limits = limits.validate()?;
    let descriptor = baseline.descriptor();
    if evidence.attempt_digest != scheduled.attempt_digest
        || evidence.run_digest != run.run_digest()
        || evidence.case_id != scheduled.case_id
        || evidence.case_id != case.case_id
        || evidence.baseline_id != scheduled.baseline_id
        || evidence.baseline_id != descriptor.baseline_id
        || evidence.attempt_ordinal != scheduled.attempt_ordinal
        || evidence.warmup != scheduled.warmup
        || evidence.scope_digest != descriptor.scope_digest
        || evidence.source_view_digest != run.input().source_view_digest
    {
        return Err(EvalError::EvidenceBindingMismatch);
    }
    if evidence.resource_samples.len() > limits.max_resource_samples
        || evidence.invocation_receipt.as_str().is_empty()
        || evidence.terminal_events != 1
        || evidence.ended_tick < evidence.started_tick
    {
        return Err(EvalError::EvidenceStatusInvalid);
    }
    validate_samples(
        &evidence.resource_samples,
        evidence.started_tick,
        evidence.ended_tick,
    )?;
    match evidence.status {
        AttemptStatus::Success | AttemptStatus::Partial => {
            if evidence.output_digest.is_none() || evidence.raw_output_ref.is_none() {
                return Err(EvalError::RawEvidenceMissing);
            }
        }
        AttemptStatus::Failed | AttemptStatus::Cancelled | AttemptStatus::TimedOut => {
            if evidence.raw_output_ref.is_none() {
                return Err(EvalError::RawEvidenceMissing);
            }
        }
        AttemptStatus::Unavailable => {
            if evidence.started_tick != evidence.ended_tick || evidence.output_digest.is_some() {
                return Err(EvalError::EvidenceStatusInvalid);
            }
        }
    }
    if evidence
        .raw_output_ref
        .as_ref()
        .is_some_and(|reference| reference.as_str().is_empty())
    {
        return Err(EvalError::RawEvidenceMissing);
    }

    let mut fingerprint = FingerprintBuilder::new(b"eliot-search/eval/case-evidence/v1");
    fingerprint.push_digest(evidence.attempt_digest);
    fingerprint.push_digest(evidence.run_digest);
    fingerprint.push_text(evidence.case_id.as_str());
    fingerprint.push_text(evidence.baseline_id.as_str());
    fingerprint.push_u64(u64::from(evidence.attempt_ordinal));
    fingerprint.push_bool(evidence.warmup);
    fingerprint.push_u64(status_tag(evidence.status));
    fingerprint.push_u64(evidence.started_tick);
    fingerprint.push_u64(evidence.ended_tick);
    fingerprint.push_digest(evidence.scope_digest);
    fingerprint.push_digest(evidence.source_view_digest);
    if let Some(output_digest) = evidence.output_digest {
        fingerprint.push_digest(output_digest);
    }
    fingerprint.push_digest(evidence.disclosure_digest);
    for sample in &evidence.resource_samples {
        fingerprint.push_u64(sample.tick);
        fingerprint.push_u64(sample.cpu_millis);
        fingerprint.push_u64(sample.memory_bytes);
        fingerprint.push_u64(sample.read_bytes);
        fingerprint.push_u64(sample.write_bytes);
    }
    Ok(ValidatedCaseEvidence {
        evidence,
        validation_digest: fingerprint.finish(),
    })
}

fn validate_samples(
    samples: &[ResourceSample],
    started_tick: u64,
    ended_tick: u64,
) -> Result<(), EvalError> {
    let mut previous: Option<ResourceSample> = None;
    for sample in samples {
        if sample.tick < started_tick || sample.tick > ended_tick {
            return Err(EvalError::EvidenceStatusInvalid);
        }
        if let Some(previous) = previous
            && (sample.tick <= previous.tick
                || sample.cpu_millis < previous.cpu_millis
                || sample.read_bytes < previous.read_bytes
                || sample.write_bytes < previous.write_bytes)
        {
            return Err(EvalError::EvidenceStatusInvalid);
        }
        previous = Some(*sample);
    }
    Ok(())
}

const fn status_tag(status: AttemptStatus) -> u64 {
    match status {
        AttemptStatus::Success => 1,
        AttemptStatus::Partial => 2,
        AttemptStatus::Failed => 3,
        AttemptStatus::Cancelled => 4,
        AttemptStatus::TimedOut => 5,
        AttemptStatus::Unavailable => 6,
    }
}

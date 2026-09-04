//! Deterministic A/B/C scheduling and immutable attempt evidence.

use std::collections::{BTreeMap, BTreeSet};

use search_contracts::{Blake3Digest32, OpaqueId, ReceiptRef};

use crate::{
    BaselineRole, ControlCase, EvalError, EvalLimits, FingerprintBuilder,
    FrozenRunManifest, ValidatedBaseline, ValidatedControlCorpus,
};

/// One deterministic attempt scheduled inside a case block.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ScheduledAttempt {
    /// Exact control-case identity.
    pub case_id: OpaqueId,
    /// Exact baseline/candidate identity.
    pub baseline_id: OpaqueId,
    /// A/B/C role.
    pub role: BaselineRole,
    /// Zero-based attempt ordinal within the warm-up/measured lane.
    pub attempt_ordinal: u32,
    /// Whether this attempt is excluded from measured aggregates.
    pub warmup: bool,
    /// Deterministic execution order inside the block.
    pub execution_ordinal: u64,
    /// Digest of exact case/baseline/attempt inputs.
    pub attempt_digest: Blake3Digest32,
}

/// Finite deterministic randomized A/B/C execution block.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct CaseExecutionBlock {
    /// Exact frozen run digest.
    pub run_digest: Blake3Digest32,
    /// Zero-based deterministic block index.
    pub block_index: u64,
    /// Exact cases in the block.
    pub case_ids: Vec<OpaqueId>,
    /// Ordered warm-up and measured attempts.
    pub attempts: Vec<ScheduledAttempt>,
    /// Digest of exact block contents and order.
    pub block_digest: Blake3Digest32,
}

/// Builds one finite case block with deterministic per-attempt A/B/C ordering.
pub fn plan_case_block(
    run: &FrozenRunManifest,
    corpus: &ValidatedControlCorpus,
    baselines: &[ValidatedBaseline],
    block_index: u64,
    first_case: usize,
    case_count: usize,
    limits: EvalLimits,
) -> Result<CaseExecutionBlock, EvalError> {
    let limits = limits.validate()?;
    if case_count == 0
        || first_case >= corpus.manifest().cases.len()
        || first_case.saturating_add(case_count) > corpus.manifest().cases.len()
        || case_count > limits.max_cases
    {
        return Err(EvalError::CaseBlockInvalid);
    }
    let baseline_map = validate_baseline_set(run, baselines)?;
    let cases = &corpus.manifest().cases[first_case..first_case + case_count];
    let mut attempts = Vec::new();
    let mut execution_ordinal = 0_u64;

    let lanes = [
        (true, run.input().warmups),
        (false, run.input().repetitions),
    ];
    for case in cases {
        for (warmup, count) in lanes {
            for attempt_ordinal in 0..count {
                let mut order = baseline_map.values().cloned().collect::<Vec<_>>();
                order.sort_by_key(|baseline| {
                    schedule_key(
                        run.input().seed,
                        block_index,
                        case,
                        baseline.descriptor().role,
                        attempt_ordinal,
                        warmup,
                    )
                });
                for baseline in order {
                    let descriptor = baseline.descriptor();
                    let attempt_digest = attempt_fingerprint(
                        run,
                        case,
                        descriptor.baseline_id.as_str(),
                        descriptor.role,
                        attempt_ordinal,
                        warmup,
                    );
                    attempts.push(ScheduledAttempt {
                        case_id: case.case_id.clone(),
                        baseline_id: descriptor.baseline_id.clone(),
                        role: descriptor.role,
                        attempt_ordinal,
                        warmup,
                        execution_ordinal,
                        attempt_digest,
                    });
                    execution_ordinal = execution_ordinal
                        .checked_add(1)
                        .ok_or(EvalError::ContractExhausted)?;
                }
            }
        }
    }

    let max_attempts = case_count
        .checked_mul(3)
        .and_then(|value| {
            value.checked_mul(
                usize::try_from(run.input().warmups)
                    .unwrap_or(usize::MAX)
                    .saturating_add(
                        usize::try_from(run.input().repetitions)
                            .unwrap_or(usize::MAX),
                    ),
            )
        })
        .ok_or(EvalError::BudgetExceeded)?;
    if attempts.len() != max_attempts {
        return Err(EvalError::CaseBlockInvalid);
    }

    let case_ids = cases.iter().map(|case| case.case_id.clone()).collect::<Vec<_>>();
    let mut fingerprint = FingerprintBuilder::new(b"eliot-search/eval/case-block/v1");
    fingerprint.push_digest(run.run_digest());
    fingerprint.push_u64(block_index);
    for attempt in &attempts {
        fingerprint.push_text(attempt.case_id.as_str());
        fingerprint.push_text(attempt.baseline_id.as_str());
        fingerprint.push_u64(role_tag(attempt.role));
        fingerprint.push_u64(u64::from(attempt.attempt_ordinal));
        fingerprint.push_bool(attempt.warmup);
        fingerprint.push_u64(attempt.execution_ordinal);
        fingerprint.push_digest(attempt.attempt_digest);
    }
    Ok(CaseExecutionBlock {
        run_digest: run.run_digest(),
        block_index,
        case_ids,
        attempts,
        block_digest: fingerprint.finish(),
    })
}

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
    validate_samples(&evidence.resource_samples, evidence.started_tick, evidence.ended_tick)?;
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

/// Idempotent process-local attempt ledger that preserves original failures.
#[derive(Clone, Debug)]
pub struct EvidenceLedger {
    max_attempts: usize,
    attempts: BTreeMap<Blake3Digest32, ValidatedCaseEvidence>,
}

impl EvidenceLedger {
    /// Creates a finite attempt ledger.
    pub fn new(max_attempts: usize) -> Result<Self, EvalError> {
        if max_attempts == 0 {
            return Err(EvalError::InvalidLimits);
        }
        Ok(Self {
            max_attempts,
            attempts: BTreeMap::new(),
        })
    }

    /// Records one exact validated attempt or returns an idempotent replay.
    pub fn record(
        &mut self,
        evidence: ValidatedCaseEvidence,
    ) -> Result<&ValidatedCaseEvidence, EvalError> {
        let key = evidence.evidence.attempt_digest;
        if let Some(existing) = self.attempts.get(&key) {
            if existing == &evidence {
                return Ok(existing);
            }
            return Err(EvalError::AttemptConflict);
        }
        if self.attempts.len() >= self.max_attempts {
            return Err(EvalError::BudgetExceeded);
        }
        self.attempts.insert(key, evidence);
        Ok(self.attempts.get(&key).expect("inserted evidence"))
    }

    /// Deterministically ordered accepted evidence.
    #[must_use]
    pub fn evidence(&self) -> impl ExactSizeIterator<Item = &ValidatedCaseEvidence> {
        self.attempts.values()
    }
}

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

fn validate_baseline_set(
    run: &FrozenRunManifest,
    baselines: &[ValidatedBaseline],
) -> Result<BTreeMap<BaselineRole, ValidatedBaseline>, EvalError> {
    if baselines.len() != 3 {
        return Err(EvalError::CaseBlockInvalid);
    }
    let mut by_role = BTreeMap::new();
    let mut scope = None;
    for baseline in baselines {
        let descriptor = baseline.descriptor();
        if descriptor.run_digest != run.run_digest() {
            return Err(EvalError::EvidenceBindingMismatch);
        }
        if scope.is_some_and(|expected| expected != descriptor.scope_digest) {
            return Err(EvalError::EvidenceBindingMismatch);
        }
        scope = Some(descriptor.scope_digest);
        if by_role.insert(descriptor.role, baseline.clone()).is_some() {
            return Err(EvalError::CaseBlockInvalid);
        }
    }
    if ![BaselineRole::A, BaselineRole::B, BaselineRole::C]
        .into_iter()
        .all(|role| by_role.contains_key(&role))
    {
        return Err(EvalError::CaseBlockInvalid);
    }
    Ok(by_role)
}

fn validate_samples(
    samples: &[ResourceSample],
    started_tick: u64,
    ended_tick: u64,
) -> Result<(), EvalError> {
    let mut previous = None;
    for sample in samples {
        if sample.tick < started_tick || sample.tick > ended_tick {
            return Err(EvalError::EvidenceStatusInvalid);
        }
        if let Some(previous) = previous {
            if sample.tick <= previous.tick
                || sample.cpu_millis < previous.cpu_millis
                || sample.read_bytes < previous.read_bytes
                || sample.write_bytes < previous.write_bytes
            {
                return Err(EvalError::EvidenceStatusInvalid);
            }
        }
        previous = Some(*sample);
    }
    Ok(())
}

fn schedule_key(
    seed: u64,
    block_index: u64,
    case: &ControlCase,
    role: BaselineRole,
    attempt_ordinal: u32,
    warmup: bool,
) -> [u8; 32] {
    let mut fingerprint = FingerprintBuilder::new(b"eliot-search/eval/schedule-key/v1");
    fingerprint.push_u64(seed);
    fingerprint.push_u64(block_index);
    fingerprint.push_text(case.case_id.as_str());
    fingerprint.push_u64(role_tag(role));
    fingerprint.push_u64(u64::from(attempt_ordinal));
    fingerprint.push_bool(warmup);
    *fingerprint.finish().as_bytes()
}

fn attempt_fingerprint(
    run: &FrozenRunManifest,
    case: &ControlCase,
    baseline_id: &str,
    role: BaselineRole,
    attempt_ordinal: u32,
    warmup: bool,
) -> Blake3Digest32 {
    let mut fingerprint = FingerprintBuilder::new(b"eliot-search/eval/attempt/v1");
    fingerprint.push_digest(run.run_digest());
    fingerprint.push_text(case.case_id.as_str());
    fingerprint.push_digest(case.fixture_digest);
    fingerprint.push_digest(case.oracle_digest);
    fingerprint.push_text(baseline_id);
    fingerprint.push_u64(role_tag(role));
    fingerprint.push_u64(u64::from(attempt_ordinal));
    fingerprint.push_bool(warmup);
    fingerprint.finish()
}

fn role_tag(role: BaselineRole) -> u64 {
    match role {
        BaselineRole::A => 1,
        BaselineRole::B => 2,
        BaselineRole::C => 3,
    }
}

fn status_tag(status: AttemptStatus) -> u64 {
    match status {
        AttemptStatus::Success => 1,
        AttemptStatus::Partial => 2,
        AttemptStatus::Failed => 3,
        AttemptStatus::Cancelled => 4,
        AttemptStatus::TimedOut => 5,
        AttemptStatus::Unavailable => 6,
    }
}

//! Deterministic finite A/B/C case-block scheduling.

use std::collections::BTreeMap;

use search_contracts::{Blake3Digest32, OpaqueId};

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
                    .saturating_add(usize::try_from(run.input().repetitions).unwrap_or(usize::MAX)),
            )
        })
        .ok_or(EvalError::BudgetExceeded)?;
    if attempts.len() != max_attempts {
        return Err(EvalError::CaseBlockInvalid);
    }

    let case_ids = cases
        .iter()
        .map(|case| case.case_id.clone())
        .collect::<Vec<_>>();
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

const fn role_tag(role: BaselineRole) -> u64 {
    match role {
        BaselineRole::A => 1,
        BaselineRole::B => 2,
        BaselineRole::C => 3,
    }
}

//! Bounded whole-call v2 admission evaluation with canonical multiset results.
// The original caller owns checkpoints; this pure result supplies no receipt authority.

use core::fmt;

use search_contracts::{
    Blake3Digest32, BoundedBytes, BoundedList, CanonicalBlake3Output, CanonicalDigestDomain,
    CanonicalText, CanonicalValue, ContractError, ContractErrorKind, DigestInputLimit,
    NonZeroRevision, blake3_canonical_array_with_usage,
};

use super::AdmissionError;
use super::evaluation_v2::{
    AdmissionDecisionV2, DEFAULT_EVALUATION_LIMITS, EvaluationLimitsV2, EvaluationMeterV2,
    evaluate_v2_metered,
};
use super::observation_v2::AdmissionObservationV2;
use super::policy_v2::{
    CheckpointFailureV2, CompiledAdmissionPolicyV2, MatchBufferV2, PolicyFingerprintV2, checkpoint,
};

/// Distinct multiset batch schema; accepted upstream identities are unchanged.
pub const BATCH_SCHEMA_VERSION: u32 = 2;
/// Canonical multiset semantics with every input occurrence retained.
pub const BATCH_PROFILE: &str = "eliot-admission-batch-multiset@2";
/// Complete final batch preimage domain.
pub const BATCH_DIGEST_DOMAIN: &str = "eliot/cbor/admission/batch/v2";
/// Maximum input occurrences, including duplicates.
pub const MAX_BATCH_ITEMS: usize = 1_024;
/// Maximum whole-call canonical rule evaluations.
pub const MAX_BATCH_RULE_EVALUATIONS: usize = 131_072;
/// Maximum whole-call matching ID clones in evaluation.
pub const MAX_BATCH_MATCHED_RULE_IDS: usize = 131_072;
/// Maximum whole-call distinct per-item reason contributions before filtering.
pub const MAX_BATCH_REASON_CONTRIBUTIONS: usize = 23_552;
/// Maximum sum of performed observation, decision and final batch preimages.
pub const MAX_BATCH_ENCODED_BYTES: usize = 8_388_608;

const FIXED_BATCH_ROWS: usize = 4;

/// Raw whole-call limits plus unchanged accepted per-item lower ceilings.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct BatchLimitsV2 {
    /// Maximum occurrences; never a deduplicated observation count.
    pub max_items: usize,
    /// Maximum rules charged across all occurrences.
    pub max_rule_evaluations: usize,
    /// Maximum matching IDs charged before their clones.
    pub max_matched_rule_ids: usize,
    /// Maximum new per-item reasons charged before insertion.
    pub max_reason_contributions: usize,
    /// Maximum actual aggregate complete encoded bytes.
    pub max_encoded_bytes: usize,
    /// Existing per-item rule/ID/reason/decision/observation ceilings.
    pub per_item: EvaluationLimitsV2,
}

impl BatchLimitsV2 {
    fn validate(self) -> Result<Self, AdmissionError> {
        for (value, maximum) in [
            (self.max_items, MAX_BATCH_ITEMS),
            (self.max_rule_evaluations, MAX_BATCH_RULE_EVALUATIONS),
            (self.max_matched_rule_ids, MAX_BATCH_MATCHED_RULE_IDS),
            (
                self.max_reason_contributions,
                MAX_BATCH_REASON_CONTRIBUTIONS,
            ),
            (self.max_encoded_bytes, MAX_BATCH_ENCODED_BYTES),
        ] {
            if value == 0 || value > maximum {
                return Err(AdmissionError::ObservationInvalid);
            }
        }
        // Integration: retain the existing body, make this method pub(super).
        self.per_item.validate()?;
        Ok(self)
    }
}

/// Fixed maxima; sufficient lower caller limits do not alter identity.
pub const DEFAULT_BATCH_LIMITS: BatchLimitsV2 = BatchLimitsV2 {
    max_items: MAX_BATCH_ITEMS,
    max_rule_evaluations: MAX_BATCH_RULE_EVALUATIONS,
    max_matched_rule_ids: MAX_BATCH_MATCHED_RULE_IDS,
    max_reason_contributions: MAX_BATCH_REASON_CONTRIBUTIONS,
    max_encoded_bytes: MAX_BATCH_ENCODED_BYTES,
    per_item: DEFAULT_EVALUATION_LIMITS,
};

/// Internally computed batch equality, never a caller-supplied digest.
#[derive(Clone, Copy, Eq, PartialEq)]
pub struct BatchFingerprintV2(Blake3Digest32);

impl BatchFingerprintV2 {
    /// Complete shared canonical CBOR/BLAKE3 equality bytes.
    #[must_use]
    pub fn as_bytes(&self) -> &[u8; 32] {
        self.0.as_bytes()
    }
}

impl fmt::Debug for BatchFingerprintV2 {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str("BatchFingerprintV2([REDACTED])")
    }
}

/// Complete immutable evaluation result in canonical multiset order.
#[derive(Clone, Eq, PartialEq)]
pub struct AdmissionBatchV2 {
    schema_version: u32,
    policy_revision: NonZeroRevision,
    policy_fingerprint: PolicyFingerprintV2,
    decisions: Vec<AdmissionDecisionV2>,
    fingerprint: BatchFingerprintV2,
}

impl AdmissionBatchV2 {
    /// Fixed batch schema version.
    #[must_use]
    pub const fn schema_version(&self) -> u32 {
        self.schema_version
    }

    /// Exact immutable evaluated policy revision.
    #[must_use]
    pub const fn policy_revision(&self) -> NonZeroRevision {
        self.policy_revision
    }

    /// Exact compiled policy identity.
    #[must_use]
    pub const fn policy_fingerprint(&self) -> PolicyFingerprintV2 {
        self.policy_fingerprint
    }

    /// One decision per occurrence, ordered by observation then decision bytes.
    #[must_use]
    pub fn decisions(&self) -> &[AdmissionDecisionV2] {
        &self.decisions
    }

    /// Internally computed canonical multiset identity.
    #[must_use]
    pub const fn fingerprint(&self) -> BatchFingerprintV2 {
        self.fingerprint
    }
}

impl fmt::Debug for AdmissionBatchV2 {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("AdmissionBatchV2")
            .field("schema", &self.schema_version)
            .field("occurrences", &self.decisions.len())
            .field("fingerprint", &self.fingerprint)
            .finish_non_exhaustive()
    }
}

/// Evaluate and charge every occurrence, retaining multiplicity after sorting.
///
/// The result is policy meaning, without receipt/native/root/access/currentness
/// authority. Original checkpoints are reused; no clock or preemption is owned.
/// Caller input/model creation and separately issued receipts are not charged.
/// Canonical row allocations may precede sink byte charging; this is not a heap cap.
///
/// # Errors
/// Entry checkpoint precedes raw-limit/count validation. Invalid limits, empty
/// input or count above the admitted maximum return ObservationInvalid. Actual
/// cumulative exhaustion returns BudgetExhausted. Accepted evaluator refusals
/// retain their order. Every error clears scratch and returns no partial result.
pub fn evaluate_batch_v2(
    policy: &CompiledAdmissionPolicyV2,
    observations: &[AdmissionObservationV2],
    scratch: &mut MatchBufferV2,
    limits: BatchLimitsV2,
    probe: &mut impl FnMut() -> Result<(), CheckpointFailureV2>,
) -> Result<AdmissionBatchV2, AdmissionError> {
    scratch.clear();
    let result = (|| {
        checkpoint(probe)?;
        let limits = limits.validate()?;
        if observations.is_empty() || observations.len() > limits.max_items {
            return Err(AdmissionError::ObservationInvalid);
        }
        let mut meter = EvaluationMeterV2::new(
            limits.max_rule_evaluations,
            limits.max_matched_rule_ids,
            limits.max_reason_contributions,
            limits.max_encoded_bytes,
        );
        let mut decisions = Vec::with_capacity(observations.len());
        for observation in observations {
            checkpoint(probe)?;
            let decision = evaluate_v2_metered(
                policy,
                observation,
                scratch,
                limits.per_item,
                Some(&mut meter),
                probe,
            )?;
            decisions.push(decision);
        }
        checkpoint(probe)?;
        decisions.sort_unstable_by(|left, right| {
            left.observation_fingerprint()
                .as_bytes()
                .cmp(right.observation_fingerprint().as_bytes())
                .then_with(|| {
                    left.fingerprint()
                        .as_bytes()
                        .cmp(right.fingerprint().as_bytes())
                })
        });
        checkpoint(probe)?;
        let max_encoded_bytes = meter.encoded_limit(MAX_BATCH_ENCODED_BYTES)?;
        let output = compute_batch_fingerprint(policy, &decisions, max_encoded_bytes)
            .map_err(encoded_limit_error)?;
        let fingerprint = BatchFingerprintV2(meter.charge_encoded(output)?);
        checkpoint(probe)?;
        let batch = AdmissionBatchV2 {
            schema_version: BATCH_SCHEMA_VERSION,
            policy_revision: policy.revision(),
            policy_fingerprint: policy.fingerprint(),
            decisions,
            fingerprint,
        };
        checkpoint(probe)?;
        Ok(batch)
    })();
    if result.is_err() {
        scratch.clear();
    }
    result
}

fn compute_batch_fingerprint(
    policy: &CompiledAdmissionPolicyV2,
    decisions: &[AdmissionDecisionV2],
    max_encoded_bytes: usize,
) -> Result<CanonicalBlake3Output, ContractError> {
    let domain = CanonicalDigestDomain::parse(BATCH_DIGEST_DOMAIN)?;
    let row_count = FIXED_BATCH_ROWS
        .checked_add(decisions.len())
        .ok_or_else(|| ContractError::new(ContractErrorKind::InvalidRange, "admission_batch_v2"))?;
    blake3_canonical_array_with_usage(
        &domain,
        row_count,
        (0..row_count).map(|index| batch_row(policy, decisions, index)),
        DigestInputLimit::new(max_encoded_bytes)?,
    )
}

fn batch_row(
    policy: &CompiledAdmissionPolicyV2,
    decisions: &[AdmissionDecisionV2],
    index: usize,
) -> Result<CanonicalValue, ContractError> {
    match index {
        0 => array(vec![
            CanonicalValue::U64(u64::from(BATCH_SCHEMA_VERSION)),
            CanonicalText::new(BATCH_PROFILE).map(CanonicalValue::Text)?,
            integer(MAX_BATCH_ITEMS)?,
            integer(MAX_BATCH_RULE_EVALUATIONS)?,
            integer(MAX_BATCH_MATCHED_RULE_IDS)?,
            integer(MAX_BATCH_REASON_CONTRIBUTIONS)?,
            integer(MAX_BATCH_ENCODED_BYTES)?,
        ]),
        1 => Ok(CanonicalValue::U64(policy.revision().get())),
        2 => bytes32(policy.fingerprint().as_bytes()),
        3 => integer(decisions.len()),
        _ => {
            let decision = decisions.get(index - FIXED_BATCH_ROWS).ok_or_else(|| {
                ContractError::new(ContractErrorKind::InvalidRange, "admission_batch_v2_row")
            })?;
            array(vec![
                bytes32(decision.observation_fingerprint().as_bytes())?,
                bytes32(decision.fingerprint().as_bytes())?,
            ])
        }
    }
}

fn bytes32(value: &[u8; 32]) -> Result<CanonicalValue, ContractError> {
    BoundedBytes::new(value.to_vec()).map(CanonicalValue::Bytes)
}

fn integer(value: usize) -> Result<CanonicalValue, ContractError> {
    u64::try_from(value)
        .map(CanonicalValue::U64)
        .map_err(|_| ContractError::new(ContractErrorKind::InvalidRange, "admission_batch_v2"))
}

fn array(values: Vec<CanonicalValue>) -> Result<CanonicalValue, ContractError> {
    BoundedList::new(values).map(CanonicalValue::Array)
}

fn encoded_limit_error(error: ContractError) -> AdmissionError {
    if error.kind() == ContractErrorKind::OversizePayload
        && matches!(error.field(), "digest_preimage" | "canonical_sink")
    {
        AdmissionError::BudgetExhausted
    } else {
        AdmissionError::ObservationInvalid
    }
}

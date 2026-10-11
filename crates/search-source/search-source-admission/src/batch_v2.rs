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
    AdmissionDecisionV2, DEFAULT_EVALUATION_LIMITS, EvaluationLimitsV2, evaluate_v2_metered,
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

/// Private composition state, shared by the sole evaluator across occurrences.
/// This supplies no public operation receipt, accepted flag or new authority.
pub(super) struct EvaluationMeterV2 {
    rule_evaluations: usize,
    matched_rule_ids: usize,
    reason_contributions: usize,
    encoded_bytes: usize,
}

impl EvaluationMeterV2 {
    fn new(limits: BatchLimitsV2) -> Self {
        Self {
            rule_evaluations: limits.max_rule_evaluations,
            matched_rule_ids: limits.max_matched_rule_ids,
            reason_contributions: limits.max_reason_contributions,
            encoded_bytes: limits.max_encoded_bytes,
        }
    }

    /// Charge the complete canonical rule count before observation/matcher work.
    pub(super) fn charge_rules(&mut self, count: usize) -> Result<(), AdmissionError> {
        charge(&mut self.rule_evaluations, count)
    }

    /// Call after the per-item ID guard and before each matching String clone.
    pub(super) fn charge_matched_rule_id(&mut self) -> Result<(), AdmissionError> {
        charge(&mut self.matched_rule_ids, 1)
    }

    /// Call only for a new per-item reason, before BTreeSet insertion.
    pub(super) fn charge_reason_contribution(&mut self) -> Result<(), AdmissionError> {
        charge(&mut self.reason_contributions, 1)
    }

    /// A derived zero is exhaustion, not a public invalid-raw-limit error.
    pub(super) fn encoded_limit(&self, per_frame: usize) -> Result<usize, AdmissionError> {
        if self.encoded_bytes == 0 {
            return Err(AdmissionError::BudgetExhausted);
        }
        Ok(per_frame.min(self.encoded_bytes))
    }

    /// Subtract only successful producer usage, returning its computed digest.
    pub(super) fn charge_encoded(
        &mut self,
        output: CanonicalBlake3Output,
    ) -> Result<Blake3Digest32, AdmissionError> {
        charge(&mut self.encoded_bytes, output.complete_preimage_bytes())?;
        Ok(output.digest())
    }
}

fn charge(remaining: &mut usize, amount: usize) -> Result<(), AdmissionError> {
    *remaining = remaining
        .checked_sub(amount)
        .ok_or(AdmissionError::BudgetExhausted)?;
    Ok(())
}

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
        let mut meter = EvaluationMeterV2::new(limits);
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

// AUTHORED_NOT_RUN / Q215: private causal batch fixtures, not execution evidence.
// These fixtures stay with the complete batch owner and its private meter.
// The shared counted producer owns encoding/hash execution; fixed profile rows stay local.
// BatchLimitsV2.per_item carries the accepted per-item EvaluationLimitsV2;
// AdmissionBatchV2.decisions() -> &[AdmissionDecisionV2]; fingerprint().as_bytes().
// No usage-summary API, public authority constructor or receipt invocation is needed.

#[cfg(test)]
mod concrete_batch_fixtures {
    use super::{AdmissionBatchV2, BatchLimitsV2, evaluate_batch_v2};
    use crate::evaluation_v2::{
        AdmissionDecisionV2, DEFAULT_EVALUATION_LIMITS, EvaluationLimitsV2, evaluate_v2,
    };
    use crate::observation_v2::{AdmissionObservationV2, AdmissionRelativeLocator};
    use crate::policy_v2::{
        CheckpointFailureV2, CompiledAdmissionPolicyV2, DEFAULT_COMPILE_LIMITS, MatchBufferV2,
        PolicyInputV2, PredicateV2, RuleInputV2, compile_policy_v2,
    };
    use crate::{
        AcceptedCapabilities, AdmissionError, AdmissionOutcome, AdmissionReasonCode,
        DEFAULT_ADMISSION_LIMITS, RuleEffect, SectionPlatform, SourceClass,
        UnvalidatedObservationInput, baseline_policy, compiled_defaults, validate_observation,
        validate_section,
    };

    fn limits() -> BatchLimitsV2 {
        BatchLimitsV2 {
            max_items: 1024,
            max_rule_evaluations: 131_072,
            max_matched_rule_ids: 131_072,
            max_reason_contributions: 23_552,
            max_encoded_bytes: 8_388_608,
            per_item: DEFAULT_EVALUATION_LIMITS,
        }
    }

    fn policy(rules: Vec<RuleInputV2>) -> CompiledAdmissionPolicyV2 {
        let defaults = compiled_defaults();
        let section = validate_section(
            &defaults,
            &SectionPlatform {
                os: "windows".into(),
                arch: "x64".into(),
            },
            &AcceptedCapabilities {
                profiles: vec![defaults.bootstrap_policy_profile.clone()],
            },
        )
        .expect("existing section validator");
        let input = PolicyInputV2 {
            schema_version: 2,
            revision: 1,
            section,
            rules,
            unknown_fields: Vec::new(),
        };
        compile_policy_v2(&input, DEFAULT_COMPILE_LIMITS, &mut || Ok(())).unwrap()
    }

    fn rule(id: &str, precedence: u32, predicate: PredicateV2, effect: RuleEffect) -> RuleInputV2 {
        RuleInputV2 {
            id: id.into(),
            precedence,
            source_class: SourceClass::Regular,
            predicate,
            effect,
            unknown_fields: Vec::new(),
        }
    }

    fn raw() -> UnvalidatedObservationInput {
        // Exact accepted independent golden recipe: complete observation194 bytes.
        UnvalidatedObservationInput {
            locator_class: "normalized-file".into(),
            location_class: "local-fixed".into(),
            source_kind: "file".into(),
            source_class: "regular".into(),
            byte_size: Some(10),
            is_generated: Some(false),
            is_vendor: Some(false),
            is_binary: Some(false),
            is_system: Some(false),
            sensitivity: Some("public".into()),
            detector_id: Some("detector:baseline-v1".into()),
            profile_id: Some(crate::BASELINE_PROFILE.into()),
            unavailable_fields: Vec::new(),
            unknown_fields: Vec::new(),
        }
    }

    fn model(input: &UnvalidatedObservationInput, path: Option<&[u8]>) -> AdmissionObservationV2 {
        let metadata = validate_observation(
            input,
            &baseline_policy(3, 16_777_216),
            DEFAULT_ADMISSION_LIMITS,
        )
        .unwrap();
        let locator = path.map(|bytes| AdmissionRelativeLocator::from_bytes(bytes).unwrap());
        AdmissionObservationV2::new(metadata, locator).unwrap()
    }

    fn observed() -> AdmissionObservationV2 {
        model(&raw(), Some(b"src/lib.rs"))
    }

    fn run(
        policy: &CompiledAdmissionPolicyV2,
        inputs: &[AdmissionObservationV2],
        ceilings: BatchLimitsV2,
    ) -> Result<AdmissionBatchV2, AdmissionError> {
        evaluate_batch_v2(
            policy,
            inputs,
            &mut policy.new_match_buffer(),
            ceilings,
            &mut || Ok(()),
        )
    }

    fn key(decision: &AdmissionDecisionV2) -> ([u8; 32], [u8; 32]) {
        (
            *decision.observation_fingerprint().as_bytes(),
            *decision.fingerprint().as_bytes(),
        )
    }

    fn assert_clean(scratch: &MatchBufferV2) {
        assert!(format!("{scratch:?}").contains("matches: 0"));
    }

    fn seed(
        policy: &CompiledAdmissionPolicyV2,
        input: &AdmissionObservationV2,
        scratch: &mut MatchBufferV2,
    ) {
        assert!(
            !policy
                .match_path_rules(input, scratch, &mut || Ok(()))
                .unwrap()
                .is_empty()
        );
        assert!(!format!("{scratch:?}").contains("matches: 0"));
    }

    #[test]
    fn capacity_is_1024_occurrences_and_refusal_has_no_success_value() {
        let compiled = policy(Vec::new());
        let input = observed();
        let full = vec![input.clone(); 1024];
        let exact = BatchLimitsV2 {
            max_encoded_bytes: 450_688,
            ..limits()
        };
        let batch = run(&compiled, &full, exact).unwrap();
        assert_eq!(batch.decisions().len(), 1024);
        assert!(
            batch
                .decisions()
                .iter()
                .all(|d| d.outcome() == AdmissionOutcome::Deny)
        );
        assert_eq!(
            run(
                &compiled,
                &full,
                BatchLimitsV2 {
                    max_encoded_bytes: 450_687,
                    ..limits()
                }
            )
            .err(),
            Some(AdmissionError::BudgetExhausted)
        );
        assert_eq!(
            run(&compiled, &vec![input; 1025], limits()).err(),
            Some(AdmissionError::ObservationInvalid)
        );
        assert_eq!(
            run(&compiled, &[], limits()).err(),
            Some(AdmissionError::ObservationInvalid)
        );
        let one = [observed()];
        assert!(
            run(
                &compiled,
                &one,
                BatchLimitsV2 {
                    max_items: 1,
                    ..limits()
                }
            )
            .is_ok()
        );
        assert_eq!(
            run(
                &compiled,
                &[one[0].clone(), one[0].clone()],
                BatchLimitsV2 {
                    max_items: 1,
                    ..limits()
                }
            )
            .err(),
            Some(AdmissionError::ObservationInvalid)
        );
    }

    #[test]
    fn permutation_is_a_multiset_with_duplicate_occurrences_retained() {
        let compiled = policy(vec![rule(
            "allow-src",
            1,
            PredicateV2::RelativePathGlobMatches("src/*.rs".into()),
            RuleEffect::Allow,
        )]);
        let a = observed();
        let b = model(&raw(), Some(b"docs/readme.md"));
        let first = run(&compiled, &[a.clone(), b.clone(), a.clone()], limits()).unwrap();
        let second = run(&compiled, &[b.clone(), a.clone(), a.clone()], limits()).unwrap();
        let without_duplicate = run(&compiled, &[a.clone(), b.clone()], limits()).unwrap();
        assert_eq!(
            first.fingerprint().as_bytes(),
            second.fingerprint().as_bytes()
        );
        assert_eq!(first.decisions(), second.decisions());
        assert_ne!(
            first.fingerprint().as_bytes(),
            without_duplicate.fingerprint().as_bytes()
        );
        let mut expected = [a.clone(), b, a]
            .iter()
            .map(|item| {
                evaluate_v2(
                    &compiled,
                    item,
                    &mut compiled.new_match_buffer(),
                    DEFAULT_EVALUATION_LIMITS,
                    &mut || Ok(()),
                )
                .unwrap()
            })
            .collect::<Vec<_>>();
        expected.sort_by_key(key);
        assert_eq!(first.decisions(), expected.as_slice());
        assert_eq!(first.decisions().len(), 3);
        assert!(
            first
                .decisions()
                .windows(2)
                .all(|pair| key(&pair[0]) <= key(&pair[1]))
        );
    }

    #[test]
    fn exact_actual_bytes_include_both_observations_decisions_and_final_preimage() {
        use search_contracts::{
            CanonicalDigestDomain, DigestInputLimit, blake3_canonical, parse_canonical_cbor,
        };
        let compiled = policy(Vec::new());
        let input = observed();
        let per_item = EvaluationLimitsV2 {
            max_observation_encoded_bytes: 194,
            max_encoded_bytes: 177,
            ..DEFAULT_EVALUATION_LIMITS
        };
        let single = evaluate_v2(
            &compiled,
            &input,
            &mut compiled.new_match_buffer(),
            per_item,
            &mut || Ok(()),
        )
        .unwrap();
        assert_eq!(single.reason_codes(), &[AdmissionReasonCode::DenyByDefault]);
        let inputs = [input.clone(), input];
        let exact = BatchLimitsV2 {
            max_encoded_bytes: 1004,
            per_item,
            ..limits()
        };
        let batch = run(&compiled, &inputs, exact).unwrap();
        let generous = run(&compiled, &inputs, limits()).unwrap();
        assert_eq!(
            batch.fingerprint().as_bytes(),
            generous.fingerprint().as_bytes()
        );
        assert_eq!(batch.decisions(), generous.decisions());

        // Independent fixed descriptor for N=2/revision1. This is fixture-only,
        // not a production sizing traversal, row encoder, or copied hash.
        let mut cbor = vec![0x86, 0x87, 0x02, 0x78, 0x20];
        cbor.extend_from_slice(b"eliot-admission-batch-multiset@2");
        cbor.extend_from_slice(&[
            0x19, 0x04, 0x00, 0x1a, 0x00, 0x02, 0x00, 0x00, 0x1a, 0x00, 0x02, 0x00, 0x00, 0x19,
            0x5c, 0x00, 0x1a, 0x00, 0x80, 0x00, 0x00, 0x01, 0x58, 0x20,
        ]);
        cbor.extend_from_slice(compiled.fingerprint().as_bytes());
        cbor.push(0x02);
        for _ in 0..2 {
            cbor.extend_from_slice(&[0x82, 0x58, 0x20]);
            cbor.extend_from_slice(single.observation_fingerprint().as_bytes());
            cbor.extend_from_slice(&[0x58, 0x20]);
            cbor.extend_from_slice(single.fingerprint().as_bytes());
        }
        assert_eq!(cbor.len(), 232);
        let domain = CanonicalDigestDomain::parse("eliot/cbor/admission/batch/v2").unwrap();
        let value = parse_canonical_cbor(&cbor).unwrap();
        assert_eq!(
            blake3_canonical(&domain, &value, DigestInputLimit::new(262).unwrap())
                .unwrap()
                .as_bytes(),
            batch.fingerprint().as_bytes()
        );
        // 2*(194+177) + (29-domain + NUL +232-CBOR) =1004 actual bytes.
        //194/371/742 leave exactly zero before decision/next item/final frame.
        for cap in [1, 193, 194, 370, 371, 564, 741, 742, 1003] {
            assert_eq!(
                run(
                    &compiled,
                    &inputs,
                    BatchLimitsV2 {
                        max_encoded_bytes: cap,
                        per_item,
                        ..limits()
                    }
                )
                .err(),
                Some(AdmissionError::BudgetExhausted),
                "cap={cap}"
            );
        }
        for lower in [
            EvaluationLimitsV2 {
                max_observation_encoded_bytes: 193,
                ..per_item
            },
            EvaluationLimitsV2 {
                max_encoded_bytes: 176,
                ..per_item
            },
        ] {
            assert_eq!(
                run(
                    &compiled,
                    &inputs,
                    BatchLimitsV2 {
                        per_item: lower,
                        ..limits()
                    }
                )
                .err(),
                Some(AdmissionError::BudgetExhausted)
            );
        }
        assert_eq!(
            run(
                &compiled,
                &inputs[..1],
                BatchLimitsV2 {
                    max_encoded_bytes: 564,
                    per_item,
                    ..limits()
                }
            )
            .unwrap()
            .decisions()
            .len(),
            1
        );
        assert_eq!(
            run(
                &compiled,
                &inputs[..1],
                BatchLimitsV2 {
                    max_encoded_bytes: 563,
                    per_item,
                    ..limits()
                }
            )
            .err(),
            Some(AdmissionError::BudgetExhausted)
        );
    }

    #[test]
    fn rule_id_and_losing_reason_charges_precede_winning_category_filter() {
        let compiled = policy(vec![
            rule(
                "permit",
                1,
                PredicateV2::RelativePathGlobMatches("src/**".into()),
                RuleEffect::Allow,
            ),
            rule("block", 2, PredicateV2::Always, RuleEffect::Deny),
        ]);
        let inputs = [observed(), observed()];
        let exact = || BatchLimitsV2 {
            max_rule_evaluations: 4,
            max_matched_rule_ids: 4,
            max_reason_contributions: 4,
            per_item: EvaluationLimitsV2 {
                max_rule_evaluations: 2,
                max_matched_rule_ids: 2,
                max_reasons: 2,
                ..DEFAULT_EVALUATION_LIMITS
            },
            ..limits()
        };
        let batch = run(&compiled, &inputs, exact()).unwrap();
        for decision in batch.decisions() {
            assert_eq!(decision.outcome(), AdmissionOutcome::Deny);
            assert_eq!(
                decision.reason_codes(),
                &[AdmissionReasonCode::DenyExplicitRule]
            );
            assert_eq!(
                decision.matched_rule_ids(),
                &["permit".to_owned(), "block".to_owned()]
            );
        }
        for deficient in [
            BatchLimitsV2 {
                max_rule_evaluations: 3,
                ..exact()
            },
            BatchLimitsV2 {
                max_matched_rule_ids: 3,
                ..exact()
            },
            BatchLimitsV2 {
                max_reason_contributions: 3,
                ..exact()
            },
        ] {
            let mut scratch = compiled.new_match_buffer();
            seed(&compiled, &inputs[0], &mut scratch);
            assert_eq!(
                evaluate_batch_v2(&compiled, &inputs, &mut scratch, deficient, &mut || Ok(()))
                    .err(),
                Some(AdmissionError::BudgetExhausted)
            );
            assert_clean(&scratch);
        }
        for per_item in [
            EvaluationLimitsV2 {
                max_rule_evaluations: 1,
                ..DEFAULT_EVALUATION_LIMITS
            },
            EvaluationLimitsV2 {
                max_matched_rule_ids: 1,
                ..DEFAULT_EVALUATION_LIMITS
            },
            EvaluationLimitsV2 {
                max_reasons: 1,
                ..DEFAULT_EVALUATION_LIMITS
            },
        ] {
            assert_eq!(
                run(
                    &compiled,
                    &inputs,
                    BatchLimitsV2 {
                        per_item,
                        ..limits()
                    }
                )
                .err(),
                Some(AdmissionError::BudgetExhausted)
            );
        }
    }

    #[test]
    fn unmatched_rules_still_charge_work_and_same_reason_charges_once_per_item() {
        let compiled = policy(vec![
            rule("hit", 1, PredicateV2::Always, RuleEffect::Allow),
            rule(
                "miss",
                2,
                PredicateV2::SizeGreaterThan(100),
                RuleEffect::Deny,
            ),
        ]);
        let inputs = [observed(), observed()];
        let exact = || BatchLimitsV2 {
            max_rule_evaluations: 4,
            max_matched_rule_ids: 2,
            max_reason_contributions: 2,
            ..limits()
        };
        let batch = run(&compiled, &inputs, exact()).unwrap();
        assert!(
            batch
                .decisions()
                .iter()
                .all(|d| d.matched_rule_ids() == &["hit".to_owned()])
        );
        assert_eq!(
            run(
                &compiled,
                &inputs,
                BatchLimitsV2 {
                    max_rule_evaluations: 3,
                    ..exact()
                }
            )
            .err(),
            Some(AdmissionError::BudgetExhausted)
        );
        let repeated = policy(vec![
            rule("a", 1, PredicateV2::Always, RuleEffect::Allow),
            rule("b", 2, PredicateV2::Always, RuleEffect::Allow),
        ]);
        let once_per_item = || BatchLimitsV2 {
            max_matched_rule_ids: 4,
            max_reason_contributions: 2,
            per_item: EvaluationLimitsV2 {
                max_reasons: 1,
                ..DEFAULT_EVALUATION_LIMITS
            },
            ..limits()
        };
        let batch = run(&repeated, &inputs, once_per_item()).unwrap();
        assert!(batch.decisions().iter().all(|d| d.reason_codes()
            == &[AdmissionReasonCode::AllowExplicitRule]
            && d.matched_rule_ids().len() == 2));
        assert_eq!(
            run(
                &repeated,
                &inputs,
                BatchLimitsV2 {
                    max_reason_contributions: 1,
                    ..once_per_item()
                }
            )
            .err(),
            Some(AdmissionError::BudgetExhausted)
        );
    }

    #[test]
    fn deny_by_default_is_charged_for_every_occurrence_without_implicit_allow() {
        let compiled = policy(Vec::new());
        let inputs = [observed(), observed()];
        let exact = || BatchLimitsV2 {
            max_reason_contributions: 2,
            ..limits()
        };
        let batch = run(&compiled, &inputs, exact()).unwrap();
        assert!(
            batch
                .decisions()
                .iter()
                .all(|decision| decision.outcome() == AdmissionOutcome::Deny
                    && decision.reason_codes() == &[AdmissionReasonCode::DenyByDefault])
        );
        assert_eq!(
            run(
                &compiled,
                &inputs,
                BatchLimitsV2 {
                    max_reason_contributions: 1,
                    ..exact()
                }
            )
            .err(),
            Some(AdmissionError::BudgetExhausted)
        );
    }

    #[test]
    fn later_item_refusal_preserves_input_processing_order_and_returns_no_partial_batch() {
        let compiled = policy(Vec::new());
        let good = observed();
        let missing = model(&raw(), None);
        assert!(
            run(
                &compiled,
                &[good.clone()],
                BatchLimitsV2 {
                    max_encoded_bytes: 564,
                    ..limits()
                }
            )
            .is_ok()
        );
        assert_eq!(
            run(&compiled, &[good.clone(), missing.clone()], limits()).err(),
            Some(AdmissionError::ObservationIncomplete)
        );
        // First good evaluation consumes194+177=371; second frame cannot start.
        let spent = || BatchLimitsV2 {
            max_encoded_bytes: 371,
            ..limits()
        };
        assert_eq!(
            run(&compiled, &[good.clone(), missing.clone()], spent()).err(),
            Some(AdmissionError::BudgetExhausted)
        );
        assert_eq!(
            run(&compiled, &[missing, good], spent()).err(),
            Some(AdmissionError::ObservationIncomplete)
        );
    }

    #[test]
    fn all_raw_dimensions_validate_and_entry_checkpoint_wins_with_cleanup() {
        let compiled = policy(vec![rule(
            "allow",
            1,
            PredicateV2::RelativePathGlobMatches("src/**".into()),
            RuleEffect::Allow,
        )]);
        let input = observed();
        let empty_policy = policy(Vec::new());
        let mut scratch = compiled.new_match_buffer();
        for (field, maximum) in [
            (0, 1024),
            (1, 131_072),
            (2, 131_072),
            (3, 23_552),
            (4, 8_388_608),
            (5, 128),
            (6, 128),
            (7, 23),
            (8, 65_536),
            (9, 16_384),
        ] {
            for value in [0, maximum + 1] {
                let invalid = || {
                    let mut raw = limits();
                    match field {
                        0 => raw.max_items = value,
                        1 => raw.max_rule_evaluations = value,
                        2 => raw.max_matched_rule_ids = value,
                        3 => raw.max_reason_contributions = value,
                        4 => raw.max_encoded_bytes = value,
                        5 => raw.per_item.max_rule_evaluations = value,
                        6 => raw.per_item.max_matched_rule_ids = value,
                        7 => raw.per_item.max_reasons = value,
                        8 => raw.per_item.max_encoded_bytes = value,
                        9 => raw.per_item.max_observation_encoded_bytes = value,
                        _ => unreachable!(),
                    }
                    raw
                };
                // No-rule input still validates unused raw count ceilings.
                assert_eq!(
                    run(&empty_policy, &[input.clone()], invalid()).err(),
                    Some(AdmissionError::ObservationInvalid)
                );
                assert_eq!(
                    run(&empty_policy, &[], invalid()).err(),
                    Some(AdmissionError::ObservationInvalid)
                );
                seed(&compiled, &input, &mut scratch);
                assert_eq!(
                    evaluate_batch_v2(
                        &compiled,
                        &[input.clone()],
                        &mut scratch,
                        invalid(),
                        &mut || Ok(())
                    )
                    .err(),
                    Some(AdmissionError::ObservationInvalid)
                );
                assert_clean(&scratch);
                for (failure, expected) in [
                    (CheckpointFailureV2::Cancelled, AdmissionError::Cancelled),
                    (
                        CheckpointFailureV2::DeadlineExceeded,
                        AdmissionError::BudgetExhausted,
                    ),
                ] {
                    seed(&compiled, &input, &mut scratch);
                    assert_eq!(
                        evaluate_batch_v2(&compiled, &[], &mut scratch, invalid(), &mut || Err(
                            failure
                        ))
                        .err(),
                        Some(expected)
                    );
                    assert_clean(&scratch);
                }
            }
        }
    }

    #[test]
    fn every_original_checkpoint_including_final_refuses_cleans_and_allows_reuse() {
        let compiled = policy(vec![rule(
            "allow",
            1,
            PredicateV2::RelativePathGlobMatches("src/**".into()),
            RuleEffect::Allow,
        )]);
        let inputs = [observed(), observed()];
        let mut scratch = compiled.new_match_buffer();
        let mut healthy_calls = 0;
        let healthy = evaluate_batch_v2(&compiled, &inputs, &mut scratch, limits(), &mut || {
            healthy_calls += 1;
            Ok(())
        })
        .unwrap();
        assert!(healthy_calls > 1);
        for (failure, expected) in [
            (CheckpointFailureV2::Cancelled, AdmissionError::Cancelled),
            (
                CheckpointFailureV2::DeadlineExceeded,
                AdmissionError::BudgetExhausted,
            ),
        ] {
            for fail_at in 1..=healthy_calls {
                seed(&compiled, &inputs[0], &mut scratch);
                let mut calls = 0;
                let failed =
                    evaluate_batch_v2(&compiled, &inputs, &mut scratch, limits(), &mut || {
                        calls += 1;
                        if calls == fail_at {
                            Err(failure)
                        } else {
                            Ok(())
                        }
                    });
                assert_eq!(failed.err(), Some(expected), "checkpoint={fail_at}");
                assert_eq!(calls, fail_at);
                assert_clean(&scratch);
                let retry =
                    evaluate_batch_v2(&compiled, &inputs, &mut scratch, limits(), &mut || Ok(()))
                        .unwrap();
                assert_eq!(
                    retry.fingerprint().as_bytes(),
                    healthy.fingerprint().as_bytes()
                );
                assert_eq!(retry.decisions(), healthy.decisions());
            }
        }
    }

    #[test]
    fn later_missing_locator_and_final_byte_refusal_clear_previously_populated_scratch() {
        let compiled = policy(vec![rule(
            "allow",
            1,
            PredicateV2::RelativePathGlobMatches("src/**".into()),
            RuleEffect::Allow,
        )]);
        let first = observed();
        let missing = model(&raw(), None);
        let mut scratch = compiled.new_match_buffer();
        seed(&compiled, &first, &mut scratch);
        assert_eq!(
            evaluate_batch_v2(
                &compiled,
                &[first.clone(), missing],
                &mut scratch,
                limits(),
                &mut || Ok(())
            )
            .err(),
            Some(AdmissionError::ObservationIncomplete)
        );
        assert_clean(&scratch);
        seed(&compiled, &first, &mut scratch);
        // This single-ID Allow descriptor is188 bytes: 177 +1 outcome byte
        // +4 reason bytes +6 ID bytes.194 observation +188 decision +193 final
        // =575; one less fails the final batch frame.
        assert_eq!(
            evaluate_batch_v2(
                &compiled,
                &[first.clone()],
                &mut scratch,
                BatchLimitsV2 {
                    max_encoded_bytes: 574,
                    ..limits()
                },
                &mut || Ok(())
            )
            .err(),
            Some(AdmissionError::BudgetExhausted)
        );
        assert_clean(&scratch);
        let retry = evaluate_batch_v2(
            &compiled,
            &[first],
            &mut scratch,
            BatchLimitsV2 {
                max_encoded_bytes: 575,
                ..limits()
            },
            &mut || Ok(()),
        )
        .unwrap();
        assert_eq!(retry.decisions()[0].outcome(), AdmissionOutcome::Allow);
    }

    #[test]
    fn all_outcomes_survive_and_debug_and_errors_exclude_payload_markers() {
        let compiled = policy(vec![rule(
            "batch-private-rule-91",
            1,
            PredicateV2::RelativePathGlobMatches("private/*".into()),
            RuleEffect::Allow,
        )]);
        let mut marker = raw();
        marker.detector_id = Some("detector:BATCH_PRIVATE_DETECTOR_91".into());
        let mut system = marker.clone();
        system.is_system = Some(true);
        let mut review = marker.clone();
        review.detector_id = None;
        review.unavailable_fields.push("detector-id".into());
        let mut unsupported = marker.clone();
        unsupported.source_kind = "device".into();
        let inputs = [marker, system, review, unsupported]
            .iter()
            .map(|input| model(input, Some(b"private/BATCH_PRIVATE_PATH_91.rs")))
            .collect::<Vec<_>>();
        let batch = run(&compiled, &inputs, limits()).unwrap();
        let mut expected = inputs
            .iter()
            .map(|item| {
                evaluate_v2(
                    &compiled,
                    item,
                    &mut compiled.new_match_buffer(),
                    DEFAULT_EVALUATION_LIMITS,
                    &mut || Ok(()),
                )
                .unwrap()
            })
            .collect::<Vec<_>>();
        assert_eq!(
            expected
                .iter()
                .map(AdmissionDecisionV2::outcome)
                .collect::<Vec<_>>(),
            vec![
                AdmissionOutcome::Allow,
                AdmissionOutcome::Deny,
                AdmissionOutcome::ReviewRequired,
                AdmissionOutcome::Unsupported
            ]
        );
        expected.sort_by_key(key);
        assert_eq!(batch.decisions(), expected.as_slice());
        let error = run(&compiled, &[model(&raw(), None)], limits())
            .err()
            .unwrap();
        for rendered in [
            format!("{batch:?}"),
            format!("{batch:#?}"),
            format!("{error:?}"),
            format!("{error:#?}"),
        ] {
            for secret in [
                "BATCH_PRIVATE_DETECTOR_91",
                "BATCH_PRIVATE_PATH_91",
                "batch-private-rule-91",
                "private/*",
                crate::BASELINE_PROFILE,
            ] {
                assert!(!rendered.contains(secret));
            }
        }
    }
}

//! VERSIONED_REBUILD retention of original normalized v1 admission inputs.
//!
//! Decoding reruns the sole owner evaluator. Historical equality is not live authority.

use super::{
    AdmissionBudget, AdmissionDecision, AdmissionError, AdmissionObservation, AdmissionReceipt,
    CancelFlag, CanonicalAdmissionPolicy, CanonicalRule, DEFAULT_ADMISSION_LIMITS,
    MAX_POLICY_RULES, POLICY_SCHEMA_VERSION, UnvalidatedObservationInput, UnvalidatedPolicyInput,
    UnvalidatedRuleInput, VerifiedAdmissionReceipt, evaluate, issue_receipt, normalize_policy,
    evaluate_with_probe_context, validate_observation, validate_policy_schema, verify_receipt,
};
use super::policy_v2::{CheckpointFailureV2, checkpoint};
use search_contracts::{self as c, CanonicalValue as V, ClosedCanonicalObject as Object};

/// New retained-original profile; existing policy/observation/receipt identities stay unchanged.
pub const ORIGINAL_ADMISSION_PROFILE: &str = "eliot-admission-original@1";
/// Finite whole-envelope byte ceiling, including every original field.
pub const MAX_ORIGINAL_ADMISSION_BYTES: usize = 256 * 1024;

/// Owner-produced original inputs and exactly reproduced historical decision/receipt.
///
/// No supplied outcome, digest or constructor can create this value. It proves
/// historical evaluation equality, without native identity, access or currentness.
#[derive(Clone, Eq, PartialEq)]
pub struct OriginalAdmissionEnvelope {
    policy: CanonicalAdmissionPolicy,
    observation: AdmissionObservation,
    decision: AdmissionDecision,
    receipt: AdmissionReceipt,
    encoded: Vec<u8>,
}
impl core::fmt::Debug for OriginalAdmissionEnvelope {
    fn fmt(&self, formatter: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        formatter
            .debug_struct("OriginalAdmissionEnvelope")
            .field("profile", &ORIGINAL_ADMISSION_PROFILE)
            .field("policy_revision", &self.policy.revision())
            .field("outcome", &self.receipt.outcome())
            .finish_non_exhaustive()
    }
}
impl OriginalAdmissionEnvelope {
    /// Exact retained original normalized policy.
    pub const fn policy(&self) -> &CanonicalAdmissionPolicy {
        &self.policy
    }
    /// Exact retained original observation, with no source body or path.
    pub const fn observation(&self) -> &AdmissionObservation {
        &self.observation
    }
    /// Decision reproduced by the actual owner evaluator.
    pub const fn decision(&self) -> &AdmissionDecision {
        &self.decision
    }
    /// Historical owner-issued receipt, not proof of the current policy fence.
    pub const fn receipt(&self) -> &AdmissionReceipt {
        &self.receipt
    }
    /// Complete canonical persisted bytes; no separately supplied identity.
    pub fn encoded(&self) -> &[u8] {
        &self.encoded
    }
    /// Separately verify the original receipt against an actual live policy.
    pub fn verify_current_policy(
        &self,
        current: &CanonicalAdmissionPolicy,
        cancel: CancelFlag,
    ) -> Result<VerifiedAdmissionReceipt, AdmissionError> {
        cancel.check()?;
        verify_receipt(&self.receipt, current, &self.observation)
    }
}

/// Produce retention bytes from original immutable inputs and an owner-issued receipt.
///
/// The actual evaluator and issuer must reproduce the complete supplied receipt.
/// No v2 fallback, source read, state mutation or current-policy assertion occurs.
pub fn retain_original_admission(
    policy: &CanonicalAdmissionPolicy,
    observation: &AdmissionObservation,
    receipt: &AdmissionReceipt,
    budget: AdmissionBudget,
    cancel: CancelFlag,
) -> Result<OriginalAdmissionEnvelope, AdmissionError> {
    cancel.check()?;
    if read_observation(observation_value(observation)?, policy)? != *observation {
        return Err(AdmissionError::ReceiptMismatch);
    }
    let decision = evaluate(policy, observation, budget, cancel)?;
    let reproduced = issue_receipt(policy, observation, &decision)?;
    if &reproduced != receipt {
        return Err(AdmissionError::ReceiptMismatch);
    }
    let encoded = encode(policy, observation, &decision, &reproduced)?;
    cancel.check()?;
    Ok(OriginalAdmissionEnvelope {
        policy: policy.clone(),
        observation: observation.clone(),
        decision,
        receipt: reproduced,
        encoded,
    })
}

/// Restore untrusted bytes by validation, normalization and actual owner evaluation.
///
/// Unknown fields/versions, altered inputs or outcomes, noncanonical ordering,
/// missing inputs and byte/budget/cancellation refusal return no envelope.
/// Live policy-currentness is deliberately checked separately.
pub fn decode_original_admission(
    bytes: &[u8],
    max_bytes: usize,
    budget: AdmissionBudget,
    cancel: CancelFlag,
) -> Result<OriginalAdmissionEnvelope, AdmissionError> {
    let mut probe = || Ok(());
    decode_original_admission_with_context(bytes, max_bytes, budget, cancel, &mut probe)
}

/// Restore retained v1 admission through the original caller's checkpoint.
///
/// The existing validator, normalizer, evaluator and receipt encoder remain
/// the sole algorithm. Checkpoint refusal before, during or after restoration
/// returns no envelope and never exposes a partial historical value.
pub fn decode_original_admission_with_probe(
    bytes: &[u8],
    max_bytes: usize,
    probe: &mut impl FnMut() -> Result<(), CheckpointFailureV2>,
) -> Result<OriginalAdmissionEnvelope, AdmissionError> {
    decode_original_admission_with_context(
        bytes,
        max_bytes,
        AdmissionBudget::default_budget(),
        CancelFlag::live(),
        probe,
    )
}

fn decode_original_admission_with_context(
    bytes: &[u8],
    max_bytes: usize,
    budget: AdmissionBudget,
    cancel: CancelFlag,
    probe: &mut impl FnMut() -> Result<(), CheckpointFailureV2>,
) -> Result<OriginalAdmissionEnvelope, AdmissionError> {
    checkpoint(probe)?;
    cancel.check()?;
    if max_bytes == 0 || max_bytes > MAX_ORIGINAL_ADMISSION_BYTES {
        return Err(AdmissionError::ObservationInvalid);
    }
    if bytes.len() > max_bytes {
        return Err(AdmissionError::BudgetExhausted);
    }
    let mut object = open(c::parse_canonical_cbor(bytes).map_err(|_| mismatch())?)?;
    if string(take(&mut object, "profile")?)? != ORIGINAL_ADMISSION_PROFILE
        || number(take(&mut object, "schema")?)? != 1
    {
        return Err(AdmissionError::PolicySchemaUnsupported);
    }
    let policy = read_policy(take(&mut object, "policy")?)?;
    let observation = read_observation(take(&mut object, "observation")?, &policy)?;
    let retained_decision = take(&mut object, "decision")?;
    let retained_receipt = take(&mut object, "receipt")?;
    finish(object)?;
    checkpoint(probe)?;
    cancel.check()?;
    let decision = evaluate_with_probe_context(&policy, &observation, budget, cancel, probe)?;
    let receipt = issue_receipt(&policy, &observation, &decision)?;
    // Comparison with complete owner values refuses unknown or missing fields
    // inside the decision/receipt, without constructing either from assertions.
    if decision_value(&decision)? != retained_decision
        || receipt_value(&receipt)? != retained_receipt
    {
        return Err(AdmissionError::ReceiptMismatch);
    }
    let encoded = encode(&policy, &observation, &decision, &receipt)?;
    checkpoint(probe)?;
    if encoded != bytes {
        return Err(AdmissionError::ReceiptMismatch);
    }
    checkpoint(probe)?;
    cancel.check()?;
    Ok(OriginalAdmissionEnvelope {
        policy,
        observation,
        decision,
        receipt,
        encoded,
    })
}

fn mismatch() -> AdmissionError {
    AdmissionError::ReceiptMismatch
}
fn object(fields: Vec<(&str, V)>) -> Result<V, AdmissionError> {
    c::BoundedMap::from_entries(
        fields
            .into_iter()
            .map(|(key, value)| c::CanonicalKey::new_non_empty(key).map(|key| (key, value)))
            .collect::<Result<Vec<_>, _>>()
            .map_err(|_| mismatch())?,
    )
    .map(V::Object)
    .map_err(|_| mismatch())
}
fn text(value: &str) -> Result<V, AdmissionError> {
    c::CanonicalText::new(value)
        .map(V::Text)
        .map_err(|_| mismatch())
}
fn bytes(value: &[u8]) -> Result<V, AdmissionError> {
    c::BoundedBytes::new(value.to_vec())
        .map(V::Bytes)
        .map_err(|_| mismatch())
}
fn list(values: impl Iterator<Item = Result<V, AdmissionError>>) -> Result<V, AdmissionError> {
    c::BoundedList::new(values.collect::<Result<Vec<_>, _>>()?)
        .map(V::Array)
        .map_err(|_| mismatch())
}
fn optional(value: Option<V>) -> V {
    value.unwrap_or(V::Null)
}
fn optional_text(value: Option<&str>) -> Result<V, AdmissionError> {
    value.map(text).transpose().map(optional)
}
fn open(value: V) -> Result<Object, AdmissionError> {
    Object::from_value(value, "original_admission").map_err(|_| mismatch())
}
fn take(value: &mut Object, key: &'static str) -> Result<V, AdmissionError> {
    value.take_required(key).map_err(|_| mismatch())
}
fn finish(value: Object) -> Result<(), AdmissionError> {
    value
        .finish()
        .map_err(|_| AdmissionError::PolicyUnknownField)
}
fn string(value: V) -> Result<String, AdmissionError> {
    if let V::Text(value) = value {
        Ok(value.into_string())
    } else {
        Err(mismatch())
    }
}
fn number(value: V) -> Result<u64, AdmissionError> {
    if let V::U64(value) = value {
        Ok(value)
    } else {
        Err(mismatch())
    }
}
fn boolean(value: V) -> Result<bool, AdmissionError> {
    if let V::Bool(value) = value {
        Ok(value)
    } else {
        Err(mismatch())
    }
}
fn array(value: V) -> Result<Vec<V>, AdmissionError> {
    if let V::Array(value) = value {
        Ok(value.into_vec())
    } else {
        Err(mismatch())
    }
}
fn nullable<T>(
    value: V,
    parse: impl FnOnce(V) -> Result<T, AdmissionError>,
) -> Result<Option<T>, AdmissionError> {
    if value == V::Null {
        Ok(None)
    } else {
        parse(value).map(Some)
    }
}
fn rule_value(rule: &CanonicalRule) -> Result<V, AdmissionError> {
    object(vec![
        ("id", text(&rule.id)?),
        ("precedence", V::U64(u64::from(rule.precedence))),
        ("source_class", text(rule.source_class.as_str())?),
        ("operator", text(rule.operator.as_str())?),
        ("effect", text(rule.effect.as_str())?),
        ("reason", text(rule.reason.as_str())?),
        ("size_threshold", optional(rule.size_threshold.map(V::U64))),
        (
            "sensitivity_threshold",
            optional_text(rule.sensitivity_threshold.map(super::SensitivityLevel::as_str))?,
        ),
        (
            "locator_class",
            optional_text(rule.locator_class.map(super::LocatorClass::as_str))?,
        ),
        ("pattern", optional_text(rule.pattern.as_deref())?),
    ])
}
pub(super) fn policy_value(policy: &CanonicalAdmissionPolicy) -> Result<V, AdmissionError> {
    object(vec![
        ("schema", V::U64(u64::from(POLICY_SCHEMA_VERSION))),
        ("revision", V::U64(policy.revision.get())),
        ("rules", list(policy.rules.iter().map(rule_value))?),
        ("max_file_bytes", V::U64(policy.max_file_bytes)),
        ("allow_generated", V::Bool(policy.allow_generated)),
        ("allow_vendor", V::Bool(policy.allow_vendor)),
        ("allow_binary", V::Bool(policy.allow_binary)),
    ])
}
fn read_rule(value: V) -> Result<UnvalidatedRuleInput, AdmissionError> {
    let mut fields = open(value)?;
    let rule = UnvalidatedRuleInput {
        id: string(take(&mut fields, "id")?)?,
        precedence: u32::try_from(number(take(&mut fields, "precedence")?)?)
            .map_err(|_| mismatch())?,
        source_class: string(take(&mut fields, "source_class")?)?,
        operator: string(take(&mut fields, "operator")?)?,
        effect: string(take(&mut fields, "effect")?)?,
        reason: string(take(&mut fields, "reason")?)?,
        size_threshold: nullable(take(&mut fields, "size_threshold")?, number)?,
        sensitivity_threshold: nullable(take(&mut fields, "sensitivity_threshold")?, string)?,
        locator_class: nullable(take(&mut fields, "locator_class")?, string)?,
        pattern: nullable(take(&mut fields, "pattern")?, string)?,
        unknown_fields: vec![],
    };
    finish(fields)?;
    Ok(rule)
}
fn read_policy(value: V) -> Result<CanonicalAdmissionPolicy, AdmissionError> {
    let mut fields = open(value)?;
    let schema_version =
        u32::try_from(number(take(&mut fields, "schema")?)?).map_err(|_| mismatch())?;
    let revision = number(take(&mut fields, "revision")?)?;
    let rules = array(take(&mut fields, "rules")?)?;
    if rules.is_empty() || rules.len() > MAX_POLICY_RULES {
        return Err(AdmissionError::PolicyConflict);
    }
    let raw = UnvalidatedPolicyInput {
        schema_version,
        revision,
        rules: rules
            .into_iter()
            .map(read_rule)
            .collect::<Result<Vec<_>, _>>()?,
        max_file_bytes: number(take(&mut fields, "max_file_bytes")?)?,
        allow_generated: boolean(take(&mut fields, "allow_generated")?)?,
        allow_vendor: boolean(take(&mut fields, "allow_vendor")?)?,
        allow_binary: boolean(take(&mut fields, "allow_binary")?)?,
        unknown_fields: vec![],
    };
    finish(fields)?;
    normalize_policy(&validate_policy_schema(&raw)?)
}
fn observation_value(observation: &AdmissionObservation) -> Result<V, AdmissionError> {
    object(vec![
        ("locator_class", text(observation.locator_class.as_str())?),
        ("location_class", text(observation.location_class.as_str())?),
        ("source_kind", text(observation.source_kind.as_str())?),
        ("source_class", text(observation.source_class.as_str())?),
        ("byte_size", optional(observation.byte_size.map(V::U64))),
        (
            "is_generated",
            optional(observation.is_generated.map(V::Bool)),
        ),
        ("is_vendor", optional(observation.is_vendor.map(V::Bool))),
        ("is_binary", optional(observation.is_binary.map(V::Bool))),
        ("is_system", optional(observation.is_system.map(V::Bool))),
        (
            "sensitivity",
            optional_text(observation.sensitivity.map(super::SensitivityLevel::as_str))?,
        ),
        (
            "detector_id",
            optional_text(observation.detector_id.as_deref())?,
        ),
        (
            "profile_id",
            optional_text(observation.profile_id.as_deref())?,
        ),
        (
            "unavailable_fields",
            list(observation.unavailable_fields.iter().map(|v| text(v)))?,
        ),
    ])
}
fn read_observation(
    value: V,
    policy: &CanonicalAdmissionPolicy,
) -> Result<AdmissionObservation, AdmissionError> {
    let mut fields = open(value)?;
    let raw = UnvalidatedObservationInput {
        locator_class: string(take(&mut fields, "locator_class")?)?,
        location_class: string(take(&mut fields, "location_class")?)?,
        source_kind: string(take(&mut fields, "source_kind")?)?,
        source_class: string(take(&mut fields, "source_class")?)?,
        byte_size: nullable(take(&mut fields, "byte_size")?, number)?,
        is_generated: nullable(take(&mut fields, "is_generated")?, boolean)?,
        is_vendor: nullable(take(&mut fields, "is_vendor")?, boolean)?,
        is_binary: nullable(take(&mut fields, "is_binary")?, boolean)?,
        is_system: nullable(take(&mut fields, "is_system")?, boolean)?,
        sensitivity: nullable(take(&mut fields, "sensitivity")?, string)?,
        detector_id: nullable(take(&mut fields, "detector_id")?, string)?,
        profile_id: nullable(take(&mut fields, "profile_id")?, string)?,
        unavailable_fields: array(take(&mut fields, "unavailable_fields")?)?
            .into_iter()
            .map(string)
            .collect::<Result<Vec<_>, _>>()?,
        unknown_fields: vec![],
    };
    finish(fields)?;
    validate_observation(&raw, policy, DEFAULT_ADMISSION_LIMITS)
}
fn decision_value(decision: &AdmissionDecision) -> Result<V, AdmissionError> {
    object(vec![
        ("policy_revision", V::U64(decision.policy_revision.get())),
        (
            "policy_fingerprint",
            bytes(decision.policy_fingerprint.as_bytes())?,
        ),
        (
            "observation_digest",
            bytes(decision.observation_digest.as_bytes())?,
        ),
        ("outcome", text(decision.outcome.as_str())?),
        (
            "reasons",
            list(decision.reason_codes.iter().map(|v| text(v.as_str())))?,
        ),
        (
            "matched_rule_ids",
            list(decision.matched_rule_ids.iter().map(|v| text(v)))?,
        ),
        ("sensitivity", text(decision.sensitivity.as_str())?),
        ("explanation", text(&decision.explanation)?),
    ])
}
fn receipt_value(receipt: &AdmissionReceipt) -> Result<V, AdmissionError> {
    object(vec![
        ("schema", V::U64(u64::from(receipt.schema_version))),
        ("policy_revision", V::U64(receipt.policy_revision.get())),
        (
            "policy_fingerprint",
            bytes(receipt.policy_fingerprint.as_bytes())?,
        ),
        (
            "observation_digest",
            bytes(receipt.observation_digest.as_bytes())?,
        ),
        ("outcome", text(receipt.outcome.as_str())?),
        (
            "reasons",
            list(receipt.reason_codes.iter().map(|v| text(v.as_str())))?,
        ),
        (
            "matched_rule_ids",
            list(receipt.matched_rule_ids.iter().map(|v| text(v)))?,
        ),
        ("sensitivity", text(receipt.sensitivity.as_str())?),
        ("receipt_digest", bytes(&receipt.receipt_digest)?),
    ])
}
fn encode(
    policy: &CanonicalAdmissionPolicy,
    observation: &AdmissionObservation,
    decision: &AdmissionDecision,
    receipt: &AdmissionReceipt,
) -> Result<Vec<u8>, AdmissionError> {
    let value = object(vec![
        ("profile", text(ORIGINAL_ADMISSION_PROFILE)?),
        ("schema", V::U64(1)),
        ("policy", policy_value(policy)?),
        ("observation", observation_value(observation)?),
        ("decision", decision_value(decision)?),
        ("receipt", receipt_value(receipt)?),
    ])?;
    let encoded = c::to_canonical_cbor(&value).map_err(|_| mismatch())?;
    if encoded.as_slice().len() > MAX_ORIGINAL_ADMISSION_BYTES {
        return Err(AdmissionError::BudgetExhausted);
    }
    Ok(encoded.as_slice().to_vec())
}

#[cfg(test)]
mod tests {
    use super::{
        MAX_ORIGINAL_ADMISSION_BYTES, OriginalAdmissionEnvelope, decode_original_admission, encode,
        retain_original_admission,
    };
    use crate::{
        AdmissionBudget, AdmissionError, AdmissionOutcome, CancelFlag, DEFAULT_ADMISSION_LIMITS,
        UnvalidatedObservationInput, baseline_policy, evaluate, issue_receipt,
        validate_observation,
    };
    use search_contracts::{self as c, CanonicalValue as Value};

    fn build_original(source_class: &str) -> OriginalAdmissionEnvelope {
        let policy = baseline_policy(1, 1024);
        let input = UnvalidatedObservationInput {
            locator_class: "normalized-file".into(),
            location_class: "local-fixed".into(),
            source_kind: "file".into(),
            source_class: source_class.into(),
            byte_size: Some(10),
            is_generated: Some(false),
            is_vendor: Some(false),
            is_binary: Some(false),
            is_system: Some(false),
            sensitivity: Some("public".into()),
            detector_id: Some("test-detector".into()),
            profile_id: Some("test-profile".into()),
            unavailable_fields: vec![],
            unknown_fields: vec![],
        };
        let observation =
            validate_observation(&input, &policy, DEFAULT_ADMISSION_LIMITS).expect("observation");
        let decision = evaluate(
            &policy,
            &observation,
            AdmissionBudget::default_budget(),
            CancelFlag::live(),
        )
        .expect("decision");
        let receipt = issue_receipt(&policy, &observation, &decision).expect("receipt");
        retain_original_admission(
            &policy,
            &observation,
            &receipt,
            AdmissionBudget::default_budget(),
            CancelFlag::live(),
        )
        .expect("retain")
    }
    fn decode(bytes: &[u8]) -> Result<OriginalAdmissionEnvelope, AdmissionError> {
        decode_original_admission(
            bytes,
            MAX_ORIGINAL_ADMISSION_BYTES,
            AdmissionBudget::default_budget(),
            CancelFlag::live(),
        )
    }
    fn decode_with_probe(
        bytes: &[u8],
        probe: &mut impl FnMut() -> Result<(), CheckpointFailureV2>,
    ) -> Result<OriginalAdmissionEnvelope, AdmissionError> {
        decode_original_admission_with_probe(bytes, MAX_ORIGINAL_ADMISSION_BYTES, probe)
    }
    fn edited(bytes: &[u8], key: &str, value: Option<Value>) -> Vec<u8> {
        let Value::Object(fields) = c::parse_canonical_cbor(bytes).expect("canonical input") else {
            panic!("object")
        };
        let mut fields: Vec<_> = fields
            .iter()
            .map(|(key, value)| (key.clone(), value.clone()))
            .collect();
        fields.retain(|(name, _)| name.as_str() != key);
        if let Some(value) = value {
            fields.push((c::CanonicalKey::new_non_empty(key).expect("key"), value));
        }
        c::to_canonical_cbor(&Value::Object(
            c::BoundedMap::from_entries(fields).expect("fields"),
        ))
        .expect("canonical edit")
        .as_slice()
        .to_vec()
    }
    #[test]
    fn original_envelope_roundtrip_keeps_nonallow_and_separates_live_currentness() {
        for source_class in ["regular", "credential"] {
            let original = build_original(source_class);
            let restored = decode(original.encoded()).expect("exact restoration");
            assert_eq!(restored, original);
            assert_eq!(restored.decision(), original.decision());
            assert_eq!(restored.receipt(), original.receipt());
            if source_class == "credential" {
                assert_eq!(restored.receipt().outcome(), AdmissionOutcome::Deny);
            }
            restored
                .verify_current_policy(original.policy(), CancelFlag::live())
                .expect("current fence");
            assert_eq!(
                restored.verify_current_policy(&baseline_policy(2, 1024), CancelFlag::live()),
                Err(AdmissionError::ReceiptStale)
            );
            assert_eq!(
                restored.verify_current_policy(&baseline_policy(1, 2048), CancelFlag::live()),
                Err(AdmissionError::ReceiptStale)
            );
        }
    }
    #[test]
    fn original_envelope_refuses_tampered_inputs_claims_versions_missing_and_unknown_fields() {
        let original = build_original("regular");
        let changed_policy = baseline_policy(2, 1024);
        let changed = encode(
            &changed_policy,
            original.observation(),
            original.decision(),
            original.receipt(),
        )
        .expect("tampered frame");
        assert_eq!(decode(&changed), Err(AdmissionError::ReceiptMismatch));
        let mut observation = original.observation().clone();
        observation.is_vendor = Some(true);
        let changed = encode(
            original.policy(),
            &observation,
            original.decision(),
            original.receipt(),
        )
        .expect("tampered observation");
        assert_eq!(decode(&changed), Err(AdmissionError::ReceiptMismatch));
        let denied = build_original("credential");
        let changed = encode(
            original.policy(),
            original.observation(),
            denied.decision(),
            denied.receipt(),
        )
        .expect("forged decision/receipt");
        assert_eq!(decode(&changed), Err(AdmissionError::ReceiptMismatch));
        assert_eq!(
            decode(&edited(
                original.encoded(),
                "unknown",
                Some(Value::Bool(true))
            )),
            Err(AdmissionError::PolicyUnknownField)
        );
        assert!(decode(&edited(original.encoded(), "observation", None)).is_err());
        assert_eq!(
            decode(&edited(original.encoded(), "schema", Some(Value::U64(2)))),
            Err(AdmissionError::PolicySchemaUnsupported)
        );
        assert!(decode(&original.encoded()[..original.encoded().len() - 1]).is_err());
    }
    #[test]
    fn original_envelope_bounds_and_cancellation_refuse_without_partial_restoration() {
        let original = build_original("regular");
        assert_eq!(
            decode_original_admission(
                original.encoded(),
                original.encoded().len() - 1,
                AdmissionBudget::default_budget(),
                CancelFlag::live()
            ),
            Err(AdmissionError::BudgetExhausted)
        );
        assert_eq!(
            decode_original_admission(
                &[],
                0,
                AdmissionBudget::default_budget(),
                CancelFlag::cancelled()
            ),
            Err(AdmissionError::Cancelled)
        );
        assert_eq!(
            decode_original_admission(
                original.encoded(),
                MAX_ORIGINAL_ADMISSION_BYTES,
                AdmissionBudget {
                    max_rule_evaluations: 0
                },
                CancelFlag::live()
            ),
            Err(AdmissionError::BudgetExhausted)
        );
    }

    #[test]
    fn original_envelope_probe_refuses_before_during_and_after_restore() {
        let original = build_original("regular");
        let mut before_calls = 0;
        assert_eq!(
            decode_with_probe(&original.encoded(), &mut || {
                before_calls += 1;
                Err(CheckpointFailureV2::Cancelled)
            }),
            Err(AdmissionError::Cancelled)
        );
        assert_eq!(before_calls, 1);

        let mut total_calls = 0;
        assert_eq!(
            decode_with_probe(&original.encoded(), &mut || {
                total_calls += 1;
                Ok(())
            }),
            Ok(original.clone())
        );
        assert!(total_calls >= 4);

        let mut during_calls = 0;
        let during_failure = total_calls / 2;
        assert_eq!(
            decode_with_probe(&original.encoded(), &mut || {
                during_calls += 1;
                if during_calls == during_failure {
                    Err(CheckpointFailureV2::Cancelled)
                } else {
                    Ok(())
                }
            }),
            Err(AdmissionError::Cancelled)
        );
        assert_eq!(during_calls, during_failure);

        let mut after_calls = 0;
        assert_eq!(
            decode_with_probe(&original.encoded(), &mut || {
                after_calls += 1;
                if after_calls == total_calls {
                    Err(CheckpointFailureV2::DeadlineExceeded)
                } else {
                    Ok(())
                }
            }),
            Err(AdmissionError::BudgetExhausted)
        );
        assert_eq!(after_calls, total_calls);
    }
}

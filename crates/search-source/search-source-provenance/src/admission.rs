//! Exact original-admission retention through the sole v2 semantic owners.

use std::sync::Arc;

use search_contracts::{
    BoundedBytes, BoundedList, CanonicalText, CanonicalValue as V, parse_canonical_cbor,
    to_canonical_cbor,
};
use search_source_admission::evaluation_v2::{DEFAULT_EVALUATION_LIMITS, evaluate_v2};
use search_source_admission::observation_v2::{
    AdmissionObservationV2, MAX_OBSERVATION_PREIMAGE_BYTES,
};
use search_source_admission::original_inputs_v2::{
    reconstitute_retained_observation_v2, retain_observation_inputs_v2,
};
use search_source_admission::policy_v2::{
    CheckpointFailureV2, CompiledAdmissionPolicyV2, POLICY_PROFILE, POLICY_SCHEMA_VERSION,
};
use search_source_admission::{AdmissionError, UnvalidatedObservationInput};
use search_source_admission_receipt::receipt_v2::{
    AdmissionReceiptV2, DEFAULT_RECEIPT_LIMITS, VerifiedAdmissionReceiptV2, issue_receipt_v2,
    verify_receipt_v2,
};

use crate::RetainedPolicyV2;

/// Closed original-admission archive, distinct from the receipt fingerprint schema.
pub const ORIGINAL_ADMISSION_PROFILE_V2: &str = "eliot.source-original-admission@2";
/// Complete archive byte ceiling; actual durable envelope limits remain separate.
pub const MAX_RETAINED_ADMISSION_BYTES: usize = 256 * 1024;

type Probe<'a> = &'a mut dyn FnMut() -> Result<(), CheckpointFailureV2>;

fn checkpoint(probe: Probe<'_>) -> Result<(), AdmissionError> {
    probe().map_err(|error| match error {
        CheckpointFailureV2::Cancelled => AdmissionError::Cancelled,
        CheckpointFailureV2::DeadlineExceeded => AdmissionError::BudgetExhausted,
    })
}

struct Original {
    policy: Arc<RetainedPolicyV2>,
    observation: AdmissionObservationV2,
    receipt: AdmissionReceiptV2,
    encoded: Vec<u8>,
}

/// Reproduced historical evidence; this value grants no current admission.
#[derive(Clone)]
pub struct OriginalAdmissionV2(Arc<Original>);

impl PartialEq for OriginalAdmissionV2 {
    fn eq(&self, other: &Self) -> bool {
        self.0.encoded == other.0.encoded
            && self.0.policy.encoded_inputs() == other.0.policy.encoded_inputs()
    }
}
impl Eq for OriginalAdmissionV2 {}

impl core::fmt::Debug for OriginalAdmissionV2 {
    fn fmt(&self, formatter: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        formatter
            .debug_struct("OriginalAdmissionV2")
            .field("encoded_bytes", &self.0.encoded.len())
            .finish_non_exhaustive()
    }
}

impl OriginalAdmissionV2 {
    /// Retain complete owner-normalized inputs and issue the original receipt.
    ///
    /// # Errors
    /// Returns the original v2 owner error or checkpoint failure without a value.
    pub fn retain(
        policy: Arc<RetainedPolicyV2>,
        observation: &AdmissionObservationV2,
        probe: &mut impl FnMut() -> Result<(), CheckpointFailureV2>,
    ) -> Result<Self, AdmissionError> {
        checkpoint(probe)?;
        let raw = retain_observation_inputs_v2(observation, probe)?;
        let locator = observation
            .relative_locator()
            .map(search_source_admission::observation_v2::AdmissionRelativeLocator::as_bytes);
        let restored = reconstitute_retained_observation_v2(
            &raw,
            locator,
            MAX_OBSERVATION_PREIMAGE_BYTES,
            probe,
        )?;
        if &restored != observation {
            return Err(AdmissionError::ObservationInvalid);
        }
        let receipt = reproduce(policy.compiled(), &restored, probe)?;
        let encoded = encode(&policy, &restored, &receipt, probe)?;
        checkpoint(probe)?;
        let retained = Self(Arc::new(Original {
            policy,
            observation: restored,
            receipt,
            encoded,
        }));
        checkpoint(probe)?;
        Ok(retained)
    }

    /// Rebuild untrusted stored bytes through the original policy and owners.
    ///
    /// No supplied fingerprint or receipt field constructs a trusted model.
    /// The full regenerated archive must equal the exact canonical stored bytes.
    ///
    /// # Errors
    /// Refuses oversized, altered, noncanonical, foreign or incomplete evidence;
    /// original cancellation/deadline failure returns no partial value.
    pub fn restore(
        policy: Arc<RetainedPolicyV2>,
        bytes: &[u8],
        probe: &mut impl FnMut() -> Result<(), CheckpointFailureV2>,
    ) -> Result<Self, AdmissionError> {
        checkpoint(probe)?;
        if bytes.is_empty() || bytes.len() > MAX_RETAINED_ADMISSION_BYTES {
            return Err(AdmissionError::BudgetExhausted);
        }
        let value = parse_canonical_cbor(bytes).map_err(|_| malformed())?;
        checkpoint(probe)?;
        let rows = rows::<7>(&value)?;
        if rows[0] != V::U64(2)
            || rows[1] != text(ORIGINAL_ADMISSION_PROFILE_V2)?
            || rows[2] != policy_reference(&policy)?
        {
            return Err(malformed());
        }
        let raw = decode_metadata(&rows[3])?;
        let locator = match &rows[4] {
            V::Null => None,
            V::Bytes(value) => Some(value.as_slice()),
            _ => return Err(malformed()),
        };
        checkpoint(probe)?;
        let observation = reconstitute_retained_observation_v2(
            &raw,
            locator,
            MAX_OBSERVATION_PREIMAGE_BYTES,
            probe,
        )?;
        let retained = Self::retain(policy, &observation, probe)?;
        if retained.encoded_source_evidence() != bytes {
            return Err(malformed());
        }
        checkpoint(probe)?;
        Ok(retained)
    }

    /// Complete canonical source evidence, containing a policy reference only.
    #[must_use]
    pub fn encoded_source_evidence(&self) -> &[u8] {
        &self.0.encoded
    }
    /// Shared complete original policy; never a current Root-policy grant.
    #[must_use]
    pub fn policy(&self) -> &Arc<RetainedPolicyV2> {
        &self.0.policy
    }
    /// Exact original owner-reconstituted observation.
    #[must_use]
    pub fn observation(&self) -> &AdmissionObservationV2 {
        &self.0.observation
    }
    /// Complete owner-issued original receipt, including the complete decision.
    #[must_use]
    pub fn receipt(&self) -> &AdmissionReceiptV2 {
        &self.0.receipt
    }

    /// Separately verify the original receipt against an actual current policy.
    ///
    /// # Errors
    /// Returns the sole receipt verifier's stale, mismatch or checkpoint error.
    pub fn verify_current(
        &self,
        current_policy: &CompiledAdmissionPolicyV2,
        probe: &mut impl FnMut() -> Result<(), CheckpointFailureV2>,
    ) -> Result<VerifiedAdmissionReceiptV2, AdmissionError> {
        verify_receipt_v2(
            &self.0.receipt,
            current_policy,
            &self.0.observation,
            DEFAULT_RECEIPT_LIMITS,
            probe,
        )
    }
}

fn reproduce(
    policy: &CompiledAdmissionPolicyV2,
    observation: &AdmissionObservationV2,
    probe: &mut impl FnMut() -> Result<(), CheckpointFailureV2>,
) -> Result<AdmissionReceiptV2, AdmissionError> {
    checkpoint(probe)?;
    let mut scratch = policy.new_match_buffer();
    let decision = evaluate_v2(
        policy,
        observation,
        &mut scratch,
        DEFAULT_EVALUATION_LIMITS,
        probe,
    )?;
    issue_receipt_v2(
        policy,
        observation,
        &decision,
        DEFAULT_RECEIPT_LIMITS,
        probe,
    )
}

fn encode(
    policy: &RetainedPolicyV2,
    observation: &AdmissionObservationV2,
    receipt: &AdmissionReceiptV2,
    probe: &mut impl FnMut() -> Result<(), CheckpointFailureV2>,
) -> Result<Vec<u8>, AdmissionError> {
    let raw = retain_observation_inputs_v2(observation, probe)?;
    let decision = receipt.decision();
    let reasons = texts(
        decision.reason_codes().iter().map(|reason| reason.as_str()),
        probe,
    )?;
    let matches = texts(
        decision.matched_rule_ids().iter().map(String::as_str),
        probe,
    )?;
    let decision_stamp = array(vec![
        text(decision.outcome().as_str())?,
        reasons,
        matches,
        text(decision.sensitivity().as_str())?,
        V::U64(decision.policy_revision().get()),
        binary(decision.policy_fingerprint().as_bytes())?,
        binary(decision.observation_fingerprint().as_bytes())?,
        binary(decision.fingerprint().as_bytes())?,
    ])?;
    let receipt_stamp = array(vec![
        V::U64(u64::from(receipt.schema_version())),
        decision_stamp,
        binary(receipt.fingerprint().as_bytes())?,
    ])?;
    let value = array(vec![
        V::U64(2),
        text(ORIGINAL_ADMISSION_PROFILE_V2)?,
        policy_reference(policy)?,
        encode_metadata(&raw, probe)?,
        observation
            .relative_locator()
            .map_or(Ok(V::Null), |locator| binary(locator.as_bytes()))?,
        binary(observation.fingerprint().as_bytes())?,
        receipt_stamp,
    ])?;
    checkpoint(probe)?;
    let bytes = to_canonical_cbor(&value).map_err(|_| malformed())?;
    if bytes.as_slice().len() > MAX_RETAINED_ADMISSION_BYTES {
        return Err(AdmissionError::BudgetExhausted);
    }
    checkpoint(probe)?;
    let output = bytes.as_slice().to_vec();
    checkpoint(probe)?;
    Ok(output)
}

fn policy_reference(policy: &RetainedPolicyV2) -> Result<V, AdmissionError> {
    array(vec![
        V::U64(u64::from(POLICY_SCHEMA_VERSION)),
        text(POLICY_PROFILE)?,
        V::U64(policy.compiled().revision().get()),
        binary(policy.fingerprint().as_bytes())?,
    ])
}

fn encode_metadata(
    raw: &UnvalidatedObservationInput,
    probe: Probe<'_>,
) -> Result<V, AdmissionError> {
    array(vec![
        text(&raw.locator_class)?,
        text(&raw.location_class)?,
        text(&raw.source_kind)?,
        text(&raw.source_class)?,
        raw.byte_size.map_or(V::Null, V::U64),
        raw.is_generated.map_or(V::Null, V::Bool),
        raw.is_vendor.map_or(V::Null, V::Bool),
        raw.is_binary.map_or(V::Null, V::Bool),
        raw.is_system.map_or(V::Null, V::Bool),
        optional_text(raw.sensitivity.as_deref())?,
        optional_text(raw.detector_id.as_deref())?,
        optional_text(raw.profile_id.as_deref())?,
        texts(raw.unavailable_fields.iter().map(String::as_str), probe)?,
    ])
}

fn decode_metadata(value: &V) -> Result<UnvalidatedObservationInput, AdmissionError> {
    let r = rows::<13>(value)?;
    let V::Array(unavailable) = &r[12] else {
        return Err(malformed());
    };
    Ok(UnvalidatedObservationInput {
        locator_class: string(&r[0])?,
        location_class: string(&r[1])?,
        source_kind: string(&r[2])?,
        source_class: string(&r[3])?,
        byte_size: match r[4] {
            V::Null => None,
            V::U64(value) => Some(value),
            _ => return Err(malformed()),
        },
        is_generated: optional_bool(&r[5])?,
        is_vendor: optional_bool(&r[6])?,
        is_binary: optional_bool(&r[7])?,
        is_system: optional_bool(&r[8])?,
        sensitivity: optional_string(&r[9])?,
        detector_id: optional_string(&r[10])?,
        profile_id: optional_string(&r[11])?,
        unavailable_fields: unavailable.iter().map(string).collect::<Result<_, _>>()?,
        unknown_fields: Vec::new(),
    })
}

fn rows<const N: usize>(value: &V) -> Result<&[V; N], AdmissionError> {
    let V::Array(values) = value else {
        return Err(malformed());
    };
    values.as_slice().try_into().map_err(|_| malformed())
}
const fn malformed() -> AdmissionError {
    AdmissionError::ReceiptMismatch
}
fn text(value: &str) -> Result<V, AdmissionError> {
    CanonicalText::new(value)
        .map(V::Text)
        .map_err(|_| malformed())
}
fn binary(value: &[u8]) -> Result<V, AdmissionError> {
    BoundedBytes::new(value.to_vec())
        .map(V::Bytes)
        .map_err(|_| malformed())
}
fn array(values: Vec<V>) -> Result<V, AdmissionError> {
    BoundedList::new(values)
        .map(V::Array)
        .map_err(|_| malformed())
}
fn optional_text(value: Option<&str>) -> Result<V, AdmissionError> {
    value.map_or(Ok(V::Null), text)
}
fn texts<'a>(values: impl Iterator<Item = &'a str>, probe: Probe<'_>) -> Result<V, AdmissionError> {
    let mut output = Vec::new();
    for value in values {
        checkpoint(probe)?;
        output.push(text(value)?);
    }
    checkpoint(probe)?;
    array(output)
}
fn string(value: &V) -> Result<String, AdmissionError> {
    if let V::Text(value) = value {
        Ok(value.as_str().to_owned())
    } else {
        Err(malformed())
    }
}
fn optional_string(value: &V) -> Result<Option<String>, AdmissionError> {
    if value == &V::Null {
        Ok(None)
    } else {
        string(value).map(Some)
    }
}
const fn optional_bool(value: &V) -> Result<Option<bool>, AdmissionError> {
    match value {
        V::Null => Ok(None),
        V::Bool(value) => Ok(Some(*value)),
        _ => Err(malformed()),
    }
}

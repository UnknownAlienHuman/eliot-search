//! Complete normalized observation inputs for explicit versioned provenance retention.
//!
//! Reconstitution uses the sole field validator and immutable v2 constructor.
//! No v1 policy evaluation, supplied fingerprint, native or admission grant exists.

use crate::observation_v2::{
    AdmissionObservationV2, AdmissionRelativeLocator, validate_encoded_ceiling,
};
use crate::policy_v2::{CheckpointFailureV2, CompiledAdmissionPolicyV2, checkpoint};
use crate::{AdmissionError, DEFAULT_ADMISSION_LIMITS, UnvalidatedObservationInput};

/// Reconstitute complete metadata under the actual compiled v2 policy ceiling.
///
/// The original caller owns the checkpoint and operation budgets. This derives
/// identity anew and neither constructs nor evaluates a v1 policy.
///
/// # Errors
/// Invalid/incomplete fields, locator or ceiling return no observation. Original
/// cancellation/deadline refusal wins at entry and returns no value after copying
/// or construction, preserving the existing closed error translation.
pub fn reconstitute_observation_v2(
    input: &UnvalidatedObservationInput,
    relative_locator: Option<&[u8]>,
    policy: &CompiledAdmissionPolicyV2,
    max_encoded_bytes: usize,
    probe: &mut impl FnMut() -> Result<(), CheckpointFailureV2>,
) -> Result<AdmissionObservationV2, AdmissionError> {
    reconstitute_fields(
        input,
        relative_locator,
        policy.rules().len(),
        policy.section().max_file_bytes(),
        max_encoded_bytes,
        probe,
    )
}

/// Reconstitute historical metadata independently of a policy's read-size cap.
///
/// The size is an observed fact. The structural ceiling is the maximum accepted
/// metadata range of the sole field validator, not a current policy permission.
/// Evaluation/receipt verification still apply the actual original/current policy.
/// No v1 policy is constructed and no observation identity is changed.
///
/// # Errors
/// Invalid fields/locator/encoded limits or caller checkpoint failures return no value.
pub fn reconstitute_retained_observation_v2(
    input: &UnvalidatedObservationInput,
    relative_locator: Option<&[u8]>,
    max_encoded_bytes: usize,
    probe: &mut impl FnMut() -> Result<(), CheckpointFailureV2>,
) -> Result<AdmissionObservationV2, AdmissionError> {
    reconstitute_fields(
        input,
        relative_locator,
        0,
        crate::DEFAULT_MAX_FILE_BYTES,
        max_encoded_bytes,
        probe,
    )
}

fn reconstitute_fields(
    input: &UnvalidatedObservationInput,
    relative_locator: Option<&[u8]>,
    rule_count: usize,
    metadata_size_ceiling: u64,
    max_encoded_bytes: usize,
    probe: &mut impl FnMut() -> Result<(), CheckpointFailureV2>,
) -> Result<AdmissionObservationV2, AdmissionError> {
    checkpoint(probe)?;
    validate_encoded_ceiling(max_encoded_bytes)?;
    let metadata = crate::validate_observation_fields(
        input,
        rule_count,
        metadata_size_ceiling,
        DEFAULT_ADMISSION_LIMITS,
    )?;
    let locator = relative_locator
        .map(AdmissionRelativeLocator::from_bytes)
        .transpose()?;
    checkpoint(probe)?;
    let observation =
        AdmissionObservationV2::new_with_encoded_limit(metadata, locator, max_encoded_bytes)?;
    checkpoint(probe)?;
    Ok(observation)
}

/// Retain all normalized flags, IDs and explicit unavailable metadata markers.
///
/// The exact locator remains separately available through `relative_locator()`.
/// These bounded fields are data only; validated metadata has no unknown fields.
///
/// # Errors
/// Original-caller checkpoint refusal returns no partial input, including when
/// cancellation is observed after the bounded copies.
pub fn retain_observation_inputs_v2(
    observation: &AdmissionObservationV2,
    probe: &mut impl FnMut() -> Result<(), CheckpointFailureV2>,
) -> Result<UnvalidatedObservationInput, AdmissionError> {
    checkpoint(probe)?;
    let metadata = observation.metadata();
    let input = UnvalidatedObservationInput {
        locator_class: metadata.locator_class.as_str().to_owned(),
        location_class: metadata.location_class.as_str().to_owned(),
        source_kind: metadata.source_kind.as_str().to_owned(),
        source_class: metadata.source_class.as_str().to_owned(),
        byte_size: metadata.byte_size,
        is_generated: metadata.is_generated,
        is_vendor: metadata.is_vendor,
        is_binary: metadata.is_binary,
        is_system: metadata.is_system,
        sensitivity: metadata.sensitivity.map(|value| value.as_str().to_owned()),
        detector_id: metadata.detector_id.clone(),
        profile_id: metadata.profile_id.clone(),
        unavailable_fields: metadata.unavailable_fields.iter().cloned().collect(),
        unknown_fields: Vec::new(),
    };
    checkpoint(probe)?;
    Ok(input)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::observation_v2::MAX_OBSERVATION_PREIMAGE_BYTES;
    use crate::policy_v2::{
        DEFAULT_COMPILE_LIMITS, POLICY_SCHEMA_VERSION, PolicyInputV2, compile_policy_v2,
    };
    use crate::{
        AcceptedCapabilities, BASELINE_PROFILE, SectionPlatform, compiled_defaults,
        validate_section,
    };

    fn fixture() -> (CompiledAdmissionPolicyV2, UnvalidatedObservationInput) {
        let mut section = compiled_defaults();
        section.max_file_bytes = 8;
        let section = validate_section(
            &section,
            &SectionPlatform {
                os: "windows".into(),
                arch: "x64".into(),
            },
            &AcceptedCapabilities {
                profiles: vec![BASELINE_PROFILE.into()],
            },
        )
        .unwrap();
        let policy = compile_policy_v2(
            &PolicyInputV2 {
                schema_version: POLICY_SCHEMA_VERSION,
                revision: 7,
                section,
                rules: vec![],
                unknown_fields: vec![],
            },
            DEFAULT_COMPILE_LIMITS,
            &mut || Ok(()),
        )
        .unwrap();
        let input = UnvalidatedObservationInput {
            locator_class: "normalized-file".into(),
            location_class: "local-fixed".into(),
            source_kind: "file".into(),
            source_class: "regular".into(),
            byte_size: Some(8),
            is_generated: Some(true),
            is_vendor: Some(false),
            is_binary: Some(true),
            is_system: Some(false),
            sensitivity: Some(crate::SensitivityLevel::Internal.as_str().into()),
            detector_id: Some("detector:retention".into()),
            profile_id: Some(BASELINE_PROFILE.into()),
            unavailable_fields: vec![],
            unknown_fields: vec![],
        };
        (policy, input)
    }

    #[test]
    fn original_input_roundtrip_preserves_every_flag_id_and_exact_locator() {
        let (policy, input) = fixture();
        let path = "Src/é.rs".as_bytes();
        let restored = reconstitute_observation_v2(
            &input,
            Some(path),
            &policy,
            MAX_OBSERVATION_PREIMAGE_BYTES,
            &mut || Ok(()),
        )
        .unwrap();
        let exported = retain_observation_inputs_v2(&restored, &mut || Ok(())).unwrap();
        assert_eq!(exported, input);
        assert_eq!(restored.relative_locator().unwrap().as_bytes(), path);
        let legacy_metadata = crate::validate_observation(
            &input,
            &crate::baseline_policy(7, 8),
            DEFAULT_ADMISSION_LIMITS,
        )
        .unwrap();
        let existing = AdmissionObservationV2::new(
            legacy_metadata,
            Some(AdmissionRelativeLocator::from_bytes(path).unwrap()),
        )
        .unwrap();
        assert_eq!(
            restored, existing,
            "existing observation identity must remain exact"
        );
        assert_eq!(
            restored,
            reconstitute_observation_v2(
                &exported,
                Some(path),
                &policy,
                MAX_OBSERVATION_PREIMAGE_BYTES,
                &mut || Ok(())
            )
            .unwrap()
        );
    }

    #[test]
    fn original_input_uses_actual_v2_ceiling_and_retains_explicit_gaps() {
        let (policy, mut input) = fixture();
        let mut invalid = input.clone();
        invalid.sensitivity = Some("project".into());
        assert_eq!(
            reconstitute_observation_v2(
                &invalid,
                None,
                &policy,
                MAX_OBSERVATION_PREIMAGE_BYTES,
                &mut || Ok(())
            ),
            Err(AdmissionError::ObservationInvalid)
        );
        input.byte_size = Some(130);
        assert_eq!(
            reconstitute_observation_v2(
                &input,
                None,
                &policy,
                MAX_OBSERVATION_PREIMAGE_BYTES,
                &mut || Ok(())
            ),
            Err(AdmissionError::ObservationInvalid)
        );
        input.byte_size = Some(129);
        assert!(
            reconstitute_observation_v2(
                &input,
                None,
                &policy,
                MAX_OBSERVATION_PREIMAGE_BYTES,
                &mut || Ok(())
            )
            .is_ok()
        );
        input.byte_size = Some(8);
        input.is_vendor = None;
        input.detector_id = None;
        input.unavailable_fields = vec!["vendor-flag".into(), "detector-id".into()];
        let observation = reconstitute_observation_v2(
            &input,
            None,
            &policy,
            MAX_OBSERVATION_PREIMAGE_BYTES,
            &mut || Ok(()),
        )
        .unwrap();
        let exported = retain_observation_inputs_v2(&observation, &mut || Ok(())).unwrap();
        assert_eq!(exported.is_vendor, None);
        assert_eq!(exported.detector_id, None);
        assert_eq!(exported.unavailable_fields, ["detector-id", "vendor-flag"]);
        assert!(observation.relative_locator().is_none());
        assert_eq!(
            observation,
            reconstitute_observation_v2(
                &exported,
                None,
                &policy,
                MAX_OBSERVATION_PREIMAGE_BYTES,
                &mut || Ok(())
            )
            .unwrap()
        );
    }

    #[test]
    fn original_input_checkpoint_refusals_return_no_partial_value() {
        let (policy, input) = fixture();
        for (failure, expected) in [
            (CheckpointFailureV2::Cancelled, AdmissionError::Cancelled),
            (
                CheckpointFailureV2::DeadlineExceeded,
                AdmissionError::BudgetExhausted,
            ),
        ] {
            assert_eq!(
                reconstitute_observation_v2(&input, None, &policy, 0, &mut || Err(failure)),
                Err(expected)
            );
        }
        let mut calls = 0;
        assert_eq!(
            reconstitute_observation_v2(
                &input,
                None,
                &policy,
                MAX_OBSERVATION_PREIMAGE_BYTES,
                &mut || {
                    calls += 1;
                    if calls == 3 {
                        Err(CheckpointFailureV2::Cancelled)
                    } else {
                        Ok(())
                    }
                }
            ),
            Err(AdmissionError::Cancelled)
        );
        assert_eq!(calls, 3);
        let observation = reconstitute_observation_v2(
            &input,
            None,
            &policy,
            MAX_OBSERVATION_PREIMAGE_BYTES,
            &mut || Ok(()),
        )
        .unwrap();
        let mut calls = 0;
        assert_eq!(
            retain_observation_inputs_v2(&observation, &mut || {
                calls += 1;
                if calls == 2 {
                    Err(CheckpointFailureV2::Cancelled)
                } else {
                    Ok(())
                }
            }),
            Err(AdmissionError::Cancelled)
        );
        assert_eq!(calls, 2);
    }
}

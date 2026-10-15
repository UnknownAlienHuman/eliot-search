use crate::batch::RegistryStateRecord;
use crate::error::RegistryError;
use crate::provenance::{PermanentSourceAdmissionV2, SourceProvenance};
use crate::source::{RegisteredSource, SourceLifecycle};
use search_contracts::{
    Blake3Digest32, NonZeroRevision, OpaqueCanonicalBytes, ReceiptRef, RootBindingId, SourceId, SourceIdentity, SourceIdentityKind,
    SourceNamespaceId, SourceOwnerGeneration,
};
use search_source_admission::observation_v2::MAX_OBSERVATION_PREIMAGE_BYTES;
use search_source_admission::original_inputs_v2::reconstitute_observation_v2;
use search_source_admission::policy_v2::{
    CheckpointFailureV2, DEFAULT_COMPILE_LIMITS, POLICY_SCHEMA_VERSION, PolicyInputV2, PredicateV2, RuleInputV2, compile_policy_v2,
};
use search_source_admission::{
    AcceptedCapabilities, AdmissionError, AdmissionOutcome, BASELINE_PROFILE, RuleEffect, SectionPlatform, SourceClass,
    UnvalidatedObservationInput, compiled_defaults, validate_section,
};
use search_source_identity::{CanonicalRelativePath, DEFAULT_IDENTITY_LIMITS, SourceBinding, SourceObservation};
use search_source_provenance::{OriginalAdmissionV2, OriginalInputsLayoutV2, RetainedPolicyV2};
use std::sync::Arc;

#[path = "batch_codec_v4_tests.rs"]
mod batch_codec_v4_tests;
fn rev(value: u64) -> NonZeroRevision {
    NonZeroRevision::new(value).expect("revision")
}
fn receipt(value: &str) -> ReceiptRef {
    ReceiptRef::new(value).expect("receipt")
}
fn original() -> OriginalAdmissionV2 {
    let platform = SectionPlatform {
        os: "windows".into(),
        arch: "x64".into(),
    };
    let capabilities = AcceptedCapabilities {
        profiles: vec![BASELINE_PROFILE.into()],
    };
    let section_input = compiled_defaults();
    let section = validate_section(&section_input, &platform, &capabilities).expect("section");
    let compiled = compile_policy_v2(
        &PolicyInputV2 {
            schema_version: POLICY_SCHEMA_VERSION,
            revision: 7,
            section,
            rules: vec![RuleInputV2 {
                id: "records-case".into(),
                precedence: 0,
                source_class: SourceClass::Regular,
                predicate: PredicateV2::Always,
                effect: RuleEffect::Allow,
                unknown_fields: Vec::new(),
            }],
            unknown_fields: Vec::new(),
        },
        DEFAULT_COMPILE_LIMITS,
        &mut || Ok(()),
    )
    .expect("compile policy v2");
    let policy = Arc::new(RetainedPolicyV2::capture(&compiled, &platform, &capabilities, &mut || Ok(())).expect("retain policy v2"));
    let observation = reconstitute_observation_v2(
        &UnvalidatedObservationInput {
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
            detector_id: Some("detector:records-case".into()),
            profile_id: Some(BASELINE_PROFILE.into()),
            unavailable_fields: Vec::new(),
            unknown_fields: Vec::new(),
        },
        Some(b"src/lib.rs"),
        policy.compiled(),
        MAX_OBSERVATION_PREIMAGE_BYTES,
        &mut || Ok(()),
    )
    .expect("normalized observation");
    OriginalAdmissionV2::retain(policy, &observation, &mut || Ok(())).expect("original admission")
}
fn permanent(original: OriginalAdmissionV2, generation: u8) -> PermanentSourceAdmissionV2 {
    PermanentSourceAdmissionV2::new(
        original,
        rev(11),
        rev(12),
        SourceOwnerGeneration::from_bytes([generation; 32]),
        &mut || Ok(()),
    )
    .expect("permanent")
}
// Synthetic identity/binding metadata only; this proves records, not native/current qualification.
fn binding(path: &str, content_bytes: u64) -> SourceBinding {
    SourceBinding::new(
        SourceIdentity {
            source_namespace_id: SourceNamespaceId::from_bytes([1; 16]),
            source_id: SourceId::from_bytes([2; 16]),
            identity_kind: SourceIdentityKind::NtfsFile,
            stable_identity_components: OpaqueCanonicalBytes::from_validated(b"synthetic:records-case".to_vec()).expect("components"),
        },
        SourceObservation {
            root_binding_id: RootBindingId::from_bytes([3; 16]),
            relative_path: CanonicalRelativePath::new(path, DEFAULT_IDENTITY_LIMITS).expect("path"),
            stable_file_identity_digest: None,
            content_digest: Blake3Digest32::from_bytes([4; 32]),
            content_bytes,
            observation_receipt: receipt("observation:synthetic"),
        },
        rev(1),
        rev(1),
    )
}
fn registered(original: OriginalAdmissionV2) -> RegisteredSource {
    RegisteredSource::new_permanent(
        binding("src/lib.rs", 10),
        permanent(original, 9),
        SourceLifecycle::Active,
        rev(1),
        4,
        receipt("record:register"),
    )
    .expect("registered")
}
#[test]
fn permanent_retains_complete_v2_original_fences_without_v1_shapes() {
    let original = original();
    let permanent = permanent(original.clone(), 9);
    let policy_layout = OriginalInputsLayoutV2::policy(permanent.original().policy(), &mut || Ok(())).expect("policy layout");
    let admission_layout = OriginalInputsLayoutV2::admission(permanent.original(), &mut || Ok(())).expect("admission layout");
    assert_eq!(permanent.original(), &original);
    assert_eq!(permanent.policy_reference(), policy_layout.reference());
    assert_eq!(permanent.admission_reference(), admission_layout.reference());
    assert!(Arc::ptr_eq(permanent.original().policy(), original.policy()));
    assert_eq!(permanent.original().receipt().decision().outcome(), AdmissionOutcome::Allow);
    assert_eq!(
        (permanent.root_record_revision(), permanent.namespace_record_revision()),
        (rev(11), rev(12))
    );
    let source = registered(original.clone());
    assert!(matches!(source.provenance(), SourceProvenance::PermanentV2(_)));
    assert!(source.legacy_admission().is_none() && source.legacy_assignment().is_none());
    assert!(source.original_admission().is_none() && source.has_original_admission());
    assert_eq!(source.permanent_admission().unwrap().original(), &original);
}
#[test]
fn permanent_registration_rejects_size_locator_and_zero_generation() {
    for (path, bytes, generation) in [("src/lib.rs", 11, 9), ("src/main.rs", 10, 9), ("src/lib.rs", 10, 0)] {
        let result = RegisteredSource::new_permanent(
            binding(path, bytes),
            permanent(original(), generation),
            SourceLifecycle::Active,
            rev(1),
            4,
            receipt("record:register"),
        );
        assert_eq!(result.unwrap_err(), RegistryError::AdmissionBindingMismatch);
    }
}
#[test]
fn permanent_reference_construction_refuses_original_cancellation_before_and_after_refs() {
    let original = original();
    let mut successful_calls = 0;
    PermanentSourceAdmissionV2::new(
        original.clone(),
        rev(11),
        rev(12),
        SourceOwnerGeneration::from_bytes([9; 32]),
        &mut || {
            successful_calls += 1;
            Ok(())
        },
    )
    .expect("baseline");
    assert!(successful_calls > 2);
    for fail_at in [1, successful_calls] {
        let mut calls = 0;
        let result = PermanentSourceAdmissionV2::new(
            original.clone(),
            rev(11),
            rev(12),
            SourceOwnerGeneration::from_bytes([9; 32]),
            &mut || {
                calls += 1;
                if calls == fail_at {
                    Err(CheckpointFailureV2::Cancelled)
                } else {
                    Ok(())
                }
            },
        );
        assert_eq!(result, Err(AdmissionError::Cancelled));
    }
}
#[test]
fn old_state_profiles_refuse_permanent_without_v1_fallback() {
    let state = RegistryStateRecord::Source(registered(original()));
    for profile in ["eliot.registry-state.v2", "eliot.registry-state.v3"] {
        assert_eq!(
            state.encode_with_profile(profile),
            Err(RegistryError::AdmissionRestorationUnavailable)
        );
    }
}
#[test]
fn retiring_source_preserves_exact_permanent_v2_provenance() {
    let original = original();
    let source = registered(original.clone());
    let provenance = source.provenance().clone();
    let retired = source.into_retired(rev(2), 5, receipt("record:retire"));
    assert_eq!(retired.provenance(), &provenance);
    assert_eq!(
        (retired.lifecycle(), retired.source_revision(), retired.registry_revision()),
        (SourceLifecycle::Retired, rev(2), 5)
    );
    assert_eq!(retired.last_receipt(), &receipt("record:retire"));
    assert!(retired.legacy_admission().is_none());
    assert!(Arc::ptr_eq(
        retired.permanent_admission().unwrap().original().policy(),
        original.policy()
    ));
}

#[path = "original_ref_manifest_tests.rs"]
mod original_ref_manifest_tests;

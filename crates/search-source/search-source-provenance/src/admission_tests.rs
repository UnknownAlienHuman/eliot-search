//! Causal archive evidence; no Native, durability or current Root qualification.

use crate::{OriginalAdmissionV2, RetainedPolicyV2};
use search_contracts::{CanonicalValue as V, parse_canonical_cbor, to_canonical_cbor};
use search_source_admission::observation_v2::MAX_OBSERVATION_PREIMAGE_BYTES;
use search_source_admission::original_inputs_v2::{
    reconstitute_observation_v2, retain_observation_inputs_v2,
};
use search_source_admission::policy_v2::{
    CheckpointFailureV2, DEFAULT_COMPILE_LIMITS, POLICY_SCHEMA_VERSION, PolicyInputV2, PredicateV2,
    RuleInputV2, compile_policy_v2,
};
use search_source_admission::{
    AcceptedCapabilities, AdmissionError, AdmissionOutcome, BASELINE_PROFILE, RuleEffect,
    SectionPlatform, SourceClass, UnvalidatedObservationInput, compiled_defaults, validate_section,
};
use std::sync::Arc;

fn policy(effect: RuleEffect, revision: u64) -> Arc<RetainedPolicyV2> {
    policy_with_size(
        effect,
        revision,
        search_source_admission::DEFAULT_MAX_FILE_BYTES,
    )
}

fn policy_with_size(
    effect: RuleEffect,
    revision: u64,
    max_file_bytes: u64,
) -> Arc<RetainedPolicyV2> {
    let platform = SectionPlatform {
        os: "windows".into(),
        arch: "x64".into(),
    };
    let capabilities = AcceptedCapabilities {
        profiles: vec![BASELINE_PROFILE.into()],
    };
    let mut section = compiled_defaults();
    section.max_file_bytes = max_file_bytes;
    let compiled = compile_policy_v2(
        &PolicyInputV2 {
            schema_version: POLICY_SCHEMA_VERSION,
            revision,
            section: validate_section(&section, &platform, &capabilities).unwrap(),
            rules: vec![RuleInputV2 {
                id: "archive-rule".into(),
                precedence: 0,
                source_class: SourceClass::Regular,
                predicate: PredicateV2::Always,
                effect,
                unknown_fields: vec![],
            }],
            unknown_fields: vec![],
        },
        DEFAULT_COMPILE_LIMITS,
        &mut || Ok(()),
    )
    .unwrap();
    Arc::new(
        RetainedPolicyV2::capture(&compiled, &platform, &capabilities, &mut || Ok(())).unwrap(),
    )
}

fn metadata() -> UnvalidatedObservationInput {
    UnvalidatedObservationInput {
        locator_class: "normalized-file".into(),
        location_class: "local-fixed".into(),
        source_kind: "file".into(),
        source_class: "regular".into(),
        byte_size: Some(8),
        is_generated: Some(false),
        is_vendor: Some(false),
        is_binary: Some(false),
        is_system: Some(false),
        sensitivity: Some("internal".into()),
        detector_id: Some("detector:archive".into()),
        profile_id: Some(BASELINE_PROFILE.into()),
        unavailable_fields: vec![],
        unknown_fields: vec![],
    }
}

fn original(effect: RuleEffect) -> OriginalAdmissionV2 {
    let policy = policy(effect, 7);
    let observation = reconstitute_observation_v2(
        &metadata(),
        Some("Src/é.rs".as_bytes()),
        policy.compiled(),
        MAX_OBSERVATION_PREIMAGE_BYTES,
        &mut || Ok(()),
    )
    .unwrap();
    OriginalAdmissionV2::retain(policy, &observation, &mut || Ok(())).unwrap()
}

#[test]
fn archive_preserves_valid_size_facts_when_original_policy_denied_the_size() {
    let broad = policy(RuleEffect::Allow, 7);
    let narrow = policy_with_size(RuleEffect::Allow, 7, 8);
    let mut raw = metadata();
    raw.byte_size = Some(4_096);
    let observation = reconstitute_observation_v2(
        &raw,
        Some(b"src/size.rs"),
        broad.compiled(),
        MAX_OBSERVATION_PREIMAGE_BYTES,
        &mut || Ok(()),
    )
    .unwrap();
    assert!(
        reconstitute_observation_v2(
            &raw,
            Some(b"src/size.rs"),
            narrow.compiled(),
            MAX_OBSERVATION_PREIMAGE_BYTES,
            &mut || Ok(())
        )
        .is_err(),
        "current-policy helper keeps its existing cap"
    );
    let archived =
        OriginalAdmissionV2::retain(Arc::clone(&narrow), &observation, &mut || Ok(())).unwrap();
    assert_eq!(
        archived.receipt().decision().outcome(),
        AdmissionOutcome::Deny
    );
    assert!(
        archived
            .receipt()
            .decision()
            .reason_codes()
            .contains(&search_source_admission::AdmissionReasonCode::DenySizeExceeded)
    );
    let restored =
        OriginalAdmissionV2::restore(narrow, archived.encoded_source_evidence(), &mut || Ok(()))
            .unwrap();
    assert_eq!(restored.receipt(), archived.receipt());
    assert_eq!(restored.observation(), &observation);
}

#[test]
fn archive_roundtrip_preserves_complete_receipts_all_four_outcomes_and_shared_policy() {
    for (effect, outcome) in [
        (RuleEffect::Allow, AdmissionOutcome::Allow),
        (RuleEffect::Deny, AdmissionOutcome::Deny),
        (RuleEffect::Review, AdmissionOutcome::ReviewRequired),
        (RuleEffect::Unsupported, AdmissionOutcome::Unsupported),
    ] {
        let retained = original(effect);
        assert_eq!(retained.receipt().decision().outcome(), outcome);
        let restored = OriginalAdmissionV2::restore(
            Arc::clone(retained.policy()),
            retained.encoded_source_evidence(),
            &mut || Ok(()),
        )
        .unwrap();
        assert_eq!(restored, retained);
        assert_eq!(restored.receipt(), retained.receipt());
        assert_eq!(restored.observation(), retained.observation());
        assert_eq!(
            retain_observation_inputs_v2(restored.observation(), &mut || Ok(())).unwrap(),
            metadata()
        );
        assert!(Arc::ptr_eq(restored.policy(), retained.policy()));
        assert_eq!(
            restored.encoded_source_evidence(),
            retained.encoded_source_evidence()
        );
        assert!(
            restored
                .verify_current(retained.policy().compiled(), &mut || Ok(()))
                .is_ok()
        );
        assert!(!format!("{restored:?}").contains("é.rs"));
        assert!(!format!("{:?}", restored.policy()).contains("archive-rule"));
    }
}

#[test]
fn archive_refuses_foreign_policy_noncanonical_truncation_and_every_altered_field() {
    let retained = original(RuleEffect::Allow);
    let bytes = retained.encoded_source_evidence();
    let foreign = policy(RuleEffect::Allow, 8);
    assert!(OriginalAdmissionV2::restore(foreign, bytes, &mut || Ok(())).is_err());
    assert!(
        OriginalAdmissionV2::restore(
            Arc::clone(retained.policy()),
            &bytes[..bytes.len() - 1],
            &mut || Ok(())
        )
        .is_err()
    );
    let mut trailing = bytes.to_vec();
    trailing.push(0);
    assert!(
        OriginalAdmissionV2::restore(Arc::clone(retained.policy()), &trailing, &mut || Ok(()))
            .is_err()
    );
    let V::Array(rows) = parse_canonical_cbor(bytes).unwrap() else {
        panic!("archive array")
    };
    for field in 0..7 {
        let mut changed: Vec<_> = rows.iter().cloned().collect();
        changed[field] = V::Null;
        let value = V::Array(search_contracts::BoundedList::new(changed).unwrap());
        let changed = to_canonical_cbor(&value).unwrap();
        assert!(
            OriginalAdmissionV2::restore(
                Arc::clone(retained.policy()),
                changed.as_slice(),
                &mut || Ok(())
            )
            .is_err(),
            "field {field}"
        );
    }
    let V::Array(fields) = &rows.as_slice()[3] else {
        panic!("metadata array")
    };
    for field in 0..13 {
        let mut changed: Vec<_> = rows.iter().cloned().collect();
        let mut metadata: Vec<_> = fields.iter().cloned().collect();
        metadata[field] = V::Null;
        changed[3] = V::Array(search_contracts::BoundedList::new(metadata).unwrap());
        let changed = to_canonical_cbor(&V::Array(
            search_contracts::BoundedList::new(changed).unwrap(),
        ))
        .unwrap();
        assert!(
            OriginalAdmissionV2::restore(
                Arc::clone(retained.policy()),
                changed.as_slice(),
                &mut || Ok(())
            )
            .is_err(),
            "metadata {field}"
        );
    }
}

#[test]
fn archive_checks_original_probe_through_terminal_and_current_policy_is_separate() {
    let retained = original(RuleEffect::Allow);
    let bytes = retained.encoded_source_evidence();
    let mut calls = 0;
    OriginalAdmissionV2::restore(Arc::clone(retained.policy()), bytes, &mut || {
        calls += 1;
        Ok(())
    })
    .unwrap();
    assert!(calls > 20);
    for fail_at in [1, calls / 2, calls] {
        for (failure, expected) in [
            (CheckpointFailureV2::Cancelled, AdmissionError::Cancelled),
            (
                CheckpointFailureV2::DeadlineExceeded,
                AdmissionError::BudgetExhausted,
            ),
        ] {
            let mut observed = 0;
            let result =
                OriginalAdmissionV2::restore(Arc::clone(retained.policy()), bytes, &mut || {
                    observed += 1;
                    if observed >= fail_at {
                        Err(failure)
                    } else {
                        Ok(())
                    }
                });
            assert_eq!(result.unwrap_err(), expected);
        }
    }
    let current = policy(RuleEffect::Allow, 8);
    assert_eq!(
        retained
            .verify_current(current.compiled(), &mut || Ok(()))
            .unwrap_err(),
        AdmissionError::ReceiptStale
    );
    assert!(
        OriginalAdmissionV2::restore(Arc::clone(retained.policy()), bytes, &mut || Ok(())).is_ok()
    );
}

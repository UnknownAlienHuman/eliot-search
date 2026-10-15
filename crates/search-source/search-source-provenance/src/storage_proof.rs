//! Actual accepted policy size; does not change any production ceiling.

use crate::{ObservedOriginalInputPart, OriginalInputsLayoutV2, RetainedPolicyV2};
use search_source_admission::policy_v2::{
    DEFAULT_COMPILE_LIMITS, MAX_RULES, MAX_TOTAL_PATTERN_BYTES, POLICY_SCHEMA_VERSION,
    PolicyInputV2, PredicateV2, RuleInputV2, compile_policy_v2,
};
use search_source_admission::{
    AcceptedCapabilities, BASELINE_PROFILE, RuleEffect, SectionPlatform, SourceClass,
    compiled_defaults, validate_section,
};

#[test]
fn accepted_max_resource_policy_cannot_fit_one_64_kib_control_value() {
    let platform = SectionPlatform {
        os: "windows".into(),
        arch: "x64".into(),
    };
    let capabilities = AcceptedCapabilities {
        profiles: vec![BASELINE_PROFILE.into()],
    };
    let pattern = "a".repeat(MAX_TOTAL_PATTERN_BYTES / MAX_RULES);
    let rules = (0..MAX_RULES)
        .map(|index| RuleInputV2 {
            id: format!(
                "r{index:03}{}",
                "x".repeat(search_source_admission::MAX_ID_BYTES - 4)
            ),
            precedence: u32::MAX - u32::try_from(index).unwrap(),
            source_class: SourceClass::Documentation,
            predicate: PredicateV2::RelativePathGlobMatches(pattern.clone()),
            effect: RuleEffect::Unsupported,
            unknown_fields: vec![],
        })
        .collect();
    let policy = compile_policy_v2(
        &PolicyInputV2 {
            schema_version: POLICY_SCHEMA_VERSION,
            revision: u64::MAX,
            section: validate_section(&compiled_defaults(), &platform, &capabilities).unwrap(),
            rules,
            unknown_fields: vec![],
        },
        DEFAULT_COMPILE_LIMITS,
        &mut || Ok(()),
    )
    .expect("actual owner accepts complete resource maxima");
    assert_eq!(policy.rules().len(), MAX_RULES);
    let retained =
        RetainedPolicyV2::capture(&policy, &platform, &capabilities, &mut || Ok(())).unwrap();
    let bytes = retained.encoded_inputs();
    let restored =
        RetainedPolicyV2::restore(bytes, &platform, &capabilities, &mut || Ok(())).unwrap();
    assert_eq!(restored.encoded_inputs(), bytes);
    assert_eq!(restored.fingerprint(), policy.fingerprint());
    assert!(
        bytes.len() > 65_536,
        "an accepted original cannot be truncated to one control value"
    );
    println!(
        "accepted_rules={} id_bytes_each={} aggregate_pattern_bytes={} archive_bytes={}",
        MAX_RULES,
        search_source_admission::MAX_ID_BYTES,
        MAX_TOTAL_PATTERN_BYTES,
        bytes.len()
    );
    let layout = OriginalInputsLayoutV2::policy(&retained, &mut || Ok(())).unwrap();
    assert_eq!(layout.parts().len(), 6);
    assert!(
        layout
            .parts()
            .iter()
            .all(|part| part.key().len() < 4_096 && part.value().len() < 65_536)
    );
    let mut observed: Vec<_> = layout
        .parts()
        .iter()
        .map(|part| ObservedOriginalInputPart {
            key: part.key(),
            value: part.value(),
        })
        .collect();
    observed.reverse();
    let restored = OriginalInputsLayoutV2::restore_policy(
        layout.reference(),
        &observed,
        &platform,
        &capabilities,
        &mut || Ok(()),
    )
    .unwrap();
    assert_eq!(restored.encoded_inputs(), retained.encoded_inputs());
    assert!(
        OriginalInputsLayoutV2::restore_policy(
            layout.reference(),
            &observed[1..],
            &platform,
            &capabilities,
            &mut || Ok(())
        )
        .is_err()
    );
    let mut duplicate = observed.clone();
    duplicate[1] = duplicate[0];
    assert!(
        OriginalInputsLayoutV2::restore_policy(
            layout.reference(),
            &duplicate,
            &platform,
            &capabilities,
            &mut || Ok(())
        )
        .is_err()
    );
    let mut altered = observed[0].value.to_vec();
    *altered.last_mut().unwrap() ^= 1;
    observed[0].value = &altered;
    assert!(
        OriginalInputsLayoutV2::restore_policy(
            layout.reference(),
            &observed,
            &platform,
            &capabilities,
            &mut || Ok(())
        )
        .is_err()
    );
}

#[test]
fn accepted_owner_vocabularies_survive_retention_including_safety_classes() {
    use search_source_admission::SensitivityLevel;
    let platform = SectionPlatform {
        os: "windows".into(),
        arch: "x64".into(),
    };
    let capabilities = AcceptedCapabilities {
        profiles: vec![BASELINE_PROFILE.into()],
    };
    let classes = [
        SourceClass::Regular,
        SourceClass::Test,
        SourceClass::Documentation,
        SourceClass::Generated,
        SourceClass::Vendor,
        SourceClass::Binary,
        SourceClass::System,
        SourceClass::Cache,
        SourceClass::BuildOutput,
        SourceClass::Credential,
        SourceClass::PrivateKey,
    ];
    let compiled = compile_policy_v2(
        &PolicyInputV2 {
            schema_version: POLICY_SCHEMA_VERSION,
            revision: 7,
            section: validate_section(&compiled_defaults(), &platform, &capabilities).unwrap(),
            rules: classes
                .into_iter()
                .enumerate()
                .map(|(index, source_class)| RuleInputV2 {
                    id: format!("vocabulary-{index}"),
                    precedence: u32::try_from(index).unwrap(),
                    source_class,
                    predicate: PredicateV2::SensitivityAtLeast(SensitivityLevel::PrivateKey),
                    effect: RuleEffect::Deny,
                    unknown_fields: vec![],
                })
                .collect(),
            unknown_fields: vec![],
        },
        DEFAULT_COMPILE_LIMITS,
        &mut || Ok(()),
    )
    .unwrap();
    let retained =
        RetainedPolicyV2::capture(&compiled, &platform, &capabilities, &mut || Ok(())).unwrap();
    let restored = RetainedPolicyV2::restore(
        retained.encoded_inputs(),
        &platform,
        &capabilities,
        &mut || Ok(()),
    )
    .unwrap();
    assert_eq!(restored.compiled().rules(), compiled.rules());
    assert_eq!(restored.fingerprint(), compiled.fingerprint());
}

//! Actual accepted policy size; does not change any production ceiling.

use crate::RetainedPolicyV2;
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
}

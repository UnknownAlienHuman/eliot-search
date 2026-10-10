use std::path::{Path, PathBuf};

fn package_root() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).to_owned()
}

fn read(root: &Path, relative: &str) -> String {
    std::fs::read_to_string(root.join(relative))
        .unwrap_or_else(|error| panic!("cannot read {relative}: {error}"))
}

#[test]
fn regression_corpus_is_split_without_dropping_contracts() {
    let root = package_root();
    let facade = read(&root, "src/tests.rs");
    assert!(
        facade.len() < 4_500,
        "test facade grew to {} bytes",
        facade.len()
    );
    assert!(facade.contains("mod layout_cases;"));
    assert!(!facade.contains("mod manifest_cases;"));
    assert!(read(&root, "src/manifest.rs").contains("mod tests;"));
    assert!(!facade.contains("#[test]"));

    let layout = read(&root, "src/tests/layout_cases.rs");
    for test in [
        "exact_reconstruction_has_no_gaps_or_overlap",
        "line_boundaries_are_preferred_when_line_fits_hard_limit",
        "overlong_unicode_line_splits_only_at_character_boundaries",
        "hidden_line_terminators_are_rejected",
        "raw_ranges_and_receipt_bound_units_agree_for_all_small_sizes",
        "hard_limit_smaller_than_one_scalar_fails_without_progress",
    ] {
        assert!(layout.contains(test), "layout corpus missing {test}");
    }

    let manifest = read(&root, "src/manifest/tests.rs");
    for test in [
        "real_materialization_roundtrip_complete_occurrences_and_representation_bind",
        "deterministic_bytes_and_source_byte_anchors_include_unicode_and_crlf",
        "v3_canonical_golden",
        "provenance_profile_kind_and_representation_changes_change_derived_identity",
        "correctly_rehashed_forged_manifests_never_become_verified",
        "truncated_identity_collision_and_duplicate_fail_closed",
        "attachment_and_predicate_cannot_be_silently_dropped",
        "live_layout_cancellation_stops_before_complete_allocation",
        "cancellation_deadline_unit_and_output_budgets_never_return_a_complete_set",
    ] {
        assert!(manifest.contains(test), "manifest corpus missing {test}");
    }
    assert!(manifest.contains("../testdata/unit_manifest_v1.hex"));
    assert!(manifest.contains("v2_cannot_be_relabelled_or_satisfy_v3_and_codec_is_closed"));
    assert!(manifest.contains("../testdata/unit_manifest_v2.hex"));

    let profiles = read(&root, "src/manifest/profile_cases.rs");
    assert!(
        profiles.contains("closed_descriptor_rejects_unknown_and_missing_fields_at_both_levels")
    );
    assert!(
        profiles.contains("unsupported_kinds_revisions_and_fixed_baseline_decisions_are_refused")
    );
    let ingress = read(&root, "src/manifest/tests/ingress.rs");
    assert!(ingress.contains("real_code_materialization_builds_source_backed_occurrences"));
    assert!(ingress.contains("recorded_bom_loss_and_empty_owner_request_are_refused"));
    for source in [layout, manifest, profiles, ingress] {
        for forbidden in [
            "std::fs",
            "std::process",
            "qdrant_client",
            "tokio::",
            "reqwest::",
        ] {
            assert!(
                !source.contains(forbidden),
                "unitizer regression corpus acquired forbidden token {forbidden}"
            );
        }
    }
}

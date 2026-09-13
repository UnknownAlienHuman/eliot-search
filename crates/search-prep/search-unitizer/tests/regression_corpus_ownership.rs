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
    assert!(facade.len() < 4_500, "test facade grew to {} bytes", facade.len());
    assert!(facade.contains("mod layout_cases;"));
    assert!(facade.contains("mod manifest_cases;"));
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

    let manifest = read(&root, "src/tests/manifest_cases.rs");
    for test in [
        "durable_manifest_binds_source_representation_and_profiles",
        "same_input_yields_byte_identical_manifest",
        "manifest_canonical_bytes_and_digest_are_golden",
        "changed_profile_revision_changes_manifest_identity",
        "changed_representation_changes_unit_identity",
        "verify_accepts_exact_and_rejects_tamper",
        "build_rejects_wrong_digest_algorithm_without_reinterpretation",
        "invalid_profile_descriptors_fail_closed",
    ] {
        assert!(manifest.contains(test), "manifest corpus missing {test}");
    }
    assert!(manifest.contains("../testdata/unit_manifest_v1.hex"));

    for source in [layout, manifest] {
        for forbidden in ["std::fs", "std::process", "qdrant_client", "tokio::", "reqwest::"] {
            assert!(
                !source.contains(forbidden),
                "unitizer regression corpus acquired forbidden token {forbidden}"
            );
        }
    }
}

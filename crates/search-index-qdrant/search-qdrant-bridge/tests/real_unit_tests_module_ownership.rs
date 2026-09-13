use std::path::{Path, PathBuf};

fn package_root() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).to_owned()
}

fn read(root: &Path, relative: &str) -> String {
    std::fs::read_to_string(root.join(relative))
        .unwrap_or_else(|error| panic!("cannot read {relative}: {error}"))
}

#[test]
fn real_unit_tests_stay_split_without_losing_inventory() {
    let root = package_root();
    let real = read(&root, "src/real.rs");
    assert!(real.contains("#[path = \"real/tests.rs\"]"));
    assert!(real.contains("mod tests;"));
    assert!(!real.contains("include!(\"real/tests.rs\")"));

    let facade = read(&root, "src/real/tests.rs");
    assert!(facade.len() < 1_000);
    for module in ["context", "errors", "filter", "identity"] {
        assert!(facade.contains(&format!("mod {module};")));
    }
    assert!(!facade.contains("#[test]"));

    let expected = [
        (
            "src/real/tests/identity.rs",
            [
                "collection_name_binds_physical_and_generation",
                "uuid_round_trip_preserves_128_bits",
                "hex_32_round_trip",
            ]
            .as_slice(),
        ),
        (
            "src/real/tests/filter.rs",
            [
                "empty_filter_and_inexact_epoch_rejected_before_dispatch",
                "indexed_field_constants_match_filter_translation",
            ]
            .as_slice(),
        ),
        (
            "src/real/tests/errors.rs",
            ["error_codes_are_stable_and_redacted"].as_slice(),
        ),
        (
            "src/real/tests/context.rs",
            ["cancelled_context_fails_before_dispatch"].as_slice(),
        ),
    ];
    for (relative, tests) in expected {
        let source = read(&root, relative);
        assert_eq!(source.matches("#[test]").count(), tests.len());
        for test in tests {
            assert!(source.contains(&format!("fn {test}")), "lost {test}");
        }
        assert!(
            source.len() < 5_000,
            "unit-test module {relative} grew to {} bytes",
            source.len()
        );
        for forbidden in ["std::process", "Command::new", "NATIVE_EXE_PATH"] {
            assert!(!source.contains(forbidden));
        }
    }
}

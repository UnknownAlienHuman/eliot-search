use std::path::{Path, PathBuf};

fn package_root() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).to_owned()
}

fn read(root: &Path, relative: &str) -> String {
    std::fs::read_to_string(root.join(relative))
        .unwrap_or_else(|error| panic!("cannot read {relative}: {error}"))
}

#[test]
fn oracle_contracts_stay_split_without_losing_test_inventory() {
    let root = package_root();
    let facade = read(&root, "tests/oracle_contracts.rs");
    assert!(
        facade.len() < 1_500,
        "oracle facade grew to {} bytes",
        facade.len()
    );
    for module in ["support", "mutation", "query"] {
        assert!(facade.contains(&format!("oracle_contracts/{module}.rs")));
    }
    assert!(!facade.contains("#[test]"));
    assert!(!facade.contains("QdrantBridge::connect"));

    let mutation = read(&root, "tests/oracle_contracts/mutation.rs");
    let query = read(&root, "tests/oracle_contracts/query.rs");
    let expected_mutation = [
        "full_ledger_rejects_every_mutation_without_changing_points",
        "full_ledger_still_replays_and_reports_identity_conflicts",
        "invalid_upsert_batch_neither_writes_nor_consumes_a_receipt",
        "invalid_close_batch_neither_writes_nor_consumes_a_receipt",
        "duplicate_delete_neither_writes_nor_consumes_a_receipt",
    ];
    let expected_query = [
        "public_query_matches_full_sort_with_ties_and_negative_scores",
        "access_and_epoch_exclusions_happen_before_scoring",
        "eligible_score_overflow_is_not_hidden_by_a_full_top_k",
        "shared_query_validation_rejects_malformed_vectors",
    ];
    assert_eq!(mutation.matches("#[test]").count(), expected_mutation.len());
    assert_eq!(query.matches("#[test]").count(), expected_query.len());
    for test in expected_mutation {
        assert!(mutation.contains(&format!("fn {test}")), "lost {test}");
    }
    for test in expected_query {
        assert!(query.contains(&format!("fn {test}")), "lost {test}");
    }

    let support = read(&root, "tests/oracle_contracts/support.rs");
    assert!(support.contains("mod bridge;"));
    assert!(support.contains("mod model;"));
    assert!(!support.contains("#[test]"));

    for relative in [
        "tests/oracle_contracts/support/bridge.rs",
        "tests/oracle_contracts/support/model.rs",
        "tests/oracle_contracts/mutation.rs",
        "tests/oracle_contracts/query.rs",
    ] {
        let source = read(&root, relative);
        assert!(
            source.len() < 8_500,
            "oracle module {relative} grew to {} bytes",
            source.len()
        );
        for forbidden in [
            "spawn_disposable_server",
            "run_qualification_suite",
            "NATIVE_EXE_PATH",
        ] {
            assert!(!source.contains(forbidden));
        }
    }
}

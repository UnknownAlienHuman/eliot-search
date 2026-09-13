use std::path::{Path, PathBuf};

fn package_root() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).to_owned()
}

fn read(root: &Path, relative: &str) -> String {
    std::fs::read_to_string(root.join(relative))
        .unwrap_or_else(|error| panic!("cannot read {relative}: {error}"))
}

#[test]
fn real_dataplane_live_suite_stays_split_without_losing_scenarios() {
    let root = package_root();
    let facade = read(&root, "tests/real_dataplane.rs");
    assert!(
        facade.len() < 2_000,
        "real_dataplane facade grew to {} bytes",
        facade.len()
    );
    for module in [
        "support",
        "collection_name",
        "parity",
        "rejection",
        "recovery",
        "pagination",
        "connection",
    ] {
        assert!(
            facade.contains(&format!("real_dataplane/{module}.rs")),
            "facade lost {module} module"
        );
    }
    for forbidden in [
        "spawn_disposable_server",
        "QdrantBridge::connect",
        "#[tokio::test]",
        "fn point(",
        "fn oracle(",
    ] {
        assert!(!facade.contains(forbidden), "facade owns {forbidden}");
    }

    let scenarios = [
        (
            "tests/real_dataplane/collection_name.rs",
            "collection_names_reject_non_qdrant_chars_without_network",
        ),
        (
            "tests/real_dataplane/parity.rs",
            "t24_real_crud_query_parity_with_oracle",
        ),
        (
            "tests/real_dataplane/rejection.rs",
            "t24_real_wrong_route_filter_and_bounds_rejected",
        ),
        (
            "tests/real_dataplane/recovery.rs",
            "t24_real_unknown_write_recovery_and_replay",
        ),
        (
            "tests/real_dataplane/pagination.rs",
            "t24_real_pagination_cancellation_and_error_redaction",
        ),
        (
            "tests/real_dataplane/connection.rs",
            "t24_real_dead_endpoint_is_typed_not_unknown",
        ),
    ];
    for (relative, test_name) in scenarios {
        let source = read(&root, relative);
        assert!(
            source.contains(&format!("fn {test_name}")),
            "scenario lost {test_name}"
        );
        assert_eq!(
            source.matches("#[tokio::test]").count()
                + source.matches("#[test]").count(),
            1,
            "{relative} must own exactly one scenario"
        );
        assert!(
            source.len() < 8_500,
            "scenario {relative} grew to {} bytes",
            source.len()
        );
    }

    let support = read(&root, "tests/real_dataplane/support.rs");
    for module in ["assertions", "live", "model"] {
        assert!(support.contains(&format!("mod {module};")));
    }
    assert!(!support.contains("#[tokio::test]"));
    assert!(!support.contains("#[test]"));

    for relative in [
        "tests/real_dataplane/support/model.rs",
        "tests/real_dataplane/support/live.rs",
        "tests/real_dataplane/support/assertions.rs",
        "tests/real_dataplane/support/assertions/mutation.rs",
        "tests/real_dataplane/support/assertions/parity.rs",
        "tests/real_dataplane/support/assertions/population.rs",
    ] {
        let source = read(&root, relative);
        assert!(
            source.len() < 8_500,
            "support module {relative} grew to {} bytes",
            source.len()
        );
        assert!(!source.contains("#[tokio::test]"));
    }
}

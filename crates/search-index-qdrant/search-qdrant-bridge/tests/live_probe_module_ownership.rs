use std::path::{Path, PathBuf};

fn package_root() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).to_owned()
}

fn read(root: &Path, relative: &str) -> String {
    std::fs::read_to_string(root.join(relative))
        .unwrap_or_else(|error| panic!("cannot read {relative}: {error}"))
}

#[test]
fn live_probe_setup_and_fixtures_stay_split_by_responsibility() {
    let root = package_root();

    let setup = read(&root, "src/live/probes/setup.rs");
    assert!(setup.len() < 1_500, "setup facade grew to {} bytes", setup.len());
    for module in ["collection", "identity", "ingest", "range", "strict_mode"] {
        assert!(setup.contains(&format!("mod {module};")));
    }
    for forbidden in [
        "CreateCollection",
        "PointStruct",
        "QueryPoints",
        "CountPoints",
        "health_check()",
    ] {
        assert!(!setup.contains(forbidden), "setup facade owns {forbidden}");
    }

    let fixtures = read(&root, "src/live/fixtures.rs");
    assert!(
        fixtures.len() < 2_000,
        "fixtures facade grew to {} bytes",
        fixtures.len()
    );
    for module in ["filter", "points", "spec"] {
        assert!(fixtures.contains(&format!("mod {module};")));
    }
    assert!(!fixtures.contains("HashMap"));
    assert!(!fixtures.contains("FieldCondition"));
    assert!(!fixtures.contains("PointStruct"));

    for relative in [
        "src/live/probes/setup/collection.rs",
        "src/live/probes/setup/identity.rs",
        "src/live/probes/setup/ingest.rs",
        "src/live/probes/setup/range.rs",
        "src/live/probes/setup/strict_mode.rs",
        "src/live/fixtures/filter.rs",
        "src/live/fixtures/points.rs",
        "src/live/fixtures/spec.rs",
    ] {
        let source = read(&root, relative);
        assert!(
            source.len() < 7_500,
            "probe module {relative} grew to {} bytes",
            source.len()
        );
        for forbidden in ["std::process", "Command::new", "NATIVE_EXE_PATH"] {
            assert!(
                !source.contains(forbidden),
                "probe module {relative} acquired process ownership: {forbidden}"
            );
        }
    }
}

#[test]
fn mandatory_setup_probe_order_remains_explicit() {
    let root = package_root();
    let suite = read(&root, "src/live/suite.rs");
    let ordered = [
        "probe_server_identity(&mut suite).await?;",
        "probe_create_and_topology(&mut suite).await?;",
        "probe_payload_indexes(&mut suite).await?;",
        "probe_ingest_batch_a(&mut suite).await?;",
        "probe_strict_negatives(&mut suite).await?;",
        "probe_signed_range(&mut suite).await?;",
    ];
    let mut previous = 0;
    for call in ordered {
        let position = suite
            .find(call)
            .unwrap_or_else(|| panic!("suite lost mandatory call: {call}"));
        assert!(position >= previous, "mandatory setup probes were reordered");
        previous = position;
    }
}

use std::path::{Path, PathBuf};

fn package_root() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).to_owned()
}

fn read(root: &Path, relative: &str) -> String {
    std::fs::read_to_string(root.join(relative))
        .unwrap_or_else(|error| panic!("cannot read {relative}: {error}"))
}

#[test]
fn live_probe_families_stay_split_by_responsibility() {
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

    let readback = read(&root, "src/live/probes/readback.rs");
    assert!(readback.len() < 1_000);
    assert!(readback.contains("mod exact;"));
    assert!(readback.contains("mod schema;"));
    assert!(!readback.contains("DeletePoints"));
    assert!(!readback.contains("CollectionStatus"));

    let search = read(&root, "src/live/probes/search.rs");
    assert!(search.len() < 1_000);
    for module in ["idf", "modifier", "open_end", "query"] {
        assert!(search.contains(&format!("mod {module};")));
    }
    assert!(!search.contains("UpsertPoints"));
    assert!(!search.contains("CountPoints"));

    let suite = read(&root, "src/live/suite.rs");
    assert!(suite.len() < 1_000, "suite facade grew to {} bytes", suite.len());
    for module in ["client", "report", "run", "state"] {
        assert!(suite.contains(&format!("mod {module};")));
    }
    assert!(!suite.contains("health_check"));
    assert!(!suite.contains("MANDATORY_LIVE_PROBES"));
    assert!(!suite.contains("LiveProbeOutcome"));

    for relative in [
        "src/live/probes/setup/collection.rs",
        "src/live/probes/setup/identity.rs",
        "src/live/probes/setup/ingest.rs",
        "src/live/probes/setup/range.rs",
        "src/live/probes/setup/strict_mode.rs",
        "src/live/probes/readback/exact.rs",
        "src/live/probes/readback/schema.rs",
        "src/live/probes/search/idf.rs",
        "src/live/probes/search/modifier.rs",
        "src/live/probes/search/open_end.rs",
        "src/live/probes/search/query.rs",
        "src/live/fixtures/filter.rs",
        "src/live/fixtures/points.rs",
        "src/live/fixtures/spec.rs",
        "src/live/suite/client.rs",
        "src/live/suite/report.rs",
        "src/live/suite/run.rs",
        "src/live/suite/state.rs",
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

    let query = read(&root, "src/live/probes/search/query.rs");
    assert!(query.contains("fn same_scores"));
    assert!(query.contains("sort_unstable_by"));

    let state = read(&root, "src/live/suite/state.rs");
    assert!(state.contains("fn finish"));
    assert!(state.contains("MANDATORY_LIVE_PROBES"));
    assert!(!state.contains("outcomes: self.outcomes.clone()"));
}

#[test]
fn mandatory_probe_order_remains_explicit() {
    let root = package_root();
    let run = read(&root, "src/live/suite/run.rs");
    let ordered = [
        "probe_server_identity(&mut suite).await?;",
        "probe_create_and_topology(&mut suite).await?;",
        "probe_payload_indexes(&mut suite).await?;",
        "probe_ingest_batch_a(&mut suite).await?;",
        "probe_strict_negatives(&mut suite).await?;",
        "probe_signed_range(&mut suite).await?;",
        "probe_independent_idf(&mut suite).await?;",
        "probe_sparse_modifier(&mut suite).await?;",
        "probe_missing_upper_bound(&mut suite).await?;",
        "probe_count_and_readback(&mut suite).await?;",
        "probe_schema_digest(&mut suite).await?;",
    ];
    let mut previous = None;
    for call in ordered {
        let position = run
            .find(call)
            .unwrap_or_else(|| panic!("suite lost mandatory call: {call}"));
        if let Some(previous) = previous {
            assert!(position > previous, "mandatory probes were reordered");
        }
        previous = Some(position);
    }
}

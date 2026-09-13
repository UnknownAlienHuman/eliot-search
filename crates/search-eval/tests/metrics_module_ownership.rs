use std::path::{Path, PathBuf};

fn package_root() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).to_owned()
}

fn read(root: &Path, relative: &str) -> String {
    std::fs::read_to_string(root.join(relative))
        .unwrap_or_else(|error| panic!("cannot read {relative}: {error}"))
}

#[test]
fn metric_pipeline_stays_split_by_evaluation_responsibility() {
    let root = package_root();
    let facade = read(&root, "src/metrics.rs");
    assert!(facade.len() < 2_000, "metrics facade grew to {} bytes", facade.len());
    for module in ["aggregate", "compare", "resource", "score", "slo", "support"] {
        assert!(facade.contains(&format!("mod {module};")));
    }
    for forbidden in [
        "pub fn score_case(",
        "pub fn aggregate_block(",
        "pub fn compare_abc(",
        "pub fn evaluate_candidate_slos(",
        "pub fn compute_resource_report(",
    ] {
        assert!(!facade.contains(forbidden));
    }

    let owners = [
        ("src/metrics/score.rs", "pub fn score_case("),
        ("src/metrics/aggregate.rs", "pub fn aggregate_block("),
        ("src/metrics/compare.rs", "pub fn compare_abc("),
        (
            "src/metrics/slo.rs",
            "pub fn evaluate_candidate_slos(",
        ),
        (
            "src/metrics/resource.rs",
            "pub fn compute_resource_report(",
        ),
    ];
    for (relative, operation) in owners {
        let source = read(&root, relative);
        assert!(source.contains(operation), "{relative} missing {operation}");
    }

    for relative in [
        "src/metrics/aggregate.rs",
        "src/metrics/compare.rs",
        "src/metrics/resource.rs",
        "src/metrics/score.rs",
        "src/metrics/slo.rs",
        "src/metrics/support.rs",
    ] {
        let source = read(&root, relative);
        assert!(
            source.len() < 15_000,
            "{relative} grew to {} bytes",
            source.len()
        );
        for forbidden in [
            "std::fs",
            "std::process",
            "qdrant_client",
            "search_qdrant",
            "reqwest::",
            "tokio::",
        ] {
            assert!(
                !source.contains(forbidden),
                "{relative} acquired forbidden token {forbidden}"
            );
        }
    }

    let score = read(&root, "src/metrics/score.rs");
    assert!(score.contains("MetricObservationState"));
    assert!(!score.contains("BaselineComparisonClass"));

    let aggregate = read(&root, "src/metrics/aggregate.rs");
    assert!(aggregate.contains("failed_attempt_count"));
    assert!(aggregate.contains("unavailable_attempt_count"));
    assert!(!aggregate.contains("SloDefinition"));

    let resource = read(&root, "src/metrics/resource.rs");
    assert!(resource.contains("ResourceLane::Warmup"));
    assert!(resource.contains("ResourceLane::Measured"));
    assert!(!resource.contains("compare_abc"));
}

#[test]
fn crate_root_preserves_metric_public_surface() {
    let root = package_root();
    let lib = read(&root, "src/lib.rs");
    assert!(lib.contains("pub use metrics::*;"));
    let facade = read(&root, "src/metrics.rs");
    for public in [
        "MetricObservationState",
        "MetricObservation",
        "CaseMetricValue",
        "CaseMetricSet",
        "score_case",
        "AggregatedMetric",
        "BaselineMetricReport",
        "aggregate_block",
        "BaselineComparisonClass",
        "MetricDelta",
        "MetricGates",
        "BaselineComparison",
        "compare_abc",
        "SloDirection",
        "SloDefinition",
        "SloStatus",
        "SloOutcome",
        "SloReport",
        "evaluate_candidate_slos",
        "ResourceLane",
        "ResourceReport",
        "compute_resource_report",
        "compute_resource_report_for_lane",
    ] {
        assert!(facade.contains(public), "metrics facade no longer exports {public}");
    }
}

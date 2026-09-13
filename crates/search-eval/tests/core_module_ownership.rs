use std::path::{Path, PathBuf};

fn package_root() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).to_owned()
}

fn read(root: &Path, relative: &str) -> String {
    std::fs::read_to_string(root.join(relative))
        .unwrap_or_else(|error| panic!("cannot read {relative}: {error}"))
}

#[test]
fn evaluation_core_stays_split_by_contract_owner() {
    let root = package_root();
    let facade = read(&root, "src/core.rs");
    assert!(facade.len() < 2_000, "core facade grew to {} bytes", facade.len());
    for module in ["baseline", "corpus", "limits", "policy", "registry", "run"] {
        assert!(facade.contains(&format!("mod {module};")));
        assert!(facade.contains(&format!("pub use {module}::*;")));
    }
    for forbidden in [
        "pub struct ControlCorpusManifest",
        "pub struct MetricRegistry",
        "pub struct AcceptancePolicy",
        "pub struct FrozenRunInput",
        "pub struct BaselineDescriptor",
        "FingerprintBuilder::new",
    ] {
        assert!(!facade.contains(forbidden));
    }

    let owners = [
        ("src/core/limits.rs", "pub struct EvalLimits"),
        ("src/core/corpus.rs", "pub fn validate_control_corpus("),
        ("src/core/registry.rs", "pub fn validate_metric_registry("),
        ("src/core/policy.rs", "pub fn validate_acceptance_policy("),
        ("src/core/run.rs", "pub fn freeze_run_manifest("),
        ("src/core/baseline.rs", "pub fn validate_baseline_descriptor("),
    ];
    for (relative, operation) in owners {
        let source = read(&root, relative);
        assert!(source.contains(operation), "{relative} missing {operation}");
        assert!(
            source.len() < 16_000,
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
            "swarm/launch-state",
            "swarm/tickets",
            "swarm/leases",
        ] {
            assert!(
                !source.contains(forbidden),
                "{relative} acquired forbidden token {forbidden}"
            );
        }
    }

    let corpus = read(&root, "src/core/corpus.rs");
    assert!(corpus.contains("REQUIRED_TOPOLOGY"));
    assert!(corpus.contains("eval/corpus-validation/v1"));
    assert!(!corpus.contains("MetricDefinition"));

    let registry = read(&root, "src/core/registry.rs");
    assert!(registry.contains("MetricDirection::ZeroTolerance"));
    assert!(registry.contains("eval/metric-registry/v1"));
    assert!(!registry.contains("AcceptancePolicy"));

    let policy = read(&root, "src/core/policy.rs");
    assert!(policy.contains("IndependentReviewRequired"));
    assert!(policy.contains("eval/acceptance-policy/v1"));
    assert!(!policy.contains("FrozenRunInput"));

    let run = read(&root, "src/core/run.rs");
    assert!(run.contains("eval/frozen-run/v1"));
    assert!(run.contains("oracle_store_separate"));
    assert!(!run.contains("BaselineRole"));
}

#[test]
fn crate_root_preserves_core_public_surface() {
    let root = package_root();
    let lib = read(&root, "src/lib.rs");
    assert!(lib.contains("pub use core::*;"));
    let facade = read(&root, "src/core.rs");
    for public in [
        "EvalLimits",
        "CaseFamily",
        "ControlCase",
        "ControlCorpusManifest",
        "ValidatedControlCorpus",
        "validate_control_corpus",
        "MetricDirection",
        "MetricDenominator",
        "MissingValuePolicy",
        "MetricDefinition",
        "MetricRegistry",
        "ValidatedMetricRegistry",
        "validate_metric_registry",
        "AcceptanceRule",
        "AcceptancePolicy",
        "ValidatedAcceptancePolicy",
        "validate_acceptance_policy",
        "RunArtifact",
        "FrozenRunInput",
        "FrozenRunManifest",
        "freeze_run_manifest",
        "BaselineRole",
        "BaselineDescriptor",
        "ValidatedBaseline",
        "validate_baseline_descriptor",
    ] {
        let exported = [
            "pub use baseline::*;",
            "pub use corpus::*;",
            "pub use limits::*;",
            "pub use policy::*;",
            "pub use registry::*;",
            "pub use run::*;",
        ]
        .iter()
        .any(|statement| facade.contains(statement));
        assert!(exported, "core facade no longer re-exports {public}");
    }
}

use std::path::{Path, PathBuf};

fn package_root() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).to_owned()
}

fn read(root: &Path, relative: &str) -> String {
    std::fs::read_to_string(root.join(relative))
        .unwrap_or_else(|error| panic!("cannot read {relative}: {error}"))
}

#[test]
fn evidence_pipeline_stays_split_by_execution_responsibility() {
    let root = package_root();
    let facade = read(&root, "src/evidence.rs");
    assert!(facade.len() < 2_000, "evidence facade grew to {} bytes", facade.len());
    for module in ["attempt", "ledger", "probe", "schedule"] {
        assert!(facade.contains(&format!("mod {module};")));
        assert!(facade.contains(&format!("pub use {module}::*;")));
    }
    for forbidden in [
        "pub fn plan_case_block(",
        "pub fn validate_case_evidence(",
        "pub struct EvidenceLedger",
        "pub fn ingest_external_probe(",
        "FingerprintBuilder::new",
    ] {
        assert!(!facade.contains(forbidden));
    }

    let owners = [
        ("src/evidence/schedule.rs", "pub fn plan_case_block("),
        ("src/evidence/attempt.rs", "pub fn validate_case_evidence("),
        ("src/evidence/ledger.rs", "pub struct EvidenceLedger"),
        ("src/evidence/probe.rs", "pub fn ingest_external_probe("),
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

    let schedule = read(&root, "src/evidence/schedule.rs");
    assert!(schedule.contains("eval/case-block/v1"));
    assert!(schedule.contains("eval/schedule-key/v1"));
    assert!(schedule.contains("eval/attempt/v1"));
    assert!(!schedule.contains("ProbeStatus"));

    let attempt = read(&root, "src/evidence/attempt.rs");
    assert!(attempt.contains("eval/case-evidence/v1"));
    assert!(attempt.contains("terminal_events != 1"));
    assert!(!attempt.contains("ValidatedProbeEvidence"));

    let ledger = read(&root, "src/evidence/ledger.rs");
    assert!(ledger.contains("AttemptConflict"));
    assert!(!ledger.contains("FingerprintBuilder"));

    let probe = read(&root, "src/evidence/probe.rs");
    assert!(probe.contains("SelfAcceptanceForbidden"));
    assert!(!probe.contains("ScheduledAttempt"));
}

#[test]
fn crate_root_preserves_evidence_public_surface() {
    let root = package_root();
    let lib = read(&root, "src/lib.rs");
    assert!(lib.contains("pub use evidence::*;"));
    let facade = read(&root, "src/evidence.rs");
    for statement in [
        "pub use attempt::*;",
        "pub use ledger::*;",
        "pub use probe::*;",
        "pub use schedule::*;",
    ] {
        assert!(facade.contains(statement), "evidence facade missing {statement}");
    }
}

use std::path::{Path, PathBuf};

fn package_root() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).to_owned()
}

fn read(root: &Path, relative: &str) -> String {
    std::fs::read_to_string(root.join(relative))
        .unwrap_or_else(|error| panic!("cannot read {relative}: {error}"))
}

#[test]
fn zero_tolerance_audits_stay_split_by_surface() {
    let root = package_root();
    let facade = read(&root, "src/audits.rs");
    assert!(facade.len() < 2_000, "audits facade grew to {} bytes", facade.len());
    for module in [
        "admission",
        "common",
        "fault",
        "leakage",
        "protocol",
        "reproducibility",
    ] {
        assert!(facade.contains(&format!("mod {module};")));
    }
    for forbidden in [
        "pub fn audit_leakage(",
        "pub fn audit_source_admission(",
        "pub fn audit_fault_matrix(",
        "pub fn audit_protocol_stress(",
        "pub fn audit_reproducibility(",
    ] {
        assert!(!facade.contains(forbidden));
    }

    let owners = [
        ("src/audits/leakage.rs", "pub fn audit_leakage("),
        (
            "src/audits/admission.rs",
            "pub fn audit_source_admission(",
        ),
        ("src/audits/fault.rs", "pub fn audit_fault_matrix("),
        (
            "src/audits/protocol.rs",
            "pub fn audit_protocol_stress(",
        ),
        (
            "src/audits/reproducibility.rs",
            "pub fn audit_reproducibility(",
        ),
        ("src/audits/common.rs", "pub enum HardBlockerClass"),
    ];
    for (relative, operation) in owners {
        let source = read(&root, relative);
        assert!(source.contains(operation), "{relative} missing {operation}");
    }

    for relative in [
        "src/audits/admission.rs",
        "src/audits/common.rs",
        "src/audits/fault.rs",
        "src/audits/leakage.rs",
        "src/audits/protocol.rs",
        "src/audits/reproducibility.rs",
    ] {
        let source = read(&root, relative);
        assert!(
            source.len() < 12_000,
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

    let leakage = read(&root, "src/audits/leakage.rs");
    assert!(leakage.contains("complete required canary-by-surface matrix"));
    assert!(!leakage.contains("FaultPoint"));

    let fault = read(&root, "src/audits/fault.rs");
    assert!(fault.contains("FaultPoint::MANDATORY"));
    assert!(fault.contains("authoritative_readback"));
    assert!(!fault.contains("LeakageSurface"));

    let protocol = read(&root, "src/audits/protocol.rs");
    assert!(protocol.contains("duplicate_terminal_requests"));
    assert!(protocol.contains("leaked_session_objects"));
    assert!(!protocol.contains("ReproducibilityObservation"));
}

#[test]
fn crate_root_preserves_audit_public_surface() {
    let root = package_root();
    let lib = read(&root, "src/lib.rs");
    assert!(lib.contains("pub use audits::*;"));
    let facade = read(&root, "src/audits.rs");
    for public in [
        "HardBlockerClass",
        "HardBlocker",
        "CanaryClass",
        "LeakageSurface",
        "LeakageObservation",
        "LeakageAudit",
        "audit_leakage",
        "AdmissionScenario",
        "AdmissionProbe",
        "AdmissionAudit",
        "audit_source_admission",
        "FaultPoint",
        "FaultCellStatus",
        "FaultCell",
        "FaultReadback",
        "FaultContainment",
        "FaultMatrixReport",
        "audit_fault_matrix",
        "ProtocolStressEvidence",
        "ProtocolStressReport",
        "audit_protocol_stress",
        "ReproducibilityObservation",
        "ReproducibilityReport",
        "audit_reproducibility",
    ] {
        assert!(facade.contains(public), "audits facade no longer exports {public}");
    }
}

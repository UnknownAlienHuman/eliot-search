use std::path::{Path, PathBuf};

fn package_root() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).to_owned()
}

fn read(root: &Path, relative: &str) -> String {
    std::fs::read_to_string(root.join(relative))
        .unwrap_or_else(|error| panic!("cannot read {relative}: {error}"))
}

#[test]
fn product_pulse_report_stays_split_by_acceptance_responsibility() {
    let root = package_root();
    let facade = read(&root, "src/report.rs");
    assert!(facade.len() < 2_000, "report facade grew to {} bytes", facade.len());
    for module in ["assemble", "coverage", "model", "receipt", "review", "support", "verdict"] {
        assert!(facade.contains(&format!("mod {module};")));
    }
    for forbidden in [
        "pub fn audit_case_coverage(",
        "pub fn assemble_product_pulse(",
        "pub fn validate_independent_review(",
        "pub fn decide_acceptance(",
        "pub fn issue_product_pulse_receipt(",
        "FingerprintBuilder::new",
    ] {
        assert!(!facade.contains(forbidden));
    }

    let owners = [
        ("src/report/coverage.rs", "pub fn audit_case_coverage("),
        ("src/report/assemble.rs", "pub fn assemble_product_pulse("),
        ("src/report/review.rs", "pub fn validate_independent_review("),
        ("src/report/verdict.rs", "pub fn decide_acceptance("),
        ("src/report/receipt.rs", "pub fn issue_product_pulse_receipt("),
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

    let coverage = read(&root, "src/report/coverage.rs");
    assert!(coverage.contains("eval/case-coverage/v1"));
    assert!(!coverage.contains("AcceptanceVerdict"));

    let assembly = read(&root, "src/report/assemble.rs");
    assert!(assembly.contains("eval/product-pulse/v1"));
    assert!(assembly.contains("ZeroTolerance"));
    assert!(!assembly.contains("reviewer_id"));

    let review = read(&root, "src/report/review.rs");
    assert!(review.contains("SelfAcceptanceForbidden"));
    assert!(!review.contains("VerdictKind"));

    let verdict = read(&root, "src/report/verdict.rs");
    assert!(verdict.contains("eval/acceptance-verdict/v1"));
    assert!(verdict.contains("require_material_value"));
    assert!(!verdict.contains("ProductPulseReceipt"));

    let receipt = read(&root, "src/report/receipt.rs");
    assert!(receipt.contains("validate_independent_review"));
    assert!(!receipt.contains("FingerprintBuilder"));
}

#[test]
fn crate_root_preserves_report_public_surface() {
    let root = package_root();
    let lib = read(&root, "src/lib.rs");
    assert!(lib.contains("pub use report::*;"));
    let facade = read(&root, "src/report.rs");
    for statement in [
        "pub use assemble::*;",
        "pub use coverage::*;",
        "pub use model::*;",
        "pub use receipt::*;",
        "pub use review::*;",
        "pub use verdict::*;",
    ] {
        assert!(facade.contains(statement), "report facade missing {statement}");
    }
}

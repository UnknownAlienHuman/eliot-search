use std::path::{Path, PathBuf};

fn repository_root() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .expect("xtask is nested under repository root")
        .to_owned()
}

fn read(root: &Path, relative: &str) -> String {
    std::fs::read_to_string(root.join(relative))
        .unwrap_or_else(|error| panic!("cannot read {relative}: {error}"))
}

#[test]
fn qdrant_qualification_keeps_one_identity_facade_and_bounded_rules() {
    let root = repository_root();
    let base = "crates/search-index-qdrant/search-qdrant-bridge/src";
    let facade = read(&root, &format!("{base}/qualified.rs"));
    for module in ["artifact", "client", "error", "gate", "idf"] {
        assert!(facade.contains(&format!("mod {module};")));
    }
    for identity in [
        "QUALIFIED_SERVER_VERSION",
        "QUALIFIED_SERVER_BUILD",
        "QUALIFIED_EXE_SHA256_HEX",
        "QUALIFIED_CLIENT_VERSION",
        "QUALIFIED_CLIENT_CHECKSUM",
        "QUALIFIED_CLIENT_GIT_SHA",
    ] {
        assert!(
            facade.contains(&format!("pub const {identity}")),
            "qualified identity escaped facade: {identity}"
        );
    }
    for implementation in [
        "pub struct ObservedArtifact",
        "pub struct ObservedClient",
        "pub struct IndependentIdfProfile",
        "pub struct QualifiedGate",
        "pub enum QualificationError",
    ] {
        assert!(!facade.contains(implementation));
    }
    assert!(!facade.contains("qdrant_client"));

    let modules = [
        ("artifact.rs", "verify_artifact"),
        ("client.rs", "verify_client"),
        ("error.rs", "QualificationError"),
        ("gate.rs", "QualifiedGate"),
        ("idf.rs", "admit_independent_idf"),
    ];
    for (file, owner) in modules {
        let relative = format!("{base}/qualified/{file}");
        let source = read(&root, &relative);
        assert!(source.contains(owner), "{relative} does not own {owner}");
        assert!(
            source.len() < 8_000,
            "{relative} grew to {} bytes",
            source.len()
        );
        assert!(!source.contains("qdrant_client"));
        assert!(!source.contains("std::fs"));
        assert!(!source.contains("std::process"));
    }
}

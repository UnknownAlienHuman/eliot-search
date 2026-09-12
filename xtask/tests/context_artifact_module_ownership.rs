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
fn context_artifact_primitives_stay_split_by_responsibility() {
    let root = repository_root();
    let facade = read(&root, "xtask/src/context_artifact.rs");
    assert!(facade.len() < 3_000, "facade grew to {} bytes", facade.len());
    for module in ["bundle", "digest", "error", "normalize", "output", "spec"] {
        assert!(facade.contains(&format!("mod {module};")));
    }
    assert!(!facade.contains("Sha256::new"));
    assert!(!facade.contains("fn parse_bundle"));
    assert!(!facade.contains("fn normalize_utf8_lf"));

    let bundle = read(&root, "xtask/src/context_artifact/bundle.rs");
    assert!(bundle.contains("mod model;"));
    assert!(bundle.contains("mod parse;"));
    assert!(bundle.contains("mod render;"));

    let parse = read(&root, "xtask/src/context_artifact/bundle/parse.rs");
    assert!(parse.contains("parse_bundle"));
    assert!(!parse.contains("Sha256::new"));
    assert!(!parse.contains("std::fs"));

    let render = read(&root, "xtask/src/context_artifact/bundle/render.rs");
    assert!(render.contains("render_bundle"));
    assert!(!render.contains("std::fs"));

    let digest = read(&root, "xtask/src/context_artifact/digest.rs");
    assert!(digest.contains("candidate_metadata_digest"));
    assert!(digest.contains("CANDIDATE_METADATA_DOMAIN"));
    assert!(!digest.contains("BUNDLE_MAGIC"));
}

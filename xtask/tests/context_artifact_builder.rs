use std::path::{Path, PathBuf};
use std::process::Command;

use serde_json::Value;
use xtask::context_artifact::parse_bundle;
use xtask::context_artifact_builder::build_candidate;

fn repository_root() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .expect("xtask is nested under repository root")
        .to_owned()
}

fn git_text(root: &Path, args: &[&str]) -> String {
    let output = Command::new("git")
        .arg("-C")
        .arg(root)
        .args(args)
        .output()
        .expect("git is available for immutable-tree builder tests");
    assert!(
        output.status.success(),
        "git command failed: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    String::from_utf8(output.stdout)
        .expect("git output is UTF-8")
        .trim()
        .to_owned()
}

#[test]
fn current_search_contracts_candidate_builds_from_head_without_writing() {
    let root = repository_root();
    let format = git_text(&root, &["rev-parse", "--show-object-format"]);
    let commit = git_text(&root, &["rev-parse", "HEAD"]);
    let tagged = format!("{format}:{commit}");
    let build = build_candidate(
        &root,
        "search-contracts",
        &tagged,
        &[],
        "artifacts/context-artifact-candidates/rust-test",
    )
    .expect("current committed search-contracts candidate builds");

    let candidate = build.candidate();
    assert_eq!(
        candidate.get("status").and_then(Value::as_str),
        Some("ARTIFACT_CANDIDATE_NOT_STORED_NOT_SIGNED")
    );
    assert_eq!(
        candidate.pointer("/repository/base_commit").and_then(Value::as_str),
        Some(tagged.as_str())
    );
    assert_eq!(
        candidate
            .pointer("/repository/working_tree_used_as_input")
            .and_then(Value::as_bool),
        Some(false)
    );
    assert_eq!(candidate["reason_codes"].as_array().map(Vec::len), Some(0));
    assert_eq!(
        candidate["control_record_mutations"].as_array().map(Vec::len),
        Some(0)
    );
    assert_eq!(
        candidate
            .pointer("/manifest_projection/schema_instance")
            .and_then(Value::as_bool),
        Some(false)
    );
    assert!(candidate["authority"].as_object().is_some_and(|authority| {
        authority
            .values()
            .all(|value| value.as_bool() == Some(false))
    }));
    assert_eq!(candidate["sources"].as_array().map(Vec::len), Some(20));
    assert_eq!(
        candidate["registry_fragments"].as_array().map(Vec::len),
        Some(5)
    );
    assert!(build.bundle_relative_path().starts_with(
        "artifacts/context-artifact-candidates/rust-test/search-contracts/"
    ));
    assert!(build.candidate_relative_path().ends_with(".json"));

    let (preamble, blocks) = parse_bundle(build.bundle_bytes())
        .expect("assembled bundle has a strict inverse");
    assert_eq!(preamble["source_count"].as_u64(), Some(20));
    assert_eq!(preamble["registry_fragment_count"].as_u64(), Some(5));
    assert_eq!(blocks.len(), 25);

    let decoded: Value = serde_json::from_slice(build.candidate_bytes())
        .expect("candidate metadata is canonical JSON");
    assert_eq!(&decoded, candidate);
}

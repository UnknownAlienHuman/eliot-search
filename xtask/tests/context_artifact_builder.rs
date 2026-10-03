use std::path::{Path, PathBuf};
use std::process::Command;
use std::time::{SystemTime, UNIX_EPOCH};

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

fn git_success(root: &Path, args: &[&str]) {
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
}

struct ScratchRepository {
    temp_root: PathBuf,
    repo: PathBuf,
}

impl ScratchRepository {
    fn clone_at(source: &Path, commit: &str) -> Self {
        let nonce = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .expect("system time follows the Unix epoch")
            .as_nanos();
        let temp_root = std::env::temp_dir().join(format!(
            "eliot-search-context-artifact-test-{}-{nonce}",
            std::process::id()
        ));
        std::fs::create_dir(&temp_root).expect("create unique scratch root");
        let repo = temp_root.join("repo");
        let output = Command::new("git")
            .args(["clone", "--quiet", "--shared", "--no-checkout"])
            .arg(source)
            .arg(&repo)
            .output()
            .expect("git is available for immutable-tree builder tests");
        assert!(
            output.status.success(),
            "git clone failed: {}",
            String::from_utf8_lossy(&output.stderr)
        );
        let scratch = Self { temp_root, repo };
        git_success(&scratch.repo, &["checkout", "--quiet", "--detach", commit]);
        scratch
    }

    fn commit_stages(&self, source: &str, message: &str) -> String {
        std::fs::write(self.repo.join("swarm/stages.toml"), source)
            .expect("write scratch stage registry");
        git_success(&self.repo, &["add", "--", "swarm/stages.toml"]);
        let output = Command::new("git")
            .arg("-C")
            .arg(&self.repo)
            .args([
                "-c",
                "user.name=xtask-test",
                "-c",
                "user.email=xtask-test@example.invalid",
                "-c",
                "commit.gpgsign=false",
                "commit",
                "--quiet",
                "-m",
                message,
            ])
            .output()
            .expect("git is available for immutable-tree builder tests");
        assert!(
            output.status.success(),
            "git commit failed: {}",
            String::from_utf8_lossy(&output.stderr)
        );
        git_text(&self.repo, &["rev-parse", "HEAD"])
    }
}

impl Drop for ScratchRepository {
    fn drop(&mut self) {
        let Ok(temp_root) = std::fs::canonicalize(&self.temp_root) else {
            return;
        };
        let Ok(temp_dir) = std::fs::canonicalize(std::env::temp_dir()) else {
            return;
        };
        if temp_root.parent() == Some(temp_dir.as_path()) {
            let _ = std::fs::remove_dir_all(temp_root);
        }
    }
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
    assert!(candidate["preflight_checks"].as_array().is_some_and(|checks| {
        checks.iter().any(|check| {
            check.get("id").and_then(Value::as_str) == Some("registry-parity")
                && check.get("status").and_then(Value::as_str) == Some("PASS")
        })
    }));
    assert_eq!(candidate["sources"].as_array().map(Vec::len), Some(21));
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
    assert_eq!(preamble["source_count"].as_u64(), Some(21));
    assert_eq!(preamble["registry_fragment_count"].as_u64(), Some(5));
    assert_eq!(blocks.len(), 26);

    let decoded: Value = serde_json::from_slice(build.candidate_bytes())
        .expect("candidate metadata is canonical JSON");
    assert_eq!(&decoded, candidate);
}

#[test]
fn w0_phase_registry_rejects_missing_or_malformed_p00_phase_arrays() {
    let source = repository_root();
    let base_commit = git_text(&source, &["rev-parse", "HEAD"]);
    let object_format = git_text(&source, &["rev-parse", "--show-object-format"]);
    let original = git_text(&source, &["show", "HEAD:swarm/stages.toml"]);
    let expected = "phases = [\"P00\"]";
    assert_eq!(original.matches(expected).count(), 1);

    let scratch = ScratchRepository::clone_at(&source, &base_commit);
    let variants = [
        ("missing", ""),
        ("scalar", "phases = \"P00\""),
        ("non-string", "phases = [true]"),
        ("extra-phase", "phases = [\"P00\", \"P01\"]"),
        ("wrong-phase", "phases = [\"P01\"]"),
    ];

    for (name, replacement) in variants {
        let mutated = original.replacen(expected, replacement, 1);
        assert_ne!(mutated, original, "fixture mutation for {name}");
        let commit = scratch.commit_stages(&mutated, name);
        let tagged_commit = format!("{object_format}:{commit}");
        let error = build_candidate(
            &scratch.repo,
            "search-contracts",
            &tagged_commit,
            &[],
            "artifacts/context-artifact-candidates/phase-test",
        )
        .expect_err("invalid W0 phase registry must fail closed");
        assert_eq!(
            error.reason(),
            "PACKAGE_REGISTRY_MISMATCH",
            "unexpected rejection for {name}"
        );
    }
}

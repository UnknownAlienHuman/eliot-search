use std::path::{Path, PathBuf};
use std::process::Command;

use xtask::context_artifact_io::{validate_output_root, write_exact_idempotent};
use xtask::git_tree::GitTree;

fn repository_root() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .expect("xtask is nested under the repository root")
        .to_owned()
}

fn git_text(root: &Path, args: &[&str]) -> String {
    let output = Command::new("git")
        .arg("-C")
        .arg(root)
        .args(args)
        .output()
        .expect("git executes");
    assert!(output.status.success(), "git command failed");
    String::from_utf8(output.stdout)
        .expect("git output is UTF-8")
        .trim()
        .to_owned()
}

#[test]
fn immutable_git_view_reads_only_the_exact_commit() {
    let root = repository_root();
    let format = git_text(&root, &["rev-parse", "--show-object-format"]);
    let head = git_text(&root, &["rev-parse", "HEAD"]);
    let tagged = format!("{format}:{head}");
    let view = GitTree::open(&root, &tagged).expect("open immutable tree");

    assert_eq!(view.tagged_commit(), tagged);
    let (cargo, entry) = view.read_bytes("Cargo.toml").expect("read Cargo.toml");
    assert!(cargo.starts_with(b"[workspace]"));
    assert!(entry.regular_blob());
    assert!(view
        .blob_identity(&entry)
        .starts_with(&format!("{format}:")));
    assert!(view.commit_exists(&tagged));
    let files = view.list_files("xtask/src").expect("list xtask sources");
    assert!(files.iter().any(|path| path == "xtask/src/lib.rs"));
}

#[test]
fn candidate_outputs_are_idempotent_and_conflict_closed() {
    let root = std::env::temp_dir().join(format!(
        "eliot-context-io-{}-{}",
        std::process::id(),
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .expect("clock")
            .as_nanos(),
    ));
    std::fs::create_dir_all(&root).expect("scratch root");
    let output = validate_output_root(
        &root,
        "artifacts/context-artifact-candidates/integration",
    )
    .expect("validated output root");
    let target = output
        .file("search-contracts", "candidate.json")
        .expect("candidate path");
    write_exact_idempotent(&root, &target, b"{}\n").expect("first write");
    write_exact_idempotent(&root, &target, b"{}\n").expect("idempotent replay");
    let error = write_exact_idempotent(&root, &target, b"{\"changed\":true}\n")
        .expect_err("different bytes conflict");
    assert_eq!(error.reason(), "CANDIDATE_OUTPUT_CONFLICT");
    let _ = std::fs::remove_dir_all(root);
}

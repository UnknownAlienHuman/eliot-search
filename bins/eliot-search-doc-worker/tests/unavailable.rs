//! Boundary: the optional document worker is honestly unavailable by default.
//!
//! Every invocation — default or forced — must refuse on stderr with a
//! content-free announcement and a nonzero exit, leaving stdout empty.

use std::process::{Command, Output, Stdio};

const EXPECTED_CODE: i32 = 2;

fn run(args: &[&str], extra_env: &[(&str, &str)]) -> Output {
    let mut command = Command::new(env!("CARGO_BIN_EXE_eliot-search-doc-worker"));
    command.args(args).stdin(Stdio::null());
    for (key, value) in extra_env {
        command.env(key, value);
    }
    command.output().expect("spawn document worker binary")
}

fn assert_refusal(output: &Output) {
    assert!(
        !output.status.success(),
        "document worker must never exit 0 while unqualified"
    );
    assert_eq!(
        output.status.code(),
        Some(EXPECTED_CODE),
        "document worker refusal must use the pinned nonzero exit"
    );
    assert!(
        output.stdout.is_empty(),
        "stdout must stay empty for machine-parseability, got: {}",
        String::from_utf8_lossy(&output.stdout)
    );
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(
        stderr.contains("UNAVAILABLE"),
        "stderr must announce UNAVAILABLE, got: {stderr}"
    );
    assert!(
        stderr.contains("DOCUMENT_PROVIDER_NOT_QUALIFIED"),
        "stderr must name the typed failure, got: {stderr}"
    );
    assert!(
        stderr.contains("P15") && stderr.contains("ADR") && stderr.contains("G6"),
        "stderr must carry the one-line qualification path, got: {stderr}"
    );
    let lowered = stderr.to_lowercase();
    for marker in [
        "secret", "token", "password", "bearer", "api_key", "api-key",
    ] {
        assert!(
            !lowered.contains(marker),
            "stderr must not leak secrets, found {marker:?} in: {stderr}"
        );
    }
    assert!(
        !stderr.contains('\\'),
        "stderr must not contain paths, got: {stderr}"
    );
    assert!(
        !stderr.contains("C:"),
        "stderr must not contain paths, got: {stderr}"
    );
}

#[test]
fn default_invocation_refuses_with_nonzero_and_empty_stdout() {
    assert_refusal(&run(&[], &[]));
}

#[test]
fn forced_flag_refuses_with_same_announcement() {
    let forced = run(&["--force"], &[]);
    let baseline = run(&[], &[]);
    assert_refusal(&forced);
    assert_eq!(
        forced.stderr, baseline.stderr,
        "forced activation must produce the same refusal, not a silent no-op"
    );
}

#[test]
fn forced_env_refuses_with_same_announcement() {
    let forced = run(&[], &[("ELIOT_SEARCH_DOC_WORKER_FORCE", "1")]);
    let baseline = run(&[], &[]);
    assert_refusal(&forced);
    assert_eq!(
        forced.stderr, baseline.stderr,
        "forced activation must produce the same refusal, not a silent no-op"
    );
}

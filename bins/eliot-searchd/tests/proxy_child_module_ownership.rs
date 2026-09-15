use std::path::{Path, PathBuf};

fn crate_root() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
}

fn read(root: &Path, relative: &str) -> String {
    std::fs::read_to_string(root.join(relative))
        .unwrap_or_else(|error| panic!("cannot read {relative}: {error}"))
}

#[test]
fn proxy_child_facade_is_thin() {
    let root = crate_root();
    let facade = read(&root, "src/proxy_child.rs");
    for module in ["lifecycle", "model", "pipe", "spec", "time", "worker"] {
        assert!(facade.contains(&format!("mod {module};")));
    }
    assert!(facade.contains("pub(super) use lifecycle::ChildIo;"));
    assert!(facade.contains("pub(super) use spec::ChildLimits;"));
    assert!(facade.contains("use super::{Terminal, MAX_PROXY_COMMAND_BYTES};"));
    assert!(facade.contains("mod tests;"));
    assert!(facade.len() < 2_000, "proxy child facade grew to {} bytes", facade.len());
    for forbidden in [
        "pub(super) struct ChildIo {",
        "Command::new",
        "forward_reply(",
        "read_child_line(",
        "child.kill()",
    ] {
        assert!(
            !facade.contains(forbidden),
            "proxy child implementation returned to facade: {forbidden}"
        );
    }
}

#[test]
fn proxy_child_owners_remain_bounded_and_vendor_free() {
    let root = crate_root();
    let owners = [
        ("src/proxy_child/spec.rs", "pub(super) struct ChildLimits"),
        ("src/proxy_child/model.rs", "pub(super) struct Exchange"),
        ("src/proxy_child/pipe.rs", "pub(super) struct DeadlineWriter"),
        ("src/proxy_child/time.rs", "pub(super) fn deadline("),
        ("src/proxy_child/worker.rs", "pub(super) fn spawn_worker("),
        ("src/proxy_child/lifecycle.rs", "pub(super) struct ChildIo"),
    ];
    for (relative, marker) in owners {
        let source = read(&root, relative);
        assert!(source.contains(marker), "{relative} lost owner {marker}");
        assert!(
            source.len() < 14_000,
            "{relative} grew to {} bytes",
            source.len()
        );
        for forbidden in ["qdrant_client", "search_qdrant", "reqwest::", "tokio::"] {
            assert!(
                !source.contains(forbidden),
                "{relative} acquired forbidden vendor token {forbidden}"
            );
        }
    }

    let spec = read(&root, "src/proxy_child/spec.rs");
    assert!(spec.contains("MAX_LINE_BYTES: usize = 64 * 1024"));
    assert!(spec.contains("MAX_RESPONSE_LINES: usize = 1_000_000"));
    assert!(spec.contains("MAX_RESPONSE_BYTES: usize = 64 * 1024 * 1024"));
    assert!(spec.contains("startup: Duration::from_secs(30)"));
    assert!(spec.contains("request: Duration::from_secs(120)"));
    assert!(spec.contains("cleanup: Duration::from_secs(5)"));

    let lifecycle = read(&root, "src/proxy_child/lifecycle.rs");
    assert!(lifecycle.contains("spawn_worker"));
    assert!(lifecycle.contains("self.child.kill()"));
    assert!(lifecycle.contains("self.wait_child"));
    assert!(lifecycle.contains("socket.shutdown(Shutdown::Both)"));
    assert!(!lifecycle.contains("forward_reply("));
    assert!(!lifecycle.contains("BufReader"));

    let worker = read(&root, "src/proxy_child/worker.rs");
    assert!(worker.contains("forward_reply("));
    assert!(worker.contains("data_root_ready"));
    assert!(worker.contains("Reply::Complete | Reply::Rejected"));
    assert!(!worker.contains("Command::new"));
    assert!(!worker.contains("child.kill()"));

    let pipe = read(&root, "src/proxy_child/pipe.rs");
    assert!(pipe.contains("LOOPBACK_DIRECT_CHILD_FRAME_TOO_LARGE"));
    assert!(pipe.contains("set_write_timeout(Some(left))"));
    assert!(!pipe.contains("std::process"));

    let timing = read(&root, "src/proxy_child/time.rs");
    assert!(timing.contains("LOOPBACK_DIRECT_CHILD_DEADLINE_EXCEEDED"));
    assert!(timing.contains("recv_timeout"));
    assert!(!timing.contains("TcpStream"));
}

#[test]
fn proxy_child_regression_corpus_is_separate() {
    let root = crate_root();
    let tests = read(&root, "src/proxy_child/tests.rs");
    assert!(tests.len() < 16_000, "proxy child tests grew to {} bytes", tests.len());
    for case in [
        "child_budgets_are_finite_distinct_declared_bounds",
        "hang_before_ready_times_out_and_reaps_within_declared_bounds",
        "hang_mid_response_times_out_without_indefinite_pipe_write",
        "hang_at_shutdown_is_reaped_within_request_plus_cleanup",
    ] {
        assert!(tests.contains(&format!("fn {case}(")), "lost test {case}");
    }
}

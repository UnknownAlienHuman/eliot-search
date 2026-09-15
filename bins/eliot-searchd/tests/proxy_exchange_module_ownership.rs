use std::path::{Path, PathBuf};

fn crate_root() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
}

fn read(root: &Path, relative: &str) -> String {
    std::fs::read_to_string(root.join(relative))
        .unwrap_or_else(|error| panic!("cannot read {relative}: {error}"))
}

#[test]
fn proxy_exchange_facade_is_thin() {
    let root = crate_root();
    let facade = read(&root, "src/proxy_exchange.rs");
    for module in ["fence", "forward", "parser", "reply"] {
        assert!(facade.contains(&format!("mod {module};")));
    }
    for export in [
        "pub(super) use fence::ExchangeFence;",
        "pub(super) use forward::forward_reply;",
        "pub(super) use parser::event_name;",
        "pub(super) use reply::Reply;",
    ] {
        assert!(facade.contains(export), "lost export {export}");
    }
    assert!(facade.contains("mod tests;"));
    assert!(facade.len() < 1_500, "exchange facade grew to {} bytes", facade.len());
    for forbidden in [
        "struct ExchangeFence {",
        "fn forward_reply(",
        "FATAL_CHILD_FRAMES",
        "write_all(",
    ] {
        assert!(
            !facade.contains(forbidden),
            "exchange implementation returned to facade: {forbidden}"
        );
    }
}

#[test]
fn proxy_exchange_owners_remain_bounded_and_closed() {
    let root = crate_root();
    let owners = [
        (
            "src/proxy_exchange/reply.rs",
            "pub(in super::super) enum Reply",
        ),
        (
            "src/proxy_exchange/fence.rs",
            "pub(in super::super) struct ExchangeFence",
        ),
        (
            "src/proxy_exchange/parser.rs",
            "pub(in super::super) fn event_name(",
        ),
        (
            "src/proxy_exchange/forward.rs",
            "pub(in super::super) fn forward_reply(",
        ),
    ];
    for (relative, marker) in owners {
        let source = read(&root, relative);
        assert!(source.contains(marker), "{relative} lost owner {marker}");
        assert!(source.len() < 8_000, "{relative} grew to {} bytes", source.len());
        for forbidden in [
            "qdrant_client",
            "search_qdrant",
            "reqwest::",
            "tokio::",
            "std::process",
            "Command::new",
        ] {
            assert!(
                !source.contains(forbidden),
                "{relative} acquired forbidden token {forbidden}"
            );
        }
    }

    let reply = read(&root, "src/proxy_exchange/reply.rs");
    for variant in ["Complete", "Rejected", "Shutdown", "Fatal"] {
        assert!(reply.contains(variant), "lost reply variant {variant}");
    }

    let fence = read(&root, "src/proxy_exchange/fence.rs");
    assert!(fence.contains("LOOPBACK_DIRECT_CHANNEL_REQUIRES_RESTART"));
    assert!(fence.contains("Reply::Complete | Reply::Rejected"));
    assert!(fence.contains("self.blocked = true"));
    assert!(!fence.contains("Write"));

    let parser = read(&root, "src/proxy_exchange/parser.rs");
    for fatal in [
        "SERVICE_MUTATION_OUTCOME_UNKNOWN",
        "SERVICE_COMMAND_LIMIT_INVALID",
        "SERVICE_COMMAND_TOO_LARGE",
        "SERVICE_COMMAND_NOT_UTF8",
        "SERVICE_READ_ERROR",
    ] {
        assert!(parser.contains(fatal), "lost fatal frame {fatal}");
    }
    assert!(parser.contains("tail.starts_with(',')"));

    let forward = read(&root, "src/proxy_exchange/forward.rs");
    assert!(forward.contains("LOOPBACK_DIRECT_CHILD_CLOSED_MID_RESPONSE"));
    assert!(forward.contains("LOOPBACK_DIRECT_RESPONSE_BYTES_EXCEEDED"));
    assert!(forward.contains("LOOPBACK_DIRECT_RESPONSE_LINE_LIMIT_EXCEEDED"));
    assert!(forward.contains("FATAL_CHILD_FRAMES.contains"));
    assert!(forward.contains("event_name(&line) == Some(\"error\")"));
}

#[test]
fn proxy_exchange_regression_corpus_is_separate() {
    let root = crate_root();
    let tests = read(&root, "src/proxy_exchange/tests.rs");
    assert!(tests.len() < 20_000, "exchange tests grew to {} bytes", tests.len());
    for case in [
        "disconnect_blocks_next_request_without_consuming_old_reply",
        "incomplete_command_write_cannot_be_replayed_automatically",
        "complete_response_releases_fence_and_stops_at_its_terminal",
        "fully_forwarded_command_rejection_does_not_poison_stream",
        "eof_and_exhausted_line_budget_leave_stream_blocked",
        "shutdown_never_reopens_the_exchange_fence",
        "child_fatal_frames_are_forwarded_once_but_never_release_the_exchange",
        "ordinary_child_validation_error_remains_reusable",
        "real_socket_disconnect_mid_large_response_blocks_fence_without_contamination",
        "slow_reader_failure_leaves_fence_blocked_without_replay",
    ] {
        assert!(tests.contains(&format!("fn {case}(")), "lost test {case}");
    }
}

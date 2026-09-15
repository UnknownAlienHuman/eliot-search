use std::path::{Path, PathBuf};

fn crate_root() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
}

fn read(root: &Path, relative: &str) -> String {
    std::fs::read_to_string(root.join(relative))
        .unwrap_or_else(|error| panic!("cannot read {relative}: {error}"))
}

#[test]
fn authenticated_proxy_entry_and_kernel_are_thin() {
    let root = crate_root();
    let entry = read(&root, "src/authenticated_proxy.rs");
    assert!(entry.contains("#[path = \"authenticated_proxy/kernel.rs\"]"));
    assert!(entry.contains("pub use kernel::maybe_run;"));
    assert!(entry.len() < 1_500, "entry grew to {} bytes", entry.len());
    for forbidden in [
        "struct DirectChild",
        "fn dispatch_provider_command",
        "ProviderRouter",
        "TcpStream",
        "Command::new",
    ] {
        assert!(
            !entry.contains(forbidden),
            "implementation returned to proxy entry: {forbidden}"
        );
    }

    let kernel = read(&root, "src/authenticated_proxy/kernel.rs");
    for module in [
        "child", "dispatch", "entry", "envelope", "hello", "key",
        "operation", "server", "spec", "terminal", "wire",
    ] {
        assert!(kernel.contains(&format!("mod {module};")));
    }
    assert!(kernel.contains("#[path = \"../proxy_exchange.rs\"]"));
    assert!(kernel.contains("#[path = \"../proxy_child.rs\"]"));
    assert!(kernel.contains("pub use entry::maybe_run;"));
    assert!(kernel.len() < 2_500, "kernel grew to {} bytes", kernel.len());
}

#[test]
fn authenticated_proxy_responsibilities_stay_separated() {
    let root = crate_root();
    let owners = [
        ("src/authenticated_proxy/kernel/spec.rs", "fn sanitize_json"),
        ("src/authenticated_proxy/kernel/key.rs", "struct ShimKeySource"),
        ("src/authenticated_proxy/kernel/terminal.rs", "enum Terminal"),
        ("src/authenticated_proxy/kernel/child.rs", "struct DirectChild"),
        ("src/authenticated_proxy/kernel/wire.rs", "fn write_provider_line"),
        ("src/authenticated_proxy/kernel/hello.rs", "fn do_hello"),
        ("src/authenticated_proxy/kernel/envelope.rs", "fn do_envelope"),
        ("src/authenticated_proxy/kernel/operation.rs", "fn do_op"),
        (
            "src/authenticated_proxy/kernel/dispatch.rs",
            "fn dispatch_provider_command",
        ),
        ("src/authenticated_proxy/kernel/server.rs", "fn run_proxy"),
        ("src/authenticated_proxy/kernel/entry.rs", "pub fn maybe_run"),
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
                "{relative} acquired forbidden provider token {forbidden}"
            );
        }
    }

    let key = read(&root, "src/authenticated_proxy/kernel/key.rs");
    assert!(key.contains("field(\"key\", &\"<redacted>\")"));
    assert!(key.contains("self.key.fill(0)"));
    assert!(!key.contains("TcpStream"));

    let child = read(&root, "src/authenticated_proxy/kernel/child.rs");
    assert!(child.contains("ExchangeFence"));
    assert!(child.contains("ChildIo::spawn"));
    assert!(!child.contains("ProviderRouter::open"));

    let envelope = read(&root, "src/authenticated_proxy/kernel/envelope.rs");
    assert!(envelope.contains("OutcomeUnknown"));
    assert!(envelope.contains("seal_response_with_receipt"));
    assert!(!envelope.contains("Command::new"));

    let server = read(&root, "src/authenticated_proxy/kernel/server.rs");
    assert!(server.contains("serve_loopback_with_source"));
    assert!(server.contains("DirectChild::spawn"));
    assert!(!server.contains("decode_envelope_frame"));

    let child_adapter = read(&root, "src/proxy_child.rs");
    assert!(child_adapter.contains("use super::{Terminal, MAX_PROXY_COMMAND_BYTES};"));
    assert!(child_adapter.contains("pub(super) use lifecycle::ChildIo;"));
    assert!(child_adapter.contains("pub(super) use spec::ChildLimits;"));

    let exchange_adapter = read(&root, "src/proxy_exchange.rs");
    assert!(exchange_adapter.contains("pub(super) use fence::ExchangeFence;"));
    assert!(exchange_adapter.contains("pub(super) use forward::forward_reply;"));
    assert!(exchange_adapter.contains("pub(super) use parser::event_name;"));
    assert!(exchange_adapter.contains("pub(super) use reply::Reply;"));
}

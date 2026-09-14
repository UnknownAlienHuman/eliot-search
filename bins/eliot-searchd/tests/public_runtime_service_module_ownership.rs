use std::path::{Path, PathBuf};

fn crate_root() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
}

fn read(root: &Path, relative: &str) -> String {
    std::fs::read_to_string(root.join(relative))
        .unwrap_or_else(|error| panic!("cannot read {relative}: {error}"))
}

#[test]
fn public_runtime_entry_and_kernel_are_thin() {
    let root = crate_root();
    let entry = read(&root, "src/public_runtime_service.rs");
    assert!(entry.contains("#[path = \"public_runtime_service/kernel.rs\"]"));
    assert!(entry.contains("pub use kernel::maybe_run;"));
    assert!(entry.len() < 1_200, "entry grew to {} bytes", entry.len());

    let kernel = read(&root, "src/public_runtime_service/kernel.rs");
    for module in [
        "codec", "diagnostics", "dispatch", "entry", "mutation", "query",
        "reporting", "runtime", "spec", "state",
    ] {
        assert!(kernel.contains(&format!("mod {module};")));
    }
    assert!(kernel.contains("#[path = \"../service_session.rs\"]"));
    assert!(kernel.contains("pub use entry::maybe_run;"));
    assert!(kernel.len() < 2_000, "kernel grew to {} bytes", kernel.len());

    for source in [&entry, &kernel] {
        for forbidden in [
            "fn run_service",
            "struct CommandState",
            "fn execute_command",
            "DirectStore::open",
            "catalog_quarantine::arm",
        ] {
            assert!(
                !source.contains(forbidden),
                "implementation returned to runtime facade: {forbidden}"
            );
        }
    }
}

#[test]
fn public_runtime_responsibilities_stay_separated() {
    let root = crate_root();
    let owners = [
        ("src/public_runtime_service/kernel/spec.rs", "MAX_COMMAND_BYTES"),
        ("src/public_runtime_service/kernel/codec.rs", "fn decode_path"),
        ("src/public_runtime_service/kernel/state.rs", "struct CommandState"),
        ("src/public_runtime_service/kernel/reporting.rs", "fn emit_verification"),
        ("src/public_runtime_service/kernel/diagnostics.rs", "fn cmd_health"),
        ("src/public_runtime_service/kernel/query.rs", "fn cmd_search_page"),
        ("src/public_runtime_service/kernel/mutation.rs", "fn cmd_index_file"),
        ("src/public_runtime_service/kernel/dispatch.rs", "fn execute_command"),
        ("src/public_runtime_service/kernel/runtime.rs", "fn run_service"),
        ("src/public_runtime_service/kernel/entry.rs", "pub fn maybe_run"),
    ];
    for (relative, marker) in owners {
        let source = read(&root, relative);
        assert!(source.contains(marker), "{relative} lost owner {marker}");
        assert!(
            source.len() < 18_000,
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

    let runtime = read(&root, "src/public_runtime_service/kernel/runtime.rs");
    assert!(runtime.contains("DataRootGuard::acquire"));
    assert!(runtime.contains("session::serve"));
    assert!(runtime.contains("guard.begin_drain"));
    assert!(runtime.contains("guard.release_cleanly"));
    assert!(!runtime.contains("catalog_quarantine::arm"));

    let dispatch = read(&root, "src/public_runtime_service/kernel/dispatch.rs");
    assert!(dispatch.contains("catalog_quarantine::check"));
    assert!(dispatch.contains("catalog_quarantine::arm"));
    assert!(dispatch.contains("catalog_quarantine::clear"));
    assert!(dispatch.contains("state.attempt.arm()"));
    assert!(!dispatch.contains("DirectStore::open"));

    let mutation = read(&root, "src/public_runtime_service/kernel/mutation.rs");
    assert!(mutation.contains("cmd_sync_directory"));
    assert!(mutation.contains("let directory = decode_path(path_hex)?;"));
    assert!(mutation.contains("attempt.arm()"));
    assert!(!mutation.contains("ContinuationError"));

    let query = read(&root, "src/public_runtime_service/kernel/query.rs");
    assert!(query.contains("verify_spine_gate"));
    assert!(query.contains("continue_page"));
    assert!(query.contains("mint_page"));
    assert!(!query.contains("catalog_quarantine::arm"));

    let session = read(&root, "src/service_session.rs");
    assert!(session.contains("MUTATION_UNKNOWN"));
    assert!(session.contains("output.failed"));
    assert!(session.contains("attempt.dispatched"));
}

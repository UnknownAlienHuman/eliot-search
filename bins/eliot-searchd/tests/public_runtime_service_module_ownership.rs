use std::path::{Path, PathBuf};

fn crate_root() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
}

fn read(root: &Path, relative: &str) -> String {
    std::fs::read_to_string(root.join(relative))
        .unwrap_or_else(|error| panic!("cannot read {relative}: {error}"))
        .replace("\r\n", "\n")
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
        "codec",
        "diagnostics",
        "dispatch",
        "entry",
        "mutation",
        "query",
        "reporting",
        "runtime",
        "session",
        "spec",
        "state",
    ] {
        assert!(kernel.contains(&format!("mod {module};")));
    }
    assert!(kernel.contains("pub use entry::maybe_run;"));
    assert!(
        kernel.len() < 2_000,
        "kernel grew to {} bytes",
        kernel.len()
    );

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
        (
            "src/public_runtime_service/kernel/spec.rs",
            "MAX_COMMAND_BYTES",
        ),
        (
            "src/public_runtime_service/kernel/codec.rs",
            "fn decode_path",
        ),
        (
            "src/public_runtime_service/kernel/state.rs",
            "struct CommandState",
        ),
        (
            "src/public_runtime_service/kernel/reporting.rs",
            "fn emit_verification",
        ),
        (
            "src/public_runtime_service/kernel/diagnostics.rs",
            "fn cmd_health",
        ),
        (
            "src/public_runtime_service/kernel/query.rs",
            "fn cmd_search_page",
        ),
        (
            "src/public_runtime_service/kernel/mutation.rs",
            "fn cmd_index_file",
        ),
        (
            "src/public_runtime_service/kernel/dispatch.rs",
            "fn execute_command",
        ),
        (
            "src/public_runtime_service/kernel/runtime.rs",
            "fn run_service",
        ),
        ("src/public_runtime_service/kernel/session.rs", "fn serve"),
        (
            "src/public_runtime_service/kernel/entry.rs",
            "pub fn maybe_run",
        ),
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
    for marker in [
        "NativeRootRequest::for_service",
        "admit_existing",
        ".read_startup()",
        ".read_command(",
        ".retain_shutdown()",
        ".close()",
        ".finish(shutdown)",
    ] {
        assert!(
            runtime.contains(marker),
            "missing native service handoff: {marker}"
        );
    }
    assert!(runtime.contains("session::serve"));
    assert!(runtime.contains("use crate::direct_store::NativeOutput;"));
    assert!(
        !root
            .join("src/public_runtime_service/kernel/native_output.rs")
            .exists()
    );
    let output = read(&root, "src/secure_direct_store/existing_read_output.rs");
    assert!(output.contains("child.write_output(self.writer, bytes)"));
    assert!(output.contains("child.flush_output(self.writer)"));
    assert!(!runtime.contains("DataRootGuard::acquire"));
    assert!(!runtime.contains("DirectStore::open"));
    assert!(!runtime.contains("guard.begin_drain"));
    assert!(!runtime.contains("guard.release_cleanly"));
    assert!(!runtime.contains("catalog_quarantine::arm"));

    let dispatch = read(&root, "src/public_runtime_service/kernel/dispatch.rs");
    assert!(dispatch.contains("store: &ReadOnlyStore<'_>"));
    assert!(dispatch.contains("SERVICE_MUTATION_UNAVAILABLE"));
    assert!(dispatch.contains("with_read_command"));
    assert!(dispatch.contains("#[cfg(test)]\nfn execute_mutating_command"));
    assert!(dispatch.contains("#[cfg(test)]\nfn execute_quarantined_writes"));
    assert!(dispatch.contains("catalog_quarantine::arm"));
    assert!(dispatch.contains("catalog_quarantine::clear"));
    assert!(dispatch.contains("state.attempt.arm()"));
    assert!(dispatch.contains("with_read_command(command, |name|"));
    let execute = dispatch
        .split("pub(super) fn execute_command(")
        .nth(1)
        .unwrap();
    assert!(
        execute.find("with_read_command(command, |name|").unwrap()
            < execute.find("store.checkpoint()?").unwrap()
    );
    assert!(runtime.contains("service_mutations_available"));
    assert!(runtime.contains("service_control_migration_available"));
    assert!(
        runtime
            .find("let refusal = command_refusal(command)")
            .unwrap()
            < runtime.find(".read_command(command").unwrap()
    );
    assert!(
        runtime.find(".refuse_command(command").unwrap()
            < runtime.find(".read_command(command").unwrap()
    );
    assert!(!dispatch.contains("DirectStore::open"));

    let diagnostics = read(&root, "src/public_runtime_service/kernel/diagnostics.rs");
    assert!(diagnostics.contains("env::args_os()"));
    assert!(!diagnostics.contains("env::args()"));

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

    let state = read(&root, "src/public_runtime_service/kernel/state.rs");
    assert!(state.contains("struct ReadCommandState"));
    assert!(state.contains("pub(super) store: &'a S"));
    let state_lines: Vec<_> = state.lines().map(str::trim).collect();
    let writable = state_lines
        .iter()
        .position(|line| line.contains("struct CommandState"))
        .unwrap();
    assert_eq!(state_lines[writable - 1], "#[cfg(test)]");
    let kernel = read(&root, "src/public_runtime_service/kernel.rs");
    let kernel_lines: Vec<_> = kernel.lines().map(str::trim).collect();
    let mutation_module = kernel_lines
        .iter()
        .position(|line| *line == "mod mutation;")
        .unwrap();
    assert_eq!(kernel_lines[mutation_module - 1], "#[cfg(test)]");

    let session = read(&root, "src/public_runtime_service/kernel/session.rs");
    assert!(session.contains("MUTATION_UNKNOWN"));
    assert!(session.contains("output.failed"));
    assert!(session.contains("attempt.dispatched"));
    assert!(session.contains("mod tests;"));
    assert!(
        root.join("src/public_runtime_service/kernel/session/tests.rs")
            .is_file()
    );
    assert!(
        !root.join("src/service_session.rs").exists(),
        "obsolete root-level session owner returned"
    );
    assert!(
        !root.join("src/service_session_tests.rs").exists(),
        "obsolete root-level session tests returned"
    );
}

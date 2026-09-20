use std::path::{Path, PathBuf};

fn crate_root() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
}

fn read(root: &Path, relative: &str) -> String {
    std::fs::read_to_string(root.join(relative))
        .unwrap_or_else(|error| panic!("cannot read {relative}: {error}"))
}

#[test]
fn app_entry_and_kernel_are_thin_stable_facades() {
    let root = crate_root();
    let entry = read(&root, "src/app.rs");
    assert!(entry.contains("#[path = \"app/kernel.rs\"]"));
    assert!(entry.contains("pub use kernel::*;"));
    assert!(entry.len() < 1_500, "facade grew to {} bytes", entry.len());

    let kernel = read(&root, "src/app/kernel.rs");
    for module in [
        "commands", "dispatch", "output", "protocol", "source_root_commands",
        "spec", "status",
    ] {
        assert!(kernel.contains(&format!("mod {module};")));
    }
    assert!(kernel.contains("pub use dispatch::run_main;"));
    assert!(kernel.contains("mod tests;"));
    assert!(kernel.len() < 2_000, "kernel grew to {} bytes", kernel.len());

    for source in [&entry, &kernel] {
        for forbidden in [
            "fn serve_control",
            "fn cmd_index_file",
            "DirectStore",
            "std::process::ExitCode",
            "std::env::args",
        ] {
            assert!(
                !source.contains(forbidden),
                "implementation returned to an app facade: {forbidden}"
            );
        }
    }
}

#[test]
fn app_responsibilities_stay_separated() {
    let root = crate_root();
    let owners = [
        ("src/app/kernel/spec.rs", "enum Command"),
        ("src/app/kernel/protocol.rs", "fn serve_control"),
        ("src/app/kernel/status.rs", "fn shell_health_effective"),
        ("src/app/kernel/output.rs", "fn emit_store_search"),
        ("src/app/kernel/commands.rs", "fn cmd_index_file"),
        ("src/app/kernel/source_root_commands.rs", "pub fn run"),
        ("src/app/kernel/dispatch.rs", "pub fn run_main"),
    ];
    for (relative, marker) in owners {
        let source = read(&root, relative);
        assert!(source.contains(marker), "{relative} lost owner {marker}");
        assert!(
            source.len() < 18_000,
            "{relative} grew to {} bytes",
            source.len()
        );
        for forbidden in ["qdrant_client", "reqwest::", "tokio::"] {
            assert!(
                !source.contains(forbidden),
                "{relative} acquired forbidden transport token {forbidden}"
            );
        }
    }

    let spec = read(&root, "src/app/kernel/spec.rs");
    assert!(spec.contains("MAX_COMMAND_BYTES"));
    assert!(spec.contains("MAX_RESPONSE_BYTES"));
    assert!(!spec.contains("DirectStore"));
    assert!(!spec.contains("std::fs"));

    let protocol = read(&root, "src/app/kernel/protocol.rs");
    assert!(protocol.contains("crate::protocol_io::read_line"));
    assert!(!protocol.contains("cmd_index_file"));
    assert!(!protocol.contains("DirectStore"));

    let commands = read(&root, "src/app/kernel/commands.rs");
    assert!(commands.contains("DataRootGuard::acquire"));
    assert!(commands.contains("collect_orphan_revisions"));
    assert!(!commands.contains("MAX_COMMAND_BYTES"));
    assert!(!commands.contains("pub fn run_main"));

    let source_roots = read(&root, "src/app/kernel/source_root_commands.rs");
    assert!(source_roots.contains("DataRootGuard::acquire"));
    assert!(source_roots.contains("SourceRootCatalog"));
    assert!(source_roots.contains("sync_registered"));
    assert!(source_roots.contains("escape_json"));
    assert!(!source_roots.contains("pub fn run_main"));

    let dispatch = read(&root, "src/app/kernel/dispatch.rs");
    assert_eq!(dispatch.matches("pub fn run_main").count(), 1);
    assert!(dispatch.contains("super::source_root_commands::run"));
    assert!(dispatch.contains("super::source_root_commands::escape_json"));
    assert!(!dispatch.contains("crate::source_root_commands"));
    assert!(!dispatch.contains("DirectStore::open"));
    assert!(!dispatch.contains("collect_orphan_revisions"));

    let entry = read(&root, "src/entry.rs");
    assert!(!entry.contains("mod source_root_commands;"));
    assert!(
        !root.join("src/source_root_commands.rs").exists(),
        "obsolete daemon-root source-root command owner returned"
    );

    let tests = read(&root, "src/app/kernel/tests.rs");
    assert!(tests.contains("oversized_frame_terminates_session_without_executing_suffix"));
    assert!(tests.contains("valid_control_session_reports_health_and_clean_shutdown"));
}

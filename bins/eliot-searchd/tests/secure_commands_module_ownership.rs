use std::path::{Path, PathBuf};

fn crate_root() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
}

fn read(root: &Path, relative: &str) -> String {
    std::fs::read_to_string(root.join(relative))
        .unwrap_or_else(|error| panic!("cannot read {relative}: {error}"))
}

#[test]
fn secure_commands_entry_and_kernel_are_thin() {
    let root = crate_root();
    let entry = read(&root, "src/secure_commands.rs");
    assert!(entry.contains("#[path = \"secure_commands/kernel.rs\"]"));
    assert!(entry.contains("pub use kernel::maybe_run;"));
    assert!(entry.len() < 1_500, "entry grew to {} bytes", entry.len());
    for forbidden in [
        "fn dispatch(",
        "cmd_gc_root",
        "DataRootGuard",
        "DirectStore",
        "io::stdout",
    ] {
        assert!(
            !entry.contains(forbidden),
            "secure command implementation returned to entry: {forbidden}"
        );
    }

    let kernel = read(&root, "src/secure_commands/kernel.rs");
    for module in ["commands", "dispatch", "entry", "output", "store", "support"] {
        assert!(kernel.contains(&format!("mod {module};")));
    }
    assert!(kernel.contains("pub use entry::maybe_run;"));
    assert!(kernel.len() < 1_500, "kernel grew to {} bytes", kernel.len());
}

#[test]
fn secure_command_responsibilities_remain_bounded() {
    let root = crate_root();
    let owners = [
        ("src/secure_commands/kernel/entry.rs", "pub fn maybe_run()"),
        (
            "src/secure_commands/kernel/dispatch.rs",
            "pub(super) fn dispatch(",
        ),
        (
            "src/secure_commands/kernel/commands.rs",
            "pub(super) fn cmd_index_directory(",
        ),
        (
            "src/secure_commands/kernel/store.rs",
            "pub(super) fn with_store<T>(",
        ),
        (
            "src/secure_commands/kernel/output.rs",
            "pub(super) fn emit_verification(",
        ),
        (
            "src/secure_commands/kernel/support.rs",
            "pub(super) const MAX_DIAGNOSTIC_REVISION_SLICE_BYTES",
        ),
    ];
    for (relative, marker) in owners {
        let source = read(&root, relative);
        assert!(source.contains(marker), "{relative} lost owner {marker}");
        assert!(
            source.len() < 14_000,
            "{relative} grew to {} bytes",
            source.len()
        );
        for forbidden in [
            "qdrant_client",
            "search_qdrant",
            "reqwest::",
            "tokio::",
            "std::process::Command",
        ] {
            assert!(
                !source.contains(forbidden),
                "{relative} acquired forbidden token {forbidden}"
            );
        }
    }

    let entry = read(&root, "src/secure_commands/kernel/entry.rs");
    assert!(entry.contains("env::args_os"));
    assert!(entry.contains("is_persistent_command"));
    assert!(entry.contains("emit_process_error"));
    assert!(!entry.contains("DirectStore"));

    let dispatch = read(&root, "src/secure_commands/kernel/dispatch.rs");
    for command in [
        "--health-data-root",
        "--index-file",
        "--index-directory",
        "--search-root",
        "--search-root-ascii-insensitive",
        "--list-sources",
        "--verify-root",
        "--retire-source",
        "--read-revision",
        "--repair-root",
        "--gc-root",
    ] {
        assert!(dispatch.contains(command), "lost dispatch command {command}");
    }
    assert!(!dispatch.contains("DataRootGuard::acquire"));
    assert!(!dispatch.contains("guarded_collect_orphan_revisions"));

    let commands = read(&root, "src/secure_commands/kernel/commands.rs");
    assert!(commands.contains("repair_control_log"));
    assert!(commands.contains("guarded_collect_orphan_revisions"));
    assert!(commands.contains("DIRECT_REVISION_SLICE_TOO_LARGE"));
    assert!(commands.contains("StorageSecurityStatus::inspect"));

    let store = read(&root, "src/secure_commands/kernel/store.rs");
    assert!(store.contains("DataRootGuard::acquire"));
    assert!(store.contains("DirectStore::open"));
    assert!(store.contains("StorageSecurityStatus::inspect"));
    assert!(!store.contains("io::stdout"));

    let support = read(&root, "src/secure_commands/kernel/support.rs");
    assert!(support.contains("24 * 1024"));
    assert!(support.contains("plaintext-development profile"));
    assert!(support.contains("USAGE_ERROR"));

    let output = read(&root, "src/secure_commands/kernel/output.rs");
    assert!(output.contains("diagnostic_internal_identifiers"));
    assert!(output.contains("direct_store_verified"));
    assert!(output.contains("source_list_complete"));
    assert!(output.contains("ExitCode::from(2)"));
}

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
    for module in [
        "commands", "dispatch", "entry", "output", "read", "store", "support",
    ] {
        assert!(kernel.contains(&format!("mod {module};")));
    }
    assert!(kernel.contains("pub use entry::maybe_run;"));
    assert!(
        kernel.len() < 1_500,
        "kernel grew to {} bytes",
        kernel.len()
    );
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
        assert!(
            dispatch.contains(command),
            "lost dispatch command {command}"
        );
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

#[test]
fn one_shot_reads_use_existing_native_custody_and_the_shared_output_adapter() {
    let root = crate_root();
    let entry = read(&root, "src/secure_commands/kernel/entry.rs");
    assert!(entry.contains("super::read::run(&raw, started)"));
    assert!(
        entry.find("super::read::run(&raw, started)").unwrap()
            < entry.find("dispatch(command, &raw)").unwrap()
    );
    let read_owner = read(&root, "src/secure_commands/kernel/read.rs");
    assert!(
        read_owner
            .split_whitespace()
            .collect::<String>()
            .contains("output.flush()")
    );
    for marker in [
        "NativeRootRequest::for_read_one_shot",
        "admit_existing",
        ".read_startup()",
        "startup.run_one_shot",
        "NativeOutput::new",
    ] {
        assert!(read_owner.contains(marker));
    }
    for forbidden in [
        "DirectStore::open",
        "DataRootGuard",
        "initialize_new",
        "begin_drain",
    ] {
        assert!(!read_owner.contains(forbidden));
    }
    let request = read(&root, "src/owner_composition/kernel/native_request.rs");
    assert!(request.contains("pub(crate) fn for_read_one_shot("));
    let lifetime = read(&root, "src/owner_composition/kernel/read_child.rs");
    let invocation = lifetime
        .split("pub(crate) fn run_one_shot<T>(")
        .nth(1)
        .unwrap()
        .split("pub(crate) fn identity")
        .next()
        .unwrap();
    assert!(invocation.find("drop(store)").unwrap() < invocation.find("drop((pins,").unwrap());
    assert!(invocation.contains("one-shot-custody-released"));
    assert!(!invocation.contains("NativeRootRequest::new"));
}

#[test]
fn source_root_inspection_is_catalog_only_and_keeps_the_original_output_operation() {
    let root = crate_root();
    let route = read(&root, "src/secure_commands/kernel/source_roots.rs");
    assert!(route.contains("SourceRootInspection::read(operation)"));
    assert!(route.contains("operation.emit_registration"));
    assert!(route.contains("!operation.output_attempted()"));
    assert!(!route.contains("read_startup"));
    assert!(!route.contains("NativeRootRequest::new"));
    assert!(!route.contains("credential"));
    let lifetime = read(&root, "src/owner_composition/kernel/registration_read.rs");
    let child = lifetime
        .split("pub(crate) fn run_registration_one_shot<T>(")
        .nth(1)
        .unwrap();
    assert!(child.find("drop(operation)").unwrap() < child.find("drop((pins,").unwrap());
    assert!(child.contains("registration-custody-released"));
    assert!(!child.contains("NativeWork::new"));
}
#[test]
fn paired_registration_route_keeps_original_native_request_and_excludes_sync_startup() {
    let root = crate_root();
    let entry = read(&root, "src/secure_commands/kernel/entry.rs");
    assert!(entry.contains("source_registration::run(&raw, started)"));
    let route = read(&root, "src/secure_commands/kernel/source_registration.rs");
    for required in ["for_source_registration", "run_source_registration", "SOURCE_ROOT_USAGE_ERROR"]
    { assert!(route.contains(required)); }
    for forbidden in ["DirectStore", "DataRootGuard", "sync_directory(", "admit_existing(", "request.emit"]
    { assert!(!route.contains(forbidden)); }
}

#[test]
fn sync_preflight_keeps_native_context_and_has_no_effect_path() {
    let root = crate_root();
    let entry = read(&root, "src/secure_commands/kernel/entry.rs");
    let selected = entry.find("source_sync::run(&raw, started)").unwrap();
    assert!(selected < entry.find("dispatch(command, &raw)").unwrap());
    let route = read(&root, "src/secure_commands/kernel/source_sync.rs");
    assert!(route.contains("selected.len() != 2"));
    assert!(route.find("selected.len() != 2").unwrap()
        < route.find("NativeRootRequest::for_source_sync").unwrap());
    assert!(route.contains("started\n        .checked_add"));
    assert!(route.contains("run_source_sync_preflight(request"));
    let request = read(&root, "src/owner_composition/kernel/native_request.rs");
    let constructor = request.split("pub(crate) fn for_source_sync(").nth(1).unwrap()
        .split("pub(crate) fn emit_validation_refusal(").next().unwrap();
    assert!(constructor.contains("DataRootOpenMode::OpenExistingMutating"));
    assert!(constructor.contains("strip_config_args_os(arguments)"));
    assert!(constructor.contains("request.root = PathBuf::from(root)"));
    let native = read(&root, "src/owner_composition/kernel/source_sync.rs");
    for required in [
        "acquire_existing(&request, &mut work)", "acquired.bind(request, work)",
        "RegistrationPin::acquire", "SOURCE_ROOT_SYNC_EFFECTS_UNAVAILABLE",
        "self.bytes.take()", "*self.consumed = true", "self.output_attempted = true",
        "count != frame.len()", "LifecycleState::Released",
        "self.owner.verify()?", ".registration\n            .verify",
    ] { assert!(native.contains(required), "missing {required}"); }
    assert!(native.find("let checked = holder").unwrap()
        < native.find("holder.terminal(output, code)").unwrap());
    assert_eq!(native.matches(".write(frame.as_bytes())").count(), 1);
    assert!(native.find("drop((registration, bytes))").unwrap()
        < native.find("close_owner(owner, output_attempted)").unwrap());
    assert!(native.find("drop((pins, root, installation, record, bound_root))").unwrap()
        < native.find("drop(exclusions)").unwrap());
    let close = native.split("fn close_owner(").nth(1).unwrap()
        .split("// Exactly one complete frame write;").next().unwrap();
    assert!(close.contains("if output_attempted && result.is_err()"));
    assert!(close.contains(".store(true, Ordering::Release)"));
    assert!(close.contains("return Err(OwnerError::OwnerAcquireOutcomeUnknown)"));
    assert!(native.contains("close_owner(owner, emitted.is_ok())"));
    for source in [&route, &native] {
        for forbidden in [
            "DataRootGuard", "DirectStore", "RevisionProtector", "sync_directory(",
            "collect_regular_files", "finish_in_place", "mark_reconciled_synced",
            "write_all", "println!", "create(true)", "rename(", "remove_file(",
            "protect(", "unprotect(",
        ] { assert!(!source.contains(forbidden), "sync regained {forbidden}"); }
    }
}

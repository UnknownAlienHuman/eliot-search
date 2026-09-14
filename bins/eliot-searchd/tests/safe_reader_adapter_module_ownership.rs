use std::path::{Path, PathBuf};

fn crate_root() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
}

fn read(root: &Path, relative: &str) -> String {
    std::fs::read_to_string(root.join(relative))
        .unwrap_or_else(|error| panic!("cannot read {relative}: {error}"))
}

#[test]
fn safe_reader_adapter_entry_and_kernel_are_thin() {
    let root = crate_root();
    let entry = read(&root, "src/safe_reader_adapter.rs");
    assert!(entry.contains("#[path = \"safe_reader_adapter/kernel.rs\"]"));
    assert!(entry.contains("pub use kernel::*;"));
    assert!(entry.len() < 1_500, "entry grew to {} bytes", entry.len());
    for forbidden in [
        "struct FinalHandleBackend",
        "fn read_full_file_via_kernel",
        "fn derive_locator",
        "SafeReadBackend",
        "std::fs",
        "#[cfg(test)]",
    ] {
        assert!(
            !entry.contains(forbidden),
            "implementation returned to adapter entry: {forbidden}"
        );
    }

    let kernel = read(&root, "src/safe_reader_adapter/kernel.rs");
    for module in ["backend", "identity", "path", "read", "spec"] {
        assert!(kernel.contains(&format!("mod {module};")));
    }
    assert!(kernel.contains("mod tests;"));
    assert!(kernel.contains("pub use read::{"));
    assert!(kernel.contains("pub use spec::{"));
    assert!(kernel.len() < 3_000, "kernel grew to {} bytes", kernel.len());
}

#[test]
fn safe_reader_adapter_responsibilities_stay_separated() {
    let root = crate_root();
    let owners = [
        ("src/safe_reader_adapter/kernel/spec.rs", "pub enum AdapterError"),
        (
            "src/safe_reader_adapter/kernel/path.rs",
            "pub fn derive_locator(",
        ),
        (
            "src/safe_reader_adapter/kernel/identity.rs",
            "pub fn file_identity_digest(",
        ),
        (
            "src/safe_reader_adapter/kernel/backend.rs",
            "impl SafeReadBackend for FinalHandleBackend",
        ),
        (
            "src/safe_reader_adapter/kernel/read.rs",
            "pub fn read_full_file_via_kernel(",
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
        for forbidden in [
            "qdrant_client",
            "search_qdrant",
            "reqwest::",
            "tokio::",
            "std::process::Command",
            "Command::new",
        ] {
            assert!(
                !source.contains(forbidden),
                "{relative} acquired forbidden token {forbidden}"
            );
        }
    }

    let spec = read(&root, "src/safe_reader_adapter/kernel/spec.rs");
    assert!(spec.contains("ADAPTER_NO_EXECUTE"));
    assert!(spec.contains("SAFE_ADAPTER_ANCESTOR_REPARSE_DENIED"));
    assert!(!spec.contains("File::open"));

    let path = read(&root, "src/safe_reader_adapter/kernel/path.rs");
    assert!(path.contains("validate_relative_token"));
    assert!(path.contains("verify_ancestor_containment"));
    assert!(path.contains("symlink_metadata"));
    assert!(!path.contains("safe_read_with_retries"));

    let identity = read(&root, "src/safe_reader_adapter/kernel/identity.rs");
    assert!(identity.contains("eliot-search/safe-adapter-root/v1"));
    assert!(identity.contains("eliot-search/safe-adapter-file/v1"));
    assert!(identity.contains("verify_handle_rebinding"));
    assert!(!identity.contains("SafeReadBackend"));

    let backend = read(&root, "src/safe_reader_adapter/kernel/backend.rs");
    assert!(backend.contains("FinalHandleBackend"));
    assert!(backend.contains("ReadSecurityDisposition::Permitted"));
    assert!(backend.contains("safe-adapter-meta-v1"));
    assert!(backend.contains("safe-adapter-read-v1"));
    assert!(!backend.contains("safe_read_with_retries"));

    let orchestrator = read(&root, "src/safe_reader_adapter/kernel/read.rs");
    assert!(orchestrator.contains("safe_read_with_retries"));
    assert!(orchestrator.contains("ADAPTER_MAX_ATTEMPTS"));
    assert!(orchestrator.contains("HandleChangedDuringRead"));
    assert!(!orchestrator.contains("symlink_metadata"));
}

#[test]
fn safe_reader_adapter_regression_corpus_is_separate() {
    let root = crate_root();
    let tests = read(&root, "src/safe_reader_adapter/kernel/tests.rs");
    assert!(tests.len() < 30_000, "adapter tests grew to {} bytes", tests.len());
    for case in [
        "token_validation_denies_escape_and_streams",
        "symlink_final_object_is_denied_before_open",
        "replacement_between_derive_and_open_is_denied",
        "ancestor_junction_escape_is_denied",
        "hardlink_outside_domain_is_denied",
        "empty_source_reads_empty_and_oversized_fails_before_allocation",
        "failures_are_content_free",
        "script_source_is_inert_data_and_byte_exact",
        "root_relocation_is_denied_explicitly",
    ] {
        assert!(tests.contains(&format!("fn {case}(")), "lost test {case}");
    }
}

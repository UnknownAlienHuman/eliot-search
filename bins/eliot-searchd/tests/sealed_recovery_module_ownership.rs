use std::path::{Path, PathBuf};

fn crate_root() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
}

fn read(root: &Path, relative: &str) -> String {
    std::fs::read_to_string(root.join(relative))
        .unwrap_or_else(|error| panic!("cannot read {relative}: {error}"))
}

#[test]
fn sealed_recovery_entry_and_kernel_are_thin() {
    let root = crate_root();
    let entry = read(&root, "src/sealed_recovery.rs");
    assert!(entry.contains("#[path = \"sealed_recovery/kernel.rs\"]"));
    assert!(entry.contains("pub use kernel::*;"));
    assert!(entry.len() < 1_500, "entry grew to {} bytes", entry.len());
    for forbidden in [
        "pub enum SealedRecoveryError",
        "pub struct SealedRecoveryReport",
        "pub fn recover_all(",
        "fs::read_dir",
        "inspect_transaction",
        "put_idempotent_verified",
    ] {
        assert!(
            !entry.contains(forbidden),
            "sealed recovery implementation returned to facade: {forbidden}"
        );
    }

    let kernel = read(&root, "src/sealed_recovery/kernel.rs");
    for module in ["api", "platform", "report", "spec"] {
        assert!(kernel.contains(&format!("mod {module};")));
    }
    assert!(kernel.contains("pub use api::recover_all;"));
    assert!(kernel.contains("pub use report::{RecoveryIssue, SealedRecoveryReport};"));
    assert!(kernel.contains("mod tests;"));
    assert!(kernel.len() < 2_500, "kernel grew to {} bytes", kernel.len());
}

#[test]
fn sealed_recovery_responsibilities_remain_bounded() {
    let root = crate_root();
    let owners = [
        (
            "src/sealed_recovery/kernel/spec.rs",
            "pub enum SealedRecoveryError",
        ),
        (
            "src/sealed_recovery/kernel/report.rs",
            "pub struct SealedRecoveryReport",
        ),
        (
            "src/sealed_recovery/kernel/api.rs",
            "pub fn recover_all(",
        ),
        (
            "src/sealed_recovery/kernel/platform/windows.rs",
            "fn reconcile_operation(",
        ),
        (
            "src/sealed_recovery/kernel/platform/unsupported.rs",
            "pub(super) fn recover_all(",
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
        ] {
            assert!(
                !source.contains(forbidden),
                "{relative} acquired forbidden token {forbidden}"
            );
        }
    }

    let api = read(&root, "src/sealed_recovery/kernel/api.rs");
    assert!(api.contains("!owner.root_lock_held() || owner.epoch() == 0"));
    assert!(api.contains("SealedRecoveryError::OwnerGuardRequired"));
    assert!(!api.contains("std::fs"));
    assert!(!api.contains("inspect_transaction"));

    let report = read(&root, "src/sealed_recovery/kernel/report.rs");
    assert!(report.contains("self.issues.len() < MAX_RECOVERY_ISSUES"));
    assert!(report.contains("self.omitted_issue_count"));
    assert!(!report.contains("std::fs"));

    let windows = read(&root, "src/sealed_recovery/kernel/platform/windows.rs");
    assert!(windows.contains("fs::read_dir"));
    assert!(windows.contains("FILE_ATTRIBUTE_REPARSE_POINT"));
    assert!(windows.contains("inspect_transaction"));
    assert!(windows.contains("put_idempotent_verified"));
    assert!(windows.contains("TransactionStatus::CommittedCleanupPending"));
    assert!(windows.contains("MAX_RECOVERY_OPERATIONS"));
    assert!(windows.contains(".transaction-"));
}

#[test]
fn sealed_recovery_contracts_and_tests_stay_closed() {
    let root = crate_root();
    let spec = read(&root, "src/sealed_recovery/kernel/spec.rs");
    for reason in [
        "SEALED_RECOVERY_UNSUPPORTED_PLATFORM",
        "SEALED_RECOVERY_OWNER_GUARD_REQUIRED",
        "SEALED_RECOVERY_TRANSACTION_DIRECTORY_INVALID",
        "SEALED_RECOVERY_TRANSACTION_FILENAME_INVALID",
        "SEALED_RECOVERY_OPERATION_CAPACITY_EXCEEDED",
        "SEALED_RECOVERY_IO_FAILURE",
        "PREPARED_OBJECT_MISSING",
        "COMMITTED_OBJECT_MISSING",
        "TRANSACTION_CONFLICT",
        "PLAINTEXT_LENGTH_MISMATCH",
        "CIPHERTEXT_LENGTH_MISMATCH",
        "PLAINTEXT_DIGEST_MISMATCH",
    ] {
        assert!(spec.contains(reason), "lost recovery reason {reason}");
    }
    assert!(spec.contains("MAX_RECOVERY_OPERATIONS: usize = 1_000_000"));
    assert!(spec.contains("MAX_RECOVERY_ISSUES: usize = 4_096"));

    let tests = read(&root, "src/sealed_recovery/kernel/tests.rs");
    assert!(tests.contains("fn recovery_issue_codes_match_the_stable_contract("));
    assert!(tests.contains("fn issue_report_is_bounded_and_fail_closed("));
}

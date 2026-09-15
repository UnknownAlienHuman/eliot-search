use std::path::{Path, PathBuf};

fn crate_root() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
}

fn read(root: &Path, relative: &str) -> String {
    std::fs::read_to_string(root.join(relative))
        .unwrap_or_else(|error| panic!("cannot read {relative}: {error}"))
}

#[test]
fn sealed_transaction_entry_and_kernel_are_thin() {
    let root = crate_root();
    let entry = read(&root, "src/sealed_transaction.rs");
    assert!(entry.contains("#[path = \"sealed_transaction/kernel.rs\"]"));
    assert!(entry.contains("pub use kernel::*;"));
    assert!(entry.len() < 1_500, "entry grew to {} bytes", entry.len());
    for forbidden in [
        "pub enum SealedTransactionError",
        "pub enum PutDisposition",
        "fn write_once(",
        "fn put_idempotent(",
        "std::fs",
        "#[cfg(test)]",
    ] {
        assert!(
            !entry.contains(forbidden),
            "transaction implementation returned to facade: {forbidden}"
        );
    }

    let kernel = read(&root, "src/sealed_transaction/kernel.rs");
    for module in ["api", "model", "platform", "spec"] {
        assert!(kernel.contains(&format!("mod {module};")));
    }
    assert!(kernel.contains("mod tests;"));
    assert!(kernel.len() < 2_500, "kernel grew to {} bytes", kernel.len());
}

#[test]
fn transaction_owners_remain_bounded_and_vendor_free() {
    let root = crate_root();
    let owners = [
        (
            "src/sealed_transaction/kernel/spec.rs",
            "pub enum SealedTransactionError",
            12_000,
        ),
        (
            "src/sealed_transaction/kernel/model.rs",
            "pub enum PutDisposition",
            10_000,
        ),
        (
            "src/sealed_transaction/kernel/platform/windows/codec.rs",
            "pub(super) const INTENT_MAGIC",
            12_000,
        ),
        (
            "src/sealed_transaction/kernel/platform/windows/io.rs",
            "pub(super) fn write_once(",
            18_000,
        ),
        (
            "src/sealed_transaction/kernel/platform/windows/put.rs",
            "pub(crate) fn put_idempotent(",
            18_000,
        ),
        (
            "src/sealed_transaction/kernel/platform/windows/status.rs",
            "pub(crate) fn inspect_transaction(",
            12_000,
        ),
    ];
    for (relative, marker, ceiling) in owners {
        let source = read(&root, relative);
        assert!(source.contains(marker), "{relative} lost owner {marker}");
        assert!(
            source.len() < ceiling,
            "{relative} grew to {} bytes",
            source.len()
        );
        for forbidden in [
            "qdrant_client",
            "search_qdrant",
            "reqwest::",
            "tokio::",
            "unsafe extern",
            "unsafe {",
        ] {
            assert!(
                !source.contains(forbidden),
                "{relative} acquired forbidden token {forbidden}"
            );
        }
    }

    let spec = read(&root, "src/sealed_transaction/kernel/spec.rs");
    assert!(spec.contains("MAX_OPERATION_ID_BYTES: usize = 128"));
    for code in [
        "SEALED_TRANSACTION_OPERATION_BUSY",
        "SEALED_TRANSACTION_INTENT_CONFLICT",
        "SEALED_TRANSACTION_RECEIPT_CONFLICT",
        "SEALED_TRANSACTION_OBJECT_CONFLICT",
        "SEALED_TRANSACTION_REPLAY_CONTENT_MISMATCH",
        "SEALED_TRANSACTION_READBACK_MISMATCH",
    ] {
        assert!(spec.contains(code), "spec lost {code}");
    }

    let codec = read(
        &root,
        "src/sealed_transaction/kernel/platform/windows/codec.rs",
    );
    assert!(codec.contains("ELIOT-SEALED-INTENT-V1"));
    assert!(codec.contains("ELIOT-SEALED-RECEIPT-V1"));
    assert!(codec.contains("plaintext_sha256"));
    assert!(codec.contains("ciphertext_bytes"));
    assert!(!codec.contains("std::fs"));
    assert!(!codec.contains("seal_immutable"));

    let io = read(&root, "src/sealed_transaction/kernel/platform/windows/io.rs");
    assert!(io.contains("file.try_lock()"));
    assert!(io.contains("fs::hard_link(&temporary, target)"));
    assert!(io.contains("eliot_searchd::native_file::observe(&file)"));
    assert!(io.contains("sealed-transactions"));
    assert!(!io.contains("open_sealed"));
    assert!(!io.contains("seal_immutable"));

    let put = read(&root, "src/sealed_transaction/kernel/platform/windows/put.rs");
    assert!(put.contains("sha256(plaintext.expose())?"));
    assert!(put.contains("existing.expose() != plaintext.expose()"));
    assert!(put.contains("seal_immutable(data_root, object_id, plaintext)?"));
    assert!(put.contains("fs::remove_file(&intent_path)"));
    assert!(!put.contains("read_to_string"));
    assert!(!put.contains("hard_link"));

    let status = read(
        &root,
        "src/sealed_transaction/kernel/platform/windows/status.rs",
    );
    assert!(status.contains("CommittedCleanupPending"));
    assert!(status.contains("TransactionStatus::Conflicted"));
    assert!(!status.contains("write_once"));
    assert!(!status.contains("seal_immutable"));
}

#[test]
fn exact_wire_and_regression_corpora_are_separate() {
    let root = crate_root();
    let windows_tests = read(
        &root,
        "src/sealed_transaction/kernel/platform/windows/tests.rs",
    );
    assert!(windows_tests.contains(
        "fn receipt_encoding_retains_the_exact_v2_wire_bytes("
    ));
    assert!(windows_tests.contains("ELIOT-SEALED-RECEIPT-V1\\n"));
    assert!(windows_tests.contains("plaintext_bytes=12\\n"));
    assert!(windows_tests.contains("ciphertext_bytes=256\\n"));

    let tests = read(&root, "src/sealed_transaction/kernel/tests.rs");
    for case in [
        "disposition_wire_values_match_the_stable_contract",
        "status_wire_values_match_the_stable_contract",
        "status_and_inspection_reject_a_malformed_operation_identity",
    ] {
        assert!(tests.contains(&format!("fn {case}(")), "lost test {case}");
    }
}

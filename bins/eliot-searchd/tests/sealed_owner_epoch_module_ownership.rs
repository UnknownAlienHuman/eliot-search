use std::path::{Path, PathBuf};

fn crate_root() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
}

fn read(root: &Path, relative: &str) -> String {
    std::fs::read_to_string(root.join(relative))
        .unwrap_or_else(|error| panic!("cannot read {relative}: {error}"))
}

#[test]
fn sealed_owner_epoch_entry_and_kernel_are_thin() {
    let root = crate_root();
    let entry = read(&root, "src/sealed_owner_epoch.rs");
    assert!(entry.contains("#[path = \"sealed_owner_epoch/kernel.rs\"]"));
    assert!(entry.contains("pub use kernel::*;"));
    assert!(entry.len() < 1_500, "entry grew to {} bytes", entry.len());
    for forbidden in [
        "pub enum OwnerEpochError",
        "pub struct OwnerEpochRecord",
        "pub struct OwnerEpochGuard",
        "put_idempotent_verified",
        "OpenOptions",
        "std::fs",
    ] {
        assert!(
            !entry.contains(forbidden),
            "owner-epoch implementation returned to facade: {forbidden}"
        );
    }

    let kernel = read(&root, "src/sealed_owner_epoch/kernel.rs");
    for module in ["codec", "identity", "model", "platform", "spec"] {
        assert!(kernel.contains(&format!("mod {module};")));
    }
    assert!(kernel.contains("pub fn latest_sealed_head("));
    assert!(kernel.contains("mod tests;"));
    assert!(kernel.len() < 3_500, "kernel grew to {} bytes", kernel.len());
}

#[test]
fn sealed_owner_epoch_responsibilities_remain_bounded() {
    let root = crate_root();
    let owners = [
        (
            "src/sealed_owner_epoch/kernel/spec.rs",
            "pub enum OwnerEpochError",
        ),
        (
            "src/sealed_owner_epoch/kernel/codec.rs",
            "pub struct OwnerEpochRecord",
        ),
        (
            "src/sealed_owner_epoch/kernel/identity.rs",
            "pub(super) fn parse_epoch_object_id(",
        ),
        (
            "src/sealed_owner_epoch/kernel/model.rs",
            "pub struct OwnerEpochGuard",
        ),
        (
            "src/sealed_owner_epoch/kernel/platform/windows.rs",
            "pub(super) fn acquire(",
        ),
    ];
    for (relative, marker) in owners {
        let source = read(&root, relative);
        assert!(source.contains(marker), "{relative} lost owner {marker}");
        assert!(
            source.len() < 16_000,
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

    let codec = read(&root, "src/sealed_owner_epoch/kernel/codec.rs");
    assert!(codec.contains("OWNER_EPOCH_MAGIC"));
    assert!(codec.contains("previous_record_sha256"));
    assert!(codec.contains("root_binding_sha256"));
    assert!(!codec.contains("open_sealed"));
    assert!(!codec.contains("std::fs"));

    let identity = read(&root, "src/sealed_owner_epoch/kernel/identity.rs");
    assert!(identity.contains("owner-epoch-{epoch:020}"));
    assert!(identity.contains("owner-epoch-op-{epoch:020}"));
    assert!(identity.contains("digits.len() != 20"));
    assert!(!identity.contains("open_sealed"));

    let model = read(&root, "src/sealed_owner_epoch/kernel/model.rs");
    assert!(model.contains("platform::acquire(data_root)"));
    assert!(model.contains("root_lease.is_held()"));
    assert!(model.contains("<redacted>"));
    assert!(!model.contains("put_idempotent_verified"));
    assert!(!model.contains("std::fs"));

    let windows = read(&root, "src/sealed_owner_epoch/kernel/platform/windows.rs");
    assert!(windows.contains("SealedRootLease::acquire"));
    assert!(windows.contains("require_epoch_capacity(records.len())"));
    assert!(windows.contains("put_idempotent_verified"));
    assert!(windows.contains("eliot-search/sealed-root-binding/v1\\0"));
    assert!(windows.contains("FILE_FLAG_OPEN_REPARSE_POINT"));
    assert!(windows.contains("OwnerEpochGuard::new("));
}

#[test]
fn sealed_owner_epoch_contracts_stay_closed() {
    let root = crate_root();
    let spec = read(&root, "src/sealed_owner_epoch/kernel/spec.rs");
    for token in [
        "ELIOT-SEALED-OWNER-EPOCH-V1",
        "MAX_OWNER_EPOCH_RECORDS: usize = 1_000_000",
        "OWNER_EPOCH_FORMAT_VERSION: u16 = 1",
        "OWNER_EPOCH_FIELD_COUNT: usize = 5",
        "SEALED_DIRECTORY: &str = \"sealed-revisions\"",
        "SEALED_SUFFIX: &str = \".els-dpapi\"",
    ] {
        assert!(spec.contains(token), "lost owner-epoch token {token}");
    }
    for reason in [
        "SEALED_OWNER_EPOCH_UNSUPPORTED_PLATFORM",
        "SEALED_OWNER_EPOCH_CHAIN_INVALID",
        "SEALED_OWNER_EPOCH_CHAIN_GAP",
        "SEALED_OWNER_EPOCH_ROOT_BINDING_MISMATCH",
        "SEALED_OWNER_EPOCH_PREDECESSOR_MISMATCH",
        "SEALED_OWNER_EPOCH_EXHAUSTED",
        "SEALED_OWNER_EPOCH_IO_FAILURE",
    ] {
        assert!(spec.contains(reason), "lost owner-epoch reason {reason}");
    }

    let windows = read(&root, "src/sealed_owner_epoch/kernel/platform/windows.rs");
    for invariant in [
        "if *epoch != expected_epoch",
        "record.previous_epoch != previous_epoch",
        "record.root_binding_sha256 != root_binding",
        "record.previous_record_sha256 != previous_digest",
        "canonical.as_bytes() != plaintext.expose()",
        "receipt.plaintext_bytes == 0",
    ] {
        assert!(windows.contains(invariant), "lost epoch invariant {invariant}");
    }

    let tests = read(&root, "src/sealed_owner_epoch/kernel/tests.rs");
    for case in [
        "epoch_record_encoding_has_exact_header_and_round_trips",
        "duplicate_epoch_field_and_missing_terminator_are_rejected",
        "invalid_first_predecessor_cannot_be_encoded",
        "generated_epoch_filenames_can_be_reopened",
        "invalid_epoch_filenames_remain_rejected",
        "full_history_refuses_append_before_creating_an_unreadable_record",
    ] {
        assert!(tests.contains(&format!("fn {case}(")), "lost test {case}");
    }
}

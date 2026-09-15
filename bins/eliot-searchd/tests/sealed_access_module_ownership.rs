use std::path::{Path, PathBuf};

fn crate_root() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
}

fn read(root: &Path, relative: &str) -> String {
    std::fs::read_to_string(root.join(relative))
        .unwrap_or_else(|error| panic!("cannot read {relative}: {error}"))
}

#[test]
fn sealed_access_entry_and_kernel_are_thin() {
    let root = crate_root();
    let entry = read(&root, "src/sealed_access.rs");
    assert!(entry.contains("#[path = \"sealed_access/kernel.rs\"]"));
    assert!(entry.contains("pub use kernel::*;"));
    assert!(entry.len() < 1_500, "entry grew to {} bytes", entry.len());
    for forbidden in [
        "pub enum SealedAccessError",
        "pub struct AccessFenceMutation",
        "pub fn append_fence(",
        "put_idempotent_verified",
        "std::fs",
    ] {
        assert!(
            !entry.contains(forbidden),
            "sealed access implementation returned to facade: {forbidden}"
        );
    }

    let kernel = read(&root, "src/sealed_access/kernel.rs");
    for module in ["append", "chain", "model", "platform", "read", "spec"] {
        assert!(kernel.contains(&format!("mod {module};")));
    }
    assert!(kernel.contains("pub use append::append_fence;"));
    assert!(kernel.contains("pub use read::{current_fence, require_active_fence};"));
    assert!(kernel.len() < 3_000, "kernel grew to {} bytes", kernel.len());
}

#[test]
fn sealed_access_responsibilities_remain_bounded() {
    let root = crate_root();
    let owners = [
        (
            "src/sealed_access/kernel/spec.rs",
            "pub enum SealedAccessError",
        ),
        (
            "src/sealed_access/kernel/model.rs",
            "pub struct AccessFenceMutation",
        ),
        (
            "src/sealed_access/kernel/chain.rs",
            "pub(super) fn load_chain(",
        ),
        (
            "src/sealed_access/kernel/append.rs",
            "pub fn append_fence(",
        ),
        (
            "src/sealed_access/kernel/read.rs",
            "pub fn require_active_fence(",
        ),
        (
            "src/sealed_access/kernel/platform/windows.rs",
            "pub(super) fn discover(",
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

    let append = read(&root, "src/sealed_access/kernel/append.rs");
    assert!(append.contains("verify_owner_root(data_root, owner)?"));
    assert!(append.contains("replay_existing_fence"));
    assert!(append.contains("zero_digest()?"));
    assert!(append.contains("put_idempotent_verified"));
    assert!(append.contains("open_sealed"));
    assert!(append.contains("AccessFenceRecord::decode"));
    assert!(!append.contains("read_dir"));

    let chain = read(&root, "src/sealed_access/kernel/chain.rs");
    assert!(chain.contains("platform::discover"));
    assert!(chain.contains("TransactionStatus::Committed"));
    assert!(chain.contains("DenyIsTerminal"));
    assert!(chain.contains("record_matches_mutation"));
    assert!(!chain.contains("std::fs"));

    let read_owner = read(&root, "src/sealed_access/kernel/read.rs");
    assert!(read_owner.contains("AccessFenceState::Allow"));
    assert!(read_owner.contains("owner.root_binding_sha256()"));
    assert!(!read_owner.contains("put_idempotent_verified"));
    assert!(!read_owner.contains("std::fs"));

    let windows = read(&root, "src/sealed_access/kernel/platform/windows.rs");
    assert!(windows.contains("fs::read_dir"));
    assert!(windows.contains("FILE_ATTRIBUTE_REPARSE_POINT"));
    assert!(windows.contains("generation_text.len() != 20"));
    assert!(windows.contains("MAX_ACCESS_FENCE_GENERATIONS"));
}

#[test]
fn sealed_access_contracts_stay_closed() {
    let root = crate_root();
    let spec = read(&root, "src/sealed_access/kernel/spec.rs");
    for reason in [
        "SEALED_ACCESS_UNSUPPORTED_PLATFORM",
        "SEALED_ACCESS_FENCE_NOT_FOUND",
        "SEALED_ACCESS_DENIED",
        "SEALED_ACCESS_AUTHORITY_BINDING_MISMATCH",
        "SEALED_ACCESS_MUTATION_CONFLICT",
        "SEALED_ACCESS_GENERATION_CONFLICT",
        "SEALED_ACCESS_REVISION_REGRESSION",
        "SEALED_ACCESS_DENY_IS_TERMINAL",
        "SEALED_ACCESS_CHAIN_INVALID",
        "SEALED_ACCESS_TRANSACTION_NOT_COMMITTED",
        "SEALED_ACCESS_CAPACITY_EXCEEDED",
        "SEALED_ACCESS_IO_FAILURE",
    ] {
        assert!(spec.contains(reason), "lost sealed-access reason {reason}");
    }
    assert!(spec.contains("MAX_ACCESS_FENCE_GENERATIONS: usize = 1_000_000"));
    assert!(spec.contains("SEALED_SUFFIX: &str = \".els-dpapi\""));
    assert!(spec.contains("SEALED_DIRECTORY: &str = \"sealed-revisions\""));

    let chain = read(&root, "src/sealed_access/kernel/chain.rs");
    for invariant in [
        "record.admitted_owner_epoch > owner.epoch()",
        "!mutation_ids.insert(record.mutation_id.clone())",
        "current.previous_record_sha256 != previous.record_sha256",
        "current.access_generation",
        "current.scope_revision < previous_record.scope_revision",
        "current.purge_generation < previous_record.purge_generation",
    ] {
        assert!(chain.contains(invariant), "lost chain invariant {invariant}");
    }

    let model = read(&root, "src/sealed_access/kernel/model.rs");
    assert!(model.contains("Self::Created => \"CREATED\""));
    assert!(model.contains("Self::Replay => \"REPLAY\""));
    assert!(model.contains("scope_revision == 0"));
    assert!(model.contains("policy_revision == 0"));
}

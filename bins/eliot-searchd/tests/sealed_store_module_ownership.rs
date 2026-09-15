use std::path::{Path, PathBuf};

fn crate_root() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
}

fn read(root: &Path, relative: &str) -> String {
    std::fs::read_to_string(root.join(relative))
        .unwrap_or_else(|error| panic!("cannot read {relative}: {error}"))
}

#[test]
fn sealed_store_entry_and_kernel_are_thin() {
    let root = crate_root();
    let entry = read(&root, "src/sealed_store.rs");
    assert!(entry.contains("#[path = \"sealed_store/kernel.rs\"]"));
    assert!(entry.contains("pub use kernel::*;"));
    assert!(entry.len() < 1_500, "entry grew to {} bytes", entry.len());
    for forbidden in [
        "pub enum SealedStoreError",
        "pub struct SensitiveBytes",
        "struct Envelope",
        "CryptProtectData",
        "std::fs",
        "unsafe extern",
        "#[cfg(test)]",
    ] {
        assert!(
            !entry.contains(forbidden),
            "sealed implementation returned to facade: {forbidden}"
        );
    }

    let kernel = read(&root, "src/sealed_store/kernel.rs");
    for module in ["api", "envelope", "model", "platform", "spec"] {
        assert!(kernel.contains(&format!("mod {module};")));
    }
    assert!(kernel.contains("mod tests;"));
    assert!(kernel.len() < 2_500, "kernel grew to {} bytes", kernel.len());
}

#[test]
fn sealed_store_format_and_responsibilities_remain_closed() {
    let root = crate_root();
    let owners = [
        (
            "src/sealed_store/kernel/spec.rs",
            "pub enum SealedStoreError",
            12_000,
        ),
        (
            "src/sealed_store/kernel/model.rs",
            "pub struct SensitiveBytes",
            10_000,
        ),
        (
            "src/sealed_store/kernel/envelope.rs",
            "pub(crate) struct Envelope",
            10_000,
        ),
        (
            "src/sealed_store/kernel/api.rs",
            "pub fn seal_immutable(",
            5_000,
        ),
        (
            "src/sealed_store/kernel/platform/windows.rs",
            "fn CryptProtectData(",
            30_000,
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
        for forbidden in ["qdrant_client", "search_qdrant", "reqwest::", "tokio::"] {
            assert!(
                !source.contains(forbidden),
                "{relative} acquired forbidden vendor token {forbidden}"
            );
        }
    }

    let spec = read(&root, "src/sealed_store/kernel/spec.rs");
    for exact in [
        "b\"ELSDPAPI\"",
        "FORMAT_VERSION: u16 = 1",
        "HEADER_BYTES: usize = 8 + 2 + 2 + 8 + 8",
        "MAX_PLAINTEXT_BYTES: usize = 64 * 1024 * 1024",
        "MAX_OBJECT_ID_BYTES: usize = 128",
        "eliot-search/sealed-object/current-user/v1\\0",
        "SEALED_STORE_OBJECT_BINDING_MISMATCH",
        "SEALED_STORE_OBJECT_CHANGED_DURING_READ",
    ] {
        assert!(spec.contains(exact), "spec lost {exact}");
    }

    let envelope = read(&root, "src/sealed_store/kernel/envelope.rs");
    assert!(envelope.contains("output.extend_from_slice(&MAGIC)"));
    assert!(envelope.contains("output.extend_from_slice(&FORMAT_VERSION.to_be_bytes())"));
    assert!(envelope.contains("expected != bytes.len()"));
    for forbidden in ["std::fs", "CryptProtectData", "LocalFree", "unsafe"] {
        assert!(!envelope.contains(forbidden), "envelope acquired {forbidden}");
    }

    let model = read(&root, "src/sealed_store/kernel/model.rs");
    assert!(model.contains("self.0.zeroize()"));
    assert!(model.contains("physical_erasure_guaranteed"));
    for forbidden in ["std::fs", "CryptProtectData", "LocalFree", "unsafe"] {
        assert!(!model.contains(forbidden), "model acquired {forbidden}");
    }

    let windows = read(&root, "src/sealed_store/kernel/platform/windows.rs");
    for exact in [
        "fn CryptProtectData(",
        "fn CryptUnprotectData(",
        "fn LocalFree(",
        "fs::hard_link(&temporary, target)",
        "eliot_searchd::native_file::observe(&file)",
        "sealed-revisions",
        ".els-dpapi",
        "wipe(bytes)",
    ] {
        assert!(windows.contains(exact), "Windows owner lost {exact}");
    }

    let dispatcher = read(&root, "src/sealed_store/kernel/platform.rs");
    assert!(dispatcher.contains("#[allow(unsafe_code)]"));
    assert!(dispatcher.contains("mod windows;"));
    assert!(dispatcher.contains("mod unsupported;"));
}

#[test]
fn unsafe_and_regressions_stay_isolated() {
    let root = crate_root();
    for relative in [
        "src/sealed_store.rs",
        "src/sealed_store/kernel.rs",
        "src/sealed_store/kernel/api.rs",
        "src/sealed_store/kernel/envelope.rs",
        "src/sealed_store/kernel/model.rs",
        "src/sealed_store/kernel/spec.rs",
        "src/sealed_store/kernel/platform/unsupported.rs",
        "src/sealed_store/kernel/tests.rs",
    ] {
        let source = read(&root, relative);
        assert!(!source.contains("unsafe extern"), "{relative} acquired FFI");
        assert!(!source.contains("unsafe {"), "{relative} acquired unsafe block");
    }

    let tests = read(&root, "src/sealed_store/kernel/tests.rs");
    for case in [
        "wipe_clears_every_byte_without_local_unsafe",
        "sensitive_owner_preserves_explicit_access_but_redacts_debug",
        "empty_plaintext_is_rejected",
        "delete_sealed_reports_logical_closure_without_claiming_physical_erasure",
    ] {
        assert!(tests.contains(&format!("fn {case}(")), "lost test {case}");
    }
}

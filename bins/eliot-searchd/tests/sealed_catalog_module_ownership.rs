use std::path::{Path, PathBuf};

fn crate_root() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
}

fn read(root: &Path, relative: &str) -> String {
    std::fs::read_to_string(root.join(relative))
        .unwrap_or_else(|error| panic!("cannot read {relative}: {error}"))
}

#[test]
fn sealed_catalog_entry_and_kernel_are_thin() {
    let root = crate_root();
    let entry = read(&root, "src/sealed_catalog.rs");
    assert!(entry.contains("#[path = \"sealed_catalog/kernel.rs\"]"));
    assert!(entry.contains("pub use kernel::*;"));
    assert!(entry.len() < 1_500, "entry grew to {} bytes", entry.len());
    for forbidden in [
        "pub enum SealedCatalogError",
        "pub struct SealedCatalogBinding",
        "pub fn bind_revision(",
        "put_idempotent_verified",
        "open_sealed",
        "BTreeMap",
    ] {
        assert!(
            !entry.contains(forbidden),
            "sealed catalog implementation returned to facade: {forbidden}"
        );
    }

    let kernel = read(&root, "src/sealed_catalog/kernel.rs");
    for module in ["bind", "codec", "model", "read", "spec"] {
        assert!(kernel.contains(&format!("mod {module};")));
    }
    assert!(kernel.contains("pub use bind::bind_revision;"));
    assert!(kernel.contains("pub use read::{read_revision, verify_revision};"));
    assert!(kernel.contains("mod tests;"));
    assert!(kernel.len() < 2_500, "kernel grew to {} bytes", kernel.len());
}

#[test]
fn sealed_catalog_responsibilities_remain_bounded() {
    let root = crate_root();
    let owners = [
        (
            "src/sealed_catalog/kernel/spec.rs",
            "pub enum SealedCatalogError",
        ),
        (
            "src/sealed_catalog/kernel/model.rs",
            "pub struct SealedCatalogBinding",
        ),
        (
            "src/sealed_catalog/kernel/codec.rs",
            "impl SealedCatalogBinding",
        ),
        (
            "src/sealed_catalog/kernel/bind.rs",
            "pub fn bind_revision(",
        ),
        (
            "src/sealed_catalog/kernel/read.rs",
            "pub fn read_revision(",
        ),
    ];
    for (relative, marker) in owners {
        let source = read(&root, relative);
        assert!(source.contains(marker), "{relative} lost owner {marker}");
        assert!(
            source.len() < 12_000,
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

    let codec = read(&root, "src/sealed_catalog/kernel/codec.rs");
    assert!(codec.contains("CATALOG_FIELD_COUNT"));
    assert!(codec.contains("value.ends_with('\\n')"));
    assert!(codec.contains("fields.len() != CATALOG_FIELD_COUNT"));
    assert!(!codec.contains("open_sealed"));
    assert!(!codec.contains("put_idempotent_verified"));

    let binding = read(&root, "src/sealed_catalog/kernel/bind.rs");
    assert!(binding.contains("put_idempotent_verified"));
    assert!(binding.contains("verify_sealed"));
    assert!(binding.contains("readback.expose() != encoded.as_bytes()"));
    assert!(binding.contains("SealedCatalogBinding::decode"));
    assert!(!binding.contains("BTreeMap"));

    let read_owner = read(&root, "src/sealed_catalog/kernel/read.rs");
    assert!(read_owner.contains("verify_sealed"));
    assert!(read_owner.contains("sha256(content.expose())?"));
    assert!(read_owner.contains("SealedCatalogError::SourceBindingMismatch"));
    assert!(!read_owner.contains("put_idempotent_verified"));
}

#[test]
fn sealed_catalog_contracts_and_tests_stay_closed() {
    let root = crate_root();
    let spec = read(&root, "src/sealed_catalog/kernel/spec.rs");
    for reason in [
        "SEALED_CATALOG_IDENTIFIER_INVALID",
        "SEALED_CATALOG_MANIFEST_INVALID",
        "SEALED_CATALOG_SOURCE_BINDING_MISMATCH",
        "SEALED_CATALOG_CONTENT_DIGEST_MISMATCH",
        "SEALED_CATALOG_CONTENT_LENGTH_MISMATCH",
        "SEALED_CATALOG_READBACK_MISMATCH",
    ] {
        assert!(spec.contains(reason), "lost catalog reason {reason}");
    }
    assert!(spec.contains("MAX_CATALOG_IDENTIFIER_BYTES: usize = 128"));
    assert!(spec.contains("CATALOG_MAGIC: &str = \"ELIOT-SEALED-CATALOG-V1\""));
    assert!(spec.contains("CATALOG_FORMAT_VERSION: u16 = 1"));
    assert!(spec.contains("CATALOG_FIELD_COUNT: usize = 8"));

    let tests = read(&root, "src/sealed_catalog/kernel/tests.rs");
    for case in [
        "binding_encode_decode_round_trip_preserves_every_immutable_field",
        "bind_revision_rejects_a_malformed_identifier_before_touching_storage",
        "catalog_receipt_carries_the_exact_terminal_binding",
    ] {
        assert!(tests.contains(&format!("fn {case}(")), "lost test {case}");
    }
}

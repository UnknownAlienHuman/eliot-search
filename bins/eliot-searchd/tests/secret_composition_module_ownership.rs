use std::path::{Path, PathBuf};

fn crate_root() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
}

fn read(root: &Path, relative: &str) -> String {
    std::fs::read_to_string(root.join(relative))
        .unwrap_or_else(|error| panic!("cannot read {relative}: {error}"))
}

#[test]
fn secret_composition_entry_and_kernel_are_thin() {
    let root = crate_root();
    let entry = read(&root, "src/secret_composition.rs");
    assert!(entry.contains("#[path = \"secret_composition/kernel.rs\"]"));
    assert!(entry.contains("pub use kernel::*;"));
    assert!(entry.len() < 1_500, "entry grew to {} bytes", entry.len());
    for forbidden in [
        "struct PairingSecretComposer",
        "trait PairingVault",
        "struct MemoryPairingVault",
        "fn derive_binding_digest",
        "fn recover_delete",
        "#[cfg(test)]",
    ] {
        assert!(
            !entry.contains(forbidden),
            "secret implementation returned to facade: {forbidden}"
        );
    }

    let kernel = read(&root, "src/secret_composition/kernel.rs");
    for module in [
        "binding",
        "composer",
        "receipts",
        "revocation",
        "rotation",
        "spec",
        "vault",
    ] {
        assert!(kernel.contains(&format!("mod {module};")));
    }
    assert!(kernel.contains("mod tests;"));
    assert!(kernel.len() < 2_500, "kernel grew to {} bytes", kernel.len());
}

#[test]
fn secret_responsibilities_stay_separated_and_bounded() {
    let root = crate_root();
    let owners = [
        ("src/secret_composition/kernel/spec.rs", "pub enum SecretCompositionError"),
        ("src/secret_composition/kernel/binding.rs", "pub fn derive_binding_digest("),
        ("src/secret_composition/kernel/vault.rs", "pub trait PairingVault"),
        ("src/secret_composition/kernel/receipts.rs", "pub enum MutationOutcome"),
        ("src/secret_composition/kernel/composer.rs", "pub struct PairingSecretComposer"),
        ("src/secret_composition/kernel/rotation.rs", "pub fn rotate("),
        ("src/secret_composition/kernel/revocation.rs", "pub fn revoke("),
    ];
    for (relative, marker) in owners {
        let source = read(&root, relative);
        assert!(source.contains(marker), "{relative} lost owner {marker}");
        assert!(
            source.len() < 18_000,
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

    let spec = read(&root, "src/secret_composition/kernel/spec.rs");
    assert!(spec.contains("eliot-search/loopback-binding/v1"));
    assert!(spec.contains("eliot-search/loopback-operation/v1"));
    assert!(spec.contains("PAIRING_SECRET_KEY_INVALID"));

    let vault = read(&root, "src/secret_composition/kernel/vault.rs");
    assert!(vault.contains("zeroize::Zeroizing"));
    assert!(vault.contains("previous.fill(0)"));
    assert!(vault.contains("blob.fill(0)"));
    assert!(vault.contains("fn is_os_backed(&self) -> bool"));
    assert!(!vault.contains("SecretCatalog"));
    assert!(!vault.contains("SecretLease"));

    let binding = read(&root, "src/secret_composition/kernel/binding.rs");
    assert!(binding.contains("BINDING_DOMAIN"));
    assert!(binding.contains("OPERATION_DIGEST_DOMAIN"));
    assert!(binding.contains("blake3::keyed_hash"));
    assert!(!binding.contains("SecretCatalog"));
    assert!(!binding.contains("MemoryPairingVault"));

    let composer = read(&root, "src/secret_composition/kernel/composer.rs");
    assert!(composer.contains("with_secret"));
    assert!(composer.contains("SecretLease::issue"));
    assert!(composer.contains("load_blob"));
    assert!(!composer.contains("MAX_RECOVERY_ATTEMPTS"));
    assert!(!composer.contains("resolve_ambiguous_store"));

    let rotation = read(&root, "src/secret_composition/kernel/rotation.rs");
    assert!(rotation.contains("resolve_ambiguous_store"));
    assert!(rotation.contains("recover_rotation"));
    assert!(rotation.contains("MutationOutcome::Recovered"));
    assert!(!rotation.contains("remove_blob"));

    let revocation = read(&root, "src/secret_composition/kernel/revocation.rs");
    assert!(revocation.contains("recover_delete"));
    assert!(revocation.contains("MAX_RECOVERY_ATTEMPTS"));
    assert!(revocation.contains("reference_absent: true"));
    assert!(!revocation.contains("generate_key"));
}

#[test]
fn secret_regression_corpus_is_separate() {
    let root = crate_root();
    let tests = read(&root, "src/secret_composition/kernel/tests.rs");
    assert!(tests.len() < 32_000, "secret tests grew to {} bytes", tests.len());
    for case in [
        "memory_vault_is_explicitly_not_an_os_store",
        "provision_issues_a_bound_lease_and_a_stable_binding_digest",
        "cross_binding_lease_is_denied",
        "expired_lease_never_exposes_the_key",
        "rotation_advances_exactly_once_and_replaces_the_key",
        "ambiguous_rotation_write_recovers_by_exact_readback",
        "ambiguous_delete_recovers_and_reports_recovered_not_committed",
        "vault_loss_is_detected_not_relabelled_deleted",
        "operation_reuse_with_other_bytes_is_rejected",
        "keyed_proofs_bind_version_session_nonce_and_challenge",
        "same_key_composes_with_request_envelopes_and_tampering_fails",
        "lease_and_composer_debug_never_dump_key_material",
    ] {
        assert!(tests.contains(&format!("fn {case}(")), "lost test {case}");
    }
}

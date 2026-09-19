use std::path::{Path, PathBuf};

fn crate_root() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
}

fn workspace_root() -> PathBuf {
    crate_root().parent().and_then(Path::parent).unwrap().to_owned()
}

fn read(root: &Path, path: &str) -> String {
    std::fs::read_to_string(root.join(path))
        .unwrap_or_else(|error| panic!("cannot read {path}: {error}"))
}

fn has_all(source: &str, owner: &str, markers: &[&str]) {
    for marker in markers {
        assert!(source.contains(marker), "{owner} lost {marker}");
    }
}

fn has_none(source: &str, owner: &str, markers: &[&str]) {
    for marker in markers {
        assert!(!source.contains(marker), "{owner} acquired {marker}");
    }
}

#[test]
fn native_secret_effects_have_one_platform_package_owner() {
    let daemon = crate_root();
    let workspace = workspace_root();
    let facade = read(&daemon, "src/revision_protection_windows.rs");
    has_all(
        &facade,
        "daemon facade",
        &["mod credential;", "mod dpapi;", "mod existing;", "mod inventory;", "mod test_cleanup;"],
    );
    has_none(
        &facade,
        "daemon facade",
        &["mod ffi;", "unsafe extern", "CredReadW", "CredWriteW", "CredDeleteW", "CryptProtectData", "BCryptGenRandom"],
    );
    assert!(facade.len() < 2_500);

    let credential = read(&workspace, "crates/search-runtime/search-os-secrets-windows/src/credential.rs");
    let native = read(&workspace, "crates/search-runtime/search-os-secrets-windows/src/credential/windows.rs");
    let dpapi = read(&workspace, "crates/search-runtime/search-os-secrets-windows/src/dpapi.rs");
    let cleanup = read(&workspace, "crates/search-runtime/search-os-secrets-windows/src/test_credential_cleanup.rs");
    has_all(
        &credential,
        "credential package",
        &["load_existing_legacy_revision_root_secret", "load_or_create_legacy_revision_root_secret", "LegacyRevisionRootSecretRequirement", "LegacyRevisionRootSecretError", "Re-read while"],
    );
    has_all(
        &native,
        "native credential package",
        &["CredReadW", "CredWriteW", "CredFree", "BCryptGenRandom", "CreateMutexW", "WaitForSingleObject", "ELIOT-Search-RevisionVault-v1", "impl Drop for CredentialAllocation", "impl Drop for WindowsVaultLock"],
    );
    has_all(
        &dpapi,
        "DPAPI package",
        &["CryptProtectData", "CryptUnprotectData", "LocalFree", "CRYPTPROTECT_UI_FORBIDDEN"],
    );
    has_all(
        &cleanup,
        "cleanup package",
        &["LegacyRevisionRootSecretCleanupError", "delete_legacy_revision_root_secret_for_test", "CredDeleteW", "CreateMutexW", "WaitForSingleObject", "ELIOT-Search-RevisionVault-v1", "ELIOT Search/revision-key/", "load_existing_legacy_revision_root_secret"],
    );

    let package_manifest = read(&workspace, "crates/search-runtime/search-os-secrets-windows/Cargo.toml");
    let daemon_manifest = read(&workspace, "bins/eliot-searchd/Cargo.toml");
    assert!(package_manifest.contains("test-credential-cleanup = []"));
    assert!(daemon_manifest.contains("features = [\"test-credential-cleanup\"]"));
}

#[test]
fn daemon_keeps_only_composition_and_inventory() {
    let root = crate_root();
    let credential = read(&root, "src/revision_protection_windows/credential.rs");
    has_all(
        &credential,
        "daemon credential composition",
        &["contains_protected_objects(revision_root)?", "load_existing_legacy_revision_root_secret", "load_or_create_legacy_revision_root_secret"],
    );
    has_none(
        &credential,
        "daemon credential composition",
        &["unsafe extern", "CredReadW", "CredWriteW", "CredDeleteW", "CredentialW", "BCryptGenRandom", "CreateMutexW", "WaitForSingleObject", "ELIOT-Search-RevisionVault-v1"],
    );

    let daemon_dpapi = read(&root, "src/revision_protection_windows/dpapi.rs");
    has_none(
        &daemon_dpapi,
        "daemon DPAPI composition",
        &["unsafe extern", "CryptProtectData", "CryptUnprotectData", "LocalFree", "DataBlob"],
    );

    let inventory = read(&root, "src/revision_protection_windows/inventory.rs");
    has_all(
        &inventory,
        "protected-object inventory",
        &["MAX_OBJECT_SCAN: usize = 2_000_000", "DIRECT_REVISION_DIRECTORY_LINK_DENIED", "DIRECT_REVISION_OBJECT_LINK_DENIED"],
    );
    assert!(!inventory.contains("CredReadW"));

    let cleanup = read(&root, "src/revision_protection_windows/test_cleanup.rs");
    has_all(&cleanup, "daemon cleanup composition", &["delete_legacy_revision_root_secret_for_test", "namespace.id"]);
    has_none(
        &cleanup,
        "daemon cleanup composition",
        &["unsafe extern", "CredDeleteW", "CreateMutexW", "WaitForSingleObject", "acquire_vault_lock"],
    );
    assert!(!root.join("src/revision_protection_windows/ffi.rs").exists());
}

#[test]
fn package_bounds_and_direct_reasons_remain_closed() {
    let root = crate_root();
    for path in [
        "src/revision_protection_windows/credential.rs",
        "src/revision_protection_windows/dpapi.rs",
        "src/revision_protection_windows/inventory.rs",
        "src/revision_protection_windows/existing.rs",
        "src/revision_protection_windows/test_cleanup.rs",
    ] {
        let source = read(&root, path);
        assert!(source.len() < 16_000, "{path} grew to {} bytes", source.len());
        has_none(&source, path, &["qdrant_client", "search_qdrant", "reqwest::", "tokio::", "std::process::Command"]);
    }

    let credential = read(&root, "src/revision_protection_windows/credential.rs");
    has_all(
        &credential,
        "daemon credential reasons",
        &["DIRECT_REVISION_KEY_MISSING", "DIRECT_REVISION_RNG_FAILED", "DIRECT_REVISION_KEY_READ_FAILED", "DIRECT_REVISION_KEY_READBACK_INVALID", "DIRECT_REVISION_KEY_TOO_LARGE", "DIRECT_REVISION_KEY_WRITE_FAILED", "DIRECT_REVISION_KEY_READBACK_MISMATCH", "DIRECT_REVISION_KEY_WRITE_OUTCOME_UNKNOWN"],
    );
    let dpapi = read(&root, "src/revision_protection_windows/dpapi.rs");
    has_all(
        &dpapi,
        "daemon DPAPI reasons",
        &["DIRECT_DPAPI_INPUT_TOO_LARGE", "DIRECT_DPAPI_OUTPUT_TOO_LARGE", "DIRECT_DPAPI_OUTPUT_INVALID", "DIRECT_DPAPI_PROTECT_FAILED", "DIRECT_DPAPI_UNPROTECT_FAILED"],
    );
}

use std::path::{Path, PathBuf};

fn crate_root() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
}

fn workspace_root() -> PathBuf {
    crate_root()
        .parent()
        .and_then(Path::parent)
        .expect("daemon is a workspace member")
        .to_owned()
}

fn read(root: &Path, relative: &str) -> String {
    std::fs::read_to_string(root.join(relative))
        .unwrap_or_else(|error| panic!("cannot read {relative}: {error}"))
}

#[test]
fn revision_protection_windows_entry_is_thin() {
    let root = crate_root();
    let entry = read(&root, "src/revision_protection_windows.rs");
    for module in ["credential", "dpapi", "existing", "inventory"] {
        assert!(entry.contains(&format!("mod {module};")));
    }
    assert!(entry.contains("#[cfg(test)]\nmod ffi;"));
    assert!(entry.contains("#[cfg(test)]\nmod test_cleanup;"));
    assert!(entry.contains("pub(super) use credential::load_or_create_root_secret;"));
    assert!(entry.contains("pub(super) use dpapi::{protect_data, unprotect_data};"));
    assert!(entry.len() < 2_500, "entry grew to {} bytes", entry.len());
    for forbidden in [
        "unsafe extern",
        "CredentialW",
        "CredReadW",
        "CredWriteW",
        "CryptProtectData",
        "CryptUnprotectData",
        "BCryptGenRandom",
        "contains_protected_objects",
        "impl super::RevisionProtector",
    ] {
        assert!(
            !entry.contains(forbidden),
            "Windows implementation returned to facade: {forbidden}"
        );
    }
}

#[test]
fn production_native_secret_effects_have_one_platform_package_owner() {
    let daemon_root = crate_root();
    let workspace = workspace_root();
    let package_credential = read(
        &workspace,
        "crates/search-runtime/search-os-secrets-windows/src/credential.rs",
    );
    let package_windows = read(
        &workspace,
        "crates/search-runtime/search-os-secrets-windows/src/credential/windows.rs",
    );
    let package_dpapi = read(
        &workspace,
        "crates/search-runtime/search-os-secrets-windows/src/dpapi.rs",
    );

    for marker in [
        "pub fn load_existing_legacy_revision_root_secret(",
        "pub fn load_or_create_legacy_revision_root_secret(",
        "pub struct LegacyRevisionRootSecret",
        "pub enum LegacyRevisionRootSecretRequirement",
        "pub enum LegacyRevisionRootSecretError",
        "Re-read while",
    ] {
        assert!(
            package_credential.contains(marker),
            "platform package lost credential owner {marker}"
        );
    }
    for marker in [
        "CredReadW",
        "CredWriteW",
        "CredFree",
        "BCryptGenRandom",
        "CreateMutexW",
        "WaitForSingleObject",
        "ELIOT-Search-RevisionVault-v1",
        "impl Drop for CredentialAllocation",
        "impl Drop for WindowsVaultLock",
    ] {
        assert!(
            package_windows.contains(marker),
            "platform package lost native credential marker {marker}"
        );
    }
    for marker in [
        "CryptProtectData",
        "CryptUnprotectData",
        "LocalFree",
        "CRYPTPROTECT_UI_FORBIDDEN",
    ] {
        assert!(
            package_dpapi.contains(marker),
            "platform package lost DPAPI marker {marker}"
        );
    }

    let daemon_credential = read(
        &daemon_root,
        "src/revision_protection_windows/credential.rs",
    );
    assert!(daemon_credential.contains("contains_protected_objects(revision_root)?"));
    assert!(daemon_credential.contains("load_existing_legacy_revision_root_secret"));
    assert!(daemon_credential.contains("load_or_create_legacy_revision_root_secret"));
    for forbidden in [
        "unsafe extern",
        "CredReadW",
        "CredWriteW",
        "CredentialW",
        "BCryptGenRandom",
        "CreateMutexW",
        "WaitForSingleObject",
        "ELIOT-Search-RevisionVault-v1",
    ] {
        assert!(
            !daemon_credential.contains(forbidden),
            "daemon restored native credential owner {forbidden}"
        );
    }

    let daemon_dpapi = read(
        &daemon_root,
        "src/revision_protection_windows/dpapi.rs",
    );
    for forbidden in [
        "unsafe extern",
        "CryptProtectData",
        "CryptUnprotectData",
        "LocalFree",
        "DataBlob",
    ] {
        assert!(
            !daemon_dpapi.contains(forbidden),
            "daemon restored native DPAPI owner {forbidden}"
        );
    }
}

#[test]
fn remaining_daemon_responsibilities_are_bounded() {
    let root = crate_root();
    let owners = [
        (
            "src/revision_protection_windows/credential.rs",
            "pub(super) fn load_or_create_root_secret(",
        ),
        (
            "src/revision_protection_windows/dpapi.rs",
            "pub(super) fn protect_data(",
        ),
        (
            "src/revision_protection_windows/inventory.rs",
            "pub(super) fn contains_protected_objects(",
        ),
        (
            "src/revision_protection_windows/existing.rs",
            "pub(crate) fn open_existing(",
        ),
        (
            "src/revision_protection_windows/test_cleanup.rs",
            "pub(super) fn delete_test_credential_for_data_root(",
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

    let inventory = read(&root, "src/revision_protection_windows/inventory.rs");
    assert!(inventory.contains("MAX_OBJECT_SCAN: usize = 2_000_000"));
    assert!(inventory.contains("DIRECT_REVISION_DIRECTORY_LINK_DENIED"));
    assert!(inventory.contains("DIRECT_REVISION_OBJECT_LINK_DENIED"));
    assert!(!inventory.contains("CredReadW"));

    let test_ffi = read(&root, "src/revision_protection_windows/ffi.rs");
    assert!(test_ffi.contains("CredDeleteW"));
    assert!(test_ffi.contains("CreateMutexW"));
    for production_api in ["CredReadW", "CredWriteW", "BCryptGenRandom", "CryptProtectData"] {
        assert!(
            !test_ffi.contains(production_api),
            "test cleanup retained production owner {production_api}"
        );
    }
}

#[test]
fn direct_compatibility_reasons_stay_at_daemon_boundary() {
    let root = crate_root();
    let credential = read(&root, "src/revision_protection_windows/credential.rs");
    for reason in [
        "DIRECT_REVISION_KEY_MISSING",
        "DIRECT_REVISION_RNG_FAILED",
        "DIRECT_REVISION_KEY_READ_FAILED",
        "DIRECT_REVISION_KEY_READBACK_INVALID",
        "DIRECT_REVISION_KEY_TOO_LARGE",
        "DIRECT_REVISION_KEY_WRITE_FAILED",
        "DIRECT_REVISION_KEY_READBACK_MISMATCH",
        "DIRECT_REVISION_KEY_WRITE_OUTCOME_UNKNOWN",
    ] {
        assert!(credential.contains(reason), "lost credential reason {reason}");
    }

    let dpapi = read(&root, "src/revision_protection_windows/dpapi.rs");
    for reason in [
        "DIRECT_DPAPI_INPUT_TOO_LARGE",
        "DIRECT_DPAPI_OUTPUT_TOO_LARGE",
        "DIRECT_DPAPI_OUTPUT_INVALID",
        "DIRECT_DPAPI_PROTECT_FAILED",
        "DIRECT_DPAPI_UNPROTECT_FAILED",
    ] {
        assert!(dpapi.contains(reason), "lost DPAPI reason {reason}");
    }
}

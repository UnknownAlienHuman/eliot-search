use std::path::{Path, PathBuf};

fn crate_root() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
}

fn read(root: &Path, relative: &str) -> String {
    std::fs::read_to_string(root.join(relative))
        .unwrap_or_else(|error| panic!("cannot read {relative}: {error}"))
}

#[test]
fn revision_protection_windows_entry_is_thin() {
    let root = crate_root();
    let entry = read(&root, "src/revision_protection_windows.rs");
    for module in [
        "credential", "dpapi", "existing", "ffi", "inventory",
    ] {
        assert!(entry.contains(&format!("mod {module};")));
    }
    assert!(entry.contains("mod test_cleanup;"));
    assert!(entry.contains("pub(super) use credential::load_or_create_root_secret;"));
    assert!(entry.contains("pub(super) use dpapi::{protect_data, unprotect_data};"));
    assert!(entry.len() < 2_500, "entry grew to {} bytes", entry.len());
    for forbidden in [
        "unsafe extern",
        "CredentialW",
        "crypt_protect_data",
        "pub(super) fn read_credential",
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
fn windows_revision_protection_responsibilities_are_bounded() {
    let root = crate_root();
    let owners = [
        (
            "src/revision_protection_windows/ffi.rs",
            "unsafe extern \"system\"",
        ),
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

    let ffi = read(&root, "src/revision_protection_windows/ffi.rs");
    for api in [
        "CredReadW",
        "CredWriteW",
        "CryptProtectData",
        "CryptUnprotectData",
        "BCryptGenRandom",
        "CreateMutexW",
        "WaitForSingleObject",
        "LocalFree",
    ] {
        assert!(ffi.contains(api), "lost native API {api}");
    }
    assert!(ffi.contains("impl Drop for CredentialAllocation"));
    assert!(ffi.contains("impl Drop for LocalAllocation"));
    assert!(ffi.contains("impl Drop for VaultLock"));

    let credential = read(&root, "src/revision_protection_windows/credential.rs");
    assert!(credential.contains("contains_protected_objects(revision_root)?"));
    assert!(credential.contains("acquire_vault_lock()"));
    assert!(credential.contains("DIRECT_REVISION_KEY_READBACK_MISMATCH"));
    assert!(credential.contains("DIRECT_REVISION_KEY_WRITE_OUTCOME_UNKNOWN"));
    assert!(credential.contains("constant_time_equal"));
    assert!(!credential.contains("CryptProtectData"));

    let dpapi = read(&root, "src/revision_protection_windows/dpapi.rs");
    assert!(dpapi.contains("CRYPTPROTECT_UI_FORBIDDEN"));
    assert!(dpapi.contains("LocalAllocation(output).into_vec"));
    assert!(dpapi.contains("super::super::zeroize(&mut entropy_copy)"));
    assert!(!dpapi.contains("CredReadW"));

    let inventory = read(&root, "src/revision_protection_windows/inventory.rs");
    assert!(inventory.contains("MAX_OBJECT_SCAN: usize = 2_000_000"));
    assert!(inventory.contains("DIRECT_REVISION_DIRECTORY_LINK_DENIED"));
    assert!(inventory.contains("DIRECT_REVISION_OBJECT_LINK_DENIED"));
    assert!(!inventory.contains("cred_read_w"));
}

#[test]
fn windows_revision_protection_contracts_stay_closed() {
    let root = crate_root();
    let ffi = read(&root, "src/revision_protection_windows/ffi.rs");
    for token in [
        "CRED_PERSIST_LOCAL_MACHINE: u32 = 2",
        "ROOT_SECRET_BYTES: usize = 32",
        "MAX_CREDENTIAL_BLOB_BYTES: usize = 5 * 512",
        "ELIOT-Search-RevisionVault-v1",
        "VAULT_LOCK_WAIT_MILLIS: u32 = 5_000",
        "WAIT_ABANDONED: u32 = 0x0000_0080",
    ] {
        assert!(ffi.contains(token), "lost Windows protection token {token}");
    }

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
        "DIRECT_DPAPI_PROTECT_FAILED",
        "DIRECT_DPAPI_UNPROTECT_FAILED",
    ] {
        assert!(dpapi.contains(reason), "lost DPAPI reason {reason}");
    }

    let existing = read(&root, "src/revision_protection_windows/existing.rs");
    assert!(existing.contains("eliot-search/revision-key-binding/v1"));
    assert!(existing.contains("eliot-search/revision-dpapi-entropy/v1"));
    assert!(!existing.contains("write_credential"));
    assert!(!existing.contains("contains_protected_objects"));
}

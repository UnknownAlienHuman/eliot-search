use super::*;

use std::fs;
use std::path::{Path, PathBuf};

use search_contracts::NonZeroRevision;
use search_safe_reader::{FinalHandleOpenRequest, SafeReadBackend};

fn barrier() -> NonZeroRevision {
    NonZeroRevision::new(1).expect("barrier")
}

fn fixture_root(name: &str) -> PathBuf {
    let stamp = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap()
        .as_nanos();
    let root = std::env::temp_dir().join(format!(
        "eliot-safe-adapter-{name}-{}-{stamp}",
        std::process::id()
    ));
    std::fs::create_dir_all(&root).unwrap();
    root
}

#[test]
fn token_validation_denies_escape_and_streams() {
    assert!(validate_relative_token("a/b.txt", 32_768).is_ok());
    for bad in [
        "",
        "/absolute",
        "\\absolute",
        "C:/drive",
        "c:/drive",
        "//host/share",
        "../escape",
        "a/../b",
        "a/./b",
        "a//b",
        "a\\b",
        "a:b",
        "CON",
        "con.txt",
        "NUL",
        "com1",
        "LPT9.log",
        "a/\0b",
    ] {
        assert_eq!(
            validate_relative_token(bad, 32_768),
            Err(AdapterError::PathDenied),
            "token={bad:?}"
        );
    }
    assert_eq!(
        validate_relative_token(&"a".repeat(32_769), 32_768),
        Err(AdapterError::PathDenied)
    );
}

#[test]
fn adapter_errors_and_handles_are_redacted() {
    let error = AdapterError::EscapeDenied;
    let debug = format!("{error:?}");
    assert!(debug.contains("EscapeDenied"));
    assert!(!debug.contains('/'));
    assert_eq!(error.to_string(), "SAFE_ADAPTER_ESCAPE_DENIED");
    assert!(qualified_profile().contains("final-handle"));
}

#[test]
fn symlink_final_object_is_denied_before_open() {
    let root = fixture_root("link");
    let target = root.join("target.txt");
    std::fs::write(&target, b"secret").unwrap();
    #[cfg(unix)]
    std::os::unix::fs::symlink(&target, root.join("link.txt")).unwrap();
    #[cfg(windows)]
    std::os::windows::fs::symlink_file(&target, root.join("link.txt")).unwrap();
    let mut backend = FinalHandleBackend::bind(&root, "link.txt", barrier(), 1024).unwrap();
    let request = FinalHandleOpenRequest {
        relative_path: search_safe_reader::RelativePathToken::new(
            "link.txt",
            search_safe_reader::DEFAULT_SAFE_READ_LIMITS,
        )
        .unwrap(),
        expected_root_identity_digest: backend.root_digest(),
    };
    assert_eq!(
        backend.open_final(&request).unwrap_err(),
        AdapterError::LinkDenied
    );
    std::fs::remove_dir_all(&root).unwrap();
}

#[test]
fn locator_derivation_denies_escape_without_opening_bytes() {
    let root = fixture_root("escape");
    let child = root.join("child.txt");
    std::fs::write(&child, b"inside").unwrap();
    let outside_dir = fixture_root("outside");
    let outside = outside_dir.join("secret.txt");
    std::fs::write(&outside, b"outside").unwrap();
    let locator = derive_locator(&child, &root).unwrap();
    assert_eq!(locator.canonical_root, fs::canonicalize(&root).unwrap());
    assert_eq!(locator.token, "child.txt");
    assert_eq!(
        derive_locator(&outside, &root),
        Err(AdapterError::EscapeDenied)
    );
    assert_eq!(
        derive_locator(&PathBuf::from("relative.txt"), &root),
        Err(AdapterError::PathDenied)
    );
    assert_eq!(
        derive_locator(&root.join("..").join("sibling.txt"), &root),
        Err(AdapterError::PathDenied)
    );
    std::fs::remove_dir_all(&root).unwrap();
    std::fs::remove_dir_all(&outside_dir).unwrap();
}

fn symlink_file(target: &Path, link: &Path) {
    #[cfg(unix)]
    std::os::unix::fs::symlink(target, link).unwrap();
    #[cfg(windows)]
    std::os::windows::fs::symlink_file(target, link).unwrap();
}

fn symlink_dir(target: &Path, link: &Path) {
    #[cfg(unix)]
    std::os::unix::fs::symlink(target, link).unwrap();
    #[cfg(windows)]
    std::os::windows::fs::symlink_dir(target, link).unwrap();
}

#[test]
fn replacement_between_derive_and_open_is_denied() {
    let root = fixture_root("replace");
    let victim = root.join("victim.txt");
    std::fs::write(&victim, b"original bytes").unwrap();
    let locator = derive_locator(&victim, &root).unwrap();
    std::fs::remove_file(&victim).unwrap();
    let outside_dir = fixture_root("replace-out");
    let outside = outside_dir.join("evil.txt");
    std::fs::write(&outside, b"substituted bytes").unwrap();
    symlink_file(&outside, &victim);
    let mut backend =
        FinalHandleBackend::bind(&locator.canonical_root, &locator.token, barrier(), 1024)
            .unwrap();
    let request = FinalHandleOpenRequest {
        relative_path: search_safe_reader::RelativePathToken::new(
            &locator.token,
            search_safe_reader::DEFAULT_SAFE_READ_LIMITS,
        )
        .unwrap(),
        expected_root_identity_digest: backend.root_digest(),
    };
    assert_eq!(
        backend.open_final(&request).unwrap_err(),
        AdapterError::LinkDenied
    );
    let _ = std::fs::remove_file(&victim);
    std::fs::remove_dir_all(&root).unwrap();
    std::fs::remove_dir_all(&outside_dir).unwrap();
}

#[test]
fn ancestor_junction_escape_is_denied() {
    let root = fixture_root("ancestor");
    let sub = root.join("sub");
    std::fs::create_dir(&sub).unwrap();
    std::fs::write(sub.join("file.txt"), b"inside").unwrap();
    let outside_dir = fixture_root("ancestor-out");
    std::fs::write(outside_dir.join("file.txt"), b"foreign bytes").unwrap();
    std::fs::remove_file(sub.join("file.txt")).unwrap();
    std::fs::remove_dir(&sub).unwrap();
    symlink_dir(&outside_dir, &sub);
    assert_eq!(
        read_full_file_via_kernel(&sub.join("file.txt"), &root, 1024).unwrap_err(),
        FullReadError::Adapter(AdapterError::EscapeDenied)
    );
    let _ = std::fs::remove_file(&sub);
    #[cfg(unix)]
    let _ = std::fs::remove_file(&sub);
    std::fs::remove_dir_all(&root).unwrap();
    std::fs::remove_dir_all(&outside_dir).unwrap();
}

#[test]
fn hardlink_outside_domain_is_denied() {
    let root = fixture_root("hardlink");
    let outside_dir = fixture_root("hardlink-out");
    let outside = outside_dir.join("shared.txt");
    std::fs::write(&outside, b"shared bytes").unwrap();
    let alias = root.join("alias.txt");
    std::fs::hard_link(&outside, &alias).unwrap();
    assert_eq!(
        read_full_file_via_kernel(&alias, &root, 1024).unwrap_err(),
        FullReadError::Adapter(AdapterError::HardlinkDenied)
    );
    std::fs::remove_file(&alias).unwrap();
    std::fs::remove_dir_all(&root).unwrap();
    std::fs::remove_dir_all(&outside_dir).unwrap();
}

#[test]
fn in_root_rename_leaves_no_product() {
    let root = fixture_root("rename");
    let before = root.join("before.txt");
    std::fs::write(&before, b"renamed bytes").unwrap();
    std::fs::rename(&before, root.join("after.txt")).unwrap();
    assert_eq!(
        read_full_file_via_kernel(&before, &root, 1024).unwrap_err(),
        FullReadError::Adapter(AdapterError::AccessDenied)
    );
    let read = read_full_file_via_kernel(&root.join("after.txt"), &root, 1024).unwrap();
    assert_eq!(read.bytes, b"renamed bytes");
    std::fs::remove_dir_all(&root).unwrap();
}

#[test]
fn revocation_by_deletion_is_denied() {
    let root = fixture_root("revoke");
    let file = root.join("revoked.txt");
    std::fs::write(&file, b"revoked bytes").unwrap();
    std::fs::remove_file(&file).unwrap();
    assert_eq!(
        read_full_file_via_kernel(&file, &root, 1024).unwrap_err(),
        FullReadError::Adapter(AdapterError::AccessDenied)
    );
    std::fs::remove_dir_all(&root).unwrap();
}

#[cfg(unix)]
#[test]
fn revocation_by_permission_bits_is_denied() {
    use std::os::unix::fs::PermissionsExt;
    let root = fixture_root("chmod");
    let file = root.join("locked.txt");
    std::fs::write(&file, b"locked bytes").unwrap();
    std::fs::set_permissions(&file, std::fs::Permissions::from_mode(0o000)).unwrap();
    let result = read_full_file_via_kernel(&file, &root, 1024);
    std::fs::set_permissions(&file, std::fs::Permissions::from_mode(0o600)).unwrap();
    match result {
        Err(FullReadError::Adapter(AdapterError::AccessDenied)) => {}
        Ok(read) => assert_eq!(read.bytes, b"locked bytes"),
        Err(other) => panic!("unexpected denial: {other}"),
    }
    std::fs::remove_dir_all(&root).unwrap();
}

#[test]
fn empty_source_reads_empty_and_oversized_fails_before_allocation() {
    let root = fixture_root("sizes");
    let empty = root.join("empty.bin");
    std::fs::write(&empty, b"").unwrap();
    let read = read_full_file_via_kernel(&empty, &root, 1024).unwrap();
    assert!(read.bytes.is_empty());
    assert_eq!(read.source_bytes, 0);
    let big = root.join("big.bin");
    std::fs::write(&big, b"0123456789").unwrap();
    assert_eq!(
        read_full_file_via_kernel(&big, &root, 4).unwrap_err(),
        FullReadError::Adapter(AdapterError::TooLarge)
    );
    let read = read_full_file_via_kernel(&empty, &root, 0).unwrap();
    assert!(read.bytes.is_empty());
    assert_eq!(
        read_full_file_via_kernel(&big, &root, 0).unwrap_err(),
        FullReadError::Adapter(AdapterError::TooLarge)
    );
    std::fs::remove_dir_all(&root).unwrap();
}

#[test]
fn failures_are_content_free() {
    let root = fixture_root("redact");
    let name = "marker-name-7f3a9c.txt";
    let marker = "marker-content-51bd77e2";
    std::fs::write(root.join(name), marker.as_bytes()).unwrap();
    let error =
        read_full_file_via_kernel(&root.join(name), &root, marker.len() - 1).unwrap_err();
    assert_eq!(error, FullReadError::Adapter(AdapterError::TooLarge));
    let rendered = format!("{error}");
    assert_eq!(rendered, "SAFE_ADAPTER_TOO_LARGE");
    assert!(!rendered.contains("marker"));
    assert!(!rendered.contains("7f3a9c"));
    let debug = format!("{error:?}");
    assert!(!debug.contains("marker"));
    std::fs::remove_dir_all(&root).unwrap();
}

#[test]
fn script_source_is_inert_data_and_byte_exact() {
    let root = fixture_root("noexec");
    let mut payload = b"@echo PWNED-MARKER-9d2c\r\nWrite-Host hi\n".to_vec();
    payload.extend_from_slice(&[0x00, 0xFF, 0xFE, 0x80, 0xC3, 0x28]);
    let file = root.join("payload.bat");
    std::fs::write(&file, &payload).unwrap();
    let read = read_full_file_via_kernel(&file, &root, 1024).unwrap();
    assert_eq!(read.bytes, payload);
    assert_eq!(read.source_bytes, payload.len() as u64);
    assert!(!root.join("PWNED-MARKER-9d2c").try_exists().unwrap());
    assert!(!root.join("payload.out").try_exists().unwrap());
    std::fs::remove_dir_all(&root).unwrap();
}

#[test]
fn root_relocation_is_denied_explicitly() {
    let base = fixture_root("reloc");
    let root_a = base.join("root_a");
    std::fs::create_dir(&root_a).unwrap();
    std::fs::write(root_a.join("file.txt"), b"relocated").unwrap();
    let locator = derive_locator(&root_a.join("file.txt"), &root_a).unwrap();
    let mut backend =
        FinalHandleBackend::bind(&locator.canonical_root, &locator.token, barrier(), 1024)
            .unwrap();
    let root_b = base.join("root_b");
    std::fs::rename(&root_a, &root_b).unwrap();
    symlink_dir(&root_b, &root_a);
    let request = FinalHandleOpenRequest {
        relative_path: search_safe_reader::RelativePathToken::new(
            &locator.token,
            search_safe_reader::DEFAULT_SAFE_READ_LIMITS,
        )
        .unwrap(),
        expected_root_identity_digest: backend.root_digest(),
    };
    match backend.open_final(&request) {
        Err(AdapterError::RootRelocated) => {}
        Ok(handle) => {
            let metadata = backend.inspect(&handle).unwrap();
            assert_ne!(metadata.root_identity_digest, backend.root_digest());
        }
        Err(other) => panic!("unexpected open outcome: {other}"),
    }
    let _ = std::fs::remove_file(&root_a);
    std::fs::remove_dir_all(&base).unwrap();
}

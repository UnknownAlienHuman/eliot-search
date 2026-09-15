//! Test-only Credential Manager cleanup with bounded verified deletion.

use std::fs;
use std::path::Path;

use super::credential::read_credential;
use super::ffi::{
    CRED_TYPE_GENERIC, acquire_vault_lock, cred_delete_w, wide,
};

pub(super) fn delete_test_credential_for_data_root(data_root: &Path) {
    let derived = read_test_namespace_hex(data_root);
    if let Some(namespace_hex) = derived
        && !delete_test_credential_verified(&namespace_hex)
    {
        eprintln!(
            "ELIOT_TEST_CLEANUP: revision-key credential still present after bounded retries: root={} namespace={namespace_hex}",
            data_root.display(),
        );
    }
}

fn delete_test_credential_verified(namespace_hex: &str) -> bool {
    debug_assert!(is_test_namespace_hex(namespace_hex));
    let wide = wide(&format!("ELIOT Search/revision-key/{namespace_hex}"));
    for attempt in 0..16_u32 {
        let Ok(_lock) = acquire_vault_lock() else {
            std::thread::sleep(core::time::Duration::from_millis(
                10_u64 << attempt.min(7),
            ));
            continue;
        };
        unsafe {
            let _ = cred_delete_w(wide.as_ptr(), CRED_TYPE_GENERIC, 0);
        }
        if credential_absent(&wide) {
            return true;
        }
        std::thread::sleep(core::time::Duration::from_millis(
            10_u64 << attempt.min(7),
        ));
    }
    false
}

fn credential_absent(target: &[u16]) -> bool {
    match read_credential(target) {
        Ok(None) => true,
        Ok(Some(mut secret)) => {
            super::super::zeroize(&mut secret);
            false
        }
        Err(_) => false,
    }
}

/// Reads this data root's namespace hex without creating anything.
pub(super) fn read_test_namespace_hex(data_root: &Path) -> Option<String> {
    let bytes = fs::read(data_root.join("control").join("namespace.id")).ok()?;
    let text = core::str::from_utf8(&bytes).ok()?.trim().to_owned();
    is_test_namespace_hex(&text).then_some(text)
}

fn is_test_namespace_hex(value: &str) -> bool {
    value.len() == 64 && value.bytes().all(|byte| byte.is_ascii_hexdigit())
}

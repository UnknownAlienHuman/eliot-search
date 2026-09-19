//! Test-only Credential Manager cleanup with bounded verified deletion.

use std::fs;
use std::path::Path;

use search_os_secrets_windows::load_existing_legacy_revision_root_secret;

use super::ffi::{
    CRED_TYPE_GENERIC, acquire_vault_lock, cred_delete_w, wide,
};

pub(super) fn delete_test_credential_for_data_root(data_root: &Path) {
    let Some((namespace_id, namespace_hex)) = read_test_namespace(data_root) else {
        return;
    };
    if !delete_test_credential_verified(&namespace_id, &namespace_hex) {
        eprintln!(
            "ELIOT_TEST_CLEANUP: revision-key credential still present after bounded retries: root={} namespace={namespace_hex}",
            data_root.display(),
        );
    }
}

fn delete_test_credential_verified(
    namespace_id: &[u8; 32],
    namespace_hex: &str,
) -> bool {
    let target = wide(&format!("ELIOT Search/revision-key/{namespace_hex}"));
    for attempt in 0..16_u32 {
        let Ok(_lock) = acquire_vault_lock() else {
            std::thread::sleep(core::time::Duration::from_millis(
                10_u64 << attempt.min(7),
            ));
            continue;
        };
        // SAFETY: target is NUL-terminated and cleanup is limited to an exact
        // test-derived namespace credential.
        unsafe {
            let _ = cred_delete_w(target.as_ptr(), CRED_TYPE_GENERIC, 0);
        }
        if credential_absent(namespace_id) {
            return true;
        }
        std::thread::sleep(core::time::Duration::from_millis(
            10_u64 << attempt.min(7),
        ));
    }
    false
}

fn credential_absent(namespace_id: &[u8; 32]) -> bool {
    matches!(
        load_existing_legacy_revision_root_secret(namespace_id),
        Ok(None)
    )
}

/// Reads this data root's namespace hex without creating anything.
pub(super) fn read_test_namespace_hex(data_root: &Path) -> Option<String> {
    read_test_namespace(data_root).map(|(_, hex)| hex)
}

fn read_test_namespace(data_root: &Path) -> Option<([u8; 32], String)> {
    let bytes = fs::read(data_root.join("control").join("namespace.id")).ok()?;
    let text = core::str::from_utf8(&bytes).ok()?.trim();
    let namespace_id = decode_namespace_hex(text)?;
    let normalized = encode_namespace_hex(&namespace_id);
    Some((namespace_id, normalized))
}

fn decode_namespace_hex(value: &str) -> Option<[u8; 32]> {
    if value.len() != 64 {
        return None;
    }
    let mut output = [0_u8; 32];
    for (index, pair) in value.as_bytes().chunks_exact(2).enumerate() {
        output[index] = (hex_nibble(pair[0])? << 4) | hex_nibble(pair[1])?;
    }
    Some(output)
}

fn hex_nibble(byte: u8) -> Option<u8> {
    match byte {
        b'0'..=b'9' => Some(byte - b'0'),
        b'a'..=b'f' => Some(byte - b'a' + 10),
        b'A'..=b'F' => Some(byte - b'A' + 10),
        _ => None,
    }
}

fn encode_namespace_hex(value: &[u8; 32]) -> String {
    const HEX: &[u8; 16] = b"0123456789abcdef";
    let mut output = String::with_capacity(64);
    for byte in value {
        let byte = *byte;
        output.push(char::from(HEX[usize::from(byte >> 4)]));
        output.push(char::from(HEX[usize::from(byte & 0x0f)]));
    }
    output
}

use std::io::Write;

use crate::result_handles::ResultHandleExpansion;
use crate::sha256;
use crate::storage_security::StorageSecurityStatus;

use super::codec::{json_string, write_line};

/// Bounded provider status for the T19 `status` command.
///
/// This is read-only like health: it never mutates search state and never
/// emits an empty success.
pub fn emit_provider_status(
    writer: &mut impl Write,
    namespace_id: &str,
    search_available: bool,
    indexed_search_available: bool,
    source_backed_search_available: bool,
    blockers: &[&str],
    storage: &StorageSecurityStatus,
) -> Result<(), String> {
    if blockers.len() > 8 {
        return Err("SERVICE_STATUS_TOO_LARGE".to_owned());
    }
    let mut blockers_json = String::from("[");
    for (index, blocker) in blockers.iter().enumerate() {
        if index > 0 {
            blockers_json.push(',');
        }
        blockers_json.push_str(&json_string(blocker));
    }
    blockers_json.push(']');
    write_line(
        writer,
        &format!(
            concat!(
                "{{\"event\":\"provider_status\",",
                "\"namespace_id\":\"{}\",\"protocol_version\":\"1.0\",",
                "\"search_available\":{},",
                "\"indexed_search_available\":{},",
                "\"source_backed_search_available\":{},",
                "\"blockers\":{},\"source_backed\":true,",
                "\"storage_backend\":{},\"encrypted_at_rest\":{}}}"
            ),
            namespace_id,
            search_available,
            indexed_search_available,
            source_backed_search_available,
            blockers_json,
            json_string(storage.backend),
            storage.encrypted_at_rest,
        ),
    )
}

pub fn emit_handle_expansion(
    writer: &mut impl Write,
    expansion: &ResultHandleExpansion,
    storage: &StorageSecurityStatus,
) -> Result<(), String> {
    write_line(
        writer,
        &format!(
            concat!(
                "{{\"event\":\"source_handle_expanded\",",
                "\"source_handle\":{},\"byte_start\":{},",
                "\"byte_end\":{},\"source_byte_length\":{},",
                "\"encoding\":\"hex\",\"bytes\":{},",
                "\"session_scoped\":true,\"source_backed\":true,",
                "\"storage_backend\":{},\"encrypted_at_rest\":{}}}"
            ),
            json_string(&expansion.source_handle),
            expansion.byte_start,
            expansion.byte_end,
            expansion.source_byte_length,
            json_string(&sha256::hex(&expansion.bytes)),
            json_string(storage.backend),
            storage.encrypted_at_rest,
        ),
    )
}

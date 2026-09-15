use std::io::Write;

use crate::direct_store::IndexedSource;
use crate::storage_security::StorageSecurityStatus;

use super::codec::{json_string, write_line};

pub fn emit_indexed_source(
    writer: &mut impl Write,
    source: &IndexedSource,
    invalidated_continuations: usize,
    invalidated_handles: usize,
    storage: &StorageSecurityStatus,
) -> Result<(), String> {
    write_line(
        writer,
        &format!(
            concat!(
                "{{\"event\":\"source_indexed\",",
                "\"source_id\":\"{}\",\"revision_id\":\"{}\",",
                "\"content_digest\":\"{}\",\"path_digest\":\"{}\",",
                "\"byte_length\":{},\"identity_strength\":\"{}\",",
                "\"changed\":{},\"invalidated_continuations\":{},",
                "\"invalidated_handles\":{},",
                "\"diagnostic_internal_identifiers\":true,",
                "\"source_backed\":true,\"durable_revision\":true,",
                "\"storage_backend\":{},\"encrypted_at_rest\":{}}}"
            ),
            source.source_id,
            source.revision_id,
            source.content_digest,
            source.path_digest,
            source.byte_length,
            source.identity_strength,
            source.changed,
            invalidated_continuations,
            invalidated_handles,
            json_string(storage.backend),
            storage.encrypted_at_rest,
        ),
    )
}

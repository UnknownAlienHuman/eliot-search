use std::io::Write;

use crate::direct_store::{StoreSearchResult, StoredMatch};
use crate::storage_security::StorageSecurityStatus;

use super::codec::{json_string, write_line};

pub fn emit_streaming_search(
    writer: &mut impl Write,
    namespace_id: &str,
    result: &StoreSearchResult,
    storage: &StorageSecurityStatus,
) -> Result<(), String> {
    write_line(
        writer,
        &format!(
            concat!(
                "{{\"event\":\"corpus_search_started\",",
                "\"namespace_id\":\"{}\",\"registered_sources\":{},",
                "\"active_sources\":{},\"source_backed\":true,",
                "\"durable_revision\":true,\"storage_backend\":{},",
                "\"encrypted_at_rest\":{}}}"
            ),
            namespace_id,
            result.registered_sources,
            result.active_sources,
            json_string(storage.backend),
            storage.encrypted_at_rest,
        ),
    )?;
    for gap in &result.gaps {
        write_line(
            writer,
            &format!(
                concat!(
                    "{{\"event\":\"source_gap\",\"source_id\":\"{}\",",
                    "\"revision_id\":\"{}\",\"reason\":\"{}\",",
                    "\"diagnostic_internal_identifiers\":true}}"
                ),
                gap.source_id, gap.revision_id, gap.reason,
            ),
        )?;
    }
    for item in &result.matches {
        emit_internal_match(writer, item)?;
    }
    write_line(
        writer,
        &format!(
            concat!(
                "{{\"event\":\"corpus_search_complete\",",
                "\"matches\":{},\"gaps\":{},\"searched_sources\":{},",
                "\"active_sources\":{},\"complete\":{},",
                "\"match_limit_reached\":{},\"source_backed\":true,",
                "\"storage_backend\":{},\"encrypted_at_rest\":{}}}"
            ),
            result.matches.len(),
            result.gaps.len(),
            result.searched_sources,
            result.active_sources,
            result.complete,
            result.match_limit_reached,
            json_string(storage.backend),
            storage.encrypted_at_rest,
        ),
    )
}

fn emit_internal_match(
    writer: &mut impl Write,
    item: &StoredMatch,
) -> Result<(), String> {
    write_line(
        writer,
        &format!(
            concat!(
                "{{\"event\":\"match\",\"source_id\":\"{}\",",
                "\"revision_id\":\"{}\",\"content_digest\":\"{}\",",
                "\"path_digest\":\"{}\",\"evidence_id\":\"{}\",",
                "\"byte_start\":{},\"byte_end\":{},",
                "\"line\":{},\"column_bytes\":{},",
                "\"diagnostic_internal_identifiers\":true,",
                "\"source_backed\":true}}"
            ),
            item.source_id,
            item.revision_id,
            item.content_digest,
            item.path_digest,
            item.evidence_id,
            item.byte_start,
            item.byte_end,
            item.line,
            item.column_bytes,
        ),
    )
}

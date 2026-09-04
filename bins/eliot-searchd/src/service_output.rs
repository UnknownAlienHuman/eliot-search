//! Bounded newline-delimited JSON output for the owner-fenced DIRECT service.

use std::io::Write;

use crate::continuation::SearchPage;
use crate::direct_store::{IndexedSource, StoreSearchResult};
use crate::result_handles::{PublicHandledMatch, ResultHandleExpansion};
use crate::sha256;
use crate::storage_security::StorageSecurityStatus;

pub(crate) const MAX_RESPONSE_BYTES: usize = 64 * 1024;

pub(crate) fn emit_indexed_source(
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

pub(crate) fn emit_streaming_search(
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

pub(crate) fn emit_search_page(
    writer: &mut impl Write,
    page: &SearchPage,
    public_matches: &[PublicHandledMatch],
    storage: &StorageSecurityStatus,
) -> Result<(), String> {
    if page.matches.len() != public_matches.len() {
        return Err("SERVICE_HANDLE_PAGE_MISMATCH".to_owned());
    }
    write_line(
        writer,
        &format!(
            concat!(
                "{{\"event\":\"search_page_started\",",
                "\"page_start\":{},\"page_end\":{},",
                "\"total_matches\":{},\"retained_matches\":{},",
                "\"session_scoped\":true,\"source_backed\":true,",
                "\"storage_backend\":{},\"encrypted_at_rest\":{}}}"
            ),
            page.page_start,
            page.page_end,
            page.coverage.total_matches,
            page.coverage.retained_matches,
            json_string(storage.backend),
            storage.encrypted_at_rest,
        ),
    )?;
    for (index, gap) in page.gaps.iter().enumerate() {
        write_line(
            writer,
            &format!(
                concat!(
                    "{{\"event\":\"source_gap\",\"gap_index\":{},",
                    "\"reason\":\"{}\",",
                    "\"diagnostic_internal_identifiers\":false}}"
                ),
                index,
                gap.reason,
            ),
        )?;
    }
    for item in public_matches {
        emit_public_match(writer, item)?;
    }
    let continuation = page
        .continuation_token
        .as_ref()
        .map_or_else(|| "null".to_owned(), |token| json_string(token));
    let expires = page
        .expires_in_ms
        .map_or_else(|| "null".to_owned(), |value| value.to_string());
    write_line(
        writer,
        &format!(
            concat!(
                "{{\"event\":\"search_page_complete\",",
                "\"page_start\":{},\"page_end\":{},\"page_matches\":{},",
                "\"exhausted\":{},\"continuation_token\":{},",
                "\"expires_in_ms\":{},\"registered_sources\":{},",
                "\"active_sources\":{},\"searched_sources\":{},",
                "\"corpus_complete\":{},\"complete\":{},",
                "\"match_limit_reached\":{},",
                "\"total_matches\":{},\"retained_matches\":{},",
                "\"candidate_window_truncated\":{},\"gap_count\":{},",
                "\"gap_details_truncated\":{},\"session_scoped\":true,",
                "\"source_backed\":true,\"storage_backend\":{},",
                "\"encrypted_at_rest\":{}}}"
            ),
            page.page_start,
            page.page_end,
            public_matches.len(),
            page.exhausted,
            continuation,
            expires,
            page.coverage.registered_sources,
            page.coverage.active_sources,
            page.coverage.searched_sources,
            page.coverage.corpus_complete,
            page.coverage.complete(),
            page.coverage.match_limit_reached,
            page.coverage.total_matches,
            page.coverage.retained_matches,
            page.coverage.candidate_window_truncated,
            page.coverage.gap_count,
            page.coverage.gap_details_truncated,
            json_string(storage.backend),
            storage.encrypted_at_rest,
        ),
    )
}

pub(crate) fn emit_handle_expansion(
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

fn emit_internal_match(
    writer: &mut impl Write,
    item: &crate::direct_store::StoredMatch,
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

fn emit_public_match(
    writer: &mut impl Write,
    item: &PublicHandledMatch,
) -> Result<(), String> {
    write_line(
        writer,
        &format!(
            concat!(
                "{{\"event\":\"match\",\"source_handle\":{},",
                "\"evidence_id\":{},\"byte_start\":{},",
                "\"byte_end\":{},\"line\":{},\"column_bytes\":{},",
                "\"source_byte_length\":{},\"expires_in_ms\":{},",
                "\"session_scoped\":true,",
                "\"diagnostic_internal_identifiers\":false,",
                "\"source_backed\":true}}"
            ),
            json_string(&item.source_handle),
            json_string(&item.evidence_id),
            item.byte_start,
            item.byte_end,
            item.line,
            item.column_bytes,
            item.source_byte_length,
            item.expires_in_ms,
        ),
    )
}

pub(crate) fn write_line(writer: &mut impl Write, value: &str) -> Result<(), String> {
    if value.len() > MAX_RESPONSE_BYTES {
        return Err("SERVICE_RESPONSE_TOO_LARGE".to_owned());
    }
    writer
        .write_all(value.as_bytes())
        .and_then(|()| writer.write_all(b"\n"))
        .and_then(|()| writer.flush())
        .map_err(|error| format!("SERVICE_WRITE_ERROR:{error}"))
}

pub(crate) fn write_error(writer: &mut impl Write, error: &str) -> Result<(), String> {
    write_line(
        writer,
        &format!("{{\"event\":\"error\",\"error\":{}}}", json_string(error)),
    )
}

pub(crate) fn json_string(value: &str) -> String {
    let mut output = String::with_capacity(value.len().saturating_add(2));
    output.push('"');
    for character in value.chars() {
        match character {
            '"' => output.push_str("\\\""),
            '\\' => output.push_str("\\\\"),
            '\n' => output.push_str("\\n"),
            '\r' => output.push_str("\\r"),
            '\t' => output.push_str("\\t"),
            character if character.is_control() => {
                use std::fmt::Write as _;
                let _ = write!(&mut output, "\\u{:04x}", u32::from(character));
            }
            character => output.push(character),
        }
    }
    output.push('"');
    output
}

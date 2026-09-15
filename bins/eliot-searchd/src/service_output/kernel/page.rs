use std::io::Write;

use crate::continuation::SearchPage;
use crate::result_handles::PublicHandledMatch;
use crate::storage_security::StorageSecurityStatus;

use super::codec::{json_string, write_line};

pub fn emit_search_page(
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
                index, gap.reason,
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
            page.coverage.completion.corpus_complete,
            page.coverage.complete(),
            page.coverage.completion.match_limit_reached,
            page.coverage.total_matches,
            page.coverage.retained_matches,
            page.coverage.truncation.candidate_window_truncated,
            page.coverage.gap_count,
            page.coverage.truncation.gap_details_truncated,
            json_string(storage.backend),
            storage.encrypted_at_rest,
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

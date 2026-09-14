//! Stable JSON event emission for one-shot and DIRECT corpus commands.

use crate::development::{ScanResult, scan_text};
use crate::direct_store::{IndexedSource, StoreSearchResult};
use crate::sha256;

pub(super) fn emit_one_shot_scan(
    source: &str,
    query: &str,
    ascii_insensitive: bool,
    text: &str,
    same_handle_verified: bool,
    source_backed: bool,
) -> Result<(), String> {
    let result = scan_text(text, query, ascii_insensitive)?;
    let content_digest = sha256::hex(&sha256::digest(text.as_bytes()));
    println!(
        concat!(
            "{{\"event\":\"scan_started\",\"source\":\"{}\",",
            "\"mode\":\"{}\",\"input_bytes\":{},",
            "\"content_digest\":\"{}\",",
            "\"same_handle_verified\":{},\"source_backed\":{},",
            "\"durable_revision\":false,\"encrypted_at_rest\":false}}"
        ),
        source,
        if ascii_insensitive {
            "ascii_insensitive"
        } else {
            "sensitive"
        },
        result.coverage.input_bytes,
        content_digest,
        same_handle_verified,
        source_backed,
    );
    emit_scan_matches(&result, &content_digest, source_backed);
    Ok(())
}

fn emit_scan_matches(
    result: &ScanResult,
    content_digest: &str,
    source_backed: bool,
) {
    for item in &result.matches {
        let evidence_id = sha256::hex(&sha256::digest_parts(
            b"eliot-search/one-shot-evidence/v1",
            &[
                content_digest.as_bytes(),
                &u64::try_from(item.byte_start)
                    .unwrap_or(u64::MAX)
                    .to_be_bytes(),
                &u64::try_from(item.byte_end)
                    .unwrap_or(u64::MAX)
                    .to_be_bytes(),
            ],
        ));
        println!(
            concat!(
                "{{\"event\":\"match\",\"evidence_id\":\"{}\",",
                "\"byte_start\":{},\"byte_end\":{},",
                "\"line\":{},\"column_bytes\":{},",
                "\"source_backed\":{}}}"
            ),
            evidence_id,
            item.byte_start,
            item.byte_end,
            item.line,
            item.column_bytes,
            source_backed,
        );
    }
    println!(
        concat!(
            "{{\"event\":\"scan_complete\",\"matches\":{},",
            "\"match_limit_reached\":{},\"complete\":{},",
            "\"source_backed\":{}}}"
        ),
        result.matches.len(),
        result.coverage.match_limit_reached,
        result.coverage.complete,
        source_backed,
    );
}

pub(super) fn emit_indexed_source(source: &IndexedSource) {
    println!(
        concat!(
            "{{\"event\":\"source_indexed\",",
            "\"source_id\":\"{}\",\"revision_id\":\"{}\",",
            "\"content_digest\":\"{}\",\"path_digest\":\"{}\",",
            "\"byte_length\":{},\"identity_strength\":\"{}\",",
            "\"changed\":{},\"source_backed\":true,",
            "\"durable_revision\":true,\"encrypted_at_rest\":false}}"
        ),
        source.source_id,
        source.revision_id,
        source.content_digest,
        source.path_digest,
        source.byte_length,
        source.identity_strength,
        source.changed,
    );
}

pub(super) fn emit_store_search(
    namespace_id: &str,
    result: &StoreSearchResult,
) {
    println!(
        concat!(
            "{{\"event\":\"corpus_search_started\",",
            "\"namespace_id\":\"{}\",\"registered_sources\":{},",
            "\"active_sources\":{},\"source_backed\":true,",
            "\"durable_revision\":true,\"encrypted_at_rest\":false}}"
        ),
        namespace_id,
        result.registered_sources,
        result.active_sources,
    );
    for gap in &result.gaps {
        println!(
            concat!(
                "{{\"event\":\"source_gap\",\"source_id\":\"{}\",",
                "\"revision_id\":\"{}\",\"reason\":\"{}\"}}"
            ),
            gap.source_id,
            gap.revision_id,
            gap.reason,
        );
    }
    for item in &result.matches {
        println!(
            concat!(
                "{{\"event\":\"match\",\"source_id\":\"{}\",",
                "\"revision_id\":\"{}\",\"content_digest\":\"{}\",",
                "\"path_digest\":\"{}\",\"evidence_id\":\"{}\",",
                "\"byte_start\":{},\"byte_end\":{},",
                "\"line\":{},\"column_bytes\":{},",
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
        );
    }
    println!(
        concat!(
            "{{\"event\":\"corpus_search_complete\",",
            "\"matches\":{},\"gaps\":{},\"searched_sources\":{},",
            "\"active_sources\":{},\"complete\":{},",
            "\"match_limit_reached\":{},\"source_backed\":true,",
            "\"encrypted_at_rest\":false}}"
        ),
        result.matches.len(),
        result.gaps.len(),
        result.searched_sources,
        result.active_sources,
        result.complete,
        result.match_limit_reached,
    );
}

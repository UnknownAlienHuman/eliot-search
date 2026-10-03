//! Content-free JSON projections for the legacy snapshot protocol.

use std::fmt::Write as _;
use std::net::SocketAddr;
use std::process;

use crate::control_store::DevelopmentControlStore;
use crate::lexical::{LexicalIndex, LexicalSearchResult};
use crate::snapshot::{SnapshotIndex, SnapshotSearchResult, hex32};

use super::protocol::escape_json;

pub(super) const fn capture_complete(snapshot: &SnapshotIndex) -> bool {
    let stats = snapshot.stats();
    !stats.truncated && stats.unreadable_files == 0 && stats.unstable_files == 0
}

pub(super) fn render_health(
    snapshot: &SnapshotIndex,
    lexical: &LexicalIndex,
    control: &DevelopmentControlStore,
    refreshed: bool,
) -> String {
    let stats = snapshot.stats();
    format!(
        concat!(
            "{{\"ok\":true,\"service\":\"eliot-searchd\",",
            "\"state\":\"READY\",\"stage\":\"W3_LOCAL_LEXICAL\",",
            "\"snapshot_id\":\"{}\",\"manifest_fingerprint\":\"{}\",",
            "\"fingerprint_algorithm\":\"{}\",\"indexed_files\":{},",
            "\"snapshot_bytes\":{},\"capture_complete\":{},",
            "\"lexical_documents\":{},\"lexical_terms\":{},",
            "\"lexical_postings\":{},\"lexical_index_fingerprint\":\"{}\",",
            "\"source_backed_search\":true,\"retained_revision_readback\":true,",
            "\"lexical_search\":true,\"encrypted_revisions\":false,",
            "\"production_ready\":false,\"control_generation\":{},",
            "\"recovered_previous_active\":{},\"refreshed\":{}}}"
        ),
        snapshot.snapshot_id(),
        hex32(snapshot.manifest_fingerprint()),
        SnapshotIndex::fingerprint_algorithm(),
        stats.indexed_files,
        stats.total_bytes,
        capture_complete(snapshot),
        lexical.document_count(),
        lexical.term_count(),
        lexical.posting_count(),
        hex32(lexical.index_fingerprint()),
        control.generation(),
        control.recovered_previous_active(),
        refreshed,
    )
}

pub(super) fn render_status(
    address: SocketAddr,
    source_root_count: usize,
    snapshot: &SnapshotIndex,
    lexical: &LexicalIndex,
    control: &DevelopmentControlStore,
) -> String {
    let stats = snapshot.stats();
    format!(
        concat!(
            "{{\"ok\":true,\"service\":\"eliot-searchd\",",
            "\"state\":\"READY\",\"stage\":\"W3_LOCAL_LEXICAL\",",
            "\"pid\":{},\"address\":\"{}\",\"source_roots\":{},",
            "\"snapshot_id\":\"{}\",\"manifest_path\":\"{}\",",
            "\"indexed_files\":{},\"snapshot_bytes\":{},",
            "\"written_revisions\":{},\"reused_revisions\":{},",
            "\"skipped_links\":{},\"skipped_policy\":{},",
            "\"skipped_binary\":{},\"unreadable_files\":{},",
            "\"unstable_files\":{},\"capture_truncated\":{},",
            "\"capture_complete\":{},\"lexical_analyzer\":\"{}\",",
            "\"lexical_documents\":{},\"lexical_terms\":{},",
            "\"lexical_postings\":{},\"lexical_index_fingerprint\":\"{}\",",
            "\"control_generation\":{},\"control_directory\":\"{}\",",
            "\"source_backed_search\":true,\"lexical_search\":true,",
            "\"encrypted_revisions\":false,\"production_ready\":false}}"
        ),
        process::id(),
        address,
        source_root_count,
        snapshot.snapshot_id(),
        escape_json(&snapshot.manifest_path().display().to_string()),
        stats.indexed_files,
        stats.total_bytes,
        stats.written_revisions,
        stats.reused_revisions,
        stats.skipped_links,
        stats.skipped_policy,
        stats.skipped_binary,
        stats.unreadable_files,
        stats.unstable_files,
        stats.truncated,
        capture_complete(snapshot),
        escape_json(lexical.analyzer_id()),
        lexical.document_count(),
        lexical.term_count(),
        lexical.posting_count(),
        hex32(lexical.index_fingerprint()),
        control.generation(),
        escape_json(&control.directory().display().to_string()),
    )
}

pub(super) fn render_search_response(query: &str, result: &SnapshotSearchResult) -> String {
    let mut output = format!(
        concat!(
            "{{\"ok\":true,\"mode\":\"DIRECT_RETAINED_REVISION\",",
            "\"query\":\"{}\",\"query_case_policy\":\"exact_plus_ascii_insensitive\",",
            "\"snapshot_id\":\"{}\",\"manifest_fingerprint\":\"{}\",",
            "\"fingerprint_algorithm\":\"{}\",\"denominator_files\":{},",
            "\"scanned_revisions\":{},\"unavailable_revisions\":{},",
            "\"complete\":{},\"truncated\":{},\"source_backed\":true,",
            "\"retained_revision_readback\":true,\"encrypted_at_rest\":false,",
            "\"production_ready\":false,\"results\":["
        ),
        escape_json(query),
        result.snapshot_id,
        hex32(result.manifest_fingerprint),
        result.fingerprint_algorithm,
        result.denominator_files,
        result.scanned_revisions,
        result.unavailable_revisions,
        result.complete,
        result.truncated,
    );
    for (index, item) in result.matches.iter().enumerate() {
        if index != 0 {
            output.push(',');
        }
        let _ = write!(
            output,
            concat!(
                "{{\"root\":{},\"path\":\"{}\",",
                "\"revision_fingerprint\":\"{}\",\"line\":{},",
                "\"column_bytes\":{},\"byte_start\":{},\"byte_end\":{},",
                "\"excerpt\":\"{}\"}}"
            ),
            item.root_index,
            escape_json(&item.relative_path),
            hex32(item.revision_fingerprint),
            item.line,
            item.column_bytes,
            item.byte_start,
            item.byte_end,
            escape_json(&item.excerpt),
        );
    }
    output.push_str("]}");
    output
}

pub(super) fn render_lexical_response(query: &str, result: &LexicalSearchResult) -> String {
    let mut output = format!(
        concat!(
            "{{\"ok\":true,\"mode\":\"LEXICAL_BM25_RETAINED_REVISION\",",
            "\"query\":\"{}\",\"snapshot_id\":\"{}\",",
            "\"manifest_fingerprint\":\"{}\",\"lexical_index_fingerprint\":\"{}\",",
            "\"analyzer\":\"{}\",\"denominator_documents\":{},",
            "\"indexed_terms\":{},\"indexed_postings\":{},",
            "\"query_term_count\":{},\"candidate_documents\":{},",
            "\"unavailable_revisions\":{},\"complete\":{},\"truncated\":{},",
            "\"source_backed\":true,\"retained_revision_readback\":true,",
            "\"encrypted_at_rest\":false,\"production_ready\":false,",
            "\"results\":["
        ),
        escape_json(query),
        result.snapshot_id,
        hex32(result.snapshot_manifest_fingerprint),
        hex32(result.index_fingerprint),
        escape_json(&result.analyzer_id),
        result.denominator_documents,
        result.indexed_terms,
        result.indexed_postings,
        result.query_term_count,
        result.candidate_documents,
        result.unavailable_revisions,
        result.complete,
        result.truncated,
    );
    for (index, item) in result.matches.iter().enumerate() {
        if index != 0 {
            output.push(',');
        }
        let _ = write!(
            output,
            concat!(
                "{{\"root\":{},\"path\":\"{}\",",
                "\"revision_fingerprint\":\"{}\",\"score\":{:.8},",
                "\"matched_terms\":{},\"line\":{},\"column_bytes\":{},",
                "\"byte_start\":{},\"excerpt\":\"{}\"}}"
            ),
            item.root_index,
            escape_json(&item.relative_path),
            hex32(item.revision_fingerprint),
            item.score,
            item.matched_terms,
            item.line,
            item.column_bytes,
            item.byte_start,
            escape_json(&item.excerpt),
        );
    }
    output.push_str("]}");
    output
}

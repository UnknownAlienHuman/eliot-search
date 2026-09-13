//! Canonical entire-query DIRECT budget and source-backed evidence gates.

use crate::development::MAX_SCAN_MATCHES;

/// Entire-query corpus budget across all admitted sources.
///
/// Per-file literal limits alone are not a corpus budget. Exhaustion yields
/// explicit typed gaps with `complete = false`, never a narrowed denominator
/// relabelled as success.
#[allow(clippy::struct_field_names)]
pub struct CorpusBudget {
    /// Maximum admitted sources attempted by one query.
    pub max_sources: usize,
    /// Maximum summed retained bytes attempted by one query.
    pub max_source_bytes: u64,
    /// Maximum emitted matches across every source.
    pub max_matches: usize,
    /// Maximum recorded gap entries.
    pub max_gaps: usize,
}

/// Canonical entire-query budget: 100k sources, 1 GiB retained bytes, 100k
/// matches and 100k gaps.
pub const CANONICAL_CORPUS_BUDGET: CorpusBudget = CorpusBudget {
    max_sources: 100_000,
    max_source_bytes: 1_073_741_824,
    max_matches: MAX_SCAN_MATCHES,
    max_gaps: 100_000,
};

/// Gap reason for sources skipped after the match ceiling filled.
pub const SPINE_GAP_MATCH_LIMIT: &str = "DIRECT_MATCH_LIMIT_REACHED";
/// Gap reason for sources skipped after the corpus byte/source budget.
pub const SPINE_GAP_BUDGET_EXHAUSTED: &str = "DIRECT_CORPUS_BUDGET_EXHAUSTED";
/// Gap reason when a stored match fails source-backed revalidation.
pub const SPINE_GAP_VALIDATION_FAILED: &str = "DIRECT_MATCH_VALIDATION_FAILED";

/// Verifies the service-boundary denominator and completeness claim.
pub const fn verify_spine_gate(
    active_sources: usize,
    searched_sources: usize,
    gaps_empty: bool,
    complete: bool,
    match_limit_reached: bool,
) -> Result<(), &'static str> {
    if match_limit_reached && complete {
        return Err("DIRECT_SPINE_GATE_DENOMINATOR_INVALID");
    }
    if complete && (!gaps_empty || searched_sources != active_sources) {
        return Err("DIRECT_SPINE_GATE_DENOMINATOR_INVALID");
    }
    Ok(())
}

/// Revalidates one emitted match against verified retained-revision text.
///
/// Exact bounds, character boundaries and literal byte equality are required;
/// a mismatch becomes a per-source gap instead of unproven evidence.
pub fn validate_source_backed_match(
    text: &str,
    query: &str,
    ascii_insensitive: bool,
    byte_start: usize,
    byte_end: usize,
) -> Result<(), &'static str> {
    if byte_start >= byte_end || byte_end > text.len() {
        return Err(SPINE_GAP_VALIDATION_FAILED);
    }
    if !text.is_char_boundary(byte_start) || !text.is_char_boundary(byte_end) {
        return Err(SPINE_GAP_VALIDATION_FAILED);
    }
    let Some(slice) = text.get(byte_start..byte_end) else {
        return Err(SPINE_GAP_VALIDATION_FAILED);
    };
    let matched = if ascii_insensitive {
        slice.len() == query.len() && slice.as_bytes().eq_ignore_ascii_case(query.as_bytes())
    } else {
        slice == query
    };
    if matched {
        Ok(())
    } else {
        Err(SPINE_GAP_VALIDATION_FAILED)
    }
}

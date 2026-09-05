//! Composition of shared preparation and literal execution after DIRECT readback.
//!
//! The caller retains its actual source/revision identity and SHA-256 binding.
//! No BLAKE3 digest, revision sequence or durable receipt is fabricated here.
//! Preparation is deterministic and memory-only; no second persistent index exists.

use search_exact::literal::{self, LiteralLimits};
use search_materializer::{MaterializationLimits, materialize_utf8};
use search_unitizer::{SourceLineSpan, UnitizationLimits, unitize_text};

use crate::development::{
    MAX_SCAN_INPUT_BYTES, MAX_SCAN_MATCHES, MAX_SCAN_QUERY_BYTES,
    ScanCoverage, ScanMatch, ScanResult,
};

const MATERIALIZATION: MaterializationLimits = MaterializationLimits {
    max_input_bytes: MAX_SCAN_INPUT_BYTES,
    max_output_bytes: MAX_SCAN_INPUT_BYTES,
    max_lines: 1_000_000,
};
const UNITIZATION: UnitizationLimits = UnitizationLimits {
    max_input_bytes: MAX_SCAN_INPUT_BYTES,
    preferred_unit_bytes: 16 * 1024,
    max_unit_bytes: 64 * 1024,
    max_lines: 1_000_000,
    max_units: 1_000_000,
};
const LITERAL: LiteralLimits = LiteralLimits {
    max_query_bytes: MAX_SCAN_QUERY_BYTES,
    max_input_bytes: MAX_SCAN_INPUT_BYTES,
    max_chunks: 1_000_000,
    max_matches: MAX_SCAN_MATCHES,
};

pub(crate) fn validate_query(query: &str) -> Result<(), &'static str> {
    if query.is_empty() {
        return Err("DIRECT_QUERY_EMPTY");
    }
    literal::validate_query(query, LITERAL).map_err(|error| error.code())
}

pub(crate) fn prepare_and_scan(
    verified_text: String,
    query: &str,
    ascii_insensitive: bool,
) -> Result<ScanResult, &'static str> {
    validate_query(query)?;
    scan_with_limits(verified_text, query, ascii_insensitive, MATERIALIZATION, UNITIZATION, LITERAL)
}

fn scan_with_limits(
    text: String,
    query: &str,
    ascii_insensitive: bool,
    materialization: MaterializationLimits,
    unitization: UnitizationLimits,
    literal: LiteralLimits,
) -> Result<ScanResult, &'static str> {
    let prepared = materialize_utf8(text.into_bytes(), materialization)
        .map_err(|error| error.code())?;
    let lines = prepared.lines().iter().map(|line| SourceLineSpan {
        line_index: line.line_index,
        source_start: line.source_start,
        source_end: line.source_end,
        content_end: line.content_end,
    }).collect::<Vec<_>>();
    let units = unitize_text(prepared.text(), &lines, unitization)
        .map_err(|error| error.code())?;
    let chunks = units.iter().map(|unit| {
        prepared.text().get(unit.source_start..unit.source_end)
            .ok_or("DIRECT_PREPARATION_COORDINATE_INVALID")
    }).collect::<Result<Vec<_>, _>>()?;
    let result = literal::scan_chunks(&chunks, query, ascii_insensitive, literal)
        .map_err(|error| error.code())?;
    let complete = result.complete();
    let matches = result.matches.into_iter().map(|range| {
        let start = u64::try_from(range.start).map_err(|_| "DIRECT_PREPARATION_COORDINATE_INVALID")?;
        let index = prepared.lines().partition_point(|line| line.source_end <= start);
        let line = prepared.lines().get(index).ok_or("DIRECT_PREPARATION_COORDINATE_INVALID")?;
        let line_start = usize::try_from(line.source_start).map_err(|_| "DIRECT_PREPARATION_COORDINATE_INVALID")?;
        Ok(ScanMatch {
            byte_start: range.start,
            byte_end: range.end,
            line: usize::try_from(line.line_index).map_err(|_| "DIRECT_PREPARATION_COORDINATE_INVALID")?,
            column_bytes: range.start.checked_sub(line_start).ok_or("DIRECT_PREPARATION_COORDINATE_INVALID")?,
        })
    }).collect::<Result<Vec<_>, &'static str>>()?;
    Ok(ScanResult {
        matches,
        coverage: ScanCoverage {
            input_bytes: result.input_bytes,
            complete,
            match_limit_reached: result.match_limit_reached,
        },
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    fn small(text: &str, query: &str) -> ScanResult {
        scan_with_limits(
            text.to_owned(), query, false, MATERIALIZATION,
            UnitizationLimits { preferred_unit_bytes: 2, max_unit_bytes: 4, ..UNITIZATION },
            LITERAL,
        ).unwrap()
    }

    #[test]
    fn query_longer_than_units_still_matches_once() {
        let result = small("a123456789z", "123456789");
        assert_eq!(result.matches.len(), 1);
        assert_eq!((result.matches[0].byte_start, result.matches[0].byte_end), (1, 10));
        assert!(result.coverage.complete);
    }

    #[test]
    fn newline_and_unicode_coordinates_are_source_byte_coordinates() {
        let text = "a\r\nβ\rc\n𐀀 target";
        let result = small(text, "target");
        let start = text.find("target").unwrap();
        assert_eq!(result.matches, vec![ScanMatch { byte_start: start, byte_end: start + 6, line: 3, column_bytes: 5 }]);
        let crossing = small("a\r\nβ", "\r\nβ");
        assert_eq!((crossing.matches[0].byte_start, crossing.matches[0].line), (1, 0));
    }

    #[test]
    fn repeated_matches_across_units_are_not_duplicated_or_lost() {
        let result = small("aaaaa", "aaa");
        assert_eq!(result.matches.iter().map(|item| item.byte_start).collect::<Vec<_>>(), vec![0, 1, 2]);
    }

    #[test]
    fn preparation_failure_is_not_complete_empty_success() {
        assert_eq!(prepare_and_scan("a\0b".to_owned(), "missing", false), Err("MATERIALIZATION_BINARY_CONTENT"));
        assert!(small("", "missing").coverage.complete);
    }

    #[test]
    fn limits_propagate_without_claiming_full_coverage() {
        let result = scan_with_limits("aaaa".to_owned(), "aa", false, MATERIALIZATION, UNITIZATION,
            LiteralLimits { max_matches: 1, ..LITERAL }).unwrap();
        assert_eq!(result.matches.len(), 1);
        assert!(!result.coverage.complete);
        assert!(result.coverage.match_limit_reached);
        assert_eq!(scan_with_limits("a\nb".to_owned(), "b", false,
            MaterializationLimits { max_lines: 1, ..MATERIALIZATION }, UNITIZATION, LITERAL),
            Err("MATERIALIZATION_TOO_MANY_LINES"));
    }
}

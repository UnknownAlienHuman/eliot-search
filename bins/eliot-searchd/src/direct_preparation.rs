//! DIRECT preparation is built during ingestion; search consumes its saved layout.
//! The storage envelope binds the actual namespace/revision/SHA-256 identities.

use search_exact::literal::{self, LiteralLimits};
use search_materializer::{LineSpan, MaterializationError, MaterializationLimits, materialize_utf8};
use search_unitizer::{SourceLineSpan, UnitSpan, UnitizationError, UnitizationLimits};
#[cfg(test)]
use search_unitizer::unitize_text;

use crate::development::{MAX_SCAN_INPUT_BYTES, MAX_SCAN_MATCHES, MAX_SCAN_QUERY_BYTES,
    ScanCoverage, ScanMatch, ScanResult};
use crate::sha256;

pub const MAX_LAYOUT_BYTES: usize = 64 * 1024 * 1024 - 512;
const MATERIALIZATION: MaterializationLimits = MaterializationLimits {
    max_input_bytes: MAX_SCAN_INPUT_BYTES, max_output_bytes: MAX_SCAN_INPUT_BYTES, max_lines: 1_000_000,
};
const UNITIZATION: UnitizationLimits = UnitizationLimits {
    max_input_bytes: MAX_SCAN_INPUT_BYTES, preferred_unit_bytes: 16 * 1024,
    max_unit_bytes: 64 * 1024, max_lines: 1_000_000, max_units: 1_000_000,
};
const LITERAL: LiteralLimits = LiteralLimits {
    max_query_bytes: MAX_SCAN_QUERY_BYTES, max_input_bytes: MAX_SCAN_INPUT_BYTES,
    max_chunks: 1_000_000, max_matches: MAX_SCAN_MATCHES,
};

/// Bind every preparation algorithm/limit, not query options, into the disk key.
pub fn profile_digest() -> [u8; 32] {
    let mut settings = Vec::new();
    for value in [MATERIALIZATION.max_input_bytes, MATERIALIZATION.max_output_bytes,
        MATERIALIZATION.max_lines, UNITIZATION.max_input_bytes, UNITIZATION.preferred_unit_bytes,
        UNITIZATION.max_unit_bytes, UNITIZATION.max_lines, UNITIZATION.max_units, MAX_LAYOUT_BYTES]
    {
        settings.extend_from_slice(&(value as u64).to_be_bytes());
    }
    sha256::digest_parts(b"eliot-search/direct-preparation-profile/v1", &[
        b"utf8-exact;no-normalization;crlf-cr-lf;nul-denied;controls-ceil-1pct-min4/v1",
        UnitizationLimits::LAYOUT_FORMAT.as_bytes(), &settings,
    ])
}

pub fn validate_query(query: &str) -> Result<(), &'static str> {
    if query.is_empty() { return Err("DIRECT_QUERY_EMPTY"); }
    literal::validate_query(query, LITERAL).map_err(literal::LiteralError::code)
}

/// Canonical layout or a closed deterministic preparation gap, never source text.
/// Storage failures are not encoded as content outcomes.
pub fn encode_preparation(bytes: &[u8]) -> Result<Vec<u8>, &'static str> {
    // Keep the existing DIRECT precedence for invalid UTF-8 versus binary policy.
    if std::str::from_utf8(bytes).is_err() { return Ok(vec![1]); }
    let prepared = match materialize_utf8(bytes.to_vec(), MATERIALIZATION) {
        Ok(value) => value,
        Err(MaterializationError::BinaryContent) => return Ok(vec![2]),
        Err(MaterializationError::TooManyLines) => return Ok(vec![3]),
        Err(error) => return Err(error.code()),
    };
    let encoded = match UNITIZATION.encode_layout(prepared.text(), &line_spans(prepared.lines()), MAX_LAYOUT_BYTES) {
        Ok(value) => value,
        Err(UnitizationError::TooManyUnits) => return Ok(vec![4]),
        Err(UnitizationError::InputTooLarge) => return Ok(vec![5]),
        Err(error) => return Err(error.code()),
    };
    let mut output = Vec::with_capacity(encoded.len() + 1);
    output.push(0);
    output.extend_from_slice(&encoded);
    Ok(output)
}

/// Search verified bytes using the saved exact layout. No preparation/storage write.
pub fn scan_prepared(
    text: &str, encoded: &[u8], query: &str, ascii_insensitive: bool,
) -> Result<ScanResult, &'static str> {
    validate_query(query)?;
    if let Some(reason) = preparation_gap(encoded)? { return Err(reason); }
    let layout = encoded.get(1..).ok_or("DIRECT_PREPARATION_INVALID")?;
    let (lines, units) = UNITIZATION.decode_layout(text, layout, MAX_LAYOUT_BYTES)
        .map_err(UnitizationError::code)?;
    scan_layout(text, &lines, &units, query, ascii_insensitive, LITERAL)
}

/// Decode only the outcome framing; source/layout validation remains mandatory for reads.
/// Reused by storage acknowledgements so a saved unsupported input is not reported as a layout.
pub const fn preparation_gap(encoded: &[u8]) -> Result<Option<&'static str>, &'static str> {
    match encoded {
        [0, layout @ ..] if !layout.is_empty() => Ok(None),
        [1] => Ok(Some("DIRECT_REVISION_NOT_UTF8")),
        [2] => Ok(Some("MATERIALIZATION_BINARY_CONTENT")),
        [3] => Ok(Some("MATERIALIZATION_TOO_MANY_LINES")),
        [4] => Ok(Some("UNITIZATION_TOO_MANY_UNITS")),
        [5] => Ok(Some("DIRECT_PREPARATION_LAYOUT_TOO_LARGE")),
        _ => Err("DIRECT_PREPARATION_INVALID"),
    }
}

fn line_spans(lines: &[LineSpan]) -> Vec<SourceLineSpan> {
    lines.iter().map(|line| SourceLineSpan {
        line_index: line.line_index, source_start: line.source_start,
        source_end: line.source_end, content_end: line.content_end,
    }).collect()
}

fn scan_layout(
    text: &str, lines: &[SourceLineSpan], units: &[UnitSpan], query: &str,
    ascii_insensitive: bool, limits: LiteralLimits,
) -> Result<ScanResult, &'static str> {
    let chunks = units.iter().map(|unit| text.get(unit.source_start..unit.source_end)
        .ok_or("DIRECT_PREPARATION_COORDINATE_INVALID")).collect::<Result<Vec<_>, _>>()?;
    let result = literal::scan_chunks(&chunks, query, ascii_insensitive, limits).map_err(literal::LiteralError::code)?;
    let complete = result.complete();
    let matches = result.matches.into_iter().map(|range| {
        let start = u64::try_from(range.start).map_err(|_| "DIRECT_PREPARATION_COORDINATE_INVALID")?;
        let line = lines.get(lines.partition_point(|line| line.source_end <= start))
            .ok_or("DIRECT_PREPARATION_COORDINATE_INVALID")?;
        let line_start = usize::try_from(line.source_start).map_err(|_| "DIRECT_PREPARATION_COORDINATE_INVALID")?;
        Ok(ScanMatch {
            byte_start: range.start, byte_end: range.end,
            line: usize::try_from(line.line_index).map_err(|_| "DIRECT_PREPARATION_COORDINATE_INVALID")?,
            column_bytes: range.start.checked_sub(line_start).ok_or("DIRECT_PREPARATION_COORDINATE_INVALID")?,
        })
    }).collect::<Result<Vec<_>, &'static str>>()?;
    Ok(ScanResult { matches, coverage: ScanCoverage {
        input_bytes: result.input_bytes, complete, match_limit_reached: result.match_limit_reached,
    } })
}

// Preserve the existing pure regression entrypoint; product queries use scan_prepared.
#[cfg(test)]
pub fn prepare_and_scan(text: String, query: &str, insensitive: bool) -> Result<ScanResult, &'static str> {
    validate_query(query)?;
    scan_with_limits(text, query, insensitive, MATERIALIZATION, UNITIZATION, LITERAL)
}
#[cfg(test)]
fn scan_with_limits(
    text: String, query: &str, insensitive: bool, materialization: MaterializationLimits,
    unitization: UnitizationLimits, literal: LiteralLimits,
) -> Result<ScanResult, &'static str> {
    let prepared = materialize_utf8(text.into_bytes(), materialization).map_err(MaterializationError::code)?;
    let lines = line_spans(prepared.lines());
    let units = unitize_text(prepared.text(), &lines, unitization).map_err(UnitizationError::code)?;
    scan_layout(prepared.text(), &lines, &units, query, insensitive, literal)
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

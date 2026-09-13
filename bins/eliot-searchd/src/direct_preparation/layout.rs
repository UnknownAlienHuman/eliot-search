//! Exact DIRECT layout encoding, decoding and literal scanning.

use search_exact::literal::{self, LiteralLimits};
use search_materializer::{LineSpan, MaterializationError, materialize_utf8};
#[cfg(test)]
use search_materializer::MaterializationLimits;
use search_unitizer::{SourceLineSpan, UnitSpan, UnitizationError};
#[cfg(test)]
use search_unitizer::{UnitizationLimits, unitize_text};

use crate::development::{ScanCoverage, ScanMatch, ScanResult};

use super::profile::{LITERAL, MATERIALIZATION, MAX_LAYOUT_BYTES, UNITIZATION};

/// Validates one bounded literal query through the shared exact-search owner.
pub fn validate_query(query: &str) -> Result<(), &'static str> {
    if query.is_empty() {
        return Err("DIRECT_QUERY_EMPTY");
    }
    literal::validate_query(query, LITERAL).map_err(literal::LiteralError::code)
}

/// Encodes a canonical layout or a closed deterministic preparation gap.
/// Storage failures are never encoded as content outcomes.
pub fn encode_preparation(bytes: &[u8]) -> Result<Vec<u8>, &'static str> {
    if std::str::from_utf8(bytes).is_err() {
        return Ok(vec![1]);
    }
    let prepared = match materialize_utf8(bytes.to_vec(), MATERIALIZATION) {
        Ok(value) => value,
        Err(MaterializationError::BinaryContent) => return Ok(vec![2]),
        Err(MaterializationError::TooManyLines) => return Ok(vec![3]),
        Err(error) => return Err(error.code()),
    };
    let encoded = match UNITIZATION.encode_layout(
        prepared.text(),
        &line_spans(prepared.lines()),
        MAX_LAYOUT_BYTES,
    ) {
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

/// Searches verified bytes using the saved exact layout without rebuilding it.
pub fn scan_prepared(
    text: &str,
    encoded: &[u8],
    query: &str,
    ascii_insensitive: bool,
) -> Result<ScanResult, &'static str> {
    validate_query(query)?;
    if let Some(reason) = preparation_gap(encoded)? {
        return Err(reason);
    }
    let layout = encoded.get(1..).ok_or("DIRECT_PREPARATION_INVALID")?;
    let (lines, units) = UNITIZATION
        .decode_layout(text, layout, MAX_LAYOUT_BYTES)
        .map_err(UnitizationError::code)?;
    scan_layout(text, &lines, &units, query, ascii_insensitive, LITERAL)
}

/// Decodes only the outcome framing; source/layout validation remains mandatory.
pub const fn preparation_gap(encoded: &[u8]) -> Result<Option<&'static str>, &'static str> {
    match encoded {
        [0, layout @ ..] if !layout.is_empty() => Ok(None),
        [1] => Ok(Some("DIRECT_REVISION_NOT_UTF8")),
        [2] => Ok(Some("MATERIALIZATION_BINARY_CONTENT")),
        [3] => Ok(Some("MATERIALIZATION_TOO_MANY_LINES")),
        [4] => Ok(Some("UNITIZATION_TOO_MANY_UNITS")),
        [5] => Ok(Some("DIRECT_PREPARATION_LAYOUT_TOO_LARGE")),
        [6] => Ok(Some("DIRECT_REVISION_HAS_BOM")),
        _ => Err("DIRECT_PREPARATION_INVALID"),
    }
}

fn line_spans(lines: &[LineSpan]) -> Vec<SourceLineSpan> {
    lines
        .iter()
        .map(|line| SourceLineSpan {
            line_index: line.line_index,
            source_start: line.source_start,
            source_end: line.source_end,
            content_end: line.content_end,
        })
        .collect()
}

fn scan_layout(
    text: &str,
    lines: &[SourceLineSpan],
    units: &[UnitSpan],
    query: &str,
    ascii_insensitive: bool,
    limits: LiteralLimits,
) -> Result<ScanResult, &'static str> {
    let chunks = units
        .iter()
        .map(|unit| {
            text.get(unit.source_start..unit.source_end)
                .ok_or("DIRECT_PREPARATION_COORDINATE_INVALID")
        })
        .collect::<Result<Vec<_>, _>>()?;
    let result = literal::scan_chunks(&chunks, query, ascii_insensitive, limits)
        .map_err(literal::LiteralError::code)?;
    let complete = result.complete();
    let matches = result
        .matches
        .into_iter()
        .map(|range| {
            let start =
                u64::try_from(range.start).map_err(|_| "DIRECT_PREPARATION_COORDINATE_INVALID")?;
            let line = lines
                .get(lines.partition_point(|line| line.source_end <= start))
                .ok_or("DIRECT_PREPARATION_COORDINATE_INVALID")?;
            let line_start = usize::try_from(line.source_start)
                .map_err(|_| "DIRECT_PREPARATION_COORDINATE_INVALID")?;
            Ok(ScanMatch {
                byte_start: range.start,
                byte_end: range.end,
                line: usize::try_from(line.line_index)
                    .map_err(|_| "DIRECT_PREPARATION_COORDINATE_INVALID")?,
                column_bytes: range
                    .start
                    .checked_sub(line_start)
                    .ok_or("DIRECT_PREPARATION_COORDINATE_INVALID")?,
            })
        })
        .collect::<Result<Vec<_>, &'static str>>()?;
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
pub(super) fn prepare_and_scan(
    text: String,
    query: &str,
    insensitive: bool,
) -> Result<ScanResult, &'static str> {
    validate_query(query)?;
    scan_with_limits(
        text,
        query,
        insensitive,
        MATERIALIZATION,
        UNITIZATION,
        LITERAL,
    )
}

#[cfg(test)]
pub(super) fn scan_with_limits(
    text: String,
    query: &str,
    insensitive: bool,
    materialization: MaterializationLimits,
    unitization: UnitizationLimits,
    literal: LiteralLimits,
) -> Result<ScanResult, &'static str> {
    let prepared =
        materialize_utf8(text.into_bytes(), materialization).map_err(MaterializationError::code)?;
    let lines = line_spans(prepared.lines());
    let units =
        unitize_text(prepared.text(), &lines, unitization).map_err(UnitizationError::code)?;
    scan_layout(prepared.text(), &lines, &units, query, insensitive, literal)
}

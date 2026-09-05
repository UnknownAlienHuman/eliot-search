//! Bounded linear literal matching across an ordered sequence of UTF-8 chunks.
//!
//! Chunk boundaries are not match boundaries. This primitive does not assert
//! source integrity, currentness, authorization or completeness of a corpus.

use core::fmt;
use core::ops::Range;

/// Finite limits for one literal scan.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct LiteralLimits {
    /// Maximum query bytes.
    pub max_query_bytes: usize,
    /// Maximum total input bytes across every chunk.
    pub max_input_bytes: usize,
    /// Maximum chunk count, including empty chunks.
    pub max_chunks: usize,
    /// Maximum returned overlapping matches.
    pub max_matches: usize,
}

/// Closed content-free literal scan failure.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum LiteralError {
    /// A configured ceiling is zero.
    InvalidLimits,
    /// Query is empty.
    EmptyQuery,
    /// Query exceeds its byte ceiling.
    QueryTooLarge,
    /// Full input exceeds its byte ceiling or overflows.
    InputTooLarge,
    /// The number of input chunks exceeds its ceiling.
    TooManyChunks,
}

impl LiteralError {
    /// Stable machine-readable reason code.
    #[must_use]
    pub const fn code(self) -> &'static str {
        match self {
            Self::InvalidLimits => "LITERAL_INVALID_LIMITS",
            Self::EmptyQuery => "LITERAL_QUERY_EMPTY",
            Self::QueryTooLarge => "LITERAL_QUERY_TOO_LARGE",
            Self::InputTooLarge => "LITERAL_INPUT_TOO_LARGE",
            Self::TooManyChunks => "LITERAL_CHUNK_LIMIT",
        }
    }
}
impl fmt::Display for LiteralError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(self.code())
    }
}
impl std::error::Error for LiteralError {}

/// Byte ranges and coverage of exactly the supplied input, not a corpus proof.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct LiteralScan {
    /// Ordered overlapping byte ranges in the concatenated input.
    pub matches: Vec<Range<usize>>,
    /// Total bytes across all supplied chunks, validated before scanning.
    pub input_bytes: usize,
    /// Bytes consumed before completion or match truncation.
    pub scanned_bytes: usize,
    /// True only after finding an additional match beyond the output ceiling.
    pub match_limit_reached: bool,
}

impl LiteralScan {
    /// Whether this supplied byte sequence was exhausted without truncation.
    /// This does not establish an authoritative corpus denominator.
    #[must_use]
    pub const fn complete(&self) -> bool {
        !self.match_limit_reached && self.scanned_bytes == self.input_bytes
    }
}

/// Matches one non-empty literal in linear time across exact ordered chunks.
///
/// KMP state is retained across every boundary, including overlapping matches.
/// ASCII folding never changes non-ASCII bytes. Input and chunk limits are
/// checked before scanning, so early match truncation cannot hide oversized input.
/// Auxiliary memory is O(query bytes + returned matches); source text is borrowed.
/// No regex, normalization, I/O, caller authorization or evidence issuance occurs.
///
/// # Errors
/// Returns a closed validation error for empty queries or exceeded finite limits.
pub fn scan_chunks(
    chunks: &[&str],
    query: &str,
    ascii_insensitive: bool,
    limits: LiteralLimits,
) -> Result<LiteralScan, LiteralError> {
    validate_query(query, limits)?;
    if chunks.len() > limits.max_chunks {
        return Err(LiteralError::TooManyChunks);
    }
    let input_bytes = chunks.iter().try_fold(0_usize, |total, chunk| {
        total.checked_add(chunk.len()).filter(|sum| *sum <= limits.max_input_bytes)
            .ok_or(LiteralError::InputTooLarge)
    })?;
    let mut result = LiteralScan {
        matches: Vec::new(), input_bytes, scanned_bytes: 0, match_limit_reached: false,
    };
    let needle = query.bytes().map(|byte| fold(byte, ascii_insensitive)).collect::<Vec<_>>();
    let prefix = prefix_table(&needle);
    let mut matched = 0;
    for chunk in chunks {
        for byte in chunk.bytes() {
            let byte = fold(byte, ascii_insensitive);
            while matched > 0 && needle[matched] != byte {
                matched = prefix[matched - 1];
            }
            if needle[matched] == byte { matched += 1; }
            result.scanned_bytes += 1;
            if matched == needle.len() {
                if result.matches.len() == limits.max_matches {
                    result.match_limit_reached = true;
                    return Ok(result);
                }
                result.matches.push(result.scanned_bytes - needle.len()..result.scanned_bytes);
                matched = prefix[matched - 1];
            }
        }
    }
    Ok(result)
}

/// Checks query and limits even when a caller has no source items to scan.
///
/// # Errors
/// Returns an error when limits are zero or query bytes are empty/over-limit.
pub const fn validate_query(query: &str, limits: LiteralLimits) -> Result<(), LiteralError> {
    if limits.max_query_bytes == 0 || limits.max_input_bytes == 0
        || limits.max_chunks == 0 || limits.max_matches == 0
    {
        return Err(LiteralError::InvalidLimits);
    }
    if query.is_empty() { return Err(LiteralError::EmptyQuery); }
    if query.len() > limits.max_query_bytes { return Err(LiteralError::QueryTooLarge); }
    Ok(())
}

const fn fold(byte: u8, ascii_insensitive: bool) -> u8 {
    if ascii_insensitive { byte.to_ascii_lowercase() } else { byte }
}

fn prefix_table(needle: &[u8]) -> Vec<usize> {
    let mut prefix = vec![0; needle.len()];
    let mut matched = 0;
    for index in 1..needle.len() {
        while matched > 0 && needle[index] != needle[matched] {
            matched = prefix[matched - 1];
        }
        if needle[index] == needle[matched] { matched += 1; }
        prefix[index] = matched;
    }
    prefix
}

#[cfg(test)]
mod tests {
    use super::*;
    const LIMITS: LiteralLimits = LiteralLimits {
        max_query_bytes: 64, max_input_bytes: 4096, max_chunks: 128, max_matches: 100,
    };

    #[test]
    fn crossing_many_boundaries_and_overlaps_are_preserved() {
        let result = scan_chunks(&["a", "b", "a", "b", "a"], "aba", false, LIMITS).unwrap();
        assert_eq!(result.matches, vec![0..3, 2..5]);
        assert!(result.complete());
    }

    #[test]
    fn ascii_folding_does_not_fold_unicode() {
        let result = scan_chunks(&["Αα", "HEL", "Lo"], "hello", true, LIMITS).unwrap();
        assert_eq!(result.matches, vec![4..9]);
        assert_eq!(scan_chunks(&["Α"], "α", true, LIMITS).unwrap().matches.len(), 0);
    }

    #[test]
    fn exactly_at_output_limit_is_complete_but_one_more_is_not() {
        let limits = LiteralLimits { max_matches: 2, ..LIMITS };
        let exact = scan_chunks(&["aaa"], "aa", false, limits).unwrap();
        assert_eq!(exact.matches, vec![0..2, 1..3]);
        assert!(exact.complete());
        let truncated = scan_chunks(&["aaaa"], "aa", false, limits).unwrap();
        assert_eq!(truncated.matches, exact.matches);
        assert!(truncated.match_limit_reached);
        assert!(!truncated.complete());
    }

    #[test]
    fn empty_sources_still_validate_query() {
        assert_eq!(scan_chunks(&[], "", false, LIMITS), Err(LiteralError::EmptyQuery));
        assert!(scan_chunks(&[], "x", false, LIMITS).unwrap().complete());
    }

    #[test]
    fn all_input_limits_are_checked_before_early_truncation() {
        let limits = LiteralLimits { max_matches: 1, max_input_bytes: 4, ..LIMITS };
        assert_eq!(scan_chunks(&["aaa", "long tail"], "a", false, limits), Err(LiteralError::InputTooLarge));
        let limits = LiteralLimits { max_chunks: 1, ..LIMITS };
        assert_eq!(scan_chunks(&["", ""], "a", false, limits), Err(LiteralError::TooManyChunks));
    }

    #[test]
    fn result_is_independent_of_every_utf8_split() {
        for text in ["ababa", "α\r\nβγβγ", "𐀀a𐀀", "AaA\r\na"] {
            for query in ["a", "aba", "βγ", "𐀀", "\r\n", "not found"] {
                for insensitive in [false, true] {
                    let whole = scan_chunks(&[text], query, insensitive, LIMITS).unwrap();
                    for split in (0..=text.len()).filter(|index| text.is_char_boundary(*index)) {
                        let chunked = scan_chunks(&[&text[..split], "", &text[split..]], query, insensitive, LIMITS).unwrap();
                        assert_eq!(chunked, whole);
                    }
                }
            }
        }
    }

    #[test]
    fn deterministic_small_corpus_matches_simple_reference() {
        for length in 0..=9 {
            for bits in 0..(1_usize << length) {
                let text = (0..length).map(|index| if bits & (1 << index) == 0 { 'a' } else { 'b' }).collect::<String>();
                for query in ["a", "b", "aa", "aba", "bab", "bbb"] {
                    let expected = text.as_bytes().windows(query.len()).enumerate()
                        .filter(|(_, bytes)| *bytes == query.as_bytes())
                        .map(|(index, _)| index..index + query.len()).collect::<Vec<_>>();
                    let split = text.len() / 2;
                    let result = scan_chunks(&[&text[..split], &text[split..]], query, false, LIMITS).unwrap();
                    assert_eq!(result.matches, expected);
                    assert!(result.complete());
                }
            }
        }
    }
}

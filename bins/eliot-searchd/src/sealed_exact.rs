//! Deterministic bounded literal search over authenticated UTF-8 bytes.

use core::fmt;

/// Maximum literal query size.
pub const MAX_QUERY_BYTES: usize = 64 * 1024;
/// Maximum emitted exact matches.
pub const MAX_MATCHES: usize = 100_000;

/// Closed exact-search failure.
#[derive(Clone, Copy, Debug, Eq, Ord, PartialEq, PartialOrd)]
pub enum SealedExactError {
    /// Authenticated plaintext is not valid UTF-8.
    InvalidUtf8,
    /// Query is empty.
    EmptyQuery,
    /// Query exceeds the finite byte ceiling.
    QueryTooLarge,
}

impl SealedExactError {
    /// Stable machine-readable reason code.
    #[must_use]
    pub const fn code(self) -> &'static str {
        match self {
            Self::InvalidUtf8 => "SEALED_EXACT_INPUT_INVALID_UTF8",
            Self::EmptyQuery => "SEALED_EXACT_QUERY_EMPTY",
            Self::QueryTooLarge => "SEALED_EXACT_QUERY_TOO_LARGE",
        }
    }
}

impl fmt::Display for SealedExactError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(self.code())
    }
}

impl std::error::Error for SealedExactError {}

/// One exact match in UTF-8 byte coordinates.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct ExactMatch {
    /// Inclusive byte start.
    pub byte_start: usize,
    /// Exclusive byte end.
    pub byte_end: usize,
    /// Zero-based logical line number.
    pub line: usize,
    /// Zero-based byte column within the line.
    pub column_bytes: usize,
}

/// Ordered exact-search result and explicit coverage.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ExactSearchResult {
    /// Matches in increasing byte order.
    pub matches: Vec<ExactMatch>,
    /// Exact input byte count.
    pub input_bytes: usize,
    /// Whether every possible UTF-8 match start was evaluated.
    pub complete: bool,
    /// Whether the finite match ceiling stopped the scan.
    pub match_limit_reached: bool,
}

/// Searches authenticated bytes with literal or ASCII-insensitive comparison.
pub fn scan_exact(
    input: &[u8],
    query: &str,
    ascii_insensitive: bool,
) -> Result<ExactSearchResult, SealedExactError> {
    if query.is_empty() {
        return Err(SealedExactError::EmptyQuery);
    }
    if query.len() > MAX_QUERY_BYTES {
        return Err(SealedExactError::QueryTooLarge);
    }
    let text = core::str::from_utf8(input).map_err(|_| SealedExactError::InvalidUtf8)?;
    if query.len() > text.len() {
        return Ok(ExactSearchResult {
            matches: Vec::new(),
            input_bytes: text.len(),
            complete: true,
            match_limit_reached: false,
        });
    }

    let text_bytes = text.as_bytes();
    let query_bytes = query.as_bytes();
    let mut line_starts = vec![0_usize];
    for (index, byte) in text_bytes.iter().copied().enumerate() {
        if byte == b'\n' {
            line_starts.push(index.saturating_add(1));
        }
    }

    let mut matches = Vec::new();
    let mut truncated = false;
    for start in 0..=text_bytes.len() - query_bytes.len() {
        if !text.is_char_boundary(start) {
            continue;
        }
        let end = start + query_bytes.len();
        if !text.is_char_boundary(end) {
            continue;
        }
        let equal = if ascii_insensitive {
            text_bytes[start..end].eq_ignore_ascii_case(query_bytes)
        } else {
            &text_bytes[start..end] == query_bytes
        };
        if !equal {
            continue;
        }
        if matches.len() == MAX_MATCHES {
            truncated = true;
            break;
        }
        let line = line_starts
            .partition_point(|line_start| *line_start <= start)
            .saturating_sub(1);
        matches.push(ExactMatch {
            byte_start: start,
            byte_end: end,
            line,
            column_bytes: start - line_starts[line],
        });
    }

    Ok(ExactSearchResult {
        matches,
        input_bytes: text.len(),
        complete: !truncated,
        match_limit_reached: truncated,
    })
}

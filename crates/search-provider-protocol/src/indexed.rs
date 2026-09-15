//! Canonical provider framing for an indexed lexical query.
//!
//! The provider operation registry historically exposed one opaque `query`
//! blob. Indexed retrieval must remain distinguishable from DIRECT query
//! bytes before capability admission, without permitting arbitrary user text
//! to impersonate another mode. The CLI therefore places this NUL-delimited
//! marker immediately after the strict `s:` query mode prefix. Ordinary
//! command-line UTF-8 text cannot contain NUL, while the protocol still
//! validates the marker explicitly instead of inferring intent from content.

/// Strict query-mode prefix emitted by the canonical CLI query encoder.
pub const STRICT_QUERY_PREFIX: &[u8] = b"s:";
/// Versioned, domain-specific indexed-query marker carried inside `op query`.
pub const INDEXED_QUERY_MARKER: &[u8] = b"\0eliot-indexed-query-v1\0";

/// Closed failure while recognizing a marked indexed-query payload.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum IndexedQueryFrameError {
    /// The indexed marker was present but no query bytes followed it.
    EmptyQuery,
}

impl IndexedQueryFrameError {
    /// Stable machine-readable reason code.
    #[must_use]
    pub const fn code(self) -> &'static str {
        match self {
            Self::EmptyQuery => "PROVIDER_INDEXED_QUERY_INVALID",
        }
    }
}

/// Builds the opaque query bytes consumed by the existing strict query
/// encoder. The encoder itself will prepend [`STRICT_QUERY_PREFIX`].
///
/// This function performs only mode framing. The caller retains ownership of
/// the finite request-size budget and rejects an empty query here.
pub fn encode_indexed_query(query: &[u8]) -> Result<Vec<u8>, IndexedQueryFrameError> {
    if query.is_empty() {
        return Err(IndexedQueryFrameError::EmptyQuery);
    }
    let mut payload = Vec::with_capacity(INDEXED_QUERY_MARKER.len().saturating_add(query.len()));
    payload.extend_from_slice(INDEXED_QUERY_MARKER);
    payload.extend_from_slice(query);
    Ok(payload)
}

/// Recognizes an indexed query inside the complete decoded `op query` blob.
///
/// Returns `Ok(None)` for an ordinary DIRECT query. Once the exact marker is
/// present, an empty tail is a typed malformed indexed request rather than an
/// ordinary query or an empty success.
pub fn decode_indexed_query(
    blob: &[u8],
) -> Result<Option<&[u8]>, IndexedQueryFrameError> {
    let Some(strict) = blob.strip_prefix(STRICT_QUERY_PREFIX) else {
        return Ok(None);
    };
    let Some(query) = strict.strip_prefix(INDEXED_QUERY_MARKER) else {
        return Ok(None);
    };
    if query.is_empty() {
        Err(IndexedQueryFrameError::EmptyQuery)
    } else {
        Ok(Some(query))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn indexed_marker_round_trips_and_cannot_be_empty() {
        let payload = encode_indexed_query(b"needle").expect("valid query");
        let mut blob = STRICT_QUERY_PREFIX.to_vec();
        blob.extend_from_slice(&payload);
        assert_eq!(decode_indexed_query(&blob), Ok(Some(&b"needle"[..])));

        let mut empty = STRICT_QUERY_PREFIX.to_vec();
        empty.extend_from_slice(INDEXED_QUERY_MARKER);
        assert_eq!(
            decode_indexed_query(&empty),
            Err(IndexedQueryFrameError::EmptyQuery)
        );
        assert_eq!(decode_indexed_query(b"s:ordinary"), Ok(None));
        assert_eq!(decode_indexed_query(b"i:ordinary"), Ok(None));
    }
}

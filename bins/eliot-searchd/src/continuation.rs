//! Bounded process-local continuation windows for DIRECT search.
//!
//! Tokens are opaque session locators, not authorization credentials. Every
//! continuation is bound to a deterministic digest of current source state and
//! is invalidated on any source mutation, fence mismatch, expiration, or
//! process exit. No backend cursor, query, path, or source bytes are encoded in
//! the token.

use std::collections::BTreeMap;
use std::time::{Duration, Instant, SystemTime, UNIX_EPOCH};

use crate::direct_store::{DirectStore, StoreGap, StoreSearchResult, StoredMatch};
use crate::sha256;

/// Maximum simultaneous process-local continuation windows.
pub(crate) const MAX_CONTINUATIONS: usize = 16;
/// Maximum matches retained across all continuation windows.
pub(crate) const MAX_RETAINED_MATCHES: usize = 25_000;
/// Maximum matches returned by one page.
pub(crate) const MAX_PAGE_SIZE: usize = 1_000;
/// Default matches returned by one page.
pub(crate) const DEFAULT_PAGE_SIZE: usize = 100;
/// Maximum source-gap details returned on the first page.
pub(crate) const MAX_GAP_DETAILS: usize = 256;
/// Finite process-local continuation lifetime.
pub(crate) const CONTINUATION_TTL: Duration = Duration::from_secs(15 * 60);

/// Closed continuation-window failure.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum ContinuationError {
    InvalidPageSize,
    NotFound,
    Expired,
    SourceFenceChanged,
    CapacityExceeded,
    TokenExhausted,
}

impl ContinuationError {
    pub(crate) const fn code(self) -> &'static str {
        match self {
            Self::InvalidPageSize => "DIRECT_CONTINUATION_PAGE_SIZE_INVALID",
            Self::NotFound => "DIRECT_CONTINUATION_NOT_FOUND",
            Self::Expired => "DIRECT_CONTINUATION_EXPIRED",
            Self::SourceFenceChanged => "DIRECT_CONTINUATION_SOURCE_FENCE_CHANGED",
            Self::CapacityExceeded => "DIRECT_CONTINUATION_CAPACITY_EXCEEDED",
            Self::TokenExhausted => "DIRECT_CONTINUATION_TOKEN_EXHAUSTED",
        }
    }
}

/// Immutable coverage carried by every page from one search execution.
#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) struct PageCoverage {
    pub(crate) registered_sources: usize,
    pub(crate) active_sources: usize,
    pub(crate) searched_sources: usize,
    pub(crate) searched_bytes: u64,
    pub(crate) corpus_complete: bool,
    pub(crate) match_limit_reached: bool,
    pub(crate) source_budget_exhausted: bool,
    pub(crate) byte_budget_exhausted: bool,
    pub(crate) total_matches: usize,
    pub(crate) retained_matches: usize,
    pub(crate) candidate_window_truncated: bool,
    pub(crate) gap_count: usize,
    pub(crate) gap_details_truncated: bool,
}

impl PageCoverage {
    #[must_use]
    pub(crate) const fn complete(&self) -> bool {
        self.corpus_complete && !self.candidate_window_truncated
    }
}

/// One page of deterministic source-backed matches.
#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) struct SearchPage {
    pub(crate) matches: Vec<StoredMatch>,
    pub(crate) gaps: Vec<StoreGap>,
    pub(crate) coverage: PageCoverage,
    pub(crate) page_start: usize,
    pub(crate) page_end: usize,
    pub(crate) exhausted: bool,
    pub(crate) continuation_token: Option<String>,
    pub(crate) expires_in_ms: Option<u64>,
}

#[derive(Clone, Debug)]
struct ContinuationRecord {
    source_fence_digest: String,
    matches: Vec<StoredMatch>,
    next_index: usize,
    coverage: PageCoverage,
    expires_at: Instant,
}

/// Finite process-local continuation catalog for one owner-fenced service.
#[derive(Debug)]
pub(crate) struct ContinuationCatalog {
    session_nonce: [u8; 32],
    next_counter: u64,
    records: BTreeMap<String, ContinuationRecord>,
    retained_matches: usize,
}

impl ContinuationCatalog {
    /// Creates one session-local catalog.
    pub(crate) fn new(namespace_id: &str) -> Self {
        let now = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap_or_default()
            .as_nanos();
        let session_nonce = sha256::digest_parts(
            b"eliot-search/direct-continuation-session/v1",
            &[
                namespace_id.as_bytes(),
                &u64::from(std::process::id()).to_be_bytes(),
                &now.to_be_bytes(),
            ],
        );
        Self {
            session_nonce,
            next_counter: 1,
            records: BTreeMap::new(),
            retained_matches: 0,
        }
    }

    /// Number of live continuation windows after bounded expiry cleanup.
    pub(crate) fn live_count(&mut self) -> usize {
        self.expire();
        self.records.len()
    }

    /// Number of matches retained by live windows.
    pub(crate) fn retained_matches(&mut self) -> usize {
        self.expire();
        self.retained_matches
    }

    /// Invalidates every window after source-state mutation.
    pub(crate) fn invalidate_all(&mut self) -> usize {
        let invalidated = self.records.len();
        self.records.clear();
        self.retained_matches = 0;
        invalidated
    }

    /// Creates the first page and an opaque continuation when more retained
    /// matches remain.
    pub(crate) fn create_page(
        &mut self,
        store: &DirectStore,
        mut result: StoreSearchResult,
        page_size: usize,
    ) -> Result<SearchPage, ContinuationError> {
        validate_page_size(page_size)?;
        self.expire();

        let total_matches = result.matches.len();
        let available_capacity = MAX_RETAINED_MATCHES.saturating_sub(self.retained_matches);
        let retained_limit = total_matches.min(available_capacity);
        let candidate_window_truncated = retained_limit < total_matches;
        result.matches.truncate(retained_limit);
        let retained_matches = result.matches.len();
        let gap_count = result.gaps.len();
        let gap_details_truncated = gap_count > MAX_GAP_DETAILS;
        result.gaps.truncate(MAX_GAP_DETAILS);

        let coverage = PageCoverage {
            registered_sources: result.registered_sources,
            active_sources: result.active_sources,
            searched_sources: result.searched_sources,
            searched_bytes: result.searched_bytes,
            corpus_complete: result.complete,
            match_limit_reached: result.match_limit_reached,
            source_budget_exhausted: result.source_budget_exhausted,
            byte_budget_exhausted: result.byte_budget_exhausted,
            total_matches,
            retained_matches,
            candidate_window_truncated,
            gap_count,
            gap_details_truncated,
        };

        let page_end = retained_matches.min(page_size);
        let page_matches = result.matches[..page_end].to_vec();
        if page_end == retained_matches {
            return Ok(SearchPage {
                matches: page_matches,
                gaps: result.gaps,
                coverage,
                page_start: 0,
                page_end,
                exhausted: true,
                continuation_token: None,
                expires_in_ms: None,
            });
        }
        if self.records.len() >= MAX_CONTINUATIONS {
            return Err(ContinuationError::CapacityExceeded);
        }

        let token = self.allocate_token()?;
        let expires_at = Instant::now() + CONTINUATION_TTL;
        let record = ContinuationRecord {
            source_fence_digest: source_fence(store),
            matches: result.matches,
            next_index: page_end,
            coverage: coverage.clone(),
            expires_at,
        };
        self.retained_matches = self
            .retained_matches
            .saturating_add(record.matches.len());
        self.records.insert(token.clone(), record);
        Ok(SearchPage {
            matches: page_matches,
            gaps: result.gaps,
            coverage,
            page_start: 0,
            page_end,
            exhausted: false,
            continuation_token: Some(token),
            expires_in_ms: Some(
                u64::try_from(CONTINUATION_TTL.as_millis()).unwrap_or(u64::MAX),
            ),
        })
    }

    /// Advances one exact window after source-fence and TTL revalidation.
    pub(crate) fn continue_page(
        &mut self,
        store: &DirectStore,
        token: &str,
        page_size: usize,
    ) -> Result<SearchPage, ContinuationError> {
        validate_page_size(page_size)?;
        self.expire();
        let mut record = self
            .records
            .remove(token)
            .ok_or(ContinuationError::NotFound)?;
        self.retained_matches = self.retained_matches.saturating_sub(record.matches.len());
        if Instant::now() >= record.expires_at {
            return Err(ContinuationError::Expired);
        }
        if record.source_fence_digest != source_fence(store) {
            return Err(ContinuationError::SourceFenceChanged);
        }

        let page_start = record.next_index;
        let page_end = page_start
            .saturating_add(page_size)
            .min(record.matches.len());
        let page_matches = record.matches[page_start..page_end].to_vec();
        record.next_index = page_end;
        let exhausted = page_end == record.matches.len();
        let expires_in_ms = if exhausted {
            None
        } else {
            let remaining = record.expires_at.saturating_duration_since(Instant::now());
            Some(u64::try_from(remaining.as_millis()).unwrap_or(u64::MAX))
        };
        if !exhausted {
            self.retained_matches = self
                .retained_matches
                .saturating_add(record.matches.len());
            self.records.insert(token.to_owned(), record.clone());
        }
        Ok(SearchPage {
            matches: page_matches,
            gaps: Vec::new(),
            coverage: record.coverage,
            page_start,
            page_end,
            exhausted,
            continuation_token: (!exhausted).then(|| token.to_owned()),
            expires_in_ms,
        })
    }

    fn expire(&mut self) {
        let now = Instant::now();
        let expired = self
            .records
            .iter()
            .filter(|(_, record)| record.expires_at <= now)
            .map(|(token, _)| token.clone())
            .collect::<Vec<_>>();
        for token in expired {
            if let Some(record) = self.records.remove(&token) {
                self.retained_matches = self.retained_matches.saturating_sub(record.matches.len());
            }
        }
    }

    fn allocate_token(&mut self) -> Result<String, ContinuationError> {
        for _ in 0..MAX_CONTINUATIONS.saturating_mul(2) {
            let counter = self.next_counter;
            self.next_counter = self
                .next_counter
                .checked_add(1)
                .ok_or(ContinuationError::TokenExhausted)?;
            let token = sha256::hex(&sha256::digest_parts(
                b"eliot-search/direct-continuation-token/v1",
                &[&self.session_nonce, &counter.to_be_bytes()],
            ));
            if !self.records.contains_key(&token) {
                return Ok(token);
            }
        }
        Err(ContinuationError::TokenExhausted)
    }
}

fn validate_page_size(page_size: usize) -> Result<(), ContinuationError> {
    if page_size == 0 || page_size > MAX_PAGE_SIZE {
        Err(ContinuationError::InvalidPageSize)
    } else {
        Ok(())
    }
}

fn source_fence(store: &DirectStore) -> String {
    let namespace = store.namespace_id();
    let sources = store.list_sources();
    let mut encoded = Vec::new();
    append(&mut encoded, namespace.as_bytes());
    for source in sources {
        append(&mut encoded, source.source_id.as_bytes());
        append(&mut encoded, source.revision_id.as_bytes());
        append(&mut encoded, source.content_digest.as_bytes());
        append(&mut encoded, source.path_digest.as_bytes());
        encoded.extend_from_slice(&source.byte_length.to_be_bytes());
        encoded.push(u8::from(source.active));
        encoded.extend_from_slice(&source.sequence.to_be_bytes());
    }
    sha256::hex(&sha256::digest_parts(
        b"eliot-search/direct-source-fence/v1",
        &[&encoded],
    ))
}

fn append(output: &mut Vec<u8>, value: &[u8]) {
    output.extend_from_slice(
        &u64::try_from(value.len())
            .unwrap_or(u64::MAX)
            .to_be_bytes(),
    );
    output.extend_from_slice(value);
}

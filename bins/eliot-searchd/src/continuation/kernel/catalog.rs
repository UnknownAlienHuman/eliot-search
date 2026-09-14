//! Finite session-local continuation catalog.

use core::fmt;
use std::collections::BTreeMap;
use std::time::Instant;

use crate::direct_store::{DirectStore, StoreSearchResult, StoredMatch};
use crate::sha256;
use crate::source_fence::digest as source_fence;

use super::entropy::qualified_entropy_32;
use super::model::{
    LiveExpansionBarrier, PageCompletion, PageCoverage, PageTruncation,
    SearchPage,
};
use super::spec::{
    CONTINUATION_TTL, ContinuationError, MAX_CONTINUATIONS, MAX_GAP_DETAILS,
    MAX_RETAINED_MATCHES, validate_page_size,
};

#[derive(Clone, Debug)]
struct ContinuationRecord {
    namespace_id: String,
    session_tag: u64,
    source_fence_digest: String,
    matches: Vec<StoredMatch>,
    next_index: usize,
    coverage: PageCoverage,
    expires_at: Instant,
}

/// Finite live-authorized continuation catalog for one owner-fenced session.
///
/// Debug output carries counts and the namespace only: session tags and token
/// plaintext never reach logs or receipts.
pub struct ContinuationCatalog {
    namespace_id: String,
    session_tag: u64,
    entropy_poisoned: bool,
    records: BTreeMap<String, ContinuationRecord>,
    retained_matches: usize,
}

impl fmt::Debug for ContinuationCatalog {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("ContinuationCatalog")
            .field("namespace_id", &self.namespace_id)
            .field("session_tag", &"<redacted>")
            .field("entropy_poisoned", &self.entropy_poisoned)
            .field("records", &self.records.len())
            .field("live_windows", &self.records.len())
            .field("retained_matches", &self.retained_matches)
            .finish()
    }
}

impl ContinuationCatalog {
    /// Creates one session-local catalog bound to an exact namespace.
    ///
    /// The session tag comes from qualified OS entropy. Entropy failure poisons
    /// the catalog so every mint and expansion fails closed.
    pub(crate) fn new(namespace_id: &str) -> Self {
        let (session_tag, entropy_poisoned) = qualified_entropy_32().map_or_else(
            |_| (0, true),
            |material| {
                let mut tag = [0_u8; 8];
                tag.copy_from_slice(&material[..8]);
                (u64::from_le_bytes(tag), false)
            },
        );
        Self {
            namespace_id: namespace_id.to_owned(),
            session_tag,
            entropy_poisoned,
            records: BTreeMap::new(),
            retained_matches: 0,
        }
    }

    /// Namespace this catalog is bound to; foreign stores are rejected.
    #[must_use]
    pub(crate) fn namespace_id(&self) -> &str {
        &self.namespace_id
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

    /// Creates the first page and an opaque continuation when retained matches
    /// remain. An exhausted single page inserts no record.
    pub(crate) fn create_page(
        &mut self,
        store: &DirectStore,
        mut result: StoreSearchResult,
        page_size: usize,
    ) -> Result<SearchPage, ContinuationError> {
        validate_page_size(page_size)?;
        if self.entropy_poisoned {
            return Err(ContinuationError::EntropyUnavailable);
        }
        if store.namespace_id() != self.namespace_id() {
            return Err(ContinuationError::SourceFenceChanged);
        }
        self.expire();

        let total_matches = result.matches.len();
        let available_capacity =
            MAX_RETAINED_MATCHES.saturating_sub(self.retained_matches);
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
            completion: PageCompletion {
                corpus_complete: result.complete,
                match_limit_reached: result.match_limit_reached,
            },
            total_matches,
            retained_matches,
            gap_count,
            truncation: PageTruncation {
                candidate_window_truncated,
                gap_details_truncated,
            },
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
        let record = ContinuationRecord {
            namespace_id: self.namespace_id().to_owned(),
            session_tag: self.session_tag,
            source_fence_digest: source_fence(store),
            matches: result.matches,
            next_index: page_end,
            coverage: coverage.clone(),
            expires_at: Instant::now() + CONTINUATION_TTL,
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
                u64::try_from(CONTINUATION_TTL.as_millis())
                    .unwrap_or(u64::MAX),
            ),
        })
    }

    /// Advances one exact window after a clean live-authority checkpoint.
    pub(crate) fn continue_page(
        &mut self,
        store: &DirectStore,
        token: &str,
        page_size: usize,
    ) -> Result<SearchPage, ContinuationError> {
        self.continue_page_with_live_barrier(
            store,
            token,
            page_size,
            LiveExpansionBarrier::clean(),
        )
    }

    /// Advances one window after session, live authority, TTL and exact source
    /// fence revalidation. Any denial drops the whole retained ranking.
    pub(crate) fn continue_page_with_live_barrier(
        &mut self,
        store: &DirectStore,
        token: &str,
        page_size: usize,
        barrier: LiveExpansionBarrier,
    ) -> Result<SearchPage, ContinuationError> {
        validate_page_size(page_size)?;
        if self.entropy_poisoned {
            return Err(ContinuationError::EntropyUnavailable);
        }
        let tag_matches = self
            .records
            .get(token)
            .is_some_and(|record| record.session_tag == self.session_tag);
        if !tag_matches {
            return Err(ContinuationError::NotFound);
        }
        if let Some(denial) = barrier.denial() {
            self.drop_window(token);
            self.expire();
            return Err(denial);
        }

        let record = self
            .records
            .get(token)
            .cloned()
            .ok_or(ContinuationError::NotFound)?;
        if Instant::now() >= record.expires_at {
            self.drop_window(token);
            self.expire();
            return Err(ContinuationError::Expired);
        }
        if record.namespace_id != store.namespace_id()
            || record.source_fence_digest != source_fence(store)
        {
            self.drop_window(token);
            self.expire();
            return Err(ContinuationError::SourceFenceChanged);
        }

        self.records.remove(token);
        self.retained_matches = self
            .retained_matches
            .saturating_sub(record.matches.len());
        let mut record = record;
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
            let remaining = record
                .expires_at
                .saturating_duration_since(Instant::now());
            Some(u64::try_from(remaining.as_millis()).unwrap_or(u64::MAX))
        };
        if !exhausted {
            self.retained_matches = self
                .retained_matches
                .saturating_add(record.matches.len());
            self.records.insert(token.to_owned(), record.clone());
        }
        self.expire();
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

    /// Marks one window immediately expired for unit tests.
    #[cfg(test)]
    pub(crate) fn force_expire_for_tests(&mut self, token: &str) -> bool {
        match self.records.get_mut(token) {
            Some(record) => {
                record.expires_at = Instant::now();
                true
            }
            None => false,
        }
    }

    fn drop_window(&mut self, token: &str) {
        if let Some(record) = self.records.remove(token) {
            self.retained_matches = self
                .retained_matches
                .saturating_sub(record.matches.len());
        }
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
            self.drop_window(&token);
        }
    }

    fn allocate_token(&self) -> Result<String, ContinuationError> {
        for _ in 0..MAX_CONTINUATIONS.saturating_mul(2) {
            let material = qualified_entropy_32()
                .map_err(|_| ContinuationError::EntropyUnavailable)?;
            let token = sha256::hex(&material);
            if !self.records.contains_key(&token) {
                return Ok(token);
            }
        }
        Err(ContinuationError::TokenExhausted)
    }
}

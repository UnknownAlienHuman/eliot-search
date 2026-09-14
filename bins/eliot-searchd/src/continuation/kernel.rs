//! Bounded live-authorized continuation windows for DIRECT search.
//!
//! Tokens are opaque qualified-entropy locators bound to one namespace and one
//! session, not authorization credentials and not deterministic hashes. Every
//! continuation record carries its namespace binding, session tag, exact source
//! fence and finite TTL; every expansion revalidates liveness before returning
//! a page, and possession alone admits nothing (invariant 13).
//!
//! Windows are ephemeral session memory only: ordinary queries persist no
//! history (an exhausted single page inserts no record), pins are bounded and
//! released on exhaustion, expiry, fence drift, live-barrier denial,
//! invalidation or session close, and tokens never survive a restart. Durable
//! replan checkpoints are explicitly out of scope for this catalog; the
//! canonical `search-continuation` owner carries durable state for the indexed
//! path.
//!
//! The crate-local [`LiveExpansionBarrier`] is the T21 live-reauthorization
//! checkpoint shared with result handles. The indexed query path enforces the
//! same checkpoint through `access_composition::recheck_before_expansion`
//! (T20); this catalog enforces it over the DIRECT source fence so a moved
//! generation, a revocation or a purge drops the whole retained ranking
//! instead of filtering one hit.

#![allow(unsafe_code)]

#[cfg(windows)]
use core::ffi::c_void;
use core::fmt;
use std::collections::BTreeMap;
use std::time::{Duration, Instant};

use crate::direct_store::{DirectStore, StoreGap, StoreSearchResult, StoredMatch};
use crate::sha256;
use crate::source_fence::digest as source_fence;

/// Maximum simultaneous process-local continuation windows.
pub const MAX_CONTINUATIONS: usize = 16;
/// Maximum matches retained across all continuation windows.
pub const MAX_RETAINED_MATCHES: usize = 25_000;
/// Maximum matches returned by one page.
pub const MAX_PAGE_SIZE: usize = 1_000;
/// Default matches returned by one page.
pub const DEFAULT_PAGE_SIZE: usize = 100;
/// Maximum source-gap details retained on the first page.
pub const MAX_GAP_DETAILS: usize = 256;
/// Finite process-local continuation lifetime.
pub const CONTINUATION_TTL: Duration = Duration::from_mins(15);

/// Closed continuation-window failure.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ContinuationError {
    InvalidPageSize,
    NotFound,
    Expired,
    SourceFenceChanged,
    CapacityExceeded,
    TokenExhausted,
    EntropyUnavailable,
    AccessRevoked,
    Purged,
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
            Self::EntropyUnavailable => "DIRECT_CONTINUATION_ENTROPY_UNAVAILABLE",
            Self::AccessRevoked => "DIRECT_CONTINUATION_ACCESS_REVOKED",
            Self::Purged => "DIRECT_CONTINUATION_PURGED",
        }
    }
}

/// Stable code when qualified OS entropy cannot be read.
pub const QUALIFIED_ENTROPY_UNAVAILABLE: &str = "DIRECT_QUALIFIED_ENTROPY_UNAVAILABLE";

/// Reads 32 bytes of qualified OS entropy for opaque token material.
///
/// Unix reads the kernel CSPRNG; Windows uses `BCryptGenRandom` with the
/// system-preferred RNG, mirroring `revision_protection_windows.rs`. Any other
/// platform fails closed: deterministic process data (PID, wall clock) is
/// never substituted, so token minting fails instead of minting guessable
/// locators.
///
/// # Errors
///
/// Returns [`QUALIFIED_ENTROPY_UNAVAILABLE`] when the OS source cannot be read.
pub fn qualified_entropy_32() -> Result<[u8; 32], &'static str> {
    let mut output = [0_u8; 32];
    fill_qualified_entropy(&mut output)?;
    Ok(output)
}

#[cfg(unix)]
fn fill_qualified_entropy(bytes: &mut [u8]) -> Result<(), &'static str> {
    use std::io::Read as _;
    std::fs::File::open("/dev/urandom")
        .and_then(|mut source| source.read_exact(bytes))
        .map_err(|_| QUALIFIED_ENTROPY_UNAVAILABLE)
}

#[cfg(windows)]
#[link(name = "Bcrypt")]
unsafe extern "system" {
    #[link_name = "BCryptGenRandom"]
    fn bcrypt_gen_random(
        algorithm: *mut c_void,
        buffer: *mut u8,
        buffer_bytes: u32,
        flags: u32,
    ) -> i32;
}

#[cfg(windows)]
fn fill_qualified_entropy(bytes: &mut [u8]) -> Result<(), &'static str> {
    const BCRYPT_USE_SYSTEM_PREFERRED_RNG: u32 = 0x0000_0002;
    const STATUS_SUCCESS: i32 = 0;
    let length = u32::try_from(bytes.len()).map_err(|_| QUALIFIED_ENTROPY_UNAVAILABLE)?;
    // SAFETY: `BCryptGenRandom` with a null algorithm handle and
    // `BCRYPT_USE_SYSTEM_PREFERRED_RNG` synchronously fills exactly
    // `buffer_bytes` bytes or reports failure. The slice is alive and
    // exclusively borrowed for the call, and its length was just checked.
    let status = unsafe {
        bcrypt_gen_random(
            core::ptr::null_mut(),
            bytes.as_mut_ptr(),
            length,
            BCRYPT_USE_SYSTEM_PREFERRED_RNG,
        )
    };
    if status == STATUS_SUCCESS {
        Ok(())
    } else {
        Err(QUALIFIED_ENTROPY_UNAVAILABLE)
    }
}

#[cfg(not(any(unix, windows)))]
fn fill_qualified_entropy(_bytes: &mut [u8]) -> Result<(), &'static str> {
    Err(QUALIFIED_ENTROPY_UNAVAILABLE)
}

/// Current live-authority snapshot rechecked before every expansion.
///
/// A moved generation, a revocation or a purge drops the whole window or
/// handle instead of filtering one hit. Result handles reuse this exact
/// snapshot so both DIRECT paths enforce identical checkpoints.
///
/// # Errors
///
/// This snapshot carries no fallible operations; denial surfaces as
/// [`ContinuationError::AccessRevoked`], [`ContinuationError::Purged`] or
/// [`ContinuationError::SourceFenceChanged`] from the checked expansion entry
/// points.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct LiveExpansionBarrier {
    /// The live generation moved past the admitted one.
    pub generation_moved: bool,
    /// The grant or security state now denies disclosure.
    pub access_revoked: bool,
    /// A purge barrier now covers the retained state.
    pub purged: bool,
}

impl LiveExpansionBarrier {
    /// Barrier observed clean: nothing moved since admission.
    #[must_use]
    pub const fn clean() -> Self {
        Self {
            generation_moved: false,
            access_revoked: false,
            purged: false,
        }
    }
}

/// Search-completeness flags carried by every page from one search execution.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct PageCompletion {
    pub(crate) corpus_complete: bool,
    pub(crate) match_limit_reached: bool,
}

/// Window and gap truncation flags carried by every page.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct PageTruncation {
    pub(crate) candidate_window_truncated: bool,
    pub(crate) gap_details_truncated: bool,
}

/// Immutable coverage carried by every page from one search execution.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct PageCoverage {
    pub(crate) registered_sources: usize,
    pub(crate) active_sources: usize,
    pub(crate) searched_sources: usize,
    pub(crate) completion: PageCompletion,
    pub(crate) total_matches: usize,
    pub(crate) retained_matches: usize,
    pub(crate) gap_count: usize,
    pub(crate) truncation: PageTruncation,
}

impl PageCoverage {
    #[must_use]
    pub(crate) const fn complete(&self) -> bool {
        self.completion.corpus_complete && !self.truncation.candidate_window_truncated
    }
}

/// One page of deterministic source-backed matches.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct SearchPage {
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
    /// The session tag comes from qualified OS entropy, never from public
    /// namespace, PID or wall-clock data. When entropy is unavailable the
    /// catalog is poisoned and every mint/expand fails closed with
    /// [`ContinuationError::EntropyUnavailable`] instead of minting guessable
    /// locators.
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

    /// Creates the first page and an opaque continuation when more retained
    /// matches remain.
    ///
    /// An exhausted single page inserts no record: ordinary queries persist no
    /// history. A store from a foreign namespace is rejected before any state
    /// is retained.
    ///
    /// # Errors
    ///
    /// Returns [`ContinuationError::InvalidPageSize`] for an empty or
    /// oversized page, [`ContinuationError::EntropyUnavailable`] when session
    /// entropy failed, [`ContinuationError::SourceFenceChanged`] for a foreign
    /// namespace, and [`ContinuationError::CapacityExceeded`] when no window
    /// fits the finite bounds.
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
        let expires_at = Instant::now() + CONTINUATION_TTL;
        let record = ContinuationRecord {
            namespace_id: self.namespace_id().to_owned(),
            session_tag: self.session_tag,
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

    /// Advances one exact window after session, TTL and source-fence
    /// revalidation.
    ///
    /// This is the clean-barrier specialization of
    /// [`Self::continue_page_with_live_barrier`]: every expansion on this path
    /// rechecks the same live-authority checkpoint with nothing moved.
    ///
    /// # Errors
    ///
    /// See [`Self::continue_page_with_live_barrier`].
    pub(crate) fn continue_page(
        &mut self,
        store: &DirectStore,
        token: &str,
        page_size: usize,
    ) -> Result<SearchPage, ContinuationError> {
        self.continue_page_with_live_barrier(store, token, page_size, LiveExpansionBarrier::clean())
    }

    /// Advances one window after the live-authority checkpoint in addition to
    /// the session, TTL and fence revalidation.
    ///
    /// A purged, revoked or moved barrier drops the whole retained ranking
    /// instead of filtering one hit; possession alone resumes nothing.
    /// Unknown or foreign-session tokens report [`ContinuationError::NotFound`]
    /// before any barrier state is disclosed. Expiry and fence drift drop the
    /// window instead of narrowing it, so a changed ranking can never be
    /// resumed against newer corpus state. The synchronous expiry path reports
    /// [`ContinuationError::Expired`]; windows already reaped by bounded
    /// cleanup report [`ContinuationError::NotFound`].
    ///
    /// # Errors
    ///
    /// Returns [`ContinuationError::InvalidPageSize`],
    /// [`ContinuationError::EntropyUnavailable`],
    /// [`ContinuationError::NotFound`] (unknown, tampered or foreign-session
    /// token), [`ContinuationError::Purged`],
    /// [`ContinuationError::AccessRevoked`],
    /// [`ContinuationError::SourceFenceChanged`] (moved generation or changed
    /// fence) or [`ContinuationError::Expired`].
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
        if barrier.purged {
            self.drop_window(token);
            self.expire();
            return Err(ContinuationError::Purged);
        }
        if barrier.access_revoked {
            self.drop_window(token);
            self.expire();
            return Err(ContinuationError::AccessRevoked);
        }
        if barrier.generation_moved {
            self.drop_window(token);
            self.expire();
            return Err(ContinuationError::SourceFenceChanged);
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
        self.retained_matches = self.retained_matches.saturating_sub(record.matches.len());
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
            let remaining = record.expires_at.saturating_duration_since(Instant::now());
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

    /// Marks one window immediately expired. Test-only seam: wall-clock TTL is
    /// otherwise unforgeable inside unit tests.
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
            if let Some(record) = self.records.remove(&token) {
                self.retained_matches = self.retained_matches.saturating_sub(record.matches.len());
            }
        }
    }

    fn allocate_token(&self) -> Result<String, ContinuationError> {
        for _ in 0..MAX_CONTINUATIONS.saturating_mul(2) {
            let material =
                qualified_entropy_32().map_err(|_| ContinuationError::EntropyUnavailable)?;
            let token = sha256::hex(&material);
            if !self.records.contains_key(&token) {
                return Ok(token);
            }
        }
        Err(ContinuationError::TokenExhausted)
    }
}

const fn validate_page_size(page_size: usize) -> Result<(), ContinuationError> {
    if page_size == 0 || page_size > MAX_PAGE_SIZE {
        Err(ContinuationError::InvalidPageSize)
    } else {
        Ok(())
    }
}

#[cfg(test)]
mod live_authorization_tests {
    use super::*;
    use std::fs;
    use std::path::PathBuf;
    use std::sync::atomic::{AtomicU64, Ordering};
    use std::time::{SystemTime, UNIX_EPOCH};

    use crate::development::DataRootGuard;
    use crate::direct_store::{DirectStore, SourceSummary, StoreSearchResult, StoredMatch};
    use crate::revision_protection::{TestCredentialGuard, lock_unit_vault_for_test};

    static NEXT: AtomicU64 = AtomicU64::new(0);

    struct Fixture {
        base: PathBuf,
        credentials: TestCredentialGuard,
        _owner: DataRootGuard,
        store: DirectStore,
    }

    impl Fixture {
        fn new(tag: &str, initial: &[u8]) -> Self {
            let stamp = SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .map_or(0, |value| value.as_nanos());
            let base = std::env::temp_dir().join(format!(
                "eliot-t21-continuation-{tag}-{}-{stamp}-{}",
                std::process::id(),
                NEXT.fetch_add(1, Ordering::Relaxed),
            ));
            fs::create_dir_all(base.join("data")).unwrap();
            fs::create_dir_all(base.join("sources")).unwrap();
            let credentials = TestCredentialGuard::for_data_root(&base.join("data"));
            let owner = DataRootGuard::acquire(&base.join("data")).unwrap();
            let mut store = DirectStore::open(owner.canonical_root()).unwrap();
            let first = base.join("sources").join("first.txt");
            fs::write(&first, initial).unwrap();
            store.index_file(&first).unwrap();
            Self {
                base,
                credentials,
                _owner: owner,
                store,
            }
        }

        fn add_source(&mut self, name: &str, contents: &[u8]) {
            let path = self.base.join("sources").join(name);
            fs::write(&path, contents).unwrap();
            self.store.index_file(&path).unwrap();
        }

        fn namespace(&self) -> String {
            self.store.namespace_id()
        }
    }

    impl Drop for Fixture {
        fn drop(&mut self) {
            self.credentials.cleanup();
            let _ = fs::remove_dir_all(&self.base);
        }
    }

    fn test_match(
        summary: &SourceSummary,
        byte_start: usize,
        byte_end: usize,
        index: usize,
    ) -> StoredMatch {
        StoredMatch {
            source_id: summary.source_id.clone(),
            revision_id: summary.revision_id.clone(),
            content_digest: summary.content_digest.clone(),
            path_digest: summary.path_digest.clone(),
            evidence_id: format!("evidence-{index}"),
            byte_start,
            byte_end,
            line: 1,
            column_bytes: 1,
        }
    }

    fn search_result(matches: Vec<StoredMatch>) -> StoreSearchResult {
        StoreSearchResult {
            matches,
            gaps: Vec::new(),
            registered_sources: 1,
            active_sources: 1,
            searched_sources: 1,
            complete: true,
            match_limit_reached: false,
        }
    }

    fn three_matches(fixture: &Fixture) -> Vec<StoredMatch> {
        let summaries = fixture.store.list_sources();
        let summary = summaries.first().unwrap();
        vec![
            test_match(summary, 0, 6, 0),
            test_match(summary, 7, 13, 1),
            test_match(summary, 14, 20, 2),
        ]
    }

    fn is_opaque_token(token: &str) -> bool {
        token.len() == 64 && token.bytes().all(|byte| byte.is_ascii_hexdigit())
    }

    #[test]
    fn tokens_are_unique_opaque_session_binders() {
        let _vault = lock_unit_vault_for_test();
        let fixture = Fixture::new("unique", b"needle one\nneedle two\nneedle three\n");
        let namespace = fixture.namespace();
        let mut first = ContinuationCatalog::new(&namespace);
        let mut second = ContinuationCatalog::new(&namespace);
        let mut tokens = Vec::new();
        for _ in 0..4 {
            let page = first
                .create_page(&fixture.store, search_result(three_matches(&fixture)), 1)
                .unwrap();
            let token = page.continuation_token.unwrap();
            assert!(is_opaque_token(&token), "{token}");
            tokens.push(token);
            let page = second
                .create_page(&fixture.store, search_result(three_matches(&fixture)), 1)
                .unwrap();
            let token = page.continuation_token.unwrap();
            assert!(is_opaque_token(&token), "{token}");
            tokens.push(token);
        }
        tokens.sort();
        tokens.dedup();
        assert_eq!(
            tokens.len(),
            8,
            "every window mints a distinct opaque token"
        );
    }

    #[test]
    fn cross_session_replay_is_not_found() {
        let _vault = lock_unit_vault_for_test();
        let fixture = Fixture::new("session", b"needle one\nneedle two\nneedle three\n");
        let namespace = fixture.namespace();
        let mut first = ContinuationCatalog::new(&namespace);
        let mut second = ContinuationCatalog::new(&namespace);
        let page = first
            .create_page(&fixture.store, search_result(three_matches(&fixture)), 1)
            .unwrap();
        let token = page.continuation_token.unwrap();
        assert_eq!(
            second.continue_page(&fixture.store, &token, 1),
            Err(ContinuationError::NotFound),
            "possession alone grants no authority in a foreign session"
        );
        assert_eq!(
            second.continue_page_with_live_barrier(
                &fixture.store,
                &token,
                1,
                LiveExpansionBarrier::clean(),
            ),
            Err(ContinuationError::NotFound),
        );
    }

    #[test]
    fn foreign_namespace_store_is_rejected_on_create_and_continue() {
        let _vault = lock_unit_vault_for_test();
        let fixture = Fixture::new("root-a", b"needle one\nneedle two\nneedle three\n");
        let foreign = Fixture::new("root-b", b"needle one\nneedle two\nneedle three\n");
        let mut catalog = ContinuationCatalog::new(&fixture.namespace());
        assert_eq!(catalog.namespace_id(), fixture.namespace());
        let foreign_matches = {
            let summaries = foreign.store.list_sources();
            let summary = summaries.first().unwrap();
            vec![test_match(summary, 0, 6, 0), test_match(summary, 7, 13, 1)]
        };
        assert_eq!(
            catalog.create_page(&foreign.store, search_result(foreign_matches), 1),
            Err(ContinuationError::SourceFenceChanged),
            "a catalog bound to one root never pages a foreign root"
        );
        let page = catalog
            .create_page(&fixture.store, search_result(three_matches(&fixture)), 1)
            .unwrap();
        let token = page.continuation_token.unwrap();
        assert_eq!(
            catalog.continue_page(&foreign.store, &token, 1),
            Err(ContinuationError::SourceFenceChanged),
        );
        assert_eq!(
            catalog.continue_page(&fixture.store, &token, 1),
            Err(ContinuationError::NotFound),
            "fence drift drops the window instead of narrowing it"
        );
    }

    #[test]
    fn expired_window_reports_expired_and_releases_pins() {
        let _vault = lock_unit_vault_for_test();
        let fixture = Fixture::new("expiry", b"needle one\nneedle two\nneedle three\n");
        let mut catalog = ContinuationCatalog::new(&fixture.namespace());
        let page = catalog
            .create_page(&fixture.store, search_result(three_matches(&fixture)), 1)
            .unwrap();
        let token = page.continuation_token.unwrap();
        assert!(catalog.force_expire_for_tests(&token));
        assert_eq!(
            catalog.continue_page(&fixture.store, &token, 1),
            Err(ContinuationError::Expired),
        );
        assert_eq!(catalog.live_count(), 0);
        assert_eq!(catalog.retained_matches(), 0);
    }

    #[test]
    fn source_mutation_invalidates_whole_window_not_one_result() {
        let _vault = lock_unit_vault_for_test();
        let mut fixture = Fixture::new("drift", b"needle one\nneedle two\nneedle three\n");
        let mut catalog = ContinuationCatalog::new(&fixture.namespace());
        let page = catalog
            .create_page(&fixture.store, search_result(three_matches(&fixture)), 1)
            .unwrap();
        let token = page.continuation_token.unwrap();
        assert_eq!(page.matches.len(), 1);
        fixture.add_source("second.txt", b"needle four\n");
        assert_eq!(
            catalog.continue_page(&fixture.store, &token, 1),
            Err(ContinuationError::SourceFenceChanged),
            "a changed fence invalidates the retained ranking, not one hit"
        );
        assert_eq!(
            catalog.continue_page(&fixture.store, &token, 1),
            Err(ContinuationError::NotFound),
        );
        assert_eq!(catalog.live_count(), 0);
        assert_eq!(catalog.retained_matches(), 0);
    }

    #[test]
    fn revoked_live_barrier_invalidates_window() {
        let _vault = lock_unit_vault_for_test();
        let fixture = Fixture::new("revoked", b"needle one\nneedle two\nneedle three\n");
        let mut catalog = ContinuationCatalog::new(&fixture.namespace());
        let page = catalog
            .create_page(&fixture.store, search_result(three_matches(&fixture)), 1)
            .unwrap();
        let token = page.continuation_token.unwrap();
        let barrier = LiveExpansionBarrier {
            generation_moved: false,
            access_revoked: true,
            purged: false,
        };
        assert_eq!(
            catalog.continue_page_with_live_barrier(&fixture.store, &token, 1, barrier),
            Err(ContinuationError::AccessRevoked),
        );
        assert_eq!(
            catalog.continue_page(&fixture.store, &token, 1),
            Err(ContinuationError::NotFound),
            "a revoked window is dropped, never resumed"
        );
        assert_eq!(catalog.retained_matches(), 0);
    }

    #[test]
    fn purged_live_barrier_invalidates_window() {
        let _vault = lock_unit_vault_for_test();
        let fixture = Fixture::new("purged", b"needle one\nneedle two\nneedle three\n");
        let mut catalog = ContinuationCatalog::new(&fixture.namespace());
        let page = catalog
            .create_page(&fixture.store, search_result(three_matches(&fixture)), 1)
            .unwrap();
        let token = page.continuation_token.unwrap();
        let barrier = LiveExpansionBarrier {
            generation_moved: false,
            access_revoked: false,
            purged: true,
        };
        assert_eq!(
            catalog.continue_page_with_live_barrier(&fixture.store, &token, 1, barrier),
            Err(ContinuationError::Purged),
        );
        assert_eq!(catalog.live_count(), 0);
        assert_eq!(catalog.retained_matches(), 0);
    }

    #[test]
    fn moved_generation_is_a_fence_change_and_drops_window() {
        let _vault = lock_unit_vault_for_test();
        let fixture = Fixture::new("generation", b"needle one\nneedle two\nneedle three\n");
        let mut catalog = ContinuationCatalog::new(&fixture.namespace());
        let page = catalog
            .create_page(&fixture.store, search_result(three_matches(&fixture)), 1)
            .unwrap();
        let token = page.continuation_token.unwrap();
        let barrier = LiveExpansionBarrier {
            generation_moved: true,
            access_revoked: false,
            purged: false,
        };
        assert_eq!(
            catalog.continue_page_with_live_barrier(&fixture.store, &token, 1, barrier),
            Err(ContinuationError::SourceFenceChanged),
        );
        assert_eq!(catalog.live_count(), 0);
    }

    #[test]
    fn tampered_unknown_and_empty_tokens_are_not_found() {
        let _vault = lock_unit_vault_for_test();
        let fixture = Fixture::new("tamper", b"needle one\nneedle two\nneedle three\n");
        let mut catalog = ContinuationCatalog::new(&fixture.namespace());
        let page = catalog
            .create_page(&fixture.store, search_result(three_matches(&fixture)), 1)
            .unwrap();
        let raw = page.continuation_token.unwrap();
        let mut token = raw.clone();
        let first = token.remove(0);
        token.insert(0, if first == '0' { '1' } else { '0' });
        assert_ne!(token, raw, "tamper must change the token");
        assert_eq!(
            catalog.continue_page(&fixture.store, &token, 1),
            Err(ContinuationError::NotFound),
        );
        assert_eq!(
            catalog.continue_page(&fixture.store, "", 1),
            Err(ContinuationError::NotFound),
        );
        assert_eq!(
            catalog.continue_page(&fixture.store, "not-hex-at-all", 1),
            Err(ContinuationError::NotFound),
        );
        assert_eq!(
            catalog.live_count(),
            1,
            "failed lookups keep the live window"
        );
    }

    #[test]
    fn invalid_page_sizes_rejected_and_exact_final_page_exhausts_without_history() {
        let _vault = lock_unit_vault_for_test();
        let fixture = Fixture::new("pages", b"needle one\nneedle two\n");
        let mut catalog = ContinuationCatalog::new(&fixture.namespace());
        assert_eq!(
            catalog.create_page(&fixture.store, search_result(three_matches(&fixture)), 0),
            Err(ContinuationError::InvalidPageSize),
        );
        assert_eq!(
            catalog.create_page(
                &fixture.store,
                search_result(three_matches(&fixture)),
                MAX_PAGE_SIZE + 1,
            ),
            Err(ContinuationError::InvalidPageSize),
        );
        let summaries = fixture.store.list_sources();
        let summary = summaries.first().unwrap();
        let exact = vec![test_match(summary, 0, 6, 0), test_match(summary, 7, 13, 1)];
        let page = catalog
            .create_page(&fixture.store, search_result(exact), 2)
            .unwrap();
        assert!(page.exhausted);
        assert!(page.continuation_token.is_none());
        assert_eq!(
            catalog.live_count(),
            0,
            "ordinary queries persist no history"
        );
        assert_eq!(catalog.retained_matches(), 0);
    }

    #[test]
    fn window_capacity_exhaustion_and_exhaustion_pin_cleanup() {
        let _vault = lock_unit_vault_for_test();
        let fixture = Fixture::new("capacity", b"needle one\nneedle two\nneedle three\n");
        let mut catalog = ContinuationCatalog::new(&fixture.namespace());
        let mut tokens = Vec::new();
        for _ in 0..MAX_CONTINUATIONS {
            let page = catalog
                .create_page(&fixture.store, search_result(three_matches(&fixture)), 1)
                .unwrap();
            tokens.push(page.continuation_token.unwrap());
        }
        assert_eq!(catalog.live_count(), MAX_CONTINUATIONS);
        assert_eq!(
            catalog.create_page(&fixture.store, search_result(three_matches(&fixture)), 1),
            Err(ContinuationError::CapacityExceeded),
        );
        for token in tokens {
            let page = catalog.continue_page(&fixture.store, &token, 10).unwrap();
            assert!(page.exhausted, "the retained tail fits one bounded page");
        }
        assert_eq!(catalog.live_count(), 0);
        assert_eq!(
            catalog.retained_matches(),
            0,
            "exhaustion releases every pin"
        );
    }

    #[test]
    fn restart_starts_empty_and_forgets_tokens() {
        let _vault = lock_unit_vault_for_test();
        let fixture = Fixture::new("restart", b"needle one\nneedle two\nneedle three\n");
        let namespace = fixture.namespace();
        let mut first = ContinuationCatalog::new(&namespace);
        let page = first
            .create_page(&fixture.store, search_result(three_matches(&fixture)), 1)
            .unwrap();
        let token = page.continuation_token.unwrap();
        drop(first);
        let mut restarted = ContinuationCatalog::new(&namespace);
        assert_eq!(restarted.live_count(), 0);
        assert_eq!(
            restarted.continue_page(&fixture.store, &token, 1),
            Err(ContinuationError::NotFound),
            "ephemeral tokens never survive a restart"
        );
    }

    #[test]
    fn invalidate_all_releases_every_pin() {
        let _vault = lock_unit_vault_for_test();
        let fixture = Fixture::new("invalidate", b"needle one\nneedle two\nneedle three\n");
        let mut catalog = ContinuationCatalog::new(&fixture.namespace());
        let mut tokens = Vec::new();
        for _ in 0..3 {
            let page = catalog
                .create_page(&fixture.store, search_result(three_matches(&fixture)), 1)
                .unwrap();
            tokens.push(page.continuation_token.unwrap());
        }
        assert_eq!(catalog.invalidate_all(), 3);
        assert_eq!(catalog.live_count(), 0);
        assert_eq!(catalog.retained_matches(), 0);
        for token in tokens {
            assert_eq!(
                catalog.continue_page(&fixture.store, &token, 1),
                Err(ContinuationError::NotFound),
            );
        }
        assert_eq!(catalog.invalidate_all(), 0, "invalidation is idempotent");
    }

    #[test]
    fn catalog_debug_redacts_session_material_and_tokens() {
        let _vault = lock_unit_vault_for_test();
        let fixture = Fixture::new("redact", b"needle one\nneedle two\nneedle three\n");
        let mut catalog = ContinuationCatalog::new(&fixture.namespace());
        let page = catalog
            .create_page(&fixture.store, search_result(three_matches(&fixture)), 1)
            .unwrap();
        let token = page.continuation_token.unwrap();
        let rendered = format!("{catalog:?}");
        assert!(rendered.contains("live_windows"), "{rendered}");
        assert!(
            !rendered.contains(&token),
            "plaintext tokens never reach debug output"
        );
    }

    #[test]
    fn qualified_entropy_produces_distinct_256bit_material() {
        let first = qualified_entropy_32().unwrap();
        let second = qualified_entropy_32().unwrap();
        assert_eq!(first.len(), 32);
        assert_eq!(second.len(), 32);
        assert_ne!(first, second, "opaque tokens require qualified entropy");
    }
}

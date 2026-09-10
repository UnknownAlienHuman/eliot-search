//! Live-authorized opaque source handles for public DIRECT search pages.
//!
//! Handles are qualified-entropy bearer locators bound to one namespace and
//! one session, not production bearer credentials and not deterministic
//! hashes. The token contains no source, path, revision, range, content, or
//! authority data. The server-side record is bound to the namespace, the
//! session tag, current source state, one immutable revision, a finite TTL,
//! and a finite per-expansion disclosure ceiling. Every expansion revalidates
//! liveness and exact byte provenance before returning bytes; possession
//! alone admits nothing (invariant 13).
//!
//! Handles are ephemeral session memory only: they are never persisted,
//! ordinary query minting creates no durable history, and tokens never survive
//! a restart. Explicitly retained durable evidence or export handles are out
//! of scope for this catalog (see [`ResultHandleCatalog::mint_durable_source`]
//! and the canonical `search-handles` owner for the indexed path). The shared
//! T21 checkpoint [`LiveExpansionBarrier`](crate::continuation::LiveExpansionBarrier)
//! is rechecked before every barrier-guarded expansion.

use core::fmt;
use std::collections::BTreeMap;
use std::time::{Duration, Instant};

use crate::continuation::{LiveExpansionBarrier, qualified_entropy_32};
use crate::direct_store::{DirectStore, RevisionSlice, StoredMatch};
use crate::sha256;
use crate::source_fence::digest as source_fence;

/// Maximum simultaneous result handles.
pub const MAX_RESULT_HANDLES: usize = 50_000;
/// Maximum exact bytes returned by one handle expansion.
pub const MAX_HANDLE_EXPANSION_BYTES: u64 = 24 * 1024;
/// Finite process-local handle lifetime.
pub const RESULT_HANDLE_TTL: Duration = Duration::from_mins(15);

/// Closed result-handle failure.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ResultHandleError {
    CapacityExceeded,
    TokenExhausted,
    NotFound,
    Expired,
    SourceFenceChanged,
    SourceUnavailable,
    RevisionChanged,
    RangeInvalid,
    ExpansionTooLarge,
    ReadbackMismatch,
    EntropyUnavailable,
    AccessRevoked,
    Purged,
    DurableRetentionRequired,
}

impl ResultHandleError {
    pub(crate) const fn code(self) -> &'static str {
        match self {
            Self::CapacityExceeded => "DIRECT_RESULT_HANDLE_CAPACITY_EXCEEDED",
            Self::TokenExhausted => "DIRECT_RESULT_HANDLE_TOKEN_EXHAUSTED",
            Self::NotFound => "DIRECT_RESULT_HANDLE_NOT_FOUND",
            Self::Expired => "DIRECT_RESULT_HANDLE_EXPIRED",
            Self::SourceFenceChanged => "DIRECT_RESULT_HANDLE_SOURCE_FENCE_CHANGED",
            Self::SourceUnavailable => "DIRECT_RESULT_HANDLE_SOURCE_UNAVAILABLE",
            Self::RevisionChanged => "DIRECT_RESULT_HANDLE_REVISION_CHANGED",
            Self::RangeInvalid => "DIRECT_RESULT_HANDLE_RANGE_INVALID",
            Self::ExpansionTooLarge => "DIRECT_RESULT_HANDLE_EXPANSION_TOO_LARGE",
            Self::ReadbackMismatch => "DIRECT_RESULT_HANDLE_READBACK_MISMATCH",
            Self::EntropyUnavailable => "DIRECT_RESULT_HANDLE_ENTROPY_UNAVAILABLE",
            Self::AccessRevoked => "DIRECT_RESULT_HANDLE_ACCESS_REVOKED",
            Self::Purged => "DIRECT_RESULT_HANDLE_PURGED",
            Self::DurableRetentionRequired => "DIRECT_RESULT_HANDLE_DURABLE_RETENTION_REQUIRED",
        }
    }
}

/// Public non-self-describing handle attached to one match.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct PublicHandledMatch {
    pub(crate) source_handle: String,
    pub(crate) evidence_id: String,
    pub(crate) byte_start: usize,
    pub(crate) byte_end: usize,
    pub(crate) line: usize,
    pub(crate) column_bytes: usize,
    pub(crate) source_byte_length: u64,
    pub(crate) expires_in_ms: u64,
}

#[derive(Clone, Debug)]
struct ResultHandleRecord {
    namespace_id: String,
    session_tag: u64,
    source_fence_digest: String,
    source_id: String,
    revision_id: String,
    content_digest: String,
    byte_length: u64,
    expires_at: Instant,
}

/// Exact bounded expansion of one opaque handle.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ResultHandleExpansion {
    pub(crate) source_handle: String,
    pub(crate) byte_start: u64,
    pub(crate) byte_end: u64,
    pub(crate) source_byte_length: u64,
    pub(crate) bytes: Vec<u8>,
}

/// Finite live-authorized result-handle catalog for one owner session.
///
/// Debug output carries counts and the namespace only: the session tag and
/// token plaintext never reach logs or receipts.
pub struct ResultHandleCatalog {
    namespace_id: String,
    session_tag: u64,
    entropy_poisoned: bool,
    records: BTreeMap<String, ResultHandleRecord>,
}

impl fmt::Debug for ResultHandleCatalog {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("ResultHandleCatalog")
            .field("namespace_id", &self.namespace_id)
            .field("session_tag", &"<redacted>")
            .field("entropy_poisoned", &self.entropy_poisoned)
            .field("records", &self.records.len())
            .field("live_handles", &self.records.len())
            .finish()
    }
}

impl ResultHandleCatalog {
    /// Creates one session-local catalog bound to an exact namespace.
    ///
    /// The session tag comes from qualified OS entropy, never from public
    /// namespace, PID or wall-clock data. When entropy is unavailable the
    /// catalog is poisoned and every mint/expand fails closed with
    /// [`ResultHandleError::EntropyUnavailable`] instead of minting guessable
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
        }
    }

    /// Namespace this catalog is bound to; foreign stores are rejected.
    #[must_use]
    pub(crate) fn namespace_id(&self) -> &str {
        &self.namespace_id
    }

    /// Number of live handles after bounded expiry cleanup.
    pub(crate) fn live_count(&mut self) -> usize {
        self.expire();
        self.records.len()
    }

    /// Invalidates every handle after source-state mutation.
    pub(crate) fn invalidate_all(&mut self) -> usize {
        let invalidated = self.records.len();
        self.records.clear();
        invalidated
    }

    /// Atomically mints one opaque handle per source-backed match.
    ///
    /// Minting is all-or-nothing: any ineligible match or exhausted bound
    /// inserts nothing. Handles are ephemeral session memory; this entry point
    /// never creates durable history.
    ///
    /// # Errors
    ///
    /// Returns [`ResultHandleError::EntropyUnavailable`] when session entropy
    /// failed, [`ResultHandleError::SourceFenceChanged`] for a foreign
    /// namespace, [`ResultHandleError::CapacityExceeded`] past the finite
    /// bound, [`ResultHandleError::SourceUnavailable`] for a missing or
    /// retired source, [`ResultHandleError::RevisionChanged`] when the match
    /// no longer equals the admitted revision, and
    /// [`ResultHandleError::TokenExhausted`] when no fresh token fits the
    /// bounded retry budget.
    pub(crate) fn mint_page(
        &mut self,
        store: &DirectStore,
        matches: &[StoredMatch],
    ) -> Result<Vec<PublicHandledMatch>, ResultHandleError> {
        if self.entropy_poisoned {
            return Err(ResultHandleError::EntropyUnavailable);
        }
        if store.namespace_id() != self.namespace_id() {
            return Err(ResultHandleError::SourceFenceChanged);
        }
        self.expire();
        if self.records.len().saturating_add(matches.len()) > MAX_RESULT_HANDLES {
            return Err(ResultHandleError::CapacityExceeded);
        }
        let summaries = store
            .list_sources()
            .into_iter()
            .map(|source| (source.source_id.clone(), source))
            .collect::<BTreeMap<_, _>>();
        let fence = source_fence(store);
        let expires_at = Instant::now() + RESULT_HANDLE_TTL;
        let expires_in_ms = u64::try_from(RESULT_HANDLE_TTL.as_millis()).unwrap_or(u64::MAX);
        let mut staged = Vec::with_capacity(matches.len());

        for item in matches {
            let source = summaries
                .get(&item.source_id)
                .ok_or(ResultHandleError::SourceUnavailable)?;
            if !source.active {
                return Err(ResultHandleError::SourceUnavailable);
            }
            if source.revision_id != item.revision_id
                || source.content_digest != item.content_digest
                || source.path_digest != item.path_digest
            {
                return Err(ResultHandleError::RevisionChanged);
            }
            let token = self.allocate_token()?;
            staged.push((
                token.clone(),
                ResultHandleRecord {
                    namespace_id: self.namespace_id().to_owned(),
                    session_tag: self.session_tag,
                    source_fence_digest: fence.clone(),
                    source_id: item.source_id.clone(),
                    revision_id: item.revision_id.clone(),
                    content_digest: item.content_digest.clone(),
                    byte_length: source.byte_length,
                    expires_at,
                },
                PublicHandledMatch {
                    source_handle: token,
                    evidence_id: item.evidence_id.clone(),
                    byte_start: item.byte_start,
                    byte_end: item.byte_end,
                    line: item.line,
                    column_bytes: item.column_bytes,
                    source_byte_length: source.byte_length,
                    expires_in_ms,
                },
            ));
        }

        let mut public = Vec::with_capacity(staged.len());
        for (token, record, item) in staged {
            self.records.insert(token, record);
            public.push(item);
        }
        Ok(public)
    }

    /// Expands one exact source range after session, TTL, namespace,
    /// source-state, revision, and immutable readback verification.
    ///
    /// This is the clean-barrier specialization of
    /// [`Self::expand_with_live_barrier`]: every expansion on this path
    /// rechecks the same live-authority checkpoint with nothing moved.
    ///
    /// # Errors
    ///
    /// See [`Self::expand_with_live_barrier`].
    pub(crate) fn expand(
        &mut self,
        store: &DirectStore,
        token: &str,
        byte_start: u64,
        byte_end: u64,
    ) -> Result<ResultHandleExpansion, ResultHandleError> {
        self.expand_with_live_barrier(
            store,
            token,
            byte_start,
            byte_end,
            LiveExpansionBarrier::clean(),
        )
    }

    /// Expands one handle after the live-authority checkpoint in addition to
    /// the session, TTL, namespace and provenance revalidation.
    ///
    /// A purged, revoked or moved barrier drops the handle instead of
    /// returning narrowed bytes; possession alone expands nothing. Unknown or
    /// foreign-session tokens report [`ResultHandleError::NotFound`] before
    /// any barrier state is disclosed. Expiry and fence drift drop the handle
    /// instead of narrowing it. The synchronous expiry path reports
    /// [`ResultHandleError::Expired`]; handles already reaped by bounded
    /// cleanup report [`ResultHandleError::NotFound`].
    ///
    /// # Errors
    ///
    /// Returns [`ResultHandleError::EntropyUnavailable`],
    /// [`ResultHandleError::NotFound`] (unknown, tampered or foreign-session
    /// token), [`ResultHandleError::Purged`],
    /// [`ResultHandleError::AccessRevoked`],
    /// [`ResultHandleError::SourceFenceChanged`] (moved generation or changed
    /// fence), [`ResultHandleError::Expired`],
    /// [`ResultHandleError::RangeInvalid`],
    /// [`ResultHandleError::ExpansionTooLarge`],
    /// [`ResultHandleError::SourceUnavailable`],
    /// [`ResultHandleError::RevisionChanged`] or
    /// [`ResultHandleError::ReadbackMismatch`].
    pub(crate) fn expand_with_live_barrier(
        &mut self,
        store: &DirectStore,
        token: &str,
        byte_start: u64,
        byte_end: u64,
        barrier: LiveExpansionBarrier,
    ) -> Result<ResultHandleExpansion, ResultHandleError> {
        if self.entropy_poisoned {
            return Err(ResultHandleError::EntropyUnavailable);
        }
        let tag_matches = self
            .records
            .get(token)
            .is_some_and(|record| record.session_tag == self.session_tag);
        if !tag_matches {
            return Err(ResultHandleError::NotFound);
        }
        if barrier.purged {
            self.records.remove(token);
            self.expire();
            return Err(ResultHandleError::Purged);
        }
        if barrier.access_revoked {
            self.records.remove(token);
            self.expire();
            return Err(ResultHandleError::AccessRevoked);
        }
        if barrier.generation_moved {
            self.records.remove(token);
            self.expire();
            return Err(ResultHandleError::SourceFenceChanged);
        }
        let record = self
            .records
            .get(token)
            .cloned()
            .ok_or(ResultHandleError::NotFound)?;
        if Instant::now() >= record.expires_at {
            self.records.remove(token);
            self.expire();
            return Err(ResultHandleError::Expired);
        }
        if record.namespace_id != store.namespace_id()
            || record.source_fence_digest != source_fence(store)
        {
            self.records.remove(token);
            self.expire();
            return Err(ResultHandleError::SourceFenceChanged);
        }
        if byte_start >= byte_end || byte_end > record.byte_length {
            return Err(ResultHandleError::RangeInvalid);
        }
        if byte_end.saturating_sub(byte_start) > MAX_HANDLE_EXPANSION_BYTES {
            return Err(ResultHandleError::ExpansionTooLarge);
        }
        let source = store
            .list_sources()
            .into_iter()
            .find(|source| source.source_id == record.source_id)
            .ok_or(ResultHandleError::SourceUnavailable)?;
        if !source.active {
            return Err(ResultHandleError::SourceUnavailable);
        }
        if source.revision_id != record.revision_id
            || source.content_digest != record.content_digest
            || source.byte_length != record.byte_length
        {
            return Err(ResultHandleError::RevisionChanged);
        }
        let RevisionSlice {
            revision_id,
            content_digest,
            byte_start: observed_start,
            byte_end: observed_end,
            bytes,
        } = store
            .read_revision_range(&record.revision_id, byte_start, byte_end)
            .map_err(|_| ResultHandleError::ReadbackMismatch)?;
        if revision_id != record.revision_id
            || content_digest != record.content_digest
            || observed_start != byte_start
            || observed_end != byte_end
            || u64::try_from(bytes.len()).unwrap_or(u64::MAX)
                != byte_end.saturating_sub(byte_start)
        {
            return Err(ResultHandleError::ReadbackMismatch);
        }
        self.expire();
        Ok(ResultHandleExpansion {
            source_handle: token.to_owned(),
            byte_start,
            byte_end,
            source_byte_length: record.byte_length,
            bytes,
        })
    }

    /// Refuses durable handle minting: this catalog is ephemeral-only.
    ///
    /// Ordinary DIRECT query handles live in session memory with a finite TTL
    /// and never survive a restart. Explicitly retained durable evidence or
    /// export handles require an immutable retained revision plus a retention
    /// lease outside this catalog (see the canonical `search-handles` owner);
    /// this entry point fails closed instead of relabeling an ephemeral
    /// locator as durable.
    ///
    /// # Errors
    ///
    /// Always returns [`ResultHandleError::DurableRetentionRequired`].
    ///
    /// This boundary is reserved for explicit retention wiring and stays
    /// fail-closed until then, so it is intentionally never called on the
    /// ephemeral DIRECT path yet.
    #[allow(dead_code)]
    pub(crate) const fn mint_durable_source() -> Result<(), ResultHandleError> {
        Err(ResultHandleError::DurableRetentionRequired)
    }

    /// Marks one handle immediately expired. Test-only seam: wall-clock TTL is
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

    fn expire(&mut self) {
        let now = Instant::now();
        self.records.retain(|_, record| record.expires_at > now);
    }

    fn allocate_token(&self) -> Result<String, ResultHandleError> {
        for _ in 0..128 {
            let material =
                qualified_entropy_32().map_err(|_| ResultHandleError::EntropyUnavailable)?;
            let token = sha256::hex(&material);
            if !self.records.contains_key(&token) {
                return Ok(token);
            }
        }
        Err(ResultHandleError::TokenExhausted)
    }
}

#[cfg(test)]
mod live_authorization_tests {
    use super::*;
    use std::fs;
    use std::path::PathBuf;
    use std::sync::atomic::{AtomicU64, Ordering};
    use std::time::{SystemTime, UNIX_EPOCH};

    use crate::continuation::LiveExpansionBarrier;
    use crate::development::DataRootGuard;
    use crate::direct_store::{DirectStore, SourceSummary, StoredMatch};
    use crate::revision_protection::{TestCredentialGuard, lock_unit_vault_for_test};
    use crate::source_fence::digest as source_fence;

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
                "eliot-t21-handles-{tag}-{}-{stamp}-{}",
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

        fn namespace(&self) -> String {
            self.store.namespace_id()
        }

        fn summary(&self) -> SourceSummary {
            self.store.list_sources().into_iter().next().unwrap()
        }

        fn handle_match(&self, byte_start: usize, byte_end: usize, index: usize) -> StoredMatch {
            let summary = self.summary();
            StoredMatch {
                source_id: summary.source_id,
                revision_id: summary.revision_id,
                content_digest: summary.content_digest,
                path_digest: summary.path_digest,
                evidence_id: format!("evidence-{index}"),
                byte_start,
                byte_end,
                line: 1,
                column_bytes: 1,
            }
        }
    }

    impl Drop for Fixture {
        fn drop(&mut self) {
            self.credentials.cleanup();
            let _ = fs::remove_dir_all(&self.base);
        }
    }

    fn is_opaque_token(token: &str) -> bool {
        token.len() == 64 && token.bytes().all(|byte| byte.is_ascii_hexdigit())
    }

    #[test]
    fn handle_tokens_are_unique_opaque() {
        let _vault = lock_unit_vault_for_test();
        let fixture = Fixture::new("unique", b"0123456789abcdef");
        let mut catalog = ResultHandleCatalog::new(&fixture.namespace());
        let matches = vec![
            fixture.handle_match(0, 4, 0),
            fixture.handle_match(4, 8, 1),
            fixture.handle_match(8, 12, 2),
        ];
        let public = catalog.mint_page(&fixture.store, &matches).unwrap();
        assert_eq!(public.len(), 3);
        let mut tokens = public
            .iter()
            .map(|item| item.source_handle.clone())
            .collect::<Vec<_>>();
        for token in &tokens {
            assert!(is_opaque_token(token), "{token}");
        }
        tokens.sort();
        tokens.dedup();
        assert_eq!(
            tokens.len(),
            3,
            "every handle mints a distinct opaque token"
        );
    }

    #[test]
    fn cross_session_and_cross_root_replay_denied() {
        let _vault = lock_unit_vault_for_test();
        let fixture = Fixture::new("session", b"0123456789abcdef");
        let foreign = Fixture::new("session-root", b"0123456789abcdef");
        let namespace = fixture.namespace();
        let mut first = ResultHandleCatalog::new(&namespace);
        let mut second = ResultHandleCatalog::new(&namespace);
        let mut cross_root = ResultHandleCatalog::new(&foreign.namespace());
        let public = first
            .mint_page(&fixture.store, &[fixture.handle_match(0, 4, 0)])
            .unwrap();
        let token = public.first().unwrap().source_handle.clone();
        assert_eq!(
            second.expand(&fixture.store, &token, 0, 4),
            Err(ResultHandleError::NotFound),
            "possession alone grants no authority in a foreign session"
        );
        assert_eq!(
            cross_root.expand(&foreign.store, &token, 0, 4),
            Err(ResultHandleError::NotFound),
            "handles never cross a root boundary"
        );
        assert_eq!(
            second.expand_with_live_barrier(
                &fixture.store,
                &token,
                0,
                4,
                LiveExpansionBarrier::clean(),
            ),
            Err(ResultHandleError::NotFound),
        );
        first.expand(&fixture.store, &token, 0, 4).unwrap();
    }

    #[test]
    fn foreign_namespace_mint_and_expand_rejected() {
        let _vault = lock_unit_vault_for_test();
        let fixture = Fixture::new("mint-a", b"0123456789abcdef");
        let foreign = Fixture::new("mint-b", b"0123456789abcdef");
        let mut catalog = ResultHandleCatalog::new(&fixture.namespace());
        assert_eq!(catalog.namespace_id(), fixture.namespace());
        let foreign_summary = foreign.store.list_sources().into_iter().next().unwrap();
        let foreign_match = StoredMatch {
            source_id: foreign_summary.source_id,
            revision_id: foreign_summary.revision_id,
            content_digest: foreign_summary.content_digest,
            path_digest: foreign_summary.path_digest,
            evidence_id: "evidence-foreign".to_owned(),
            byte_start: 0,
            byte_end: 4,
            line: 1,
            column_bytes: 1,
        };
        assert_eq!(
            catalog.mint_page(&foreign.store, &[foreign_match]),
            Err(ResultHandleError::SourceFenceChanged),
            "a catalog bound to one root never mints for a foreign root"
        );
        let public = catalog
            .mint_page(&fixture.store, &[fixture.handle_match(0, 4, 0)])
            .unwrap();
        let token = public.first().unwrap().source_handle.clone();
        assert_eq!(
            catalog.expand(&foreign.store, &token, 0, 4),
            Err(ResultHandleError::SourceFenceChanged),
        );
        assert_eq!(
            catalog.expand(&fixture.store, &token, 0, 4),
            Err(ResultHandleError::NotFound),
            "namespace drift drops the handle instead of narrowing it"
        );
    }

    #[test]
    fn expired_handle_reports_expired() {
        let _vault = lock_unit_vault_for_test();
        let fixture = Fixture::new("expiry", b"0123456789abcdef");
        let mut catalog = ResultHandleCatalog::new(&fixture.namespace());
        let public = catalog
            .mint_page(&fixture.store, &[fixture.handle_match(0, 4, 0)])
            .unwrap();
        let token = public.first().unwrap().source_handle.clone();
        assert!(catalog.force_expire_for_tests(&token));
        assert_eq!(
            catalog.expand(&fixture.store, &token, 0, 4),
            Err(ResultHandleError::Expired),
        );
        assert_eq!(catalog.live_count(), 0);
    }

    #[test]
    fn revoked_and_purged_barriers_drop_handle() {
        let _vault = lock_unit_vault_for_test();
        let fixture = Fixture::new("barrier", b"0123456789abcdef");
        let mut catalog = ResultHandleCatalog::new(&fixture.namespace());
        for (index, (barrier, expected)) in [
            (
                LiveExpansionBarrier {
                    generation_moved: false,
                    access_revoked: true,
                    purged: false,
                },
                ResultHandleError::AccessRevoked,
            ),
            (
                LiveExpansionBarrier {
                    generation_moved: false,
                    access_revoked: false,
                    purged: true,
                },
                ResultHandleError::Purged,
            ),
            (
                LiveExpansionBarrier {
                    generation_moved: true,
                    access_revoked: false,
                    purged: false,
                },
                ResultHandleError::SourceFenceChanged,
            ),
        ]
        .into_iter()
        .enumerate()
        {
            let public = catalog
                .mint_page(&fixture.store, &[fixture.handle_match(0, 4, index)])
                .unwrap();
            let token = public.first().unwrap().source_handle.clone();
            assert_eq!(
                catalog.expand_with_live_barrier(&fixture.store, &token, 0, 4, barrier),
                Err(expected),
            );
            assert_eq!(
                catalog.expand(&fixture.store, &token, 0, 4),
                Err(ResultHandleError::NotFound),
                "a barrier-denied handle is dropped, never resumed"
            );
        }
        assert_eq!(catalog.live_count(), 0);
    }

    #[test]
    fn retired_source_denies_expansion_and_drops_handle() {
        let _vault = lock_unit_vault_for_test();
        let mut fixture = Fixture::new("retire", b"0123456789abcdef");
        let mut catalog = ResultHandleCatalog::new(&fixture.namespace());
        let source_id = fixture.summary().source_id;
        let public = catalog
            .mint_page(&fixture.store, &[fixture.handle_match(0, 4, 0)])
            .unwrap();
        let token = public.first().unwrap().source_handle.clone();
        fixture.store.retire_source(&source_id).unwrap();
        assert_eq!(
            catalog.expand(&fixture.store, &token, 0, 4),
            Err(ResultHandleError::SourceFenceChanged),
            "retirement invalidates the live fence before any byte is read"
        );
        assert_eq!(
            catalog.expand(&fixture.store, &token, 0, 4),
            Err(ResultHandleError::NotFound),
        );
    }

    #[test]
    fn exact_provenance_readback_roundtrip() {
        let _vault = lock_unit_vault_for_test();
        let fixture = Fixture::new("readback", b"0123456789abcdef");
        let mut catalog = ResultHandleCatalog::new(&fixture.namespace());
        let public = catalog
            .mint_page(&fixture.store, &[fixture.handle_match(0, 4, 0)])
            .unwrap();
        let token = public.first().unwrap().source_handle.clone();
        let expansion = catalog.expand(&fixture.store, &token, 4, 10).unwrap();
        assert_eq!(expansion.byte_start, 4);
        assert_eq!(expansion.byte_end, 10);
        assert_eq!(expansion.source_byte_length, 16);
        assert_eq!(expansion.bytes, b"456789");
        assert_eq!(
            catalog.expand(&fixture.store, "not-a-token", 4, 10),
            Err(ResultHandleError::NotFound),
        );
    }

    #[test]
    fn range_widening_and_oversize_denied() {
        let _vault = lock_unit_vault_for_test();
        let wide = vec![b'x'; 30 * 1024];
        let fixture = Fixture::new("ranges", &wide);
        let mut catalog = ResultHandleCatalog::new(&fixture.namespace());
        let public = catalog
            .mint_page(&fixture.store, &[fixture.handle_match(0, 6, 0)])
            .unwrap();
        let token = public.first().unwrap().source_handle.clone();
        assert_eq!(
            catalog.expand(&fixture.store, &token, 0, 30 * 1024),
            Err(ResultHandleError::ExpansionTooLarge),
            "one expansion never exceeds the finite disclosure ceiling"
        );
        assert_eq!(
            catalog.expand(&fixture.store, &token, 0, 30 * 1024 + 1),
            Err(ResultHandleError::RangeInvalid),
            "ranges cannot widen past the retained revision"
        );
        assert_eq!(
            catalog.expand(&fixture.store, &token, 8, 8),
            Err(ResultHandleError::RangeInvalid),
        );
        assert_eq!(
            catalog.expand(&fixture.store, &token, 9, 8),
            Err(ResultHandleError::RangeInvalid),
        );
        catalog
            .expand(&fixture.store, &token, 0, 24 * 1024)
            .unwrap();
    }

    #[test]
    fn durable_mint_is_denied_for_ephemeral_catalog() {
        let _vault = lock_unit_vault_for_test();
        let fixture = Fixture::new("durable", b"0123456789abcdef");
        let mut catalog = ResultHandleCatalog::new(&fixture.namespace());
        assert_eq!(
            ResultHandleCatalog::mint_durable_source(),
            Err(ResultHandleError::DurableRetentionRequired),
            "ordinary query handles are ephemeral-only; durable evidence needs explicit retention"
        );
        assert_eq!(catalog.live_count(), 0, "denied mints insert no record");
    }

    #[test]
    fn restart_forgets_handles() {
        let _vault = lock_unit_vault_for_test();
        let fixture = Fixture::new("restart", b"0123456789abcdef");
        let namespace = fixture.namespace();
        let mut first = ResultHandleCatalog::new(&namespace);
        let public = first
            .mint_page(&fixture.store, &[fixture.handle_match(0, 4, 0)])
            .unwrap();
        let token = public.first().unwrap().source_handle.clone();
        drop(first);
        let mut restarted = ResultHandleCatalog::new(&namespace);
        assert_eq!(restarted.live_count(), 0);
        assert_eq!(
            restarted.expand(&fixture.store, &token, 0, 4),
            Err(ResultHandleError::NotFound),
            "ephemeral handles never survive a restart"
        );
    }

    #[test]
    fn handle_capacity_bounded_and_releasable() {
        let _vault = lock_unit_vault_for_test();
        let fixture = Fixture::new("quota", b"0123456789abcdef");
        let mut catalog = ResultHandleCatalog::new(&fixture.namespace());
        let summary = fixture.summary();
        for chunk in 0..5 {
            let matches = (0..10_000)
                .map(|index| StoredMatch {
                    source_id: summary.source_id.clone(),
                    revision_id: summary.revision_id.clone(),
                    content_digest: summary.content_digest.clone(),
                    path_digest: summary.path_digest.clone(),
                    evidence_id: format!("evidence-{chunk}-{index}"),
                    byte_start: 0,
                    byte_end: 4,
                    line: 1,
                    column_bytes: 1,
                })
                .collect::<Vec<_>>();
            catalog.mint_page(&fixture.store, &matches).unwrap();
        }
        assert_eq!(catalog.live_count(), MAX_RESULT_HANDLES);
        assert_eq!(
            catalog.mint_page(&fixture.store, &[fixture.handle_match(0, 4, 0)]),
            Err(ResultHandleError::CapacityExceeded),
        );
        assert_eq!(catalog.invalidate_all(), MAX_RESULT_HANDLES);
        assert_eq!(catalog.live_count(), 0);
        catalog
            .mint_page(&fixture.store, &[fixture.handle_match(0, 4, 0)])
            .unwrap();
    }

    #[test]
    fn expired_handles_reaped_by_next_operation() {
        let _vault = lock_unit_vault_for_test();
        let fixture = Fixture::new("reap", b"0123456789abcdef");
        let mut catalog = ResultHandleCatalog::new(&fixture.namespace());
        let public = catalog
            .mint_page(
                &fixture.store,
                &[fixture.handle_match(0, 4, 0), fixture.handle_match(4, 8, 1)],
            )
            .unwrap();
        assert!(catalog.force_expire_for_tests(&public[0].source_handle));
        catalog
            .mint_page(&fixture.store, &[fixture.handle_match(8, 12, 2)])
            .unwrap();
        assert_eq!(catalog.live_count(), 2, "the aged-out handle is reaped");
    }

    #[test]
    fn foreign_session_tag_is_not_found() {
        let _vault = lock_unit_vault_for_test();
        let fixture = Fixture::new("tag", b"0123456789abcdef");
        let mut catalog = ResultHandleCatalog::new(&fixture.namespace());
        let summary = fixture.summary();
        let token = "f".repeat(64);
        catalog.records.insert(
            token.clone(),
            ResultHandleRecord {
                source_fence_digest: source_fence(&fixture.store),
                source_id: summary.source_id,
                revision_id: summary.revision_id,
                content_digest: summary.content_digest,
                byte_length: summary.byte_length,
                expires_at: std::time::Instant::now() + RESULT_HANDLE_TTL,
                namespace_id: fixture.namespace(),
                session_tag: catalog.session_tag.wrapping_add(1),
            },
        );
        assert_eq!(
            catalog.expand(&fixture.store, &token, 0, 4),
            Err(ResultHandleError::NotFound),
            "a record from another session is unusable here"
        );
    }

    #[test]
    fn catalog_debug_redacts_tokens() {
        let _vault = lock_unit_vault_for_test();
        let fixture = Fixture::new("redact", b"0123456789abcdef");
        let mut catalog = ResultHandleCatalog::new(&fixture.namespace());
        let public = catalog
            .mint_page(&fixture.store, &[fixture.handle_match(0, 4, 0)])
            .unwrap();
        let token = public.first().unwrap().source_handle.clone();
        let rendered = format!("{catalog:?}");
        assert!(rendered.contains("live_handles"), "{rendered}");
        assert!(
            !rendered.contains(&token),
            "plaintext tokens never reach debug output"
        );
    }
}

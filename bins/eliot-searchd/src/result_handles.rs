//! Process-local opaque source handles for public DIRECT search pages.
//!
//! Handles are session locators, not production bearer credentials. The token
//! contains no source, path, revision, range, content, or authority data. The
//! server-side record is bound to current source state, one immutable revision,
//! a finite TTL, and a finite per-expansion disclosure ceiling.

use std::collections::BTreeMap;
use std::time::{Duration, Instant, SystemTime, UNIX_EPOCH};

use crate::direct_store::{DirectStore, RevisionSlice, StoredMatch};
use crate::sha256;

/// Maximum simultaneous result handles.
pub(crate) const MAX_RESULT_HANDLES: usize = 50_000;
/// Maximum exact bytes returned by one handle expansion.
pub(crate) const MAX_HANDLE_EXPANSION_BYTES: u64 = 24 * 1024;
/// Finite process-local handle lifetime.
pub(crate) const RESULT_HANDLE_TTL: Duration = Duration::from_secs(15 * 60);

/// Closed result-handle failure.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum ResultHandleError {
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
        }
    }
}

/// Public non-self-describing handle attached to one match.
#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) struct PublicHandledMatch {
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
    source_fence_digest: String,
    source_id: String,
    revision_id: String,
    content_digest: String,
    byte_length: u64,
    expires_at: Instant,
}

/// Exact bounded expansion of one opaque handle.
#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) struct ResultHandleExpansion {
    pub(crate) source_handle: String,
    pub(crate) byte_start: u64,
    pub(crate) byte_end: u64,
    pub(crate) source_byte_length: u64,
    pub(crate) bytes: Vec<u8>,
}

/// Finite process-local result-handle catalog for one owner session.
#[derive(Debug)]
pub(crate) struct ResultHandleCatalog {
    session_nonce: [u8; 32],
    next_counter: u64,
    records: BTreeMap<String, ResultHandleRecord>,
}

impl ResultHandleCatalog {
    /// Creates one session-local catalog.
    pub(crate) fn new(namespace_id: &str) -> Self {
        let now = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap_or_default()
            .as_nanos();
        let session_nonce = sha256::digest_parts(
            b"eliot-search/direct-result-handle-session/v1",
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
        }
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
    pub(crate) fn mint_page(
        &mut self,
        store: &DirectStore,
        matches: &[StoredMatch],
    ) -> Result<Vec<PublicHandledMatch>, ResultHandleError> {
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

    /// Expands one exact source range after TTL, source-state, revision, and
    /// immutable readback verification.
    pub(crate) fn expand(
        &mut self,
        store: &DirectStore,
        token: &str,
        byte_start: u64,
        byte_end: u64,
    ) -> Result<ResultHandleExpansion, ResultHandleError> {
        self.expire();
        let record = self
            .records
            .get(token)
            .cloned()
            .ok_or(ResultHandleError::NotFound)?;
        if Instant::now() >= record.expires_at {
            self.records.remove(token);
            return Err(ResultHandleError::Expired);
        }
        if record.source_fence_digest != source_fence(store) {
            self.records.remove(token);
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
        Ok(ResultHandleExpansion {
            source_handle: token.to_owned(),
            byte_start,
            byte_end,
            source_byte_length: record.byte_length,
            bytes,
        })
    }

    fn expire(&mut self) {
        let now = Instant::now();
        self.records.retain(|_, record| record.expires_at > now);
    }

    fn allocate_token(&mut self) -> Result<String, ResultHandleError> {
        for _ in 0..128 {
            let counter = self.next_counter;
            self.next_counter = self
                .next_counter
                .checked_add(1)
                .ok_or(ResultHandleError::TokenExhausted)?;
            let token = sha256::hex(&sha256::digest_parts(
                b"eliot-search/direct-result-handle-token/v1",
                &[&self.session_nonce, &counter.to_be_bytes()],
            ));
            if !self.records.contains_key(&token) {
                return Ok(token);
            }
        }
        Err(ResultHandleError::TokenExhausted)
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

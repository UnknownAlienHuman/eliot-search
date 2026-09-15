//! Ephemeral handle minting, finite catalog state and token lifecycle.

use core::fmt;
use std::collections::BTreeMap;
use std::time::Instant;

use crate::continuation::qualified_entropy_32;
use crate::direct_store::{DirectStore, StoredMatch};
use crate::sha256;
use crate::source_fence::digest as source_fence;

use super::error::ResultHandleError;
use super::model::{PublicHandledMatch, ResultHandleRecord};
use super::spec::{MAX_RESULT_HANDLES, RESULT_HANDLE_TTL};

/// Finite live-authorized result-handle catalog for one owner session.
pub struct ResultHandleCatalog {
    pub(super) namespace_id: String,
    pub(super) session_tag: u64,
    pub(super) entropy_poisoned: bool,
    pub(super) records: BTreeMap<String, ResultHandleRecord>,
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

    #[must_use]
    pub(crate) fn namespace_id(&self) -> &str {
        &self.namespace_id
    }

    pub(crate) fn live_count(&mut self) -> usize {
        self.expire();
        self.records.len()
    }

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
        let expires_in_ms =
            u64::try_from(RESULT_HANDLE_TTL.as_millis()).unwrap_or(u64::MAX);
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

    /// Durable handles require an external immutable-retention lease.
    #[allow(dead_code)]
    pub(crate) const fn mint_durable_source() -> Result<(), ResultHandleError> {
        Err(ResultHandleError::DurableRetentionRequired)
    }

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

    pub(super) fn expire(&mut self) {
        let now = Instant::now();
        self.records
            .retain(|_, record| record.expires_at > now);
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

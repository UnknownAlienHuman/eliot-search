//! Live-authorized exact expansion and immutable provenance revalidation.

use std::time::Instant;

use crate::continuation::LiveExpansionBarrier;
use crate::direct_store::{DirectStore, RevisionSlice};
use crate::source_fence::digest as source_fence;

use super::catalog::ResultHandleCatalog;
use super::error::ResultHandleError;
use super::model::ResultHandleExpansion;
use super::spec::MAX_HANDLE_EXPANSION_BYTES;

impl ResultHandleCatalog {
    /// Immediate compatibility seam for existing source/authority fixtures.
    #[cfg(test)]
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

    #[cfg(test)]
    pub(crate) fn expand_with_live_barrier(
        &mut self,
        store: &DirectStore,
        token: &str,
        byte_start: u64,
        byte_end: u64,
        barrier: LiveExpansionBarrier,
    ) -> Result<ResultHandleExpansion, ResultHandleError> {
        self.prepare_expand_with_live_barrier(store, token, byte_start, byte_end, barrier)
            .map(|prepared| prepared.expansion)
    }

    /// Retains the handle borrow and original deadline through response delivery.
    pub(crate) fn prepare_expand(
        &mut self,
        store: &DirectStore,
        token: &str,
        byte_start: u64,
        byte_end: u64,
    ) -> Result<PreparedExpansion<'_>, ResultHandleError> {
        self.prepare_expand_with_live_barrier(
            store,
            token,
            byte_start,
            byte_end,
            LiveExpansionBarrier::clean(),
        )
    }

    /// Expands one handle after the live-authority checkpoint and exact
    /// namespace, fence, source, revision and readback revalidation.
    fn prepare_expand_with_live_barrier(
        &mut self,
        store: &DirectStore,
        token: &str,
        byte_start: u64,
        byte_end: u64,
        barrier: LiveExpansionBarrier,
    ) -> Result<PreparedExpansion<'_>, ResultHandleError> {
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
        // Readback and expiry housekeeping may outlast the initial TTL gate.
        // Do not return source bytes from an expired (or swept) handle.
        PreparedExpansion::new(
            self,
            ResultHandleExpansion {
                source_handle: token.to_owned(),
                byte_start,
                byte_end,
                source_byte_length: record.byte_length,
                bytes,
            },
            record.expires_at,
            Instant::now(),
        )
    }
}

/// Verified bytes awaiting output, not a new record or a transferable authority.
/// An exclusive catalog borrow keeps the original handle stable until drop.
#[must_use]
pub(crate) struct PreparedExpansion<'a> {
    catalog: &'a mut ResultHandleCatalog,
    expansion: ResultHandleExpansion,
    expires_at: Instant,
}

impl<'a> PreparedExpansion<'a> {
    fn new(
        catalog: &'a mut ResultHandleCatalog,
        expansion: ResultHandleExpansion,
        expires_at: Instant,
        now: Instant,
    ) -> Result<Self, ResultHandleError> {
        let mut prepared = Self { catalog, expansion, expires_at };
        prepared.revalidate_at(now)?;
        Ok(prepared)
    }

    fn revalidate_at(&mut self, now: Instant) -> Result<(), ResultHandleError> {
        let token = &self.expansion.source_handle;
        if now >= self.expires_at || !self.catalog.records.contains_key(token) {
            self.catalog.records.remove(token);
            self.catalog.expire();
            return Err(ResultHandleError::Expired);
        }
        Ok(())
    }

    /// Checks again after caller-side diagnostics and just before output.
    /// The callback must enforce the supplied absolute deadline on writes.
    pub(crate) fn deliver(
        self,
        emit: impl FnOnce(&ResultHandleExpansion, Instant) -> Result<(), String>,
    ) -> Result<(), String> {
        self.deliver_at(Instant::now(), emit)
    }

    fn deliver_at(
        mut self,
        now: Instant,
        emit: impl FnOnce(&ResultHandleExpansion, Instant) -> Result<(), String>,
    ) -> Result<(), String> {
        self.revalidate_at(now).map_err(|error| error.code().to_owned())?;
        // No post-output check: writer success is not retroactively denied.
        // Dropping the preparation discards its bytes, never advances a cursor
        // or renews the handle. Session failure owns whole-session invalidation.
        emit(&self.expansion, self.expires_at)
    }
}

#[cfg(test)]
mod deadline_tests;

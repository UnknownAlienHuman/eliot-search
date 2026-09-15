//! Bounded retained-revision preparation cursor and batch orchestration.

use std::time::Instant;

use zeroize::Zeroizing;

use super::persist::persist_canonical;
use super::spec::{
    BATCH_SLICE, MAX_BATCH_REVISIONS, MAX_BATCH_SOURCE_BYTES,
};
use super::super::super::DirectStore;
use crate::direct_preparation::{
    CanonicalPreparationReceipt, canonical_materializer_digest,
    canonical_unitizer_digest,
};
use crate::sha256;

/// Stateless admin bookmark, never a bearer token or proof of the processed
/// prefix. It is bound to source-event history, preparation profiles and the
/// storage backend.
pub struct PreparationCursor {
    checkpoint: [u8; 32],
    after: String,
}

impl PreparationCursor {
    pub(crate) fn parse(value: &str) -> Result<Self, String> {
        if value.len() != 132
            || !value.is_ascii()
            || !value.starts_with("v1.")
            || value.as_bytes()[67] != b'.'
        {
            return Err("DIRECT_PREPARATION_CURSOR_INVALID".to_owned());
        }
        let checkpoint = sha256::decode_digest(&value[3..67])
            .ok_or_else(|| "DIRECT_PREPARATION_CURSOR_INVALID".to_owned())?;
        let after = &value[68..];
        if sha256::hex(&checkpoint) != value[3..67]
            || sha256::decode_digest(after).is_none()
            || after.bytes().any(|byte| byte.is_ascii_uppercase())
        {
            return Err("DIRECT_PREPARATION_CURSOR_INVALID".to_owned());
        }
        Ok(Self {
            checkpoint,
            after: after.to_owned(),
        })
    }
}

/// One bounded suffix batch. Exhaustion is not a complete-corpus search proof.
pub struct PreparationBatch {
    pub(crate) stored: usize,
    pub(crate) layouts: usize,
    pub(crate) source_bytes: u64,
    pub(crate) gaps: Vec<(String, &'static str)>,
    pub(crate) next_cursor: Option<String>,
    /// Canonical per-revision bindings: `(revision_id, representation_hex, gap)`.
    /// No receipt is stored.
    pub(crate) manifests: Vec<(String, String, Option<&'static str>)>,
}

impl PreparationBatch {
    /// Canonical manifest bindings in processing order as
    /// `(revision_id, representation_hex, gap)` triples.
    #[must_use]
    pub fn manifests(&self) -> &[(String, String, Option<&'static str>)] {
        &self.manifests
    }
}

impl DirectStore {
    fn preparation_checkpoint(&self) -> [u8; 32] {
        let materializer = canonical_materializer_digest().unwrap_or([0; 32]);
        let unitizer = canonical_unitizer_digest().unwrap_or([0; 32]);
        sha256::digest_parts(
            b"eliot-search/direct-preparation-cursor/v2",
            &[
                &self.inner.preparation_catalog_digest(),
                &materializer,
                &unitizer,
                self.protector.backend_name().as_bytes(),
            ],
        )
    }

    /// Canonical single-revision prepare returning real provenance.
    pub(crate) fn prepare_revision_canonical(
        &self,
        revision_id: &str,
    ) -> Result<CanonicalPreparationReceipt, String> {
        crate::catalog_presence::require_existing(&self.root)?;
        self.inner.verify_control()?;
        let metadata = self
            .inner
            .retained_revision(revision_id)
            .ok_or_else(|| "DIRECT_REVISION_NOT_FOUND".to_owned())?;
        let bytes = Zeroizing::new(self.read_revision_detailed(&metadata)?);
        persist_canonical(
            &self.root,
            &self.protector,
            &self.inner.namespace_id(),
            &metadata,
            &bytes,
        )
    }

    /// Cheap pre-dispatch validation against the admitted catalog snapshot.
    pub(crate) fn validate_preparation_cursor(
        &self,
        cursor: Option<&PreparationCursor>,
    ) -> Result<(), String> {
        if let Some(cursor) = cursor
            && (cursor.checkpoint != self.preparation_checkpoint()
                || self.inner.retained_revision(&cursor.after).is_none())
        {
            return Err("DIRECT_PREPARATION_CURSOR_STALE".to_owned());
        }
        Ok(())
    }

    /// Restart-safe bounded backfill of retained revisions, including retired
    /// history. Each object retains its immutable commit/readback boundary.
    pub(crate) fn prepare_root(
        &self,
        cursor: Option<&PreparationCursor>,
    ) -> Result<PreparationBatch, String> {
        crate::catalog_presence::require_existing(&self.root)?;
        self.inner.verify_control()?;
        self.validate_preparation_cursor(cursor)?;
        let checkpoint = self.preparation_checkpoint();
        let namespace = self.inner.namespace_id();
        let mut pending = self
            .inner
            .retained_revisions_after(cursor.map(|value| value.after.as_str()))
            .peekable();
        let mut batch = PreparationBatch {
            stored: 0,
            layouts: 0,
            source_bytes: 0,
            gaps: Vec::new(),
            next_cursor: None,
            manifests: Vec::new(),
        };
        let started = Instant::now();
        let mut last = None;

        while let Some(metadata) = pending.peek() {
            if batch.stored >= MAX_BATCH_REVISIONS
                || batch
                    .source_bytes
                    .checked_add(metadata.byte_length)
                    .is_none_or(|bytes| bytes > MAX_BATCH_SOURCE_BYTES)
                || (batch.stored > 0 && started.elapsed() >= BATCH_SLICE)
            {
                break;
            }
            let metadata = pending
                .next()
                .ok_or_else(|| "DIRECT_PREPARATION_NO_PROGRESS".to_owned())?;
            let bytes = Zeroizing::new(self.read_revision_detailed(&metadata)?);
            let receipt = persist_canonical(
                &self.root,
                &self.protector,
                &namespace,
                &metadata,
                &bytes,
            )?;
            batch.source_bytes += metadata.byte_length;
            batch.stored += 1;
            if let Some(reason) = receipt.gap {
                batch.gaps.push((metadata.revision_id.clone(), reason));
            } else {
                batch.layouts += 1;
            }
            batch.manifests.push((
                metadata.revision_id.clone(),
                receipt.representation_hex(),
                receipt.gap,
            ));
            last = Some(metadata.revision_id);
        }

        if pending.peek().is_some() {
            let last = last
                .ok_or_else(|| "DIRECT_PREPARATION_NO_PROGRESS".to_owned())?;
            batch.next_cursor =
                Some(format!("v1.{}.{last}", sha256::hex(&checkpoint)));
        }
        self.inner.verify_control()?;
        Ok(batch)
    }
}

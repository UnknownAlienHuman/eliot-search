//! Exact retained-revision reads, verification and content classification.

use crate::plaintext_direct_store::{
    RevisionMetadata, RevisionSlice, StoreVerification,
    verify_revision_identity,
};
use crate::sha256;

use super::DirectStore;

const MAX_READ_RANGE_BYTES: usize = 8 * 1024 * 1024;

impl DirectStore {
    /// Reopens the event log and verifies every referenced revision object.
    pub(crate) fn verify(&self) -> Result<StoreVerification, String> {
        crate::catalog_presence::require_existing(&self.root)?;
        self.inner.verify_control()?;
        let mut total_revision_bytes = 0_u64;
        let mut verified_revisions = 0_usize;
        for metadata in self.inner.retained_revisions() {
            let bytes = self
                .read_revision_detailed(&metadata)
                .map_err(|error| {
                    format!("DIRECT_REVISION_VERIFY_FAILED:{error}")
                })?;
            total_revision_bytes = total_revision_bytes
                .checked_add(
                    u64::try_from(bytes.len())
                        .map_err(|_| "DIRECT_TOTAL_BYTES_OVERFLOW".to_owned())?,
                )
                .ok_or_else(|| "DIRECT_TOTAL_BYTES_OVERFLOW".to_owned())?;
            verified_revisions = verified_revisions.saturating_add(1);
        }
        let sources = self.inner.list_sources();
        Ok(StoreVerification {
            source_events: self.inner.source_event_count(),
            registered_sources: sources.len(),
            active_sources: sources.iter().filter(|source| source.active).count(),
            referenced_revisions: self.inner.retained_revisions().len(),
            verified_revisions,
            total_revision_bytes,
        })
    }

    /// Reads one bounded exact range from a verified immutable revision.
    pub(crate) fn read_revision_range(
        &self,
        revision_id: &str,
        byte_start: u64,
        byte_end: u64,
    ) -> Result<RevisionSlice, String> {
        let metadata = self
            .inner
            .retained_revision(revision_id)
            .ok_or_else(|| "DIRECT_REVISION_NOT_FOUND".to_owned())?;
        if byte_start >= byte_end || byte_end > metadata.byte_length {
            return Err("DIRECT_REVISION_RANGE_INVALID".to_owned());
        }
        if byte_end.saturating_sub(byte_start)
            > u64::try_from(MAX_READ_RANGE_BYTES).unwrap_or(u64::MAX)
        {
            return Err("DIRECT_REVISION_RANGE_TOO_LARGE".to_owned());
        }
        let bytes = self.read_revision_detailed(&metadata)?;
        let start = usize::try_from(byte_start)
            .map_err(|_| "DIRECT_REVISION_RANGE_INVALID".to_owned())?;
        let end = usize::try_from(byte_end)
            .map_err(|_| "DIRECT_REVISION_RANGE_INVALID".to_owned())?;
        Ok(RevisionSlice {
            revision_id: metadata.revision_id,
            content_digest: metadata.content_digest,
            byte_start,
            byte_end,
            bytes: bytes[start..end].to_vec(),
        })
    }

    pub(super) fn read_verified_revision(
        &self,
        metadata: &RevisionMetadata,
    ) -> Result<Vec<u8>, &'static str> {
        self.read_revision_detailed(metadata)
            .map_err(|error| classify_revision_error(&error))
    }
}

fn classify_revision_error(error: &str) -> &'static str {
    if error.contains("KEY_BINDING") || error.contains("KEY_MISSING") {
        "DIRECT_REVISION_KEY_UNAVAILABLE"
    } else if error.contains("DPAPI") || error.contains("ENCRYPTION") {
        "DIRECT_REVISION_DECRYPT_FAILED"
    } else if error.contains("CONTENT") {
        "DIRECT_REVISION_CONTENT_MISMATCH"
    } else if error.contains("LENGTH") || error.contains("TOO_LARGE") {
        "DIRECT_REVISION_LENGTH_MISMATCH"
    } else if error.contains("MISSING") {
        "DIRECT_REVISION_MISSING"
    } else if error.contains("OBJECT") || error.contains("FILE") {
        "DIRECT_REVISION_OBJECT_INVALID"
    } else {
        "DIRECT_REVISION_READ_FAILED"
    }
}

pub(super) fn verify_plaintext(
    metadata: &RevisionMetadata,
    bytes: &[u8],
) -> Result<(), String> {
    let length = u64::try_from(bytes.len())
        .map_err(|_| "DIRECT_REVISION_LENGTH_MISMATCH".to_owned())?;
    if length != metadata.byte_length {
        return Err("DIRECT_REVISION_LENGTH_MISMATCH".to_owned());
    }
    if sha256::hex(&sha256::digest(bytes)) != metadata.content_digest {
        return Err("DIRECT_REVISION_CONTENT_MISMATCH".to_owned());
    }
    verify_revision_identity(metadata)
}

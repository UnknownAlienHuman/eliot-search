//! Exact retained-revision acquisition through an injected read port.

use crate::MaterializationError;
use crate::request::{CancellationToken, ValidatedMaterializationRequest};
use search_contracts::{Blake3Digest32, NonZeroRevision, OpaqueId};

/// Revision bytes attested by the revision pipeline for one exact revision.
#[derive(Clone, Eq, PartialEq)]
pub struct StoredRevisionBytes {
    bytes: Vec<u8>,
    content_digest: Blake3Digest32,
    residency: OpaqueId,
}

impl StoredRevisionBytes {
    /// Builds port-attested revision bytes. The port owns digest correctness;
    /// [`open_exact_revision`] binds it to the validated request.
    #[must_use]
    pub const fn new(bytes: Vec<u8>, content_digest: Blake3Digest32, residency: OpaqueId) -> Self {
        Self {
            bytes,
            content_digest,
            residency,
        }
    }

    /// Attested bytes.
    #[must_use]
    pub fn bytes(&self) -> &[u8] {
        &self.bytes
    }

    /// Port-attested content digest.
    #[must_use]
    pub const fn content_digest(&self) -> Blake3Digest32 {
        self.content_digest
    }

    /// Port-attested residency identity.
    #[must_use]
    pub const fn residency(&self) -> &OpaqueId {
        &self.residency
    }

    /// Byte length.
    #[must_use]
    pub const fn len(&self) -> usize {
        self.bytes.len()
    }

    /// Reports whether the attested bytes are empty.
    #[must_use]
    pub const fn is_empty(&self) -> bool {
        self.bytes.is_empty()
    }
}

impl core::fmt::Debug for StoredRevisionBytes {
    fn fmt(&self, formatter: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        formatter
            .debug_struct("StoredRevisionBytes")
            .field("bytes", &format_args!("<{} bytes>", self.bytes.len()))
            .field("content_digest", &self.content_digest)
            .field("residency", &self.residency)
            .finish()
    }
}

/// Injected exact revision-read port. Implementations reopen exactly the
/// retained revision and attest its digest and residency; they never
/// enumerate roots or read pathnames.
pub trait RevisionReadPort {
    /// Reads exactly the retained revision or fails with a typed error
    /// (notably [`MaterializationError::RevisionUnavailable`]).
    fn read_exact(
        &self,
        source: &OpaqueId,
        revision: NonZeroRevision,
        byte_count: u64,
    ) -> Result<StoredRevisionBytes, MaterializationError>;
}

/// Bounded process-memory byte guard for one verified revision.
#[derive(Clone, Eq, PartialEq)]
pub struct RevisionBytesGuard {
    bytes: Vec<u8>,
    source_id: OpaqueId,
    revision: NonZeroRevision,
    residency: OpaqueId,
    content_digest: Blake3Digest32,
}

impl RevisionBytesGuard {
    /// Verified revision bytes.
    #[must_use]
    pub fn bytes(&self) -> &[u8] {
        &self.bytes
    }

    /// Consumes the guard into its verified bytes.
    #[must_use]
    pub fn into_bytes(self) -> Vec<u8> {
        self.bytes
    }

    /// Byte length.
    #[must_use]
    pub const fn len(&self) -> usize {
        self.bytes.len()
    }

    /// Reports whether the verified bytes are empty.
    #[must_use]
    pub const fn is_empty(&self) -> bool {
        self.bytes.is_empty()
    }

    /// Verified source identity.
    #[must_use]
    pub const fn source_id(&self) -> &OpaqueId {
        &self.source_id
    }

    /// Verified retained revision.
    #[must_use]
    pub const fn revision(&self) -> NonZeroRevision {
        self.revision
    }

    /// Verified residency identity.
    #[must_use]
    pub const fn residency(&self) -> &OpaqueId {
        &self.residency
    }

    /// Verified content digest.
    #[must_use]
    pub const fn content_digest(&self) -> Blake3Digest32 {
        self.content_digest
    }
}

impl core::fmt::Debug for RevisionBytesGuard {
    fn fmt(&self, formatter: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        formatter
            .debug_struct("RevisionBytesGuard")
            .field("bytes", &format_args!("<{} bytes>", self.bytes.len()))
            .field("source_id", &self.source_id)
            .field("revision", &self.revision)
            .field("residency", &self.residency)
            .field("content_digest", &self.content_digest)
            .finish()
    }
}

/// Reopens exactly the retained revision and binds it to the request.
///
/// Verifies port-attested residency, content digest and byte length against
/// the validated request. Mismatch, unavailable residency or retention loss
/// is an explicit typed error, never substituted content.
pub fn open_exact_revision(
    request: &ValidatedMaterializationRequest,
    port: &dyn RevisionReadPort,
    cancel: CancellationToken<'_>,
) -> Result<RevisionBytesGuard, MaterializationError> {
    if cancel.is_cancelled() {
        return Err(MaterializationError::Cancelled);
    }
    let stored = port.read_exact(
        request.source_id(),
        request.revision(),
        request.byte_count(),
    )?;
    if stored.residency != *request.residency() {
        return Err(MaterializationError::ResidencyMismatch);
    }
    if stored.content_digest != request.content_digest() {
        return Err(MaterializationError::RevisionDigestMismatch);
    }
    let actual =
        u64::try_from(stored.bytes.len()).map_err(|_| MaterializationError::OffsetOverflow)?;
    if actual != request.byte_count() {
        return Err(MaterializationError::InputLengthMismatch);
    }
    if stored.bytes.is_empty() {
        return Err(MaterializationError::EmptyInput);
    }
    Ok(RevisionBytesGuard {
        bytes: stored.bytes,
        source_id: request.source_id().clone(),
        revision: request.revision(),
        residency: stored.residency,
        content_digest: stored.content_digest,
    })
}

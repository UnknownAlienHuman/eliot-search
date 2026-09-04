//! Exact committed retired-point manifests.

use search_contracts::{
    Blake3Digest32, CollectionGenerationId, Epoch, ReceiptRef,
};
use search_epoch_pins::RouteIdentity;

use crate::ReclaimError;

/// Exact provider-neutral 128-bit point identifier.
#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub struct ReclaimPointId(pub [u8; 16]);

/// Exact retired-point manifest emitted after visible-epoch commit.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct RetiredPointManifest {
    /// Physical collection generation containing the points.
    pub collection_generation_id: CollectionGenerationId,
    /// Logical collection route containing the points.
    pub route: RouteIdentity,
    /// First epoch at which these points are not visible.
    pub retirement_epoch_exclusive: Epoch,
    /// Digest of the canonical exact-ID manifest.
    pub manifest_digest: Blake3Digest32,
    /// Publication receipt that committed retirement.
    pub publication_receipt_ref: ReceiptRef,
    /// Canonically ordered exact point identifiers.
    pub point_ids: Vec<ReclaimPointId>,
}

/// Minimal committed publication evidence accepted by ordinary reclaim.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct PublicationCommitProof {
    /// Matching collection generation.
    pub collection_generation_id: CollectionGenerationId,
    /// Matching logical route.
    pub route: RouteIdentity,
    /// Matching retirement epoch.
    pub retirement_epoch_exclusive: Epoch,
    /// Matching retired-manifest digest.
    pub retired_manifest_digest: Blake3Digest32,
    /// Matching publication receipt.
    pub publication_receipt_ref: ReceiptRef,
    /// Visible epoch after the publication transaction committed.
    pub committed_visible_epoch: Epoch,
}

/// Retired manifest whose exact publication commit was verified.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct CommittedRetiredManifest(RetiredPointManifest);

impl CommittedRetiredManifest {
    /// Borrows the exact committed manifest.
    #[must_use]
    pub const fn manifest(&self) -> &RetiredPointManifest {
        &self.0
    }

    /// Consumes the proof wrapper.
    #[must_use]
    pub fn into_manifest(self) -> RetiredPointManifest {
        self.0
    }
}

/// Validates canonical exact IDs and matching committed publication evidence.
///
/// # Errors
///
/// Rejects empty, duplicate, unsorted, uncommitted, or mismatched manifests.
pub fn validate_retired_manifest(
    manifest: RetiredPointManifest,
    publication: &PublicationCommitProof,
) -> Result<CommittedRetiredManifest, ReclaimError> {
    if manifest.point_ids.is_empty() {
        return Err(ReclaimError::EmptyManifest);
    }
    if manifest
        .point_ids
        .windows(2)
        .any(|pair| pair[0] >= pair[1])
    {
        return Err(ReclaimError::InvalidPointSet);
    }
    if manifest.collection_generation_id != publication.collection_generation_id
        || manifest.route != publication.route
        || manifest.retirement_epoch_exclusive != publication.retirement_epoch_exclusive
        || manifest.manifest_digest != publication.retired_manifest_digest
        || manifest.publication_receipt_ref != publication.publication_receipt_ref
        || publication.committed_visible_epoch < manifest.retirement_epoch_exclusive
    {
        return Err(ReclaimError::PublicationMismatch);
    }
    Ok(CommittedRetiredManifest(manifest))
}

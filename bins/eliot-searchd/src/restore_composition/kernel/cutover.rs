//! Registry-verified namespace cutover and serving authorization.

use search_contracts::{
    OpaqueId, ReceiptRef, SourceNamespaceId, SourceOwnerGeneration,
};
use search_source_registry::cutover::VerifiedCutoverReceipt;

use super::model::{StagedRestore, is_destination_verified};
use super::spec::RestoreCompositionError;

/// Registry-verified ownership cutover proof.
///
/// The only constructor consumes a [`VerifiedCutoverReceipt`] produced by the
/// registry authority (`search-source-registry`), which already proved
/// fence-before-activation. Export bytes alone can never construct a verified
/// proof.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct OwnerCutoverProof {
    /// Namespace under cutover.
    pub namespace: SourceNamespaceId,
    /// Fenced old owner.
    pub old_owner: OpaqueId,
    /// Activated new owner.
    pub new_owner: OpaqueId,
    /// Owner generation before the fence.
    pub old_generation: SourceOwnerGeneration,
    /// Owner generation after activation.
    pub new_generation: SourceOwnerGeneration,
    /// Covered source count (must equal the export inventory).
    pub covered_sources: usize,
    /// Covered membership count (must equal the export inventory).
    pub covered_memberships: usize,
    /// Cutover authorization receipt reference.
    pub receipt_ref: ReceiptRef,
}

impl OwnerCutoverProof {
    /// Carries a registry-verified receipt into the restore composition.
    ///
    /// # Errors
    ///
    /// Returns [`RestoreCompositionError::OwnerCutoverRequired`] when the
    /// receipt does not advance the owner generation, reuses the same owner,
    /// or leaves any source unresolved.
    pub fn from_registry_verified(
        receipt: &VerifiedCutoverReceipt,
        old_owner: OpaqueId,
        new_owner: OpaqueId,
        unresolved_sources: usize,
        receipt_ref: ReceiptRef,
    ) -> Result<Self, RestoreCompositionError> {
        if unresolved_sources != 0 {
            return Err(RestoreCompositionError::OwnerCutoverRequired);
        }
        if old_owner == new_owner || receipt.old_generation == receipt.new_generation {
            return Err(RestoreCompositionError::OwnerCutoverRequired);
        }
        Ok(Self {
            namespace: receipt.namespace_id,
            old_owner,
            new_owner,
            old_generation: receipt.old_generation,
            new_generation: receipt.new_generation,
            covered_sources: receipt.covered_sources,
            covered_memberships: receipt.covered_memberships,
            receipt_ref,
        })
    }
}

/// Applies a registry-verified ownership cutover.
///
/// # Errors
///
/// Returns [`RestoreCompositionError::OutcomeUnknown`] after an interruption,
/// or [`RestoreCompositionError::OwnerCutoverRequired`] when the proof does
/// not bind this export inventory exactly.
pub fn apply_owner_cutover(
    staged: &mut StagedRestore,
    proof: OwnerCutoverProof,
) -> Result<(), RestoreCompositionError> {
    if staged.interrupted {
        return Err(RestoreCompositionError::OutcomeUnknown);
    }
    if staged.cutover.is_some() {
        return Err(RestoreCompositionError::OwnerCutoverRequired);
    }
    if proof.namespace != staged.export.namespace
        || proof.old_owner != staged.export.old_owner
        || proof.new_owner == proof.old_owner
        || proof.old_generation == proof.new_generation
        || proof.covered_sources != staged.export.source_count
        || proof.covered_memberships != staged.export.membership_count
    {
        return Err(RestoreCompositionError::OwnerCutoverRequired);
    }
    staged.cutover = Some(proof);
    Ok(())
}

/// Authorizes serving as one explicit owner.
///
/// Export alone never changes the owner: before an accepted cutover only the
/// old owner may serve, and afterwards the old owner is fenced.
///
/// # Errors
///
/// Returns [`RestoreCompositionError::OutcomeUnknown`] after an interruption,
/// [`RestoreCompositionError::OldOwnerStillServing`] when the fenced old
/// owner attempts to serve, [`RestoreCompositionError::OwnerCutoverRequired`]
/// when no verified cutover backs a new owner, or
/// [`RestoreCompositionError::RevalidationIncomplete`] before destination
/// verification.
pub fn authorize_serve(
    staged: &StagedRestore,
    as_owner: &OpaqueId,
) -> Result<(), RestoreCompositionError> {
    if staged.interrupted {
        return Err(RestoreCompositionError::OutcomeUnknown);
    }
    match &staged.cutover {
        Some(proof) => {
            if as_owner == &staged.export.old_owner {
                return Err(RestoreCompositionError::OldOwnerStillServing);
            }
            if as_owner != &proof.new_owner {
                return Err(RestoreCompositionError::OwnerCutoverRequired);
            }
        }
        None => {
            if as_owner != &staged.export.old_owner {
                return Err(RestoreCompositionError::OwnerCutoverRequired);
            }
        }
    }
    if !is_destination_verified(staged) {
        return Err(RestoreCompositionError::RevalidationIncomplete);
    }
    Ok(())
}

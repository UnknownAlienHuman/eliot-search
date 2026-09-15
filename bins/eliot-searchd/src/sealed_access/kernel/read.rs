//! Current sealed-fence read and live authority admission.

use std::path::Path;

use crate::sealed_access_codec::{AccessFenceState, validate_fence_id, validate_identifier};
use crate::sealed_owner_epoch::OwnerEpochGuard;
use crate::sealed_root_identity::verify_owner_root;

use super::chain::load_chain;
use super::model::{AccessFenceSnapshot, ActiveAccessFence};
use super::spec::SealedAccessError;

/// Reads and validates the current fence head, including its complete chain.
pub fn current_fence(
    data_root: &Path,
    owner: &OwnerEpochGuard,
    fence_id: &str,
) -> Result<AccessFenceSnapshot, SealedAccessError> {
    verify_owner_root(data_root, owner)?;
    validate_fence_id(fence_id)?;
    load_chain(data_root, owner, fence_id)?
        .last()
        .map(|entry| entry.snapshot.clone())
        .ok_or(SealedAccessError::FenceNotFound)
}

/// Requires the exact current `ALLOW` fence for one catalog-bound revision.
pub fn require_active_fence(
    data_root: &Path,
    owner: &OwnerEpochGuard,
    fence_id: &str,
    source_id: &str,
    source_revision_id: &str,
    catalog_object_id: &str,
) -> Result<ActiveAccessFence, SealedAccessError> {
    for value in [source_id, source_revision_id, catalog_object_id] {
        validate_identifier(value)?;
    }
    let snapshot = current_fence(data_root, owner, fence_id)?;
    let record = &snapshot.record;
    if record.source_id != source_id
        || record.source_revision_id != source_revision_id
        || record.catalog_object_id != catalog_object_id
    {
        return Err(SealedAccessError::AuthorityBindingMismatch);
    }
    if record.state != AccessFenceState::Allow {
        return Err(SealedAccessError::AccessDenied);
    }
    if record.admitted_owner_epoch > owner.epoch()
        || record.owner_root_binding_sha256 != owner.root_binding_sha256()
    {
        return Err(SealedAccessError::AuthorityBindingMismatch);
    }
    Ok(ActiveAccessFence::new(snapshot))
}

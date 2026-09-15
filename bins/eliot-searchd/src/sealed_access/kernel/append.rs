//! Exact sealed access-fence append and immutable readback.

use std::path::Path;

use crate::sealed_access_codec::{
    ACCESS_FENCE_FORMAT_VERSION, AccessFenceRecord, AccessFenceState,
    zero_digest,
};
use crate::sealed_digest::sha256;
use crate::sealed_owner_epoch::OwnerEpochGuard;
use crate::sealed_root_identity::verify_owner_root;
use crate::sealed_store::{SensitiveBytes, open_sealed};
use crate::sealed_transaction_guard::put_idempotent_verified;

use super::chain::{
    load_chain, object_id, replay_existing_fence, transaction_id,
    validate_successor_request,
};
use super::model::{
    AccessAppendDisposition, AccessFenceMutation, AccessFenceReceipt,
    AccessFenceSnapshot,
};
use super::spec::{MAX_ACCESS_FENCE_GENERATIONS, SealedAccessError};

/// Appends one exact `ALLOW` or terminal `DENY` mutation.
pub fn append_fence(
    data_root: &Path,
    owner: &OwnerEpochGuard,
    mutation: AccessFenceMutation,
) -> Result<AccessFenceReceipt, SealedAccessError> {
    verify_owner_root(data_root, owner)?;
    mutation.validate()?;
    let chain = load_chain(data_root, owner, &mutation.fence_id)?;

    if let Some(receipt) = replay_existing_fence(&chain, &mutation)? {
        return Ok(receipt);
    }

    let (generation, previous_generation, previous_sha, access_generation) =
        if let Some(previous) = chain.last() {
            validate_successor_request(&previous.snapshot.record, &mutation)?;
            (
                previous
                    .snapshot
                    .record
                    .generation
                    .checked_add(1)
                    .ok_or(SealedAccessError::CapacityExceeded)?,
                previous.snapshot.record.generation,
                previous.snapshot.record_sha256,
                previous
                    .snapshot
                    .record
                    .access_generation
                    .checked_add(1)
                    .ok_or(SealedAccessError::CapacityExceeded)?,
            )
        } else {
            if mutation.state != AccessFenceState::Allow {
                return Err(SealedAccessError::AccessDenied);
            }
            (1, 0, zero_digest()?, 1)
        };
    if usize::try_from(generation).unwrap_or(usize::MAX)
        > MAX_ACCESS_FENCE_GENERATIONS
    {
        return Err(SealedAccessError::CapacityExceeded);
    }

    let record = AccessFenceRecord {
        format_version: ACCESS_FENCE_FORMAT_VERSION,
        fence_id: mutation.fence_id,
        mutation_id: mutation.mutation_id,
        generation,
        previous_generation,
        previous_record_sha256: previous_sha,
        source_id: mutation.source_id,
        source_revision_id: mutation.source_revision_id,
        catalog_object_id: mutation.catalog_object_id,
        scope_id: mutation.scope_id,
        scope_revision: mutation.scope_revision,
        policy_id: mutation.policy_id,
        policy_revision: mutation.policy_revision,
        access_generation,
        purge_generation: mutation.purge_generation,
        state: mutation.state,
        admitted_owner_epoch: owner.epoch(),
        owner_root_binding_sha256: owner.root_binding_sha256(),
    };
    let encoded = record.encode()?;
    let record_sha256 = sha256(encoded.as_bytes())?;
    let object_id = object_id(&record.fence_id, generation);
    let transaction_id = transaction_id(&record.fence_id, generation);
    let transaction = put_idempotent_verified(
        data_root,
        &transaction_id,
        &object_id,
        &SensitiveBytes::new(encoded.as_bytes().to_vec())?,
    )?;
    if transaction.object_id != object_id
        || transaction.operation_id != transaction_id
        || transaction.plaintext_sha256 != record_sha256
        || transaction.plaintext_bytes
            != u64::try_from(encoded.len())
                .map_err(|_| SealedAccessError::ChainInvalid)?
    {
        return Err(SealedAccessError::ChainInvalid);
    }
    let readback = open_sealed(data_root, &object_id)?;
    if readback.expose() != encoded.as_bytes()
        || AccessFenceRecord::decode(readback.expose())? != record
    {
        return Err(SealedAccessError::ChainInvalid);
    }
    let affected = AccessFenceSnapshot {
        record,
        record_sha256,
        object_id,
        transaction_id,
    };
    Ok(AccessFenceReceipt {
        affected: affected.clone(),
        current: affected,
        disposition: AccessAppendDisposition::Created,
        readback_verified: true,
    })
}

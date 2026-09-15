//! Complete sealed-fence chain loading, replay and successor validation.

use std::collections::BTreeSet;
use std::path::Path;

use crate::sealed_access_codec::{AccessFenceRecord, AccessFenceState};
use crate::sealed_digest::sha256;
use crate::sealed_owner_epoch::OwnerEpochGuard;
use crate::sealed_store::{SensitiveBytes, open_sealed};
use crate::sealed_transaction::{
    TransactionStatus, inspect_transaction,
};
use crate::sealed_transaction_guard::put_idempotent_verified;

use super::model::{
    AccessAppendDisposition, AccessFenceMutation, AccessFenceReceipt,
    AccessFenceSnapshot, LoadedFence,
};
use super::platform;
use super::spec::{MAX_ACCESS_FENCE_GENERATIONS, SealedAccessError};

pub(super) fn replay_existing_fence(
    chain: &[LoadedFence],
    mutation: &AccessFenceMutation,
) -> Result<Option<AccessFenceReceipt>, SealedAccessError> {
    let Some(existing) = chain
        .iter()
        .find(|entry| entry.snapshot.record.mutation_id == mutation.mutation_id)
    else {
        return Ok(None);
    };
    if !record_matches_mutation(&existing.snapshot.record, mutation) {
        return Err(SealedAccessError::MutationConflict);
    }
    let current = chain
        .last()
        .ok_or(SealedAccessError::ChainInvalid)?
        .snapshot
        .clone();
    Ok(Some(AccessFenceReceipt {
        affected: existing.snapshot.clone(),
        current,
        disposition: AccessAppendDisposition::Replay,
        readback_verified: true,
    }))
}

pub(super) fn load_chain(
    data_root: &Path,
    owner: &OwnerEpochGuard,
    fence_id: &str,
) -> Result<Vec<LoadedFence>, SealedAccessError> {
    let inventory = platform::discover(data_root, fence_id)?;
    if inventory.len() > MAX_ACCESS_FENCE_GENERATIONS {
        return Err(SealedAccessError::CapacityExceeded);
    }
    let mut chain: Vec<LoadedFence> = Vec::with_capacity(inventory.len());
    let mut mutation_ids = BTreeSet::new();

    for (index, (generation, object_id)) in inventory.into_iter().enumerate() {
        let expected_generation = u64::try_from(index)
            .map_err(|_| SealedAccessError::CapacityExceeded)?
            .checked_add(1)
            .ok_or(SealedAccessError::CapacityExceeded)?;
        if generation != expected_generation {
            return Err(SealedAccessError::ChainInvalid);
        }
        let plaintext = open_sealed(data_root, &object_id)?;
        let record = AccessFenceRecord::decode(plaintext.expose())?;
        if record.fence_id != fence_id
            || record.generation != generation
            || record.owner_root_binding_sha256 != owner.root_binding_sha256()
            || record.admitted_owner_epoch > owner.epoch()
            || !mutation_ids.insert(record.mutation_id.clone())
        {
            return Err(SealedAccessError::ChainInvalid);
        }
        let encoded = record.encode()?;
        if encoded.as_bytes() != plaintext.expose() {
            return Err(SealedAccessError::ChainInvalid);
        }
        let record_sha256 = sha256(plaintext.expose())?;
        if let Some(previous) = chain.last() {
            validate_successor_record(&previous.snapshot, &record)?;
        }
        let transaction_id = transaction_id(fence_id, generation);
        let transaction = put_idempotent_verified(
            data_root,
            &transaction_id,
            &object_id,
            &SensitiveBytes::new(encoded.into_bytes())?,
        )?;
        if transaction.plaintext_sha256 != record_sha256
            || transaction.object_id != object_id
            || transaction.operation_id != transaction_id
        {
            return Err(SealedAccessError::ChainInvalid);
        }
        let observed = inspect_transaction(data_root, &transaction_id)?;
        if observed.status != TransactionStatus::Committed {
            return Err(SealedAccessError::TransactionNotCommitted);
        }
        chain.push(LoadedFence {
            snapshot: AccessFenceSnapshot {
                record,
                record_sha256,
                object_id,
                transaction_id,
            },
        });
    }
    Ok(chain)
}

pub(super) fn validate_successor_record(
    previous: &AccessFenceSnapshot,
    current: &AccessFenceRecord,
) -> Result<(), SealedAccessError> {
    let previous_record = &previous.record;
    if previous_record.state == AccessFenceState::Deny {
        return Err(SealedAccessError::DenyIsTerminal);
    }
    if current.previous_generation != previous_record.generation
        || current.previous_record_sha256 != previous.record_sha256
        || current.generation != previous_record.generation.saturating_add(1)
        || current.access_generation
            != previous_record.access_generation.saturating_add(1)
    {
        return Err(SealedAccessError::GenerationConflict);
    }
    if !same_authority(previous_record, current) {
        return Err(SealedAccessError::AuthorityBindingMismatch);
    }
    if current.scope_revision < previous_record.scope_revision
        || current.policy_revision < previous_record.policy_revision
        || current.purge_generation < previous_record.purge_generation
    {
        return Err(SealedAccessError::RevisionRegression);
    }
    Ok(())
}

pub(super) fn validate_successor_request(
    previous: &AccessFenceRecord,
    mutation: &AccessFenceMutation,
) -> Result<(), SealedAccessError> {
    if previous.state == AccessFenceState::Deny {
        return Err(SealedAccessError::DenyIsTerminal);
    }
    if previous.source_id != mutation.source_id
        || previous.source_revision_id != mutation.source_revision_id
        || previous.catalog_object_id != mutation.catalog_object_id
        || previous.scope_id != mutation.scope_id
        || previous.policy_id != mutation.policy_id
    {
        return Err(SealedAccessError::AuthorityBindingMismatch);
    }
    if mutation.scope_revision < previous.scope_revision
        || mutation.policy_revision < previous.policy_revision
        || mutation.purge_generation < previous.purge_generation
    {
        return Err(SealedAccessError::RevisionRegression);
    }
    Ok(())
}

fn same_authority(
    left: &AccessFenceRecord,
    right: &AccessFenceRecord,
) -> bool {
    left.fence_id == right.fence_id
        && left.source_id == right.source_id
        && left.source_revision_id == right.source_revision_id
        && left.catalog_object_id == right.catalog_object_id
        && left.scope_id == right.scope_id
        && left.policy_id == right.policy_id
        && left.owner_root_binding_sha256 == right.owner_root_binding_sha256
}

fn record_matches_mutation(
    record: &AccessFenceRecord,
    mutation: &AccessFenceMutation,
) -> bool {
    record.fence_id == mutation.fence_id
        && record.mutation_id == mutation.mutation_id
        && record.source_id == mutation.source_id
        && record.source_revision_id == mutation.source_revision_id
        && record.catalog_object_id == mutation.catalog_object_id
        && record.scope_id == mutation.scope_id
        && record.scope_revision == mutation.scope_revision
        && record.policy_id == mutation.policy_id
        && record.policy_revision == mutation.policy_revision
        && record.purge_generation == mutation.purge_generation
        && record.state == mutation.state
}

pub(super) fn object_id(fence_id: &str, generation: u64) -> String {
    format!("access-fence-{fence_id}-{generation:020}")
}

pub(super) fn transaction_id(fence_id: &str, generation: u64) -> String {
    format!("access-fence-op-{fence_id}-{generation:020}")
}

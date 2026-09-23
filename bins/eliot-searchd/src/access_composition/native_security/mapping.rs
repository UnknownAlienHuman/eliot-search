//! Lossless access/control mapping; no policy or denied-set inference.

use std::collections::BTreeSet;

use search_access::{
    AccessError, LiveSecurityState, MAX_SECURITY_DEPENDENTS, SecurityDependentReceipt,
    SecurityRestriction,
};
use search_contracts::{Blake3Digest32, BoundedList, BoundedSet, LiveDenySnapshotRef, ReceiptRef};
use search_control_redb::{
    policy_codec::{AccessPolicyRecord, encode_access_policy},
    security_restriction::{SecurityPolicyState, SecurityRestrictionCommit, SecurityRestrictionMutation},
};

use super::{NativeSecurityBinding, NativeSecurityError};

pub(super) fn live(state: &SecurityPolicyState) -> LiveSecurityState {
    LiveSecurityState {
        generation: state.policy.live_deny_generation,
        denied_memberships: state.denied_memberships.iter().copied().collect(),
        purged_memberships: state.purged_memberships.iter().copied().collect(),
        fail_closed: state.fail_closed,
        snapshot_digest: state.snapshot_digest,
    }
}

pub(super) fn validate_binding(
    binding: &NativeSecurityBinding,
    native: &SecurityRestrictionMutation,
) -> Result<(), NativeSecurityError> {
    let next = native.replacement();
    if binding.dependents.is_empty()
        || next.policy.namespace_id != binding.namespace
        || next.policy.owner_generation != binding.source_owner
        || next.security_domain_ref != binding.domain
        || native.required_dependents() != &binding.dependents
    {
        return Err(AccessError::SecurityOperationConflict.into());
    }
    if let Some(old) = native.expected_state() {
        if old.policy.namespace_id != binding.namespace
            || old.policy.owner_generation != binding.source_owner
            || old.security_domain_ref != binding.domain
        {
            return Err(AccessError::SecurityOperationConflict.into());
        }
        validate_metadata(&old.policy, &next.policy)?;
    }
    Ok(())
}

pub(super) fn validate_metadata(
    before: &AccessPolicyRecord,
    after: &AccessPolicyRecord,
) -> Result<(), NativeSecurityError> {
    if before.namespace_id != after.namespace_id || before.owner_generation != after.owner_generation {
        return Err(AccessError::SecurityOperationConflict.into());
    }
    if after.policy_revision.get() < before.policy_revision.get()
        || after.shadow_fence_revision.get() < before.shadow_fence_revision.get()
        || after.purge_fence_revision.get() < before.purge_fence_revision.get()
    {
        return Err(AccessError::SecurityGenerationRegression.into());
    }
    Ok(())
}

pub(super) fn validate_target(
    binding: &NativeSecurityBinding,
    command: &SecurityRestriction,
    policy: &AccessPolicyRecord,
) -> Result<(), NativeSecurityError> {
    if command.security_domain_ref != binding.domain
        || command.required_dependents != binding.dependents
        || policy.namespace_id != binding.namespace
        || policy.owner_generation != binding.source_owner
        || policy.policy_revision != command.policy_revision
        || policy.live_deny_generation != command.next_live.generation
    {
        return Err(AccessError::SecurityOperationConflict.into());
    }
    Ok(())
}

pub(super) fn command(native: &SecurityRestrictionMutation) -> Result<SecurityRestriction, NativeSecurityError> {
    let old = native.expected_state().ok_or(AccessError::SecurityOperationConflict)?;
    let next = native.replacement();
    validate_metadata(&old.policy, &next.policy)?;
    if old.security_domain_ref != next.security_domain_ref {
        return Err(AccessError::SecurityOperationConflict.into());
    }
    Ok(SecurityRestriction {
        operation_id: native.operation_id().clone(),
        security_domain_ref: next.security_domain_ref.clone(),
        expected_policy_revision: old.policy.policy_revision,
        policy_revision: next.policy.policy_revision,
        expected_live: live(old),
        next_live: live(next),
        required_dependents: native.required_dependents().clone(),
    })
}

pub(super) fn replacement(
    command: &SecurityRestriction,
    policy: AccessPolicyRecord,
) -> Result<SecurityPolicyState, NativeSecurityError> {
    Ok(SecurityPolicyState {
        policy,
        security_domain_ref: command.security_domain_ref.clone(),
        snapshot_digest: command.next_live.snapshot_digest,
        denied_memberships: BoundedSet::from_items(command.next_live.denied_memberships.iter().copied())
            .map_err(|_| AccessError::SecurityFailClosed)?,
        purged_memberships: BoundedSet::from_items(command.next_live.purged_memberships.iter().copied())
            .map_err(|_| AccessError::SecurityFailClosed)?,
        fail_closed: command.next_live.fail_closed,
    })
}

pub(super) fn snapshot_ref(committed: &SecurityRestrictionCommit) -> LiveDenySnapshotRef {
    let state = committed.mutation().replacement();
    LiveDenySnapshotRef {
        security_domain_ref: state.security_domain_ref.clone(),
        live_deny_generation: state.policy.live_deny_generation,
        snapshot_digest: state.snapshot_digest,
    }
}

// An address for the actual verified native operation receipt, not a synthetic
// success receipt. The opaque commit's private fields can only originate in redb.
pub(super) fn receipt_ref(committed: &SecurityRestrictionCommit) -> Result<ReceiptRef, NativeSecurityError> {
    const HEX: &[u8; 16] = b"0123456789abcdef";
    let mut reference = String::from("control-security-v1:");
    for byte in committed.receipt().operation_id.0 {
        reference.push(char::from(HEX[usize::from(byte >> 4)]));
        reference.push(char::from(HEX[usize::from(byte & 15)]));
    }
    reference.push(':');
    reference.push_str(&committed.receipt().after_generation.to_string());
    ReceiptRef::new(reference).map_err(|_| AccessError::SecurityOperationConflict.into())
}

pub(super) fn validate_receipts(
    committed: &SecurityRestrictionCommit,
    published: &LiveDenySnapshotRef,
    received: &BoundedList<SecurityDependentReceipt, MAX_SECURITY_DEPENDENTS>,
) -> Result<(), NativeSecurityError> {
    if *published != snapshot_ref(committed)
        || received.len() != committed.mutation().required_dependents().len()
    {
        return Err(AccessError::SecurityFailClosed.into());
    }
    let mutation_ref = receipt_ref(committed)?;
    let mut owners = BTreeSet::new();
    let mut receipts = BTreeSet::new();
    for receipt in received {
        if !committed.mutation().required_dependents().contains(&receipt.dependent)
            || !owners.insert(&receipt.dependent)
            || !receipts.insert(&receipt.receipt_ref)
            || receipt.mutation_receipt_ref != mutation_ref
            || receipt.live_snapshot_ref != *published
        {
            return Err(AccessError::SecurityOperationConflict.into());
        }
    }
    Ok(())
}

pub(super) fn command_digest(
    command: &SecurityRestriction,
    before: &AccessPolicyRecord,
    after: &AccessPolicyRecord,
) -> Blake3Digest32 {
    let mut hash = blake3::Hasher::new();
    hash.update(b"eliot-search/security-restriction-command/blake3/v1\0");
    hash.update(&encode_access_policy(before));
    hash.update(&encode_access_policy(after));
    frame(&mut hash, command.operation_id.as_str().as_bytes());
    frame(&mut hash, command.security_domain_ref.as_str().as_bytes());
    for state in [&command.expected_live, &command.next_live] {
        hash.update(&state.generation.to_be_bytes());
        hash.update(&[u8::from(state.fail_closed)]);
        hash.update(state.snapshot_digest.as_bytes());
        for memberships in [&state.denied_memberships, &state.purged_memberships] {
            count(&mut hash, memberships.len());
            for membership in memberships { hash.update(membership.as_bytes()); }
        }
    }
    count(&mut hash, command.required_dependents.len());
    for owner in &command.required_dependents { frame(&mut hash, owner.as_str().as_bytes()); }
    Blake3Digest32::from_bytes(*hash.finalize().as_bytes())
}

fn frame(hash: &mut blake3::Hasher, bytes: &[u8]) {
    count(hash, bytes.len());
    hash.update(bytes);
}

fn count(hash: &mut blake3::Hasher, length: usize) {
    // Called only on contract-bounded text/sets after canonical validation.
    hash.update(&u64::try_from(length).expect("bounded security command field").to_be_bytes());
}

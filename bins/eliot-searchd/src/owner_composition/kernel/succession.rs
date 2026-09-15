//! Guarded succession planning and establishment under an existing OS lock.

use std::path::Path;

use search_contracts::{InstallationIncarnationId, OwnerEpoch};
use search_runtime_owner::OwnerError;

use super::installation::{InstallationBinding, load_or_create_installation};
use super::lifecycle::LiveOwner;
use super::observation::{
    ObservedRoot, mint_owner_token, observe_executable,
    observe_physical_root,
};
use super::record::DurableOwnerRecord;
use super::slots::{newest_valid, read_slot, write_slot};
use super::spec::{DrainReasonText, LifecycleState};

/// Establishes the single live owner under an already-held OS exclusion.
///
/// Binds the minted-once installation identity, the fresh physical-root and
/// executable observations, a fresh process-creation token and the next
/// monotone epoch, then publishes exactly one durable record with exact
/// readback.
pub fn establish(canonical_root: &Path) -> Result<LiveOwner, OwnerError> {
    let installation = load_or_create_installation(canonical_root)?;
    let observed = observe_physical_root(canonical_root)?;
    let executable = observe_executable()?;
    verify_sealed_head_agrees(canonical_root)?;
    let (target, prior) = newest_valid(canonical_root)?;
    let record = plan_successor(
        &installation,
        &observed,
        executable,
        prior.as_deref(),
    )?;
    write_slot(canonical_root, target, &record)?;
    let reloaded =
        read_slot(canonical_root, target).ok_or(OwnerError::OwnerAcquireOutcomeUnknown)?;
    if *reloaded != record {
        return Err(OwnerError::OwnerRecordDigestMismatch);
    }
    Ok(LiveOwner {
        canonical_root: canonical_root.to_owned(),
        installation_incarnation_id: InstallationIncarnationId::from_bytes(
            record.installation_incarnation_id,
        ),
        data_root_id: observed.data_root_id,
        epoch: OwnerEpoch::new(record.epoch)
            .map_err(|_| OwnerError::ContractExhausted)?,
        record,
        recovered_previous_active: prior
            .as_ref()
            .is_some_and(|previous| previous.lifecycle != LifecycleState::Released),
        poisoned: false,
    })
}

/// Verifies that a co-present sealed epoch mirror binds the same root.
///
/// No sealed objects means no second authority and passes. A head that
/// cannot be authenticated, decoded or root-matched quarantines; epoch
/// numbers across the two counters are never compared.
pub(super) fn verify_sealed_head_agrees(
    canonical_root: &Path,
) -> Result<(), OwnerError> {
    let head = crate::sealed_owner_epoch::latest_sealed_head(canonical_root)
        .map_err(|_| OwnerError::OwnerRecoveryQuarantined)?;
    let Some(record) = head else {
        return Ok(());
    };
    let current = crate::sealed_root_identity::root_binding_sha256(canonical_root)
        .map_err(|_| OwnerError::OwnerRecoveryQuarantined)?;
    if record.root_binding_sha256 != current {
        return Err(OwnerError::OwnerGuardMismatch);
    }
    Ok(())
}

/// Plans the exact successor record for a fresh or bound predecessor.
///
/// Installation or physical-root disagreement denies the succession;
/// a non-monotone chain or an exhausted counter fails closed.
fn plan_successor(
    installation: &InstallationBinding,
    observed: &ObservedRoot,
    executable: [u8; 32],
    prior: Option<&DurableOwnerRecord>,
) -> Result<DurableOwnerRecord, OwnerError> {
    let token = mint_owner_token(observed, &executable)?;
    let (epoch, previous_epoch, previous_record_digest, generation) = match prior {
        None => (1_u64, 0_u64, [0; 32], 1_u64),
        Some(previous) => {
            if previous.installation_id != installation.installation_id
                || previous.installation_incarnation_id
                    != installation.installation_incarnation_id
            {
                return Err(OwnerError::OwnerGuardMismatch);
            }
            if previous.data_root_id != *observed.data_root_id.as_bytes()
                || previous.canonical_path_digest
                    != observed.canonical_path_digest
                || previous.volume_identity_digest
                    != observed.volume_identity_digest
            {
                return Err(OwnerError::OwnerGuardMismatch);
            }
            previous.validate_shape()?;
            let epoch = previous
                .epoch
                .checked_add(1)
                .ok_or(OwnerError::ContractExhausted)?;
            let generation = previous
                .generation
                .checked_add(1)
                .ok_or(OwnerError::ContractExhausted)?;
            (epoch, previous.epoch, previous.record_digest, generation)
        }
    };
    let mut record = DurableOwnerRecord {
        installation_id: installation.installation_id,
        installation_incarnation_id: installation.installation_incarnation_id,
        data_root_id: *observed.data_root_id.as_bytes(),
        epoch,
        previous_epoch,
        previous_record_digest,
        canonical_path_digest: observed.canonical_path_digest,
        volume_identity_digest: observed.volume_identity_digest,
        executable_digest: executable,
        owner_token: token,
        owner_pid: std::process::id(),
        lifecycle: LifecycleState::Active,
        drain_reason: DrainReasonText::None,
        generation,
        record_digest: [0; 32],
    };
    record.validate_shape()?;
    record.refresh_digest();
    Ok(record)
}

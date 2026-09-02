//! Exact source-ownership, epoch, and publication transitions.

use search_contracts::{
    Epoch, NamespaceOwnershipStatus, PublicationIntent, PublicationIntentState,
    SourceNamespaceOwnership, SourceOwnerCutoverReceipt,
};

use crate::{DomainError, DomainErrorKind};

/// Semantic kind of an accepted source-ownership edge.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum OwnershipTransition {
    /// Active owner prepared one exact cutover.
    PrepareCutover,
    /// Old owner became durably fenced.
    FenceOldOwner,
    /// Fenced old owner became retired.
    RetireOldOwner,
    /// Exact cutover receipt activated a distinct new owner.
    ActivateNewOwner,
}

/// Validates that `next` is exactly the next representable epoch.
///
/// # Errors
///
/// Rejects skipped, reused, or reverse epochs.
pub fn validate_epoch_transition(current: Epoch, next: Epoch) -> Result<(), DomainError> {
    let expected = current.checked_next().map_err(DomainError::from)?;
    if next == expected {
        Ok(())
    } else {
        Err(DomainError::new(
            DomainErrorKind::InvalidStateTransition,
            "epoch.next",
        ))
    }
}

/// Applies one closed source-namespace ownership transition.
///
/// # Errors
///
/// Rejects namespace changes, non-contiguous record revisions, generation
/// reuse, skipped states, dual-owner activation, and incomplete cutover proof.
pub fn transition_source_ownership(
    current: &SourceNamespaceOwnership,
    next: SourceNamespaceOwnership,
    cutover_receipt: Option<&SourceOwnerCutoverReceipt>,
) -> Result<(OwnershipTransition, SourceNamespaceOwnership), DomainError> {
    if current.source_namespace_id != next.source_namespace_id {
        return Err(DomainError::new(
            DomainErrorKind::InvariantViolation,
            "source_ownership.namespace",
        ));
    }
    let expected_revision = current
        .ownership_record_revision
        .checked_next()
        .map_err(DomainError::from)?;
    if next.ownership_record_revision != expected_revision {
        return Err(DomainError::new(
            DomainErrorKind::InvalidStateTransition,
            "source_ownership.record_revision",
        ));
    }
    if next.source_owner_generation == current.source_owner_generation {
        return Err(DomainError::new(
            DomainErrorKind::InvariantViolation,
            "source_ownership.generation",
        ));
    }

    let same_owner = current.owner_system_id == next.owner_system_id
        && current.owner_installation_incarnation_id == next.owner_installation_incarnation_id;

    let transition = match (current.status, next.status, same_owner) {
        (NamespaceOwnershipStatus::Active, NamespaceOwnershipStatus::CutoverPrepared, true) => {
            if current.owner_epoch != next.owner_epoch
                || current.cutover_receipt_ref.is_some()
                || next.cutover_receipt_ref.is_none()
            {
                return Err(DomainError::new(
                    DomainErrorKind::InvariantViolation,
                    "source_ownership.prepare_cutover",
                ));
            }
            OwnershipTransition::PrepareCutover
        }
        (NamespaceOwnershipStatus::CutoverPrepared, NamespaceOwnershipStatus::Fenced, true) => {
            if current.owner_epoch != next.owner_epoch
                || current.cutover_receipt_ref != next.cutover_receipt_ref
            {
                return Err(DomainError::new(
                    DomainErrorKind::InvariantViolation,
                    "source_ownership.fence",
                ));
            }
            OwnershipTransition::FenceOldOwner
        }
        (NamespaceOwnershipStatus::Fenced, NamespaceOwnershipStatus::Retired, true) => {
            if current.owner_epoch != next.owner_epoch
                || current.cutover_receipt_ref != next.cutover_receipt_ref
            {
                return Err(DomainError::new(
                    DomainErrorKind::InvariantViolation,
                    "source_ownership.retire",
                ));
            }
            OwnershipTransition::RetireOldOwner
        }
        (
            NamespaceOwnershipStatus::Fenced | NamespaceOwnershipStatus::Retired,
            NamespaceOwnershipStatus::Active,
            false,
        ) => {
            let receipt = cutover_receipt.ok_or_else(|| {
                DomainError::new(
                    DomainErrorKind::InvariantViolation,
                    "source_ownership.activation_receipt",
                )
            })?;
            receipt.validate().map_err(DomainError::from)?;
            if next.owner_epoch <= current.owner_epoch
                || current.cutover_receipt_ref.is_none()
                || next.cutover_receipt_ref != current.cutover_receipt_ref
                || receipt.cutover.source_namespace_id != current.source_namespace_id
                || receipt.old_owner.owner_system_id != current.owner_system_id
                || receipt.new_owner.owner_system_id != next.owner_system_id
                || receipt.new_owner.source_owner_generation_after_activation
                    != next.source_owner_generation
                || receipt.new_owner.activation_revision != next.ownership_record_revision
                || receipt.old_owner.terminal_status != current.status
            {
                return Err(DomainError::new(
                    DomainErrorKind::InvariantViolation,
                    "source_ownership.activation",
                ));
            }
            OwnershipTransition::ActivateNewOwner
        }
        _ => {
            return Err(DomainError::new(
                DomainErrorKind::InvalidStateTransition,
                "source_ownership.status",
            ));
        }
    };

    Ok((transition, next))
}

/// Advances a publication intent through its exact closed state machine.
///
/// # Errors
///
/// Rejects skipped, reverse, and reopened terminal states.
pub fn transition_publication(
    current: &PublicationIntent,
    next_state: PublicationIntentState,
) -> Result<PublicationIntent, DomainError> {
    let valid = matches!(
        (current.state, next_state),
        (
            PublicationIntentState::Prepared,
            PublicationIntentState::IntentDurable
                | PublicationIntentState::Aborted
                | PublicationIntentState::PublicationBlocked
        ) | (
            PublicationIntentState::IntentDurable,
            PublicationIntentState::NewPointsAcknowledged
                | PublicationIntentState::InvalidationOnlyCommitted
                | PublicationIntentState::Compensating
                | PublicationIntentState::Aborted
                | PublicationIntentState::PublicationBlocked
        ) | (
            PublicationIntentState::NewPointsAcknowledged,
            PublicationIntentState::OldPointsClosedAcknowledged
                | PublicationIntentState::Compensating
                | PublicationIntentState::Aborted
                | PublicationIntentState::PublicationBlocked
        ) | (
            PublicationIntentState::OldPointsClosedAcknowledged,
            PublicationIntentState::ReadbackVerified
                | PublicationIntentState::Compensating
                | PublicationIntentState::Aborted
                | PublicationIntentState::PublicationBlocked
        ) | (
            PublicationIntentState::ReadbackVerified,
            PublicationIntentState::ControlCommitted
                | PublicationIntentState::Compensating
                | PublicationIntentState::Aborted
                | PublicationIntentState::PublicationBlocked
        ) | (
            PublicationIntentState::ControlCommitted,
            PublicationIntentState::Reclaimable
        ) | (
            PublicationIntentState::Compensating,
            PublicationIntentState::Aborted | PublicationIntentState::PublicationBlocked
        )
    );
    if !valid {
        return Err(DomainError::new(
            DomainErrorKind::InvalidStateTransition,
            "publication_intent.state",
        ));
    }
    let mut next = current.clone();
    next.state = next_state;
    Ok(next)
}

#[cfg(test)]
mod tests {
    use search_contracts::{
        BoundedList, Epoch, InstallationIncarnationId, NamespaceOwnershipStatus, NonZeroRevision,
        OpaqueId, OwnerEpoch, PolicyRevision, PublicationIntent, PublicationIntentId,
        PublicationIntentState, ReceiptRef, SourceNamespaceId, SourceNamespaceOwnership,
        SourceOwnerGeneration,
    };

    use super::{
        OwnershipTransition, transition_publication, transition_source_ownership,
        validate_epoch_transition,
    };
    use crate::DomainErrorKind;

    fn ownership(
        status: NamespaceOwnershipStatus,
        revision: u64,
        generation: u8,
        receipt: Option<ReceiptRef>,
    ) -> SourceNamespaceOwnership {
        SourceNamespaceOwnership {
            source_namespace_id: SourceNamespaceId::from_bytes([1; 16]),
            owner_system_id: OpaqueId::new("owner-a").expect("owner"),
            owner_installation_incarnation_id: InstallationIncarnationId::from_bytes([2; 16]),
            owner_epoch: OwnerEpoch::new(1).expect("epoch"),
            ownership_record_revision: NonZeroRevision::new(revision).expect("revision"),
            source_owner_generation: SourceOwnerGeneration::from_bytes([generation; 32]),
            source_admission_policy_revision: PolicyRevision::new(1),
            status,
            cutover_receipt_ref: receipt,
        }
    }

    fn publication(state: PublicationIntentState) -> PublicationIntent {
        PublicationIntent {
            publication_intent_id: PublicationIntentId::from_bytes([1; 16]),
            target_epoch: Epoch::new(1).expect("epoch"),
            prepared_manifest_ref: ReceiptRef::new("receipt:manifest").expect("receipt"),
            owner_source_membership_access_guards: BoundedList::empty(),
            state,
        }
    }

    #[test]
    fn ownership_cannot_skip_the_prepared_state() {
        let current = ownership(NamespaceOwnershipStatus::Active, 1, 1, None);
        let receipt = ReceiptRef::new("receipt:cutover").expect("receipt");
        let skipped = ownership(
            NamespaceOwnershipStatus::Fenced,
            2,
            2,
            Some(receipt.clone()),
        );
        assert!(transition_source_ownership(&current, skipped, None).is_err());

        let prepared = ownership(
            NamespaceOwnershipStatus::CutoverPrepared,
            2,
            2,
            Some(receipt),
        );
        let (kind, accepted) =
            transition_source_ownership(&current, prepared, None).expect("prepare cutover");
        assert_eq!(kind, OwnershipTransition::PrepareCutover);
        assert_eq!(accepted.status, NamespaceOwnershipStatus::CutoverPrepared);
    }

    #[test]
    fn epochs_must_be_contiguous() {
        assert!(
            validate_epoch_transition(Epoch::new(1).expect("epoch"), Epoch::new(2).expect("epoch"))
                .is_ok()
        );
        let error =
            validate_epoch_transition(Epoch::new(1).expect("epoch"), Epoch::new(3).expect("epoch"))
                .expect_err("skipped epoch");
        assert_eq!(error.kind(), DomainErrorKind::InvalidStateTransition);
    }

    #[test]
    fn publication_cannot_skip_readback() {
        let error = transition_publication(
            &publication(PublicationIntentState::NewPointsAcknowledged),
            PublicationIntentState::ControlCommitted,
        )
        .expect_err("readback and old-point closure are mandatory");
        assert_eq!(error.kind(), DomainErrorKind::InvalidStateTransition);
    }

    #[test]
    fn committed_publication_can_only_become_reclaimable() {
        assert!(
            transition_publication(
                &publication(PublicationIntentState::ControlCommitted),
                PublicationIntentState::Reclaimable,
            )
            .is_ok()
        );
        assert!(
            transition_publication(
                &publication(PublicationIntentState::ControlCommitted),
                PublicationIntentState::Prepared,
            )
            .is_err()
        );
    }
}

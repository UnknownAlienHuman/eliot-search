//! Content-minimized health and shared mutation-boundary classification.

use search_domain::{MutationObservation, classify_mutation_outcome};

use crate::{
    LiveOwnerStatus, OwnerHealth, OwnerHealthReason, OwnerHealthState, OwnerLifecycle,
    OwnerObservation, OwnerSnapshot, OwnerState,
};

/// Computes bounded owner health from an immutable snapshot and complete observation.
#[must_use]
pub fn owner_health(snapshot: &OwnerSnapshot, observation: &OwnerObservation) -> OwnerHealth {
    match snapshot.state() {
        OwnerState::Vacant { root, .. } | OwnerState::Released { root, .. } => {
            if matches!(observation, OwnerObservation::Absent { root: observed, .. } if observed == root)
            {
                OwnerHealth::healthy(
                    OwnerHealthState::Vacant,
                    snapshot.state().highest_epoch(),
                    Some(*root),
                )
            } else {
                one_reason(
                    OwnerHealthState::Quarantined,
                    snapshot,
                    OwnerHealthReason::DurableReadbackIncomplete,
                )
            }
        }
        OwnerState::Active { record } | OwnerState::Draining { record, .. } => {
            let expected_state = if record.lifecycle() == OwnerLifecycle::Active {
                OwnerHealthState::Active
            } else {
                OwnerHealthState::Draining
            };
            let OwnerObservation::LiveMatchingOwner {
                record: observed,
                status: LiveOwnerStatus::Active,
                ..
            } = observation
            else {
                return one_reason(
                    OwnerHealthState::Quarantined,
                    snapshot,
                    OwnerHealthReason::OwnershipPrimitiveUnverified,
                );
            };
            let mut reasons = Vec::new();
            if observed.record_digest() != record.record_digest() {
                reasons.push(OwnerHealthReason::RecordDigestMismatch);
            }
            if observed.binding().epoch() != record.binding().epoch() {
                reasons.push(OwnerHealthReason::EpochMismatch);
            }
            if observed.binding().owner().process() != record.binding().owner().process() {
                reasons.push(OwnerHealthReason::ProcessIdentityMismatch);
            }
            if observed.binding().owner().executable() != record.binding().owner().executable() {
                reasons.push(OwnerHealthReason::ExecutableIdentityMismatch);
            }
            if reasons.is_empty() {
                OwnerHealth::healthy(
                    expected_state,
                    Some(record.binding().epoch()),
                    Some(record.binding().root()),
                )
            } else {
                bounded_health(
                    OwnerHealthState::Quarantined,
                    Some(record.binding().epoch()),
                    Some(record.binding().root()),
                    reasons,
                )
            }
        }
        OwnerState::Acquiring { .. }
        | OwnerState::AcquireOutcomeUnknown { .. }
        | OwnerState::Releasing { .. }
        | OwnerState::ReleaseOutcomeUnknown { .. } => one_reason(
            OwnerHealthState::OutcomeUnknown,
            snapshot,
            OwnerHealthReason::DurableReadbackIncomplete,
        ),
        OwnerState::Quarantined { .. } => one_reason(
            OwnerHealthState::Quarantined,
            snapshot,
            OwnerHealthReason::Quarantined,
        ),
    }
}

/// Converts explicit mutation-boundary facts to the shared semantic outcome.
#[must_use]
pub const fn classify_owner_mutation_boundary(
    observation: MutationObservation,
) -> search_domain::MutationOutcome {
    classify_mutation_outcome(observation)
}

fn bounded_health(
    state: OwnerHealthState,
    owner_epoch: Option<search_contracts::OwnerEpoch>,
    root: Option<crate::DataRootIdentity>,
    reasons: impl IntoIterator<Item = OwnerHealthReason>,
) -> OwnerHealth {
    OwnerHealth::with_reasons(state, owner_epoch, root, reasons)
        .unwrap_or_else(|_| OwnerHealth::healthy(OwnerHealthState::Quarantined, owner_epoch, root))
}

fn one_reason(
    state: OwnerHealthState,
    snapshot: &OwnerSnapshot,
    reason: OwnerHealthReason,
) -> OwnerHealth {
    bounded_health(
        state,
        snapshot.state().highest_epoch(),
        Some(snapshot.state().root()),
        [reason],
    )
}

#[cfg(test)]
mod tests {
    use search_domain::{
        AdmissionState, CancellationState, MutationObservation, MutationOutcomeClass,
        MutationRetryability, MutationStart, PostconditionState, SafetyState,
    };

    use super::classify_owner_mutation_boundary;

    #[test]
    fn cancellation_after_possible_mutation_remains_unknown() {
        let outcome = classify_owner_mutation_boundary(MutationObservation {
            mutation_start: MutationStart::MayHaveStarted,
            postcondition: PostconditionState::Unverified,
            cancellation: CancellationState::Cancelled,
            safety: SafetyState::Consistent,
            admission: AdmissionState::Admitted,
        });
        assert_eq!(outcome.class, MutationOutcomeClass::Unknown);
        assert_eq!(outcome.retryability, MutationRetryability::AfterReadback);
    }
}

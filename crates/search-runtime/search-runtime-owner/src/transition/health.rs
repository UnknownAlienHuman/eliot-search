//! Content-minimized health and shared mutation-boundary classification.

use search_domain::{
    AdmissionState, CancellationState, MutationObservation, MutationStart, PostconditionState,
    SafetyState, classify_mutation_outcome,
};

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
                OwnerHealth::with_reasons(
                    OwnerHealthState::Quarantined,
                    Some(record.binding().epoch()),
                    Some(record.binding().root()),
                    reasons,
                )
                .expect("owner-health reason count is statically bounded")
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
    mutation_may_have_started: bool,
    postcondition_verified: bool,
    cancelled: bool,
    quarantined: bool,
    rejected_before_mutation: bool,
) -> search_domain::MutationOutcome {
    classify_mutation_outcome(MutationObservation {
        mutation_start: if mutation_may_have_started {
            MutationStart::MayHaveStarted
        } else {
            MutationStart::NotStarted
        },
        postcondition: if postcondition_verified {
            PostconditionState::Verified
        } else {
            PostconditionState::Unverified
        },
        cancellation: if cancelled {
            CancellationState::Cancelled
        } else {
            CancellationState::NotCancelled
        },
        safety: if quarantined {
            SafetyState::Quarantined
        } else {
            SafetyState::Consistent
        },
        admission: if rejected_before_mutation {
            AdmissionState::RejectedBeforeMutation
        } else {
            AdmissionState::Admitted
        },
    })
}

fn one_reason(
    state: OwnerHealthState,
    snapshot: &OwnerSnapshot,
    reason: OwnerHealthReason,
) -> OwnerHealth {
    OwnerHealth::with_reasons(
        state,
        snapshot.state().highest_epoch(),
        Some(snapshot.state().root()),
        [reason],
    )
    .expect("one owner-health reason is always bounded")
}

#[cfg(test)]
mod tests {
    use search_domain::{MutationOutcomeClass, MutationRetryability};

    use super::classify_owner_mutation_boundary;

    #[test]
    fn cancellation_after_possible_mutation_remains_unknown() {
        let outcome = classify_owner_mutation_boundary(true, false, true, false, false);
        assert_eq!(outcome.class, MutationOutcomeClass::Unknown);
        assert_eq!(outcome.retryability, MutationRetryability::AfterReadback);
    }
}

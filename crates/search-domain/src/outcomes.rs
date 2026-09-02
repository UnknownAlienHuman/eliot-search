//! Pure mutation-outcome and retry classification.

/// Whether and how an unresolved operation may be retried.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum MutationRetryability {
    /// Same immutable operation identity may be retried.
    SafeWithSameIdentity,
    /// Exact authoritative readback is required first.
    AfterReadback,
    /// A replacement operation identity is required.
    RequiresNewIdentity,
    /// Retry is forbidden.
    Never,
}

/// Closed semantic outcome class.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum MutationOutcomeClass {
    /// Verified postcondition.
    Success,
    /// Rejected before mutation admission.
    Rejected,
    /// Cancelled before mutation started.
    CancelledBeforeMutation,
    /// Mutation may have occurred; exact outcome is unresolved.
    Unknown,
    /// Contradictory state requires quarantine.
    Quarantined,
}

/// Whether mutation may have begun.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum MutationStart {
    /// Mutation definitely did not start.
    NotStarted,
    /// An external or durable mutation may have started.
    MayHaveStarted,
}

/// Exact postcondition readback state.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum PostconditionState {
    /// Postcondition is not verified.
    Unverified,
    /// Exact readback verified the postcondition.
    Verified,
}

/// Cancellation observation.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum CancellationState {
    /// No cancellation was observed.
    NotCancelled,
    /// Cancellation was requested or observed.
    Cancelled,
}

/// Safety classification of observed state.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum SafetyState {
    /// No contradiction requiring quarantine was observed.
    Consistent,
    /// Contradictory state requires quarantine.
    Quarantined,
}

/// Admission outcome before mutation.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum AdmissionState {
    /// Operation passed admission or admission status is not load-bearing.
    Admitted,
    /// Operation was rejected before mutation.
    RejectedBeforeMutation,
}

/// Evidence observed at an operation boundary.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct MutationObservation {
    /// Mutation-start observation.
    pub mutation_start: MutationStart,
    /// Exact postcondition state.
    pub postcondition: PostconditionState,
    /// Cancellation observation.
    pub cancellation: CancellationState,
    /// Safety state.
    pub safety: SafetyState,
    /// Admission state.
    pub admission: AdmissionState,
}

/// Classified mutation outcome and retry rule.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct MutationOutcome {
    /// Semantic outcome class.
    pub class: MutationOutcomeClass,
    /// Retry rule implied by evidence.
    pub retryability: MutationRetryability,
}

/// Classifies evidence without inventing success after cancellation or timeout.
#[must_use]
pub const fn classify_mutation_outcome(observation: MutationObservation) -> MutationOutcome {
    if matches!(observation.safety, SafetyState::Quarantined) {
        return MutationOutcome {
            class: MutationOutcomeClass::Quarantined,
            retryability: MutationRetryability::Never,
        };
    }
    if matches!(observation.postcondition, PostconditionState::Verified) {
        return MutationOutcome {
            class: MutationOutcomeClass::Success,
            retryability: MutationRetryability::Never,
        };
    }
    if matches!(observation.mutation_start, MutationStart::MayHaveStarted) {
        return MutationOutcome {
            class: MutationOutcomeClass::Unknown,
            retryability: MutationRetryability::AfterReadback,
        };
    }
    if matches!(
        observation.admission,
        AdmissionState::RejectedBeforeMutation
    ) {
        return MutationOutcome {
            class: MutationOutcomeClass::Rejected,
            retryability: MutationRetryability::RequiresNewIdentity,
        };
    }
    if matches!(observation.cancellation, CancellationState::Cancelled) {
        return MutationOutcome {
            class: MutationOutcomeClass::CancelledBeforeMutation,
            retryability: MutationRetryability::SafeWithSameIdentity,
        };
    }
    MutationOutcome {
        class: MutationOutcomeClass::Rejected,
        retryability: MutationRetryability::SafeWithSameIdentity,
    }
}

#[cfg(test)]
mod tests {
    use super::{
        AdmissionState, CancellationState, MutationObservation, MutationOutcomeClass,
        MutationRetryability, MutationStart, PostconditionState, SafetyState,
        classify_mutation_outcome,
    };

    #[test]
    fn cancellation_after_possible_mutation_is_unknown() {
        let outcome = classify_mutation_outcome(MutationObservation {
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

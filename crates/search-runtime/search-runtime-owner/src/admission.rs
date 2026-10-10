//! Pure data-root open policy for #266.
//!
//! Classifies one requested open mode against one complete ownership
//! observation into the next durable step. The result is a decision, never a
//! capability: this module mints no guard, token, lease or epoch and performs
//! no I/O.
//!
//! `RequireEmptyLayoutProof` and `RequireExactRecovery` name proofs the daemon adapter
//! still owes: native exclusion plus an empty root, and the original durable
//! operation identity. Neither is satisfied by this classification alone.

use crate::OwnerError;

/// Requested way to open one data root.
#[derive(Clone, Copy, Debug, Eq, Ord, PartialEq, PartialOrd)]
pub enum DataRootOpenMode {
    /// Content-free inspection, and reads of an already-initialized root.
    InspectExistingReadOnly,
    /// Open an existing root for mutation.
    OpenExistingMutating,
    /// Establish the initial layout of a root with no owner evidence.
    InitializeNew,
    /// Resolve one exact durable operation whose outcome is unknown.
    OpenNamedRecovery,
}

/// Complete ownership classification of one data root.
#[derive(Clone, Copy, Debug, Eq, Ord, PartialEq, PartialOrd)]
pub enum RootOpenState {
    /// No owner evidence exists.
    Absent,
    /// A verified live owner record exists.
    Initialized,
    /// Contradictory or incomplete evidence blocks every change.
    Quarantined,
    /// A mutation may have crossed the external boundary.
    OutcomeUnknown,
    /// Durable state is malformed.
    Corrupt,
    /// A root held by another installation or incarnation.
    ForeignOwner,
    /// A location or format the product does not support.
    Unsupported,
}

/// Next durable step one open request admits.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum RootOpenDecision {
    /// Classification only; no durable access is admitted.
    InspectClassification,
    /// Read the already-durable layout.
    ReadExisting,
    /// A live verified owner must supply mutating authority.
    RequireLiveOwner,
    /// Establish an empty layout, after native and empty-root proof.
    RequireEmptyLayoutProof,
    /// Resolve one exact named recovery operation.
    RequireExactRecovery,
    /// Refused with a closed reason.
    Refused(OwnerError),
}

/// Classifies one open request against one observed state.
#[must_use]
pub const fn classify_root_open(mode: DataRootOpenMode, state: RootOpenState) -> RootOpenDecision {
    use crate::RootOpenDecision as D;
    use crate::{DataRootOpenMode as M, RootOpenState as S};
    match (mode, state) {
        (M::InspectExistingReadOnly, S::Initialized) => D::ReadExisting,
        (M::OpenExistingMutating, S::Initialized) => D::RequireLiveOwner,
        (M::InitializeNew, S::Absent) => D::RequireEmptyLayoutProof,
        (M::OpenNamedRecovery, S::Quarantined | S::OutcomeUnknown) => D::RequireExactRecovery,
        (M::InspectExistingReadOnly, _) => D::InspectClassification,
        _ => D::Refused(refusal_reason(mode, state)),
    }
}

/// Closed refusal reason for every combination the classifier does not admit.
const fn refusal_reason(mode: DataRootOpenMode, state: RootOpenState) -> OwnerError {
    use crate::{DataRootOpenMode as M, RootOpenState as S};
    match (state, mode) {
        (S::Absent, M::OpenExistingMutating | M::InspectExistingReadOnly) => {
            OwnerError::OwnerInvalidTransition
        }
        (S::Absent, M::InitializeNew | M::OpenNamedRecovery) => {
            OwnerError::OwnerRecoveryEvidenceMissing
        }
        (S::Initialized, M::InitializeNew) | (S::ForeignOwner, _) => {
            OwnerError::DataRootAlreadyOwned
        }
        (S::Initialized, _) => OwnerError::OwnerRecoveryEvidenceMissing,
        (S::Quarantined, _) => OwnerError::OwnerRecoveryQuarantined,
        (S::OutcomeUnknown, _) => OwnerError::OwnerAcquireOutcomeUnknown,
        (S::Corrupt, _) => OwnerError::OwnerIdentityAmbiguous,
        (S::Unsupported, _) => OwnerError::DataRootInvalid,
    }
}

#[cfg(test)]
mod tests {
    use super::{
        DataRootOpenMode as M, RootOpenDecision as D, RootOpenState as S, classify_root_open,
    };
    use crate::OwnerError as E;

    #[test]
    fn initialized_root_admits_read_and_requires_live_owner_to_mutate() {
        assert_eq!(
            classify_root_open(M::InspectExistingReadOnly, S::Initialized),
            D::ReadExisting
        );
        assert_eq!(
            classify_root_open(M::OpenExistingMutating, S::Initialized),
            D::RequireLiveOwner
        );
        assert_eq!(
            classify_root_open(M::InitializeNew, S::Initialized),
            D::Refused(E::DataRootAlreadyOwned)
        );
    }

    #[test]
    fn absent_root_grants_no_read_capability_and_no_implicit_create() {
        assert_eq!(
            classify_root_open(M::InspectExistingReadOnly, S::Absent),
            D::InspectClassification
        );
        assert_eq!(
            classify_root_open(M::OpenExistingMutating, S::Absent),
            D::Refused(E::OwnerInvalidTransition),
        );
        assert_eq!(
            classify_root_open(M::InitializeNew, S::Absent),
            D::RequireEmptyLayoutProof
        );
        // Inspection of an absent root never reaches durable bytes.
        assert_ne!(
            classify_root_open(M::InspectExistingReadOnly, S::Absent),
            D::ReadExisting
        );
    }

    #[test]
    fn named_recovery_is_reachable_only_for_unresolved_outcomes() {
        assert_eq!(
            classify_root_open(M::OpenNamedRecovery, S::Quarantined),
            D::RequireExactRecovery
        );
        assert_eq!(
            classify_root_open(M::OpenNamedRecovery, S::OutcomeUnknown),
            D::RequireExactRecovery,
        );
        for state in [
            S::Absent,
            S::Initialized,
            S::Corrupt,
            S::ForeignOwner,
            S::Unsupported,
        ] {
            assert_ne!(
                classify_root_open(M::OpenNamedRecovery, state),
                D::RequireExactRecovery
            );
            assert!(matches!(
                classify_root_open(M::OpenNamedRecovery, state),
                D::Refused(_)
            ));
        }
    }

    #[test]
    fn recovery_never_yields_an_ordinary_read_or_mutate_capability() {
        for state in [S::Quarantined, S::OutcomeUnknown] {
            let decision = classify_root_open(M::OpenNamedRecovery, state);
            assert_ne!(decision, D::ReadExisting);
            assert_ne!(decision, D::RequireLiveOwner);
            assert_ne!(decision, D::RequireEmptyLayoutProof);
        }
    }

    #[test]
    fn inspection_is_content_free_on_every_state_but_initialized() {
        for state in [
            S::Absent,
            S::Quarantined,
            S::OutcomeUnknown,
            S::Corrupt,
            S::ForeignOwner,
            S::Unsupported,
        ] {
            assert_eq!(
                classify_root_open(M::InspectExistingReadOnly, state),
                D::InspectClassification,
            );
        }
        assert_eq!(
            classify_root_open(M::InspectExistingReadOnly, S::Initialized),
            D::ReadExisting
        );
    }

    #[test]
    fn closed_states_refuse_all_but_inspection() {
        for state in [
            S::Corrupt,
            S::ForeignOwner,
            S::Unsupported,
            S::Quarantined,
            S::OutcomeUnknown,
        ] {
            for mode in [M::OpenExistingMutating, M::InitializeNew] {
                assert!(matches!(classify_root_open(mode, state), D::Refused(_)));
            }
        }
    }
}

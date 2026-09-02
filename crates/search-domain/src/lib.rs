//! Pure deterministic policy kernels for ELIOT Search.
//!
//! This package owns semantic validation, ordering, transitions, eligibility,
//! currentness, assurance, coverage, and fingerprint verification. It performs
//! no I/O and owns no mutable external capability state.

#![forbid(unsafe_code)]
#![deny(missing_docs)]
#![allow(clippy::module_name_repetitions)]

pub mod assurance;
pub mod coverage;
pub mod currentness;
pub mod decisions;
pub mod eligibility;
pub mod error;
pub mod ordering;
pub mod outcomes;
pub mod transitions;
pub mod visibility;

pub use assurance::{
    AssuranceDecision, AssuranceReason, AssuranceRequirement, evaluate_assurance, minimum_assurance,
};
pub use coverage::{
    CoverageClassification, CoverageEvidence, classify_coverage, proves_complete_negative,
};
pub use currentness::{
    CurrentnessAxis, CurrentnessDecision, CurrentnessReason, CurrentnessRequirement,
    CurrentnessState, SnapshotDrift, SnapshotDriftAxis, classify_snapshot_drift,
    emission_requires_revalidation, evaluate_currentness,
};
pub use decisions::{
    Decision, DecisionError, ReasonSet, compute_and_verify_plan_fingerprint,
    compute_and_verify_query_snapshot_fingerprint, compute_plan_fingerprint,
    compute_query_snapshot_fingerprint,
};
pub use eligibility::{
    BaseEligibilityPredicate, EligibilityAtom, EligibilityDecision, EligibilityReason,
    EligibilitySet, ScopeRelation, build_base_eligibility_predicate, decide_eligibility,
    prove_retrieval_idf_filter_equivalence, verify_safe_leg_filter_digests,
};
pub use error::{DomainError, DomainErrorKind};
pub use ordering::{CandidateOrderKey, stable_candidate_order, stable_sort_candidates};
pub use outcomes::{
    AdmissionState, CancellationState, MutationObservation, MutationOutcome, MutationOutcomeClass,
    MutationRetryability, MutationStart, PostconditionState, SafetyState,
    classify_mutation_outcome,
};
pub use transitions::{
    OwnershipTransition, transition_publication, transition_source_ownership,
    validate_epoch_transition,
};
pub use visibility::{
    CapabilityState, RequirementState, VisibilityDecision, VisibilityReason, VisibilityRequirement,
    evaluate_visibility,
};

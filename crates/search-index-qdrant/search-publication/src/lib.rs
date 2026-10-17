//! Linearizable exact-point publication and crash recovery.
//!
//! Qdrant aliases and process health are never the visibility linearization
//! point. Visibility changes only after exact stage/closure readback and one
//! guarded control-state compare-and-swap.

#![forbid(unsafe_code)]
#![allow(clippy::missing_errors_doc, clippy::module_name_repetitions)]

mod admission;
mod error;
mod machine;
mod model;
mod recovery;

pub use admission::{
    AdmissionPurpose, AdmissionWork, ProjectionEvidence, ProjectionUpdate,
    PublicationAdmissionOwners, PublicationMembershipState, PublicationOwnerObservation,
    PublicationProposal, PublicationScope, PublicationTarget, validate_prepared_generation,
};

pub use error::PublicationError;
pub use machine::{
    AbortControlCommitObservation, AbortFinalizationRequest, AbortedPublicationResolution,
    DEFAULT_MAX_PUBLICATION_POINTS, DurableIntent, PublicationCoordinator,
    PublicationEffectCommand, PublicationEffectPort, PublicationEffectPreparationOwners,
    PublicationMutationAttempt, PublicationPhase, PublicationReadbackPort, PublicationRestoreInput,
    PublicationTransaction,
};
pub use model::{
    ClosureReceipt, CompensationPlan, PreparedGeneration, PublicationAbandonReadback,
    PublicationControlReadback, PublicationEffectBinding, PublicationEffectKind,
    PublicationEffectOutcome, PublicationEffectReadback, PublicationGuards,
    PublicationIntentReadback, PublicationPointExpectation, PublicationPointReadback,
    PublicationRecoveryDecision, PublicationSnapshotReadback, ReadbackVerified, RetiredManifest,
    SnapshotPublishReceipt, StageReceipt, VisibleCommitReceipt,
};
pub use recovery::recover;

#[cfg(test)]
mod admission_tests;

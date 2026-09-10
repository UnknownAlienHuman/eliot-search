//! Linearizable exact-point publication and crash recovery.
//!
//! Qdrant aliases and process health are never the visibility linearization
//! point. Visibility changes only after exact stage/closure readback and one
//! guarded control-state compare-and-swap.

#![forbid(unsafe_code)]
#![allow(clippy::missing_errors_doc, clippy::module_name_repetitions)]

mod error;
mod machine;
mod model;
mod recovery;

pub use error::PublicationError;
pub use machine::{
    AbortControlCommitObservation, AbortFinalizationRequest, AbortedPublicationResolution,
    DEFAULT_MAX_PUBLICATION_POINTS, DurableIntent, PublicationCoordinator, PublicationPhase,
    PublicationRestoreInput, PublicationTransaction,
};
pub use model::{
    AbandonFence, ClosureReceipt, CompensationPlan, CompensationReceipt, ControlCommitObservation,
    PreparedPublication, PublicationGuards, PublicationRecoveryDecision,
    PublicationRecoveryObservation, ReadbackVerified, RestorationReceipt, RetiredManifest,
    SnapshotPublishReceipt, StageReceipt, VisibleCommitReceipt,
};
pub use recovery::recover;

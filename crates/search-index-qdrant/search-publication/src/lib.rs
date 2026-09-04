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
    DEFAULT_MAX_PUBLICATION_POINTS, DurableIntent, PublicationCoordinator,
    PublicationPhase, PublicationTransaction,
};
pub use model::{
    AbandonFence, ClosureReceipt, CompensationReceipt, ControlCommitObservation,
    PreparedPublication, PublicationGuards, PublicationRecoveryDecision,
    PublicationRecoveryObservation, ReadbackVerified, RetiredManifest,
    SnapshotPublishReceipt, StageReceipt, VisibleCommitReceipt,
};
pub use recovery::recover;

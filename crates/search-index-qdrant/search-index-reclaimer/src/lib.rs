//! Exact ordinary reclamation of retired index points.
//!
//! This package deletes rebuildable retired Qdrant points by exact identifier.
//! It owns neither publication visibility nor security purge and exposes no
//! broad-filter deletion operation.

#![forbid(unsafe_code)]
#![deny(missing_docs)]
#![allow(clippy::missing_errors_doc, clippy::module_name_repetitions)]

mod error;
mod manifest;
mod plan;
mod receipt;

pub use error::ReclaimError;
pub use manifest::{
    CommittedRetiredManifest, PublicationCommitProof, ReclaimPointId,
    RetiredPointManifest, validate_retired_manifest,
};
pub use plan::{
    ReclaimBatch, ReclaimBudget, ReclaimPlan, ReclaimPlanDigest, ReclaimSettings,
    plan,
};
pub use receipt::{
    ReclaimBatchOutcome, ReclaimBatchReceipt, ReclaimCheckpoint, ReclaimReceipt,
    ReclaimReceiptKind, checkpoint, complete, resume, verify_batch_receipt,
};

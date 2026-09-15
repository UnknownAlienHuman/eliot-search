//! Rebuild composition behind the stable module facade.

mod cutover;
mod error;
mod manifest;
mod pins;
mod plan;
mod reclaim;

pub use cutover::{
    CommittedCutover, StagedCutover, commit_cutover, stage_cutover,
};
pub use error::RebuildError;
pub use manifest::{
    RetainedManifest, RetainedPoint, retained_manifest_digest,
    validate_retained_manifest,
};
pub use pins::{
    QuerySession, begin_pinned_query, expire_continuation_pins_bounded,
    release_owner_pins_of,
};
pub use plan::{
    FullReadbackProof, IndexReadbackView, RebuildBudget, RebuildPlan,
    propose_rebuild, verify_full_readback,
};
pub use reclaim::{
    ReclaimTuning, authorize_reclaim, is_ordinary_reclaim_receipt,
};

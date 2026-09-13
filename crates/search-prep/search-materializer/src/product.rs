//! Exact revision acquisition and end-to-end baseline materialization.
//!
//! The public product boundary is a thin facade. Exact revision acquisition,
//! public product models, deterministic digests, execution, canonical encoding,
//! verification and tests remain separate private owners.

mod codec;
mod digest;
mod model;
mod pipeline;
mod read;
mod verify;

pub use codec::canonicalize_materialization;
pub use model::{
    CanonicalMaterializationBytes, MaterializationAdmissionPlan, MaterializationContext,
    MaterializationProduct, MaterializationVerificationReceipt, MaterializationWarning,
    ResourceReceipt,
};
pub use pipeline::materialize_text_or_code;
pub use read::{RevisionBytesGuard, RevisionReadPort, StoredRevisionBytes, open_exact_revision};
pub use verify::{prepare_admission, verify_materialization};

#[cfg(test)]
mod tests;

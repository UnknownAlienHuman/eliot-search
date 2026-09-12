//! Executable deterministic context-artifact candidate builder.
//!
//! Repository inputs come only from one immutable Git commit. Successful
//! output consists of two ordinary local files below the configured advisory
//! artifact root: one length-framed context bundle and one canonical metadata
//! record. The builder cannot create control records, manifests, tickets,
//! leases, handoffs, acceptance receipts or launch authority.

mod assemble;
mod extract;
mod model;
mod preflight;
mod write;

pub use assemble::build_candidate;
pub use model::{CandidateBuild, CandidateCheck, ContextArtifactBuildError};
pub use write::write_candidate;

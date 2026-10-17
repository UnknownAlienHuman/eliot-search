//! T27 publication composition: guarded control-floor orchestration.
//!
//! The stable daemon-local surface delegates to bounded private owners for
//! guard observations, exact compensation contracts, logical retirement,
//! crash recovery and the single-active publication state machine.
//!
//! Qdrant SDK types and transport execution do not live in this module. The
//! compatibility compensation port preserves the existing process-test API;
//! a concrete live adapter must translate a complete route/payload/context at
//! the Qdrant boundary rather than pretending point identifiers alone are a
//! production upsert request.

#![forbid(unsafe_code)]

#[path = "publication_composition/kernel.rs"]
mod kernel;

#[cfg(feature = "wave3-index")]
#[path = "publication_composition/canonical_control.rs"]
pub(crate) mod canonical_control;

#[allow(unused_imports)] // Retains the stable Root facade until the canonical publisher callsite cuts over.
pub use kernel::*;

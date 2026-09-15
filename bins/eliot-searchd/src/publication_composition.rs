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

pub use kernel::*;

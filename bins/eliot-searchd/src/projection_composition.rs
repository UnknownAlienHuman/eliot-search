//! Membership-scoped projection composition.
//!
//! The stable daemon-local surface delegates to bounded private owners for
//! admitted request models, deterministic scope/payload digests, pure plan
//! composition, immutable scoped CAS persistence and reconstruction proofs.

#[path = "projection_composition/kernel.rs"]
mod kernel;

pub use kernel::*;

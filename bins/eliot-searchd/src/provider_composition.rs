//! Canonical provider-protocol composition for the loopback daemon.
//!
//! The stable daemon-local surface is exported here. Closed wire specification,
//! capability gating, keyed envelope proofing, bounded framing and rendering,
//! connection state, child-outcome mapping, and workspace-currentness remain
//! behind one private owner while they are separated by responsibility.

#[path = "provider_composition/kernel.rs"]
mod kernel;

pub use kernel::*;

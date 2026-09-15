//! Purpose-bound OS-secret leases for loopback pairing.
//!
//! The stable daemon-local surface delegates to bounded private owners for
//! purpose binding, vault effects, catalog lifecycle, finite leases, keyed
//! proofs and exact recovery after ambiguous platform mutations.

#[path = "secret_composition/kernel.rs"]
mod kernel;

pub use kernel::*;

//! Persistent bounded observation-root catalog for the primary daemon.
//!
//! The stable daemon-local surface delegates to bounded owners for the
//! persisted registry, root/path policy, live catalog state and explicit
//! currentness gaps. Registration remains observation configuration only.

#[path = "source_roots/kernel.rs"]
mod kernel;

pub use kernel::*;

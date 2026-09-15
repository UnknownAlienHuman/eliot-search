//! Concrete bounded runtime helpers for the primary daemon.
//!
//! The stable daemon-local surface delegates to bounded private owners for
//! truthful readiness, one-shot scanning and the single live data-root guard.

#[path = "development/kernel.rs"]
mod kernel;

pub use kernel::*;

//! Authenticated bounded loopback transport for the development daemon.
//!
//! The stable daemon-local surface delegates to bounded private owners for
//! pairing, strict wire framing, listener lifetime and regression coverage.

#[path = "endpoint/kernel.rs"]
mod kernel;

pub use kernel::*;

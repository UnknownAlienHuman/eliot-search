//! Append-only sealed access authority.
//!
//! The stable surface delegates to bounded private owners for the closed
//! failure vocabulary, access-fence models, chain validation/replay, exact
//! append/read admission and platform-specific sealed-object discovery.

#![cfg_attr(not(windows), allow(dead_code))]

#[path = "sealed_access/kernel.rs"]
mod kernel;

pub use kernel::*;

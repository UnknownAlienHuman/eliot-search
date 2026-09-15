//! Monotone sealed owner epochs for one Windows data root.
//!
//! The stable surface delegates to bounded private owners for the closed
//! failure vocabulary, strict epoch-record codec, guard lifetime, fixed-width
//! identities, platform acquisition/discovery and regression coverage.

#![cfg_attr(not(windows), allow(dead_code))]

#[path = "sealed_owner_epoch/kernel.rs"]
mod kernel;

pub use kernel::*;

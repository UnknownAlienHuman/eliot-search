//! Bounded startup recovery for DPAPI-sealed transactions.
//!
//! The stable daemon-local surface delegates to bounded private owners for
//! recovery contracts, report accounting, owner-guard admission, platform
//! dispatch, strict transaction enumeration, and exact reconciliation.

#![cfg_attr(not(windows), allow(dead_code))]

#[path = "sealed_recovery/kernel.rs"]
mod kernel;

pub use kernel::*;

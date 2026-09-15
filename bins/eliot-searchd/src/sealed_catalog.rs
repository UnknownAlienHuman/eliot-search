//! DPAPI-sealed source-revision catalog bindings.
//!
//! The stable daemon-local surface delegates to bounded private owners for
//! closed failure vocabulary, the immutable binding model, canonical manifest
//! codec, transactional binding publication, authenticated reads, and tests.

#![cfg_attr(not(windows), allow(dead_code))]

#[path = "sealed_catalog/kernel.rs"]
mod kernel;

pub use kernel::*;

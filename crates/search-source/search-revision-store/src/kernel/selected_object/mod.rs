//! Selected-reference domain bindings and pre-seal source metadata.
//!
//! This module owns the pure metadata layer for selected-reference objects:
//! typed domain bindings with genuine UUIDs and full tagged BLAKE3 digests,
//! typed source identities, the pre-seal occurrence descriptor, and the late
//! physical-object metadata binding applied after AES. It performs no filesystem,
//! cryptographic, or secret-store I/O. Pure metadata confers no admission,
//! access, clock, or key authority. The root supplies actual metered UUIDs and
//! qualified records; this module binds and verifies them only.

pub mod domain;
pub mod source;

pub use domain::*;
pub use source::*;

#[cfg(test)]
mod tests;

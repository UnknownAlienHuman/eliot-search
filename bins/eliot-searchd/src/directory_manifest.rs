//! Immutable directory inventory generations and explicit reconciliation.
//!
//! The stable daemon-local surface delegates to bounded private owners for
//! manifest models, exact codec/digests, filesystem roots and discovery,
//! immutable publication, verification/migration, and directory reconciliation.

#[path = "directory_manifest/kernel.rs"]
mod kernel;

pub use kernel::*;

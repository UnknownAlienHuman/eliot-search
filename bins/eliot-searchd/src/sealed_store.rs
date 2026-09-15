//! Windows `CurrentUser` DPAPI-backed immutable sealed-object storage.
//!
//! The stable harness/daemon surface delegates to bounded private owners for
//! closed limits and errors, zeroizing plaintext ownership, strict envelope
//! coding, platform dispatch and the Windows DPAPI/file boundary.

#[path = "sealed_store/kernel.rs"]
mod kernel;

pub use kernel::*;

//! Bounded restore, explicit key migration and registry-owned cutover (T39).
//!
//! Daemon-side composition remains a thin boundary over retention-owned
//! revalidation, OS-secret binding and registry-owned namespace cutover.
//! Restore state never becomes ready without exact readback and accepted
//! ownership evidence.

#![forbid(unsafe_code)]

#[path = "restore_composition/kernel.rs"]
mod kernel;

pub use kernel::*;

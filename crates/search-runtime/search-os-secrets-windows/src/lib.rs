//! Windows current-user DPAPI effect adapter.
//!
//! This crate owns the native `CryptProtectData` / `CryptUnprotectData`
//! boundary and every DPAPI-owned `LocalAlloc` buffer. It performs no
//! filesystem, Credential Manager, registry, clock, process, source-catalog or
//! policy I/O. Callers own persistence and supply exact non-secret entropy.
//!
//! Two bounded surfaces are exposed:
//!
//! - short lifecycle secrets use [`SecretBytes`], [`ProtectionScope`] and
//!   [`ProtectedSecret`];
//! - the frozen legacy revision compatibility path accepts exact envelope bytes
//!   plus the already-derived 32-byte optional entropy.

#![deny(unsafe_op_in_unsafe_fn)]
#![deny(missing_docs)]
#![warn(clippy::all, clippy::pedantic)]
#![allow(clippy::missing_errors_doc, clippy::module_name_repetitions)]

mod dpapi;
mod model;

pub use dpapi::*;
pub use model::*;

#[cfg(test)]
mod tests;

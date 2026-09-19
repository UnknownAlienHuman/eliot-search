//! Windows current-user secret effect adapter.
//!
//! This crate owns the native Credential Manager, `BCryptGenRandom`, named
//! vault mutex, `CryptProtectData` / `CryptUnprotectData`, and every native
//! allocation returned by those APIs. It performs no filesystem, registry,
//! clock, process, source-catalog or policy I/O.
//!
//! Three bounded surfaces are exposed:
//!
//! - legacy revision root-secret read/create receives one namespace identity
//!   plus the caller's already-established missing-key requirement;
//! - short lifecycle secrets use [`SecretBytes`], [`ProtectionScope`] and
//!   [`ProtectedSecret`];
//! - the frozen legacy revision compatibility path accepts exact envelope bytes
//!   plus the already-derived 32-byte optional entropy.

#![deny(unsafe_op_in_unsafe_fn)]
#![deny(missing_docs)]
#![warn(clippy::all, clippy::pedantic)]
#![allow(clippy::missing_errors_doc, clippy::module_name_repetitions)]

mod credential;
mod dpapi;
mod model;

pub use credential::*;
pub use dpapi::*;
pub use model::*;

#[cfg(test)]
mod tests;

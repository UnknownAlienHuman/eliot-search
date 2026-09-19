//! Windows current-user secret effect adapter.
//!
//! This crate owns the native Credential Manager, `BCryptGenRandom`, named
//! vault mutex, `CryptProtectData` / `CryptUnprotectData`, and every native
//! allocation returned by those APIs. It performs no filesystem, registry,
//! clock, process, source-catalog or policy I/O.
//!
//! Three bounded production surfaces are exposed:
//!
//! - legacy revision root-secret read/create receives one namespace identity
//!   plus the caller's already-established missing-key requirement;
//! - short lifecycle secrets use [`SecretBytes`], [`ProtectionScope`] and
//!   [`ProtectedSecret`];
//! - the frozen legacy revision compatibility path accepts exact envelope bytes
//!   plus the already-derived 32-byte optional entropy.
//!
//! The optional `test-credential-cleanup` feature adds one exact-namespace,
//! readback-verified cleanup operation for native test harnesses.

#![deny(unsafe_op_in_unsafe_fn)]
#![deny(missing_docs)]
#![warn(clippy::all, clippy::pedantic)]
#![allow(clippy::missing_errors_doc, clippy::module_name_repetitions)]

mod credential;
mod dpapi;
mod model;
#[cfg(feature = "test-credential-cleanup")]
mod test_credential_cleanup;

pub use credential::*;
pub use dpapi::*;
pub use model::*;
#[cfg(feature = "test-credential-cleanup")]
pub use test_credential_cleanup::*;

#[cfg(test)]
mod tests;

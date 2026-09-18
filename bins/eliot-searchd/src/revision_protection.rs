//! Platform revision-object protection composition.
//!
//! The pure legacy envelope contract belongs to `search-os-secrets`.
//! This module assembles that contract with the existing credential and DPAPI
//! effects while preserving the historical DIRECT compatibility surface.

#![allow(
    clippy::missing_errors_doc,
    clippy::module_name_repetitions,
    clippy::similar_names,
    clippy::too_many_lines,
    clippy::unnecessary_wraps,
    clippy::unused_self
)]

mod envelope;
mod protector;

pub(crate) use protector::{PROTECTED_OBJECT_EXTENSION, RevisionProtector};

#[cfg(windows)]
use zeroize::Zeroize;

// Native allocation cleanup in the existing FFI adapter uses this helper.
#[cfg(windows)]
fn zeroize(bytes: &mut [u8]) {
    bytes.zeroize();
}

#[cfg(windows)]
#[path = "revision_protection_windows.rs"]
mod windows;

#[cfg(test)]
mod test_support;
#[cfg(test)]
pub use test_support::{TestCredentialGuard, lock_unit_vault_for_test};

#[cfg(test)]
#[path = "revision_protection_tests.rs"]
mod tests;

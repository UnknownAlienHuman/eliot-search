#![allow(unsafe_code)]

//! Windows revision-protection composition.
//!
//! Credential Manager, CSPRNG, vault locking, DPAPI execution, and their native
//! allocations belong to `search-os-secrets-windows`. This module retains only
//! protected-object inventory, frozen envelope/digest composition, historical
//! `DIRECT_*` reason translation, and test-only credential cleanup.

mod credential;
mod dpapi;
mod existing;
#[cfg(test)]
mod ffi;
mod inventory;
#[cfg(test)]
mod test_cleanup;

pub(super) use credential::load_or_create_root_secret;
pub(super) use dpapi::{protect_data, unprotect_data};
#[cfg(test)]
pub(super) use test_cleanup::{
    delete_test_credential_for_data_root, read_test_namespace_hex,
};

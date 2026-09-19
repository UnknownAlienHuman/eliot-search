//! Windows revision-protection composition.
//!
//! Credential Manager, CSPRNG, vault locking, DPAPI execution, native
//! allocations, and bounded test credential deletion belong to
//! `search-os-secrets-windows`. This module retains protected-object inventory,
//! frozen envelope/digest composition, historical `DIRECT_*` reason translation,
//! namespace-file parsing, and test cleanup orchestration.

mod credential;
mod dpapi;
mod existing;
mod inventory;
#[cfg(test)]
mod test_cleanup;

pub(super) use credential::load_or_create_root_secret;
pub(super) use dpapi::{protect_data, unprotect_data};
#[cfg(test)]
pub(super) use test_cleanup::{
    delete_test_credential_for_data_root, read_test_namespace_hex,
};

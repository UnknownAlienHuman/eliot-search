//! Windows revision-protection composition.
//!
//! Credential Manager, CSPRNG, vault locking, DPAPI execution, native
//! allocations, and bounded test credential deletion belong to
//! `search-os-secrets-windows`. This module retains protected-object inventory,
//! frozen envelope/digest composition, historical `DIRECT_*` reason translation,
//! namespace-file parsing, and test cleanup orchestration.

#[path = "revision_protection_windows/credential.rs"]
mod credential;
#[path = "revision_protection_windows/dpapi.rs"]
mod dpapi;
#[path = "revision_protection_windows/existing.rs"]
mod existing;
#[path = "revision_protection_windows/inventory.rs"]
mod inventory;
#[cfg(test)]
#[path = "revision_protection_windows/test_cleanup.rs"]
mod test_cleanup;

pub(super) use credential::load_or_create_root_secret;
pub(super) use dpapi::{protect_data, unprotect_data};
#[cfg(test)]
pub(super) use test_cleanup::{
    delete_test_credential_for_data_root, read_test_namespace_hex,
};

#![allow(unsafe_code)]

//! Windows-native revision protection adapter.
//!
//! Raw FFI ownership, credential lifecycle, DPAPI translation, protected
//! object inventory and test-only cleanup remain private and separately bounded.

mod credential;
mod dpapi;
mod existing;
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

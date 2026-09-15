//! Platform selection for sealed transaction storage.

#[cfg(not(windows))]
mod unsupported;
#[cfg(windows)]
mod windows;

#[cfg(not(windows))]
pub(crate) use unsupported::{
    inspect_transaction, put_idempotent, transaction_status,
};
#[cfg(windows)]
pub(crate) use windows::{
    inspect_transaction, put_idempotent, transaction_status,
};

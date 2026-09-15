//! Platform selection for sealed storage.

#[cfg(not(windows))]
mod unsupported;
#[cfg(windows)]
#[allow(unsafe_code)]
mod windows;

#[cfg(not(windows))]
pub(crate) use unsupported::{
    delete_sealed, open_sealed, seal_immutable, verify_sealed,
};
#[cfg(windows)]
pub(crate) use windows::{
    delete_sealed, open_sealed, seal_immutable, verify_sealed,
};

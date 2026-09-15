//! Platform-specific owner-epoch acquisition and sealed-head observation.

#[cfg(not(windows))]
mod unsupported;
#[cfg(windows)]
mod windows;

#[cfg(not(windows))]
pub(super) use unsupported::{acquire, latest_head};
#[cfg(windows)]
pub(super) use windows::{acquire, latest_head};

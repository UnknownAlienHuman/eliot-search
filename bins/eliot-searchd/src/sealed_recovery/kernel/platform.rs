//! Platform dispatch for sealed transaction recovery.

#[cfg(not(windows))]
#[path = "platform/unsupported.rs"]
mod implementation;
#[cfg(windows)]
#[path = "platform/windows.rs"]
mod implementation;

pub(super) use implementation::recover_all;

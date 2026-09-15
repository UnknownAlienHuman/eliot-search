//! Platform-specific sealed access-fence object discovery.

#[cfg(not(windows))]
mod unsupported;
#[cfg(windows)]
mod windows;

#[cfg(not(windows))]
pub(super) use unsupported::discover;
#[cfg(windows)]
pub(super) use windows::discover;

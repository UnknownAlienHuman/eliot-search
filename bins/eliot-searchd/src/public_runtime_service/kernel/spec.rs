//! Closed runtime-service protocol and input bounds.

pub(super) const PROTOCOL_VERSION: u16 = 1;
pub(super) const MAX_COMMAND_BYTES: usize = 256 * 1024;
pub(super) const MAX_PATH_BYTES: usize = 32 * 1024;
pub(super) const MAX_DIAGNOSTIC_REVISION_SLICE_BYTES: u64 = 24 * 1024;

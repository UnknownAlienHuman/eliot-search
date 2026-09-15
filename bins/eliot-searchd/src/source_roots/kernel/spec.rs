//! Closed observation-root bounds and persisted catalog header.

pub const MAX_SOURCE_ROOTS: usize = 32;
pub const MAX_SOURCE_ROOT_FILE_BYTES: usize = 64 * 1024;
pub const MAX_SOURCE_ROOT_PATH_BYTES: usize = 512;
/// Maximum watcher hints retained in memory. Hints are dirty markers only;
/// overflow becomes an explicit gap that forces a full refresh.
#[allow(dead_code)]
pub const MAX_WATCHER_HINTS: usize = 64;
/// Maximum observation gaps reported in one truth snapshot.
pub const MAX_OBSERVATION_GAPS: usize = 40;
/// Exact first line of the persisted registration catalog.
pub(crate) const HEADER: &str = "# ELIOT Search source roots v1";

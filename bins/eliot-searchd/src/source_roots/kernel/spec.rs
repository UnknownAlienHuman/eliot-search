//! Closed observation-root bounds.

pub use search_source_registry::{
    LEGACY_SOURCE_ROOT_CATALOG_MAX_BYTES as MAX_SOURCE_ROOT_FILE_BYTES,
    LEGACY_SOURCE_ROOT_CATALOG_MAX_ROOTS as MAX_SOURCE_ROOTS,
};
#[cfg(test)]
pub(crate) use search_source_registry::LEGACY_SOURCE_ROOT_CATALOG_HEADER as HEADER;

pub const MAX_SOURCE_ROOT_PATH_BYTES: usize = 512;
/// Maximum watcher hints retained in memory. Hints are dirty markers only;
/// overflow becomes an explicit gap that forces a full refresh.
#[allow(dead_code)]
pub const MAX_WATCHER_HINTS: usize = 64;
/// Maximum observation gaps reported in one truth snapshot.
pub const MAX_OBSERVATION_GAPS: usize = 40;

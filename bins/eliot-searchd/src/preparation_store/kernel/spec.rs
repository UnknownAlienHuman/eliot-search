//! Daemon-only bounded-work constants for preparation orchestration.

use std::time::Duration;

pub(crate) use search_materializer::api::{
    LEGACY_PREPARATION_MAX_OBJECT_BYTES as MAX_OBJECT_BYTES,
    LEGACY_PREPARATION_REFERENCE_BYTES as REF_BYTES,
};

pub(crate) const MAX_BATCH_REVISIONS: usize = 64;
pub(crate) const MAX_BATCH_SOURCE_BYTES: u64 = 256 * 1024 * 1024;
pub(crate) const BATCH_SLICE: Duration = Duration::from_secs(10);

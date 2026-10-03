//! Data-root I/O composition for the development DIRECT corpus.
//!
//! Stable store methods remain distributed by responsibility: lifecycle and
//! catalog mutation, exact journal append/readback, safe source snapshots,
//! filesystem containment helpers, and test-only plaintext revision fixtures.

mod filesystem;
mod journal;
mod lifecycle;
#[cfg(test)]
mod revision;
mod snapshot;
mod validation;

pub(super) use filesystem::{
    collect_regular_files, ensure_directory, ensure_regular_file, is_reparse,
    path_identity_bytes,
};
pub(super) use snapshot::read_file_snapshot;

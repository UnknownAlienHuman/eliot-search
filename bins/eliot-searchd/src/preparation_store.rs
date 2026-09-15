//! Immutable DIRECT preparation objects plus content-free lookup references.
//!
//! The stable store-local surface delegates to bounded private owners for
//! exact binding/reference/object coding, immutable publication/readback,
//! migration inspection and bounded retained-revision backfill.

#[path = "control_migration_preparation.rs"]
mod migration_inventory;
#[path = "preparation_store/kernel.rs"]
mod kernel;

pub use kernel::{PreparationBatch, PreparationCursor};

pub(super) use kernel::{
    PreparationEvidence, inspect, load, persist, persist_source,
};

// Compatibility surface for the read-only migration inventory child. These
// remain store-internal and do not widen the daemon API.
pub(super) use kernel::{
    MAX_OBJECT_BYTES, REF_BYTES, binding, decode_reference,
    decode_reference_fields, extension, lookup_key, object_id,
};
pub(super) use super::storage_io::{ensure_directory, read_regular_file};
pub(super) use crate::direct_preparation::profile_digest;
pub(super) use crate::sha256;

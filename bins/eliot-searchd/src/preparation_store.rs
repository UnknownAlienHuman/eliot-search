//! Immutable DIRECT preparation objects plus content-free lookup references.
//!
//! The stable store-local surface delegates to bounded private owners for
//! exact binding/reference/object coding, immutable publication/readback,
//! migration inspection and bounded retained-revision backfill.

#[path = "preparation_store/kernel.rs"]
mod kernel;
#[path = "control_migration_preparation.rs"]
#[cfg(test)]
mod migration_inventory;

pub use kernel::{PreparationBatch, PreparationCursor};

pub(super) use kernel::load_preparation as load;
#[cfg(test)]
pub(super) use kernel::persist_source;
#[cfg(test)]
pub(super) use kernel::{
    PreparationEvidence, inspect_preparation as inspect, persist_preparation as persist,
};

// Compatibility surface for the read-only migration inventory child. These
// remain store-internal and do not widen the daemon API.
#[cfg(test)]
pub(super) use super::storage_io::{ensure_directory, read_regular_file};
#[cfg(test)]
pub(super) use crate::direct_preparation::profile_digest;
#[cfg(test)]
pub(super) use crate::sha256;
#[cfg(test)]
pub(super) use kernel::{
    MAX_OBJECT_BYTES, REF_BYTES, binding, decode_reference, decode_reference_fields, extension,
    lookup_key, object_id,
};

//! DIRECT preparation-store composition behind the stable store-local facade.

#[path = "kernel/batch.rs"]
mod batch;
#[path = "kernel/codec.rs"]
mod codec;
#[path = "kernel/inspect.rs"]
#[cfg(test)]
mod inspect;
#[path = "kernel/load.rs"]
mod load;
#[path = "kernel/paths.rs"]
mod paths;
#[path = "kernel/persist.rs"]
mod persist;
#[path = "kernel/spec.rs"]
mod spec;

pub use batch::{PreparationBatch, PreparationCursor};

#[cfg(test)]
pub(crate) use inspect::{PreparationEvidence, inspect as inspect_preparation};
pub(crate) use load::load as load_preparation;
#[cfg(test)]
pub(crate) use persist::persist as persist_preparation;
#[cfg(test)]
pub(crate) use persist::persist_source;

// Re-exported only for the existing migration-inventory child module.
#[cfg(test)]
pub(crate) use codec::{
    binding, decode_reference, decode_reference_fields, extension, lookup_key, object_id,
};
#[cfg(test)]
pub(crate) use spec::{MAX_OBJECT_BYTES, REF_BYTES};

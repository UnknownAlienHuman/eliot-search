//! DIRECT preparation-store composition behind the stable store-local facade.

mod batch;
mod codec;
mod inspect;
mod load;
mod paths;
mod persist;
mod spec;

pub use batch::{PreparationBatch, PreparationCursor};

pub(crate) use inspect::{PreparationEvidence, inspect};
pub(crate) use load::load;
pub(crate) use persist::{persist, persist_source};

// Re-exported only for the existing migration-inventory child module.
pub(crate) use codec::{
    binding, decode_reference, decode_reference_fields, extension,
    lookup_key, object_id,
};
pub(crate) use spec::{MAX_OBJECT_BYTES, REF_BYTES};

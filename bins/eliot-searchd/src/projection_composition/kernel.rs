//! Membership-scoped projection composition behind the stable daemon-local facade.

mod cas;
mod compose;
mod digest;
mod error;
mod model;
mod reference;
mod spec;

pub use cas::{
    load_projection_manifest_bytes, store_projection_manifest,
    verify_stored_projection,
};
pub use compose::{
    compose_scoped_projection, expected_payload_indexes_for_bridge,
};
pub use digest::{compute_payload_digest, compute_scope_key};
pub use error::ProjectionCompositionError;
pub use model::{
    AdmittedUnitReceipt, ComposingUnit, CompositionRequest,
    MembershipReceipt, ProjectionReference, StoredProjection,
};

#[cfg(test)]
mod tests;

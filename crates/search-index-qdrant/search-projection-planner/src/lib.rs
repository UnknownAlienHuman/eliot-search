//! Pure deterministic planning for Qdrant projection points and manifests.
//!
//! The planner consumes one admitted source-to-projection membership binding,
//! prepared unit/vector contracts and one immutable projection profile set. It
//! emits the exact S9.5 payload, S11 point identity, named-vector values and an
//! immutable exact-ID manifest. It performs no Qdrant, redb, filesystem or CAS
//! I/O and owns no source or access authority.

#![forbid(unsafe_code)]
#![deny(missing_docs)]
#![allow(
    clippy::doc_markdown,
    clippy::large_enum_variant,
    clippy::missing_errors_doc,
    clippy::module_name_repetitions,
    clippy::too_many_arguments,
    clippy::too_many_lines
)]

mod digest;
mod error;
mod manifest;
mod model;
mod plan;
mod schema;

pub use digest::{manifest_digest, payload_digest, vector_digest};
pub use error::ProjectionError;
pub use manifest::{
    canonicalize_manifest, diff_manifests, validate_manifest_entries,
    verify_manifest_integrity, verify_manifest_reconstruction,
};
pub use model::{
    CollectionSchema, ExpectedReadbackShape, ManifestDiff, MinimalPointPayload,
    NamedVector, PayloadIndexKind, PointSpec, PreparedUnit, PreparedVector,
    ProjectionBudget, ProjectionInput, ProjectionManifest,
    ProjectionManifestEntry, ProjectionMembershipBinding, ProjectionPlan,
    ProjectionProfiles, ValidatedProjectionInput, VectorKind,
    VectorRequirement, VectorValue,
};
pub use plan::{
    build_minimal_payload, build_point_spec, plan_projection,
    validate_membership_isolation, validate_projection_input,
};
pub use schema::{
    REQUIRED_PAYLOAD_INDEXES, expected_payload_indexes,
    validate_schema_requirements,
};

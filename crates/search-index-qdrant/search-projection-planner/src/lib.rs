//! Pure deterministic planning for exact Qdrant projection point sets.
//!
//! This package performs no Qdrant, filesystem, CAS, redb, source-admission,
//! or access-authority I/O. It validates one immutable projection-membership
//! scope, derives canonical S11 identities, builds exact S9.5 payloads and
//! named vectors, and emits an immutable exact-ID manifest for publication.

#![forbid(unsafe_code)]
#![deny(missing_docs)]
#![allow(
    clippy::large_enum_variant,
    clippy::missing_errors_doc,
    clippy::module_name_repetitions,
    clippy::too_many_arguments,
    clippy::too_many_lines
)]

mod canonical;
mod error;
mod manifest;
mod model;
mod plan;
mod schema;

pub use error::ProjectionError;
pub use manifest::{
    ManifestDiff, ProjectionManifest, ProjectionManifestEntry,
    ProjectionManifestScope, canonicalize_manifest, diff_manifests,
    verify_manifest_reconstruction,
};
pub use model::{
    ExpectedReadbackShape, MinimalPointPayload, NamedVectorInput, PlannedVector,
    PointSpec, ProjectionBudget, ProjectionDigestPort, ProjectionInput,
    ProjectionPlan, ProjectionProfiles, ProjectionScope, ProjectionUnitInput,
    ValidatedProjectionInput, VectorRequirement, VectorValue,
};
pub use plan::{
    build_minimal_payload, build_point_spec, plan_projection,
    validate_membership_isolation, validate_projection_input,
};
pub use schema::{
    CollectionSchema, PayloadIndexKind, PayloadIndexRequirement,
    REQUIRED_PAYLOAD_INDEXES, expected_payload_indexes,
    validate_schema_dimensions, validate_schema_requirements,
};

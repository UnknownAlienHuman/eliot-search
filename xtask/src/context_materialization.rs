//! Bounded Rust port of deterministic context-materialization planner helpers.
//!
//! The public surface is intentionally stable while implementation is split by
//! responsibility: closed schema constants, digest framing, scalar grammars,
//! output-root fencing and immutable reference validation. Full repository
//! orchestration and candidate publication are separate bounded modules.

mod digest;
mod error;
mod output;
mod references;
mod scalars;
mod spec;

pub use digest::{authority_map, operation_id, plan_digest};
pub use error::MaterializationPlanError;
pub use output::advisory_output_target;
pub use references::{
    ArtifactRef, OptionalSignature, SignatureValue, validate_artifact_ref,
    validate_optional_signature,
};
pub use scalars::{
    actor_identity_valid, opaque_id_valid, require_actor, require_opaque,
    require_rfc3339, require_sha, require_u64, rfc3339_valid,
    sha256_hex_valid,
};
pub use spec::{
    AUTHORITY_FIELDS, DECISION_COMMIT, DECISION_MISSING,
    DECISION_PARTIAL_SIGNATURE, DECISION_SIGNATURES, INSTANCE_STATUS,
    OPERATION_DOMAIN, PLAN_DOMAIN, PLAN_ROOT, REASON_MISSING_SELECTION,
    REASON_PARTIAL_SIGNATURE, RECORD_KIND, REPOSITORY, SCHEMA_VERSION, STATUS,
};

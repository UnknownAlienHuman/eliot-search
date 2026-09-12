//! Deterministic context-artifact candidate primitives.
//!
//! Public behavior remains stable while implementation is split into closed
//! schema constants, normalization, digest framing, output fencing and bundle
//! codec modules. Immutable Git-tree and filesystem effects remain separate
//! boundaries.

mod bundle;
mod digest;
mod error;
mod normalize;
mod output;
mod spec;

pub use bundle::{
    BundleBlock, expected_header, parse_bundle, render_bundle,
};
pub use digest::{
    assert_candidate_digest, authority_map, candidate_id,
    candidate_metadata_digest,
};
pub use error::ContextArtifactError;
pub use normalize::{normalize_utf8_lf, require_json_value};
pub use output::advisory_output_target;
pub use spec::{
    ADDITIONAL_FAILURE_CODES, ARTIFACT_FORMAT, ARTIFACT_ROOT, AUTHORITY_FIELDS,
    BUNDLE_END, BUNDLE_MAGIC, CANDIDATE_ID_DOMAIN,
    CANDIDATE_METADATA_DOMAIN, MAX_BUNDLE_BYTES, RECORD_KIND, SCHEMA_VERSION,
    STATUS, UNRESOLVED_MANIFEST_FIELDS,
};

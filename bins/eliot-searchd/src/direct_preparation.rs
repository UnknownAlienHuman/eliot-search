//! DIRECT preparation composition boundary.
//!
//! The daemon composes exact materialization, unitization, literal scanning and
//! retained-object publication. Profile construction, durable representation
//! binding, layout scanning and corpus-gate mechanics remain distinct owners.

mod binding;
mod layout;
mod profile;
mod spine;

pub use binding::{
    CanonicalPreparationReceipt, encode_canonical_preparation, representation_id,
    verify_canonical_representation,
};
pub use layout::{encode_preparation, preparation_gap, scan_prepared, validate_query};
pub use profile::{
    CANONICAL_MATERIALIZER_NAME, CANONICAL_MATERIALIZER_REVISION,
    CANONICAL_UNITIZER_NAME, CANONICAL_UNITIZER_REVISION, CONTENT_DIGEST_ALGORITHM,
    DIGEST_ALGORITHM_BLAKE3_256, DIGEST_ALGORITHM_SHA256, MANIFEST_DIGEST_ALGORITHM,
    MAX_LAYOUT_BYTES, REPRESENTATION_DIGEST_ALGORITHM, canonical_materializer_digest,
    canonical_materializer_profile, canonical_unitizer_digest, canonical_unitizer_profile,
    profile_digest,
};
pub use spine::{
    CANONICAL_CORPUS_BUDGET, CorpusBudget, SPINE_GAP_BUDGET_EXHAUSTED,
    SPINE_GAP_MATCH_LIMIT, SPINE_GAP_VALIDATION_FAILED, validate_source_backed_match,
    verify_spine_gate,
};

#[cfg(test)]
mod tests;

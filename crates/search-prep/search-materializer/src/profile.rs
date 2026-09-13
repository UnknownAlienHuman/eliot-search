//! Materializer profile identity, validation and change classification.
//!
//! Public profile names stay stable while profile types, deterministic identity,
//! validation/default construction, change classification and tests have separate
//! private owners.

mod change;
mod digest;
mod model;
mod validate;

pub use change::{MaterializerProfileChange, classify_profile_change};
pub use digest::{digest32, profile_digest};
pub use model::{
    BomPolicy, CoordinateSpace, DEFAULT_PROFILE_LIMITS, InvalidSequencePolicy, LossBehavior,
    MAX_PROFILE_NAME_BYTES, MaterializationProfileLimits, MaterializerProfileDescriptor,
    MaterializerProfileId, NewlinePolicy, SourceEncoding, SourceKind, UnicodeNormalization,
    ValidatedMaterializerProfile,
};
pub use validate::{baseline_profile_descriptor, validate_materializer_profile};

#[cfg(test)]
mod tests;

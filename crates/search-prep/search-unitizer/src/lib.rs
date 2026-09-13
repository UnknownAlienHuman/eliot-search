//! Deterministic exact-range unitization for materialized UTF-8 revisions.
//!
//! Pure layout and receipt-bound unitization share one implementation. Layouts
//! neither admit a source nor create a persistence or qualification receipt.

#![forbid(unsafe_code)]
#![deny(missing_docs)]
#![allow(
    clippy::missing_errors_doc,
    clippy::module_name_repetitions,
    clippy::must_use_candidate,
    clippy::too_many_lines
)]

mod error;
pub use error::UnitizationError;

mod unitization;
pub use unitization::{
    DEFAULT_UNITIZATION_LIMITS, SourceLineSpan, TextUnit, UnitIdentity,
    UnitizationInput, UnitizationLimits, UnitizationReceipt, UnitizationResult, unitize,
};

mod layout;
pub use layout::{UnitSpan, unitize_text};

mod manifest;
pub use manifest::{
    CanonicalUnitManifestBytes, MaterializerProvenance, UNIT_MANIFEST_DIGEST_ALGORITHM,
    UNIT_MANIFEST_FORMAT, UNIT_MANIFEST_VERSION, UnitDescriptor, UnitManifest, UnitManifestDiff,
    UnitManifestVerificationReceipt, UnitizerProfileChange, UnitizerProfileDescriptor,
    UnitizerProfileId, ValidatedUnitizerProfile, build_unit_manifest, canonicalize_unit_manifest,
    classify_unitizer_profile_change, decode_unit_manifest, diff_unit_manifests, manifest_digest,
    unitizer_profile_digest, validate_unitizer_profile, verify_unit_manifest,
};

#[cfg(test)]
use search_contracts::{Blake3Digest32, NonZeroRevision, OpaqueId, ReceiptRef};

#[cfg(test)]
mod tests;

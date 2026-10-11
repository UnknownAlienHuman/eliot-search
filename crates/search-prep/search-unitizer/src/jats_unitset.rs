//! Complete JATS structural observations beside unchanged exact raw v3 coverage.
mod budget;
mod build;
mod codec;
mod decode;
mod model;
mod profile;
pub use budget::{JatsUnitizationBudget, JatsUnitizerLimits, MAX_JATS_UNITIZER_LIMITS};
pub use build::{JatsUnitizationSource, unitize_jats, verify_jats_unit_manifest};
pub use codec::canonicalize_jats_unit_manifest;
pub use decode::decode_jats_unit_manifest;
pub use model::{
    CanonicalJatsUnitManifestBytes, JatsMappedValue, JatsSourceBinding, JatsStructuralUnit,
    JatsUnitAttribute, JatsUnitComment, JatsUnitManifest, JatsUnitSpan, JatsUnitText,
    JatsUnitizationError, JatsUnitizationUsage, VerifiedJatsUnitSet,
};
pub use profile::JatsUnitizerProfile;
#[cfg(test)]
mod fixtures;

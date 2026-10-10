//! Durable profile-bound unit manifests with exact provenance.
//!
//! Profile identity, public data, deterministic construction, canonical codec,
//! verification and exact identity diff remain separate private owners behind
//! this stable crate-internal facade.

mod build;
mod codec;
mod digest;
mod input;
mod model;
mod profile;
mod spec;
mod v3_profile;
mod verify;

pub use build::build_unit_manifest;
pub use codec::{canonicalize_unit_manifest, decode_unit_manifest};
pub use input::{UnitSetInput, UnitizationBudget, prepare_unit_set_input};
pub use model::{
    CanonicalUnitManifestBytes, MaterializerProvenance, UnitDescriptor, UnitManifest,
    UnitManifestDiff, UnitManifestVerificationReceipt, V3SourceBinding, VerifiedUnitSet,
};
pub use profile::{
    UnitizerProfileChange, UnitizerProfileDescriptor, UnitizerProfileId, ValidatedUnitizerProfile,
    classify_unitizer_profile_change, unitizer_profile_digest, validate_unitizer_profile,
};
pub use spec::{
    MAX_UNITIZER_PROFILE_NAME_BYTES, UNIT_MANIFEST_DIGEST_ALGORITHM, UNIT_MANIFEST_FORMAT,
    UNIT_MANIFEST_VERSION,
};
pub use v3_profile::{
    V3AnchorPolicy, V3AttachmentPolicy, V3EmptyPolicy, V3OmissionPolicy, V3OverlapPolicy,
    V3RepresentationKind, V3UnitizerProfileDescriptor, ValidatedV3UnitizerProfile,
    validate_v3_unitizer_profile,
};
pub use verify::{diff_unit_manifests, manifest_digest, verify_unit_manifest};

#[cfg(test)]
mod profile_cases;
#[cfg(test)]
mod tests;

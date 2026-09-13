//! Durable profile-bound unit manifests with exact provenance.
//!
//! Profile identity, public data, deterministic construction, binary codec,
//! verification and exact identity diff remain separate private owners behind
//! this stable crate-internal facade.

mod build;
mod codec;
mod digest;
mod model;
mod profile;
mod spec;
mod verify;

pub use build::build_unit_manifest;
pub use codec::{canonicalize_unit_manifest, decode_unit_manifest};
pub use model::{
    CanonicalUnitManifestBytes, MaterializerProvenance, UnitDescriptor, UnitManifest,
    UnitManifestDiff, UnitManifestVerificationReceipt,
};
pub use profile::{
    UnitizerProfileChange, UnitizerProfileDescriptor, UnitizerProfileId,
    ValidatedUnitizerProfile, classify_unitizer_profile_change, unitizer_profile_digest,
    validate_unitizer_profile,
};
pub use spec::{
    MAX_UNITIZER_PROFILE_NAME_BYTES, UNIT_MANIFEST_DIGEST_ALGORITHM, UNIT_MANIFEST_FORMAT,
    UNIT_MANIFEST_VERSION,
};
pub use verify::{diff_unit_manifests, manifest_digest, verify_unit_manifest};

//! Public entry module for `search-materializer`.
//!
//! All cross-package materializer behavior enters through this module. The
//! logical surface below preserves the semantics required by `FUNCTIONS.md`
//! and the package agent contract, even where concrete Rust spelling was
//! improved over the logical names:
//!
//! - `materialize(revision, profile, budget)` is realized by
//!   [`materialize_text_or_code`] over an exact retained revision opened
//!   through [`RevisionReadPort`]; the receipt-free [`materialize_utf8`]
//!   and receipt-bound [`materialize`] UTF-8 paths stay available at the
//!   crate root with byte-identical behavior.
//! - `validate_coordinate_map(map, source, output)` is realized by
//!   [`validate_coordinate_map`] over explicit space lengths plus
//!   [`validate_map_bundle`] for identity, loss and assurance binding.
//! - `derive_assurance_ceiling(loss_map)` is provided verbatim alongside
//!   [`derive_assurance`], which additionally binds warnings and profile.
//! - `qualify_provider(descriptor, fixtures)` is realized by
//!   [`qualify_provider`] and [`validate_provider_descriptor`]; both stay
//!   gated ([`MaterializationError::ProviderNotQualified`]) until P17.

pub use crate::assurance::{
    AssuranceCeiling, MaterializationAssurance, derive_assurance, derive_assurance_ceiling,
};
pub use crate::decode::{
    DecodedLine, DecodedRepresentation, EncodingDecision, StepCounter, decode_text_or_code,
    detect_or_validate_encoding,
};
pub use crate::maps::{
    COORDINATE_MAP_VERSION, CoordinateMap, CoordinateSegment, LossKind, LossMap, LossRecord,
    MapBundle, MapIdentities, MapValidationReceipt, SegmentRelation, build_coordinate_map,
    build_loss_map, validate_coordinate_map, validate_map_bundle,
};
pub use crate::normalize::{CanonicalLine, CanonicalRepresentation, normalize_representation};
pub use crate::product::{
    CanonicalMaterializationBytes, MaterializationAdmissionPlan, MaterializationContext,
    MaterializationProduct, MaterializationVerificationReceipt, MaterializationWarning,
    ResourceReceipt, RevisionBytesGuard, RevisionReadPort, StoredRevisionBytes,
    canonicalize_materialization, materialize_text_or_code, open_exact_revision, prepare_admission,
    verify_materialization,
};
pub use crate::profile::{
    BomPolicy, CoordinateSpace, DEFAULT_PROFILE_LIMITS, InvalidSequencePolicy, LossBehavior,
    MAX_PROFILE_NAME_BYTES, MaterializationProfileLimits, MaterializerProfileChange,
    MaterializerProfileDescriptor, MaterializerProfileId, NewlinePolicy, SourceEncoding,
    SourceKind, UnicodeNormalization, ValidatedMaterializerProfile, baseline_profile_descriptor,
    classify_profile_change, profile_digest, validate_materializer_profile,
};
pub use crate::provider::{
    ProviderDescriptor, ProviderFailureKind, ProviderFallbackDecision, ProviderOutputClaim,
    classify_provider_failure, qualify_provider, validate_provider_descriptor,
    verify_provider_output_claim,
};
pub use crate::request::{
    AcceptedProfiles, CancellationToken, DEFAULT_MATERIALIZATION_BUDGET, MaterializationBudget,
    MaterializationRequest, ValidatedMaterializationRequest, validate_materialization_request,
};
pub use crate::{
    DEFAULT_MATERIALIZATION_LIMITS, LineEnding, LineEndingEvidence, LineSpan, MaterializationError,
    MaterializationLimits, MaterializationReceipt, MaterializedRevision, MaterializedText,
    RetainedRevision, materialize, materialize_utf8,
};

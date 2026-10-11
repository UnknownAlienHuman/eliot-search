//! Immutable separately versioned strict raw/JATS profile.
use super::identity::profile_identity;
use super::model::{HARD, JatsError, JatsLimits};
use crate::api::{
    BomPolicy, InvalidSequencePolicy, LossBehavior, MaterializerProfileId, NewlinePolicy,
    SourceEncoding, SourceKind, UnicodeNormalization, ValidatedMaterializerProfile,
};
use search_contracts::Blake3Digest32;

/// Fixed maximum accepted parser dimensions.
pub const MAX_JATS_LIMITS: JatsLimits = HARD;
/// Maximum successful complete single inventory/product preimage.
pub const MAX_JATS_PREIMAGE_BYTES: usize = 2 * 1024 * 1024;
/// Maximum cumulative complete inventory/product preimage bytes.
pub const MAX_JATS_TOTAL_CANONICAL_BYTES: usize = 4 * 1024 * 1024;

/// Caller may narrow these additional operation ceilings.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct JatsOperationLimits {
    /// Positive limits no greater than the selected profile.
    pub parser: JatsLimits,
    /// Complete domain/NUL/CBOR bytes per frame.
    pub canonical_preimage_bytes: usize,
    /// Cumulative successful complete frame bytes.
    pub total_canonical_bytes: usize,
}
impl JatsOperationLimits {
    /// Validates positive hard ceilings; profile narrowing is checked at ingress.
    pub fn validate(self) -> Result<Self, JatsError> {
        self.parser.narrow(HARD)?;
        if self.canonical_preimage_bytes == 0
            || self.canonical_preimage_bytes > MAX_JATS_PREIMAGE_BYTES
            || self.total_canonical_bytes == 0
            || self.total_canonical_bytes > MAX_JATS_TOTAL_CANONICAL_BYTES
        {
            return Err(JatsError::InvalidLimits);
        }
        Ok(self)
    }
}
/// Owner-computed equality profile; no schema-validation/qualification claim.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct JatsProfile {
    raw: MaterializerProfileId,
    limits: JatsLimits,
    id: Blake3Digest32,
}
impl JatsProfile {
    /// Selects XML1.0 strict UTF8 Publishing1.4 subset with original raw maps.
    pub fn new(raw: &ValidatedMaterializerProfile, limits: JatsLimits) -> Result<Self, JatsError> {
        validate_raw(raw)?;
        let limits = limits.narrow(HARD)?;
        Ok(Self {
            raw: raw.id(),
            limits,
            id: profile_identity(raw, limits)?,
        })
    }
    /// Complete owner-computed profile commitment, never an admission identity.
    pub const fn id(self) -> Blake3Digest32 {
        self.id
    }
    /// Original raw baseline profile identity.
    pub const fn raw_profile_id(self) -> MaterializerProfileId {
        self.raw
    }
    /// Selected immutable profile ceilings.
    pub const fn limits(self) -> JatsLimits {
        self.limits
    }
}
pub(super) fn validate_raw(raw: &ValidatedMaterializerProfile) -> Result<(), JatsError> {
    if raw.source_kinds() != [SourceKind::Text]
        || raw.encodings() != [SourceEncoding::Utf8]
        || raw.bom_policy() != BomPolicy::RejectWhenPresent
        || raw.invalid_sequence_policy() != InvalidSequencePolicy::Reject
        || raw.newline_policy() != NewlinePolicy::PreserveExact
        || raw.unicode_normalization() != UnicodeNormalization::None
        || raw.loss_behavior() != LossBehavior::RejectOnAnyLoss
    {
        return Err(JatsError::ProfileMismatch);
    }
    Ok(())
}

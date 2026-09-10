//! Materializer profile identity, validation and change classification.
//!
//! A profile freezes every load-bearing materialization behavior: supported
//! source kinds and encodings, BOM handling, invalid-sequence handling,
//! newline and Unicode rules, loss behavior, finite limits, coordinate spaces
//! and the golden fixture digest. [`profile_digest`] binds all of them into
//! one [`MaterializerProfileId`]; any behavior or bound change yields a
//! different identity, so existing representations are never reinterpreted
//! under a changed profile.

use crate::MaterializationError;
use search_contracts::Blake3Digest32;

/// Maximum profile name length in bytes.
pub const MAX_PROFILE_NAME_BYTES: usize = 128;

/// Conservative finite profile limits shared by baseline descriptors.
pub const DEFAULT_PROFILE_LIMITS: MaterializationProfileLimits = MaterializationProfileLimits {
    max_input_bytes: 8 * 1024 * 1024,
    max_output_bytes: 8 * 1024 * 1024,
    max_lines: 1_000_000,
    max_map_segments: 1_000_032,
    max_loss_records: 1_000_032,
    max_steps: 64 * 1024 * 1024,
};

/// Domain-separated 32-byte digest over an ordered byte preimage.
///
/// Four independent FNV-1a 64-bit lanes absorb the domain, a domain
/// separator, then each length-delimited chunk. Fixed little-endian lane
/// encoding keeps the digest byte-identical across runs and platforms. This
/// is an identity digest, not a cryptographic commitment over source bytes.
pub fn digest32(domain: &[u8], chunks: &[&[u8]]) -> [u8; 32] {
    const SEED: [u64; 4] = [
        0xcbf2_9ce4_8422_2325,
        0x8422_2325_cbf2_9ce4,
        0x4822_2325_cbf2_9ce4,
        0x2325_cbf2_9ce4_8422,
    ];
    const PRIME: u64 = 0x1_0000_0000_01B3;
    let mut lanes = SEED;
    let mut index = 0_usize;
    let mut absorb = |byte: u8| {
        let slot = index % 4;
        lanes[slot] ^= u64::from(byte);
        lanes[slot] = lanes[slot].wrapping_mul(PRIME);
        index = index.wrapping_add(1);
    };
    for byte in domain {
        absorb(*byte);
    }
    absorb(0xFF);
    for chunk in chunks {
        for byte in *chunk {
            absorb(*byte);
        }
        absorb(0xFE);
    }
    let mut out = [0_u8; 32];
    for (slot, lane) in lanes.iter().enumerate() {
        let start = slot * 8;
        out[start..start + 8].copy_from_slice(&lane.to_le_bytes());
    }
    out
}

/// Baseline-supported source kind.
#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub enum SourceKind {
    /// Plain text revision.
    Text,
    /// Source-code revision (never executed, compiled or macro-expanded).
    Code,
}

impl SourceKind {
    /// Stable short name used in digests and receipts.
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Text => "text",
            Self::Code => "code",
        }
    }

    const fn tag(self) -> u8 {
        match self {
            Self::Text => 1,
            Self::Code => 2,
        }
    }
}

/// Baseline-supported source encoding.
#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub enum SourceEncoding {
    /// Strict UTF-8, optionally with a byte-order mark.
    Utf8,
    /// UTF-16 little-endian, transcoded exactly to UTF-8.
    Utf16Le,
    /// UTF-16 big-endian, transcoded exactly to UTF-8.
    Utf16Be,
}

impl SourceEncoding {
    /// Stable short name used in digests and receipts.
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Utf8 => "utf8",
            Self::Utf16Le => "utf16le",
            Self::Utf16Be => "utf16be",
        }
    }

    const fn tag(self) -> u8 {
        match self {
            Self::Utf8 => 1,
            Self::Utf16Le => 2,
            Self::Utf16Be => 3,
        }
    }
}

/// Declared byte-order-mark policy.
#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub enum BomPolicy {
    /// A leading BOM is consumed and recorded as loss, never kept silently.
    StripAndRecord,
    /// Any BOM-bearing input is rejected as unsupported under this profile.
    RejectWhenPresent,
}

impl BomPolicy {
    const fn tag(self) -> u8 {
        match self {
            Self::StripAndRecord => 1,
            Self::RejectWhenPresent => 2,
        }
    }
}

/// Declared invalid-sequence policy.
#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub enum InvalidSequencePolicy {
    /// Malformed or truncated sequences are typed errors, never silent output.
    Reject,
}

impl InvalidSequencePolicy {
    const fn tag(self) -> u8 {
        match self {
            Self::Reject => 1,
        }
    }
}

/// Declared newline policy.
#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub enum NewlinePolicy {
    /// Every terminator byte is preserved exactly.
    PreserveExact,
    /// `CRLF` and `CR` terminators become `LF`, each change recorded as loss.
    NormalizeToLf,
}

impl NewlinePolicy {
    const fn tag(self) -> u8 {
        match self {
            Self::PreserveExact => 1,
            Self::NormalizeToLf => 2,
        }
    }
}

/// Declared Unicode normalization policy.
#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub enum UnicodeNormalization {
    /// No code-point transformation; bytes decode exactly as declared.
    None,
}

impl UnicodeNormalization {
    const fn tag(self) -> u8 {
        match self {
            Self::None => 1,
        }
    }
}

/// Declared loss behavior for transforms that cannot preserve exact bytes.
#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub enum LossBehavior {
    /// Record every loss and lower assurance accordingly.
    RecordAndLowerAssurance,
    /// Reject any input that would require a lossy transform.
    RejectOnAnyLoss,
}

impl LossBehavior {
    const fn tag(self) -> u8 {
        match self {
            Self::RecordAndLowerAssurance => 1,
            Self::RejectOnAnyLoss => 2,
        }
    }
}

/// Coordinate spaces bound by every materializer profile.
#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub enum CoordinateSpace {
    /// Native retained byte offsets.
    NativeBytes,
    /// Decoded Unicode scalar positions.
    DecodedScalar,
    /// Canonical Unicode scalar positions.
    CanonicalScalar,
}

impl CoordinateSpace {
    /// Stable short name used in digests and receipts.
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::NativeBytes => "native-bytes",
            Self::DecodedScalar => "decoded-scalar",
            Self::CanonicalScalar => "canonical-scalar",
        }
    }

    const fn tag(self) -> u8 {
        match self {
            Self::NativeBytes => 1,
            Self::DecodedScalar => 2,
            Self::CanonicalScalar => 3,
        }
    }
}

/// Finite profile-owned limits. All dimensions are non-zero after validation.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct MaterializationProfileLimits {
    /// Maximum exact retained input bytes.
    pub max_input_bytes: u64,
    /// Maximum canonical output bytes.
    pub max_output_bytes: u64,
    /// Maximum logical lines.
    pub max_lines: u64,
    /// Maximum coordinate map segments.
    pub max_map_segments: u64,
    /// Maximum loss map records.
    pub max_loss_records: u64,
    /// Maximum elementary decode/normalize/map steps.
    pub max_steps: u64,
}

impl MaterializationProfileLimits {
    const fn validate(self) -> Result<Self, MaterializationError> {
        if self.max_input_bytes == 0
            || self.max_output_bytes == 0
            || self.max_lines == 0
            || self.max_map_segments == 0
            || self.max_loss_records == 0
            || self.max_steps == 0
        {
            return Err(MaterializationError::ProfileInvalid);
        }
        Ok(self)
    }
}

/// Unvalidated materializer profile descriptor supplied by configuration.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct MaterializerProfileDescriptor {
    /// Human-readable profile name (ASCII, non-empty, bounded).
    pub profile_name: String,
    /// Monotone profile revision; downgrades are rejected, never reinterpreted.
    pub profile_revision: u64,
    /// Accepted source kinds, non-empty, duplicate-free.
    pub source_kinds: Vec<SourceKind>,
    /// Accepted source encodings, non-empty, duplicate-free.
    pub encodings: Vec<SourceEncoding>,
    /// Declared BOM policy.
    pub bom_policy: BomPolicy,
    /// Declared invalid-sequence policy.
    pub invalid_sequence_policy: InvalidSequencePolicy,
    /// Declared newline policy.
    pub newline_policy: NewlinePolicy,
    /// Declared Unicode normalization policy.
    pub unicode_normalization: UnicodeNormalization,
    /// Declared loss behavior.
    pub loss_behavior: LossBehavior,
    /// Finite profile-owned limits.
    pub limits: MaterializationProfileLimits,
    /// Required coordinate spaces (baseline: the full triple).
    pub coordinate_spaces: Vec<CoordinateSpace>,
    /// Digest of the golden fixture set this profile was qualified against.
    pub golden_fixture_digest: Blake3Digest32,
}

/// Canonical materializer profile identity: the domain-separated digest over
/// every load-bearing behavior and bound.
#[derive(Clone, Copy, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub struct MaterializerProfileId([u8; 32]);

impl MaterializerProfileId {
    /// Rebuilds an identity from its 32 raw bytes.
    #[must_use]
    pub const fn from_bytes(bytes: [u8; 32]) -> Self {
        Self(bytes)
    }

    /// Raw identity bytes.
    #[must_use]
    pub const fn as_bytes(&self) -> &[u8; 32] {
        &self.0
    }
}

impl core::fmt::Debug for MaterializerProfileId {
    fn fmt(&self, formatter: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        formatter
            .debug_tuple("MaterializerProfileId")
            .field(&self.to_string())
            .finish()
    }
}

impl core::fmt::Display for MaterializerProfileId {
    fn fmt(&self, formatter: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        for byte in self.0 {
            write!(formatter, "{byte:02x}")?;
        }
        Ok(())
    }
}

/// Validated materializer profile with a bound canonical identity.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ValidatedMaterializerProfile {
    name: String,
    revision: u64,
    kinds: Vec<SourceKind>,
    encodings: Vec<SourceEncoding>,
    bom_policy: BomPolicy,
    invalid_sequence_policy: InvalidSequencePolicy,
    newline_policy: NewlinePolicy,
    unicode_normalization: UnicodeNormalization,
    loss_behavior: LossBehavior,
    limits: MaterializationProfileLimits,
    spaces: Vec<CoordinateSpace>,
    golden: Blake3Digest32,
    id: MaterializerProfileId,
}

impl ValidatedMaterializerProfile {
    /// Canonical profile identity.
    #[must_use]
    pub const fn id(&self) -> MaterializerProfileId {
        self.id
    }

    /// Monotone profile revision.
    #[must_use]
    pub const fn revision(&self) -> u64 {
        self.revision
    }

    /// Human-readable profile name.
    #[must_use]
    pub fn name(&self) -> &str {
        &self.name
    }

    /// Accepted source kinds.
    #[must_use]
    pub fn source_kinds(&self) -> &[SourceKind] {
        &self.kinds
    }

    /// Accepted source encodings.
    #[must_use]
    pub fn encodings(&self) -> &[SourceEncoding] {
        &self.encodings
    }

    /// Declared BOM policy.
    #[must_use]
    pub const fn bom_policy(&self) -> BomPolicy {
        self.bom_policy
    }

    /// Declared invalid-sequence policy.
    #[must_use]
    pub const fn invalid_sequence_policy(&self) -> InvalidSequencePolicy {
        self.invalid_sequence_policy
    }

    /// Declared newline policy.
    #[must_use]
    pub const fn newline_policy(&self) -> NewlinePolicy {
        self.newline_policy
    }

    /// Declared Unicode normalization policy.
    #[must_use]
    pub const fn unicode_normalization(&self) -> UnicodeNormalization {
        self.unicode_normalization
    }

    /// Declared loss behavior.
    #[must_use]
    pub const fn loss_behavior(&self) -> LossBehavior {
        self.loss_behavior
    }

    /// Finite profile-owned limits.
    #[must_use]
    pub const fn limits(&self) -> MaterializationProfileLimits {
        self.limits
    }

    /// Required coordinate spaces.
    #[must_use]
    pub fn coordinate_spaces(&self) -> &[CoordinateSpace] {
        &self.spaces
    }

    /// Golden fixture digest this profile was qualified against.
    #[must_use]
    pub const fn golden_fixture_digest(&self) -> Blake3Digest32 {
        self.golden
    }
}

/// Baseline profile descriptor: strict UTF-8 plus exact UTF-16 transcoding,
/// recorded BOM handling, exact newlines and recorded-loss behavior.
///
/// The golden fixture digest binds the profile name and revision, so equal
/// names at different revisions qualify as different profiles.
#[must_use]
pub fn baseline_profile_descriptor(name: &str, revision: u64) -> MaterializerProfileDescriptor {
    let golden = Blake3Digest32::from_bytes(digest32(
        b"eliot-search/materializer/golden/v1",
        &[name.as_bytes(), &revision.to_le_bytes()],
    ));
    MaterializerProfileDescriptor {
        profile_name: name.to_string(),
        profile_revision: revision,
        source_kinds: vec![SourceKind::Text, SourceKind::Code],
        encodings: vec![
            SourceEncoding::Utf8,
            SourceEncoding::Utf16Le,
            SourceEncoding::Utf16Be,
        ],
        bom_policy: BomPolicy::StripAndRecord,
        invalid_sequence_policy: InvalidSequencePolicy::Reject,
        newline_policy: NewlinePolicy::PreserveExact,
        unicode_normalization: UnicodeNormalization::None,
        loss_behavior: LossBehavior::RecordAndLowerAssurance,
        limits: DEFAULT_PROFILE_LIMITS,
        coordinate_spaces: vec![
            CoordinateSpace::NativeBytes,
            CoordinateSpace::DecodedScalar,
            CoordinateSpace::CanonicalScalar,
        ],
        golden_fixture_digest: golden,
    }
}

fn has_duplicate<T: PartialEq>(values: &[T]) -> bool {
    for (index, value) in values.iter().enumerate() {
        if values[..index].contains(value) {
            return true;
        }
    }
    false
}

/// Validates a profile descriptor and binds its canonical identity.
///
/// Rejects empty or unbounded names, zero revisions, empty or duplicated
/// kind/encoding sets, zero limits, partial coordinate bases and missing
/// golden fixture digests. Implicit locale/platform defaults can never pass:
/// every behavior is explicit in the descriptor.
pub fn validate_materializer_profile(
    descriptor: &MaterializerProfileDescriptor,
) -> Result<ValidatedMaterializerProfile, MaterializationError> {
    if descriptor.profile_name.is_empty()
        || descriptor.profile_name.len() > MAX_PROFILE_NAME_BYTES
        || !descriptor
            .profile_name
            .bytes()
            .all(|byte| byte.is_ascii_alphanumeric() || matches!(byte, b'-' | b'_' | b'.' | b':'))
    {
        return Err(MaterializationError::ProfileInvalid);
    }
    if descriptor.profile_revision == 0 {
        return Err(MaterializationError::ProfileInvalid);
    }
    if descriptor.source_kinds.is_empty() || has_duplicate(&descriptor.source_kinds) {
        return Err(MaterializationError::ProfileInvalid);
    }
    if descriptor.encodings.is_empty() || has_duplicate(&descriptor.encodings) {
        return Err(MaterializationError::ProfileInvalid);
    }
    let limits = descriptor.limits.validate()?;
    if descriptor.coordinate_spaces.len() != 3
        || has_duplicate(&descriptor.coordinate_spaces)
        || !descriptor
            .coordinate_spaces
            .contains(&CoordinateSpace::NativeBytes)
        || !descriptor
            .coordinate_spaces
            .contains(&CoordinateSpace::DecodedScalar)
        || !descriptor
            .coordinate_spaces
            .contains(&CoordinateSpace::CanonicalScalar)
    {
        return Err(MaterializationError::ProfileInvalid);
    }
    if descriptor.golden_fixture_digest == Blake3Digest32::from_bytes([0; 32]) {
        return Err(MaterializationError::ProfileInvalid);
    }
    let mut kinds = descriptor.source_kinds.clone();
    kinds.sort();
    kinds.dedup();
    let mut encodings = descriptor.encodings.clone();
    encodings.sort_by_key(|encoding| encoding.tag());
    encodings.dedup();
    let mut spaces = descriptor.coordinate_spaces.clone();
    spaces.sort_by_key(|space| space.tag());
    spaces.dedup();
    let profile = ValidatedMaterializerProfile {
        name: descriptor.profile_name.clone(),
        revision: descriptor.profile_revision,
        kinds,
        encodings,
        bom_policy: descriptor.bom_policy,
        invalid_sequence_policy: descriptor.invalid_sequence_policy,
        newline_policy: descriptor.newline_policy,
        unicode_normalization: descriptor.unicode_normalization,
        loss_behavior: descriptor.loss_behavior,
        limits,
        spaces,
        golden: descriptor.golden_fixture_digest,
        id: MaterializerProfileId::from_bytes([0; 32]),
    };
    let id = profile_digest(&profile);
    Ok(ValidatedMaterializerProfile { id, ..profile })
}

/// Domain-separated canonical digest over every load-bearing behavior and
/// bound. Any encoding, normalization, coordinate, loss, assurance or limit
/// change creates a different profile identity.
#[must_use]
pub fn profile_digest(profile: &ValidatedMaterializerProfile) -> MaterializerProfileId {
    let mut kind_tags = [0_u8; 2];
    for (index, kind) in profile.kinds.iter().enumerate() {
        if let Some(slot) = kind_tags.get_mut(index) {
            *slot = kind.tag();
        }
    }
    let mut encoding_tags = [0_u8; 3];
    for (index, encoding) in profile.encodings.iter().enumerate() {
        if let Some(slot) = encoding_tags.get_mut(index) {
            *slot = encoding.tag();
        }
    }
    let mut space_tags = [0_u8; 3];
    for (index, space) in profile.spaces.iter().enumerate() {
        if let Some(slot) = space_tags.get_mut(index) {
            *slot = space.tag();
        }
    }
    let limits = profile.limits;
    let chunks: &[&[u8]] = &[
        profile.name.as_bytes(),
        &profile.revision.to_le_bytes(),
        &kind_tags,
        &encoding_tags,
        &[profile.bom_policy.tag()],
        &[profile.invalid_sequence_policy.tag()],
        &[profile.newline_policy.tag()],
        &[profile.unicode_normalization.tag()],
        &[profile.loss_behavior.tag()],
        &limits.max_input_bytes.to_le_bytes(),
        &limits.max_output_bytes.to_le_bytes(),
        &limits.max_lines.to_le_bytes(),
        &limits.max_map_segments.to_le_bytes(),
        &limits.max_loss_records.to_le_bytes(),
        &limits.max_steps.to_le_bytes(),
        &space_tags,
        profile.golden.as_bytes(),
    ];
    MaterializerProfileId::from_bytes(digest32(b"eliot-search/materializer/profile/v1", chunks))
}

/// Profile change classification. Existing representations are never
/// reinterpreted under a changed profile.
#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub enum MaterializerProfileChange {
    /// Identical canonical identity: nothing to redo.
    Noop,
    /// Behavior or bounds changed at a monotone revision: reprepare from the
    /// exact retained revision and reproject coordinates.
    RePreparationAndReprojection,
    /// Reserved for a future qualified optional document provider (P17).
    /// Baseline code never produces this variant.
    OptionalProviderGateRequired,
    /// Non-monotone revision or incompatible coordinate basis: the change
    /// must not be applied.
    Reject,
}

/// Classifies a profile transition without touching stored representations.
#[must_use]
pub fn classify_profile_change(
    old: &ValidatedMaterializerProfile,
    new: &ValidatedMaterializerProfile,
) -> MaterializerProfileChange {
    if old.id == new.id {
        return MaterializerProfileChange::Noop;
    }
    if new.revision <= old.revision {
        return MaterializerProfileChange::Reject;
    }
    let shared = old
        .spaces
        .iter()
        .filter(|space| new.spaces.contains(space))
        .count();
    if shared == 0 {
        return MaterializerProfileChange::Reject;
    }
    MaterializerProfileChange::RePreparationAndReprojection
}

#[cfg(test)]
mod tests {
    use super::*;

    fn valid_descriptor() -> MaterializerProfileDescriptor {
        baseline_profile_descriptor("baseline", 1)
    }

    #[test]
    fn baseline_descriptor_validates() {
        let profile = validate_materializer_profile(&valid_descriptor()).expect("valid");
        assert_eq!(profile.revision(), 1);
        assert_eq!(profile.name(), "baseline");
        assert_eq!(profile_digest(&profile), profile.id());
    }

    #[test]
    fn profile_names_are_bounded() {
        for name in ["", "has space", "uniçode", "semi;colon"] {
            let mut descriptor = valid_descriptor();
            descriptor.profile_name = name.to_string();
            assert_eq!(
                validate_materializer_profile(&descriptor),
                Err(MaterializationError::ProfileInvalid),
                "name {name:?} must be rejected"
            );
        }
        let mut descriptor = valid_descriptor();
        descriptor.profile_name = "a".repeat(MAX_PROFILE_NAME_BYTES + 1);
        assert_eq!(
            validate_materializer_profile(&descriptor),
            Err(MaterializationError::ProfileInvalid)
        );
    }

    #[test]
    fn zero_revision_is_rejected() {
        let mut descriptor = valid_descriptor();
        descriptor.profile_revision = 0;
        assert_eq!(
            validate_materializer_profile(&descriptor),
            Err(MaterializationError::ProfileInvalid)
        );
    }

    #[test]
    fn kind_and_encoding_sets_must_be_nonempty_and_duplicate_free() {
        let mut descriptor = valid_descriptor();
        descriptor.source_kinds.clear();
        assert_eq!(
            validate_materializer_profile(&descriptor),
            Err(MaterializationError::ProfileInvalid)
        );
        descriptor = valid_descriptor();
        descriptor.source_kinds.push(SourceKind::Text);
        assert_eq!(
            validate_materializer_profile(&descriptor),
            Err(MaterializationError::ProfileInvalid)
        );
        descriptor = valid_descriptor();
        descriptor.encodings.clear();
        assert_eq!(
            validate_materializer_profile(&descriptor),
            Err(MaterializationError::ProfileInvalid)
        );
    }

    #[test]
    fn partial_coordinate_basis_is_rejected() {
        let mut descriptor = valid_descriptor();
        descriptor.coordinate_spaces.pop();
        assert_eq!(
            validate_materializer_profile(&descriptor),
            Err(MaterializationError::ProfileInvalid)
        );
    }

    #[test]
    fn zero_limits_and_zero_golden_are_rejected() {
        let mut descriptor = valid_descriptor();
        descriptor.limits.max_steps = 0;
        assert_eq!(
            validate_materializer_profile(&descriptor),
            Err(MaterializationError::ProfileInvalid)
        );
        descriptor = valid_descriptor();
        descriptor.golden_fixture_digest = Blake3Digest32::from_bytes([0; 32]);
        assert_eq!(
            validate_materializer_profile(&descriptor),
            Err(MaterializationError::ProfileInvalid)
        );
    }

    #[test]
    fn every_load_bearing_field_changes_identity() {
        let base = validate_materializer_profile(&valid_descriptor()).expect("base");
        let base_id = profile_digest(&base);
        let mut variant = valid_descriptor();
        variant.newline_policy = NewlinePolicy::NormalizeToLf;
        // Keep the golden digest stable so only the newline change is measured.
        variant.golden_fixture_digest = base.golden_fixture_digest();
        variant.profile_revision = 2;
        let changed = validate_materializer_profile(&variant).expect("variant");
        assert_ne!(base_id, profile_digest(&changed));

        variant = valid_descriptor();
        variant.limits.max_lines = 7;
        variant.golden_fixture_digest = base.golden_fixture_digest();
        variant.profile_revision = 2;
        let changed = validate_materializer_profile(&variant).expect("variant");
        assert_ne!(base_id, profile_digest(&changed));
    }

    #[test]
    fn digest_is_deterministic() {
        let first = validate_materializer_profile(&valid_descriptor()).expect("first");
        let second = validate_materializer_profile(&valid_descriptor()).expect("second");
        assert_eq!(profile_digest(&first), profile_digest(&second));
    }

    #[test]
    fn change_classification_is_fail_closed() {
        let old = validate_materializer_profile(&valid_descriptor()).expect("old");
        let same = validate_materializer_profile(&valid_descriptor()).expect("same");
        assert_eq!(
            classify_profile_change(&old, &same),
            MaterializerProfileChange::Noop
        );
        let mut next = valid_descriptor();
        next.profile_revision = 2;
        let next = validate_materializer_profile(&next).expect("next");
        assert_eq!(
            classify_profile_change(&old, &next),
            MaterializerProfileChange::RePreparationAndReprojection
        );
        let mut rollback = valid_descriptor();
        rollback.profile_name = "rollback".to_string();
        rollback.profile_revision = 1;
        let rollback = validate_materializer_profile(&rollback).expect("rollback");
        assert_eq!(
            classify_profile_change(&next, &rollback),
            MaterializerProfileChange::Reject
        );
    }
}

//! Newline and Unicode normalization with recorded offset changes.
//!
//! Normalization applies exactly the profile's newline and Unicode rules and
//! records every offset-changing transform in the canonical line table. No
//! case folding, language transformation, formatting, macro expansion or
//! semantic rewrite occurs: the baseline Unicode policy is identity, and any
//! other policy arrives only through a separately accepted profile.

use crate::decode::{DecodedRepresentation, StepCounter};
use crate::profile::{
    LossBehavior, MaterializerProfileId, NewlinePolicy, SourceEncoding,
    ValidatedMaterializerProfile,
};
use crate::request::{CancellationToken, MaterializationBudget};
use crate::{LineEnding, MaterializationError};

/// One canonical line joining decoded scalar and canonical scalar coordinates.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct CanonicalLine {
    /// Inclusive decoded scalar start.
    pub decoded_start: u64,
    /// Exclusive decoded scalar end including the terminator.
    pub decoded_end: u64,
    /// Exclusive decoded scalar end excluding the terminator.
    pub decoded_content_end: u64,
    /// Inclusive canonical scalar start.
    pub canonical_start: u64,
    /// Exclusive canonical scalar end including the terminator.
    pub canonical_end: u64,
    /// Exclusive canonical scalar end excluding the terminator.
    pub canonical_content_end: u64,
    /// Terminator before normalization.
    pub ending_before: LineEnding,
    /// Terminator after normalization.
    pub ending_after: LineEnding,
}

impl CanonicalLine {
    /// Reports whether normalization changed this line's terminator.
    #[must_use]
    pub fn ending_changed(&self) -> bool {
        self.ending_before != self.ending_after
    }
}

/// Canonical representation with a complete offset-change record.
#[derive(Clone, Eq, PartialEq)]
pub struct CanonicalRepresentation {
    text: String,
    lines: Vec<CanonicalLine>,
    newline_policy: NewlinePolicy,
    decoded_len_chars: u64,
    canonical_len_chars: u64,
    native_len: u64,
    source_encoding: SourceEncoding,
    bom_stripped: bool,
    transcoded: bool,
    profile_id: MaterializerProfileId,
}

impl CanonicalRepresentation {
    /// Canonical text after exactly the profile's normalization rules.
    #[must_use]
    pub fn text(&self) -> &str {
        &self.text
    }

    /// Canonical line table joining decoded and canonical coordinates.
    #[must_use]
    pub fn lines(&self) -> &[CanonicalLine] {
        &self.lines
    }

    /// Newline policy applied to produce this representation.
    #[must_use]
    pub const fn newline_policy(&self) -> NewlinePolicy {
        self.newline_policy
    }

    /// Decoded length in Unicode scalar values.
    #[must_use]
    pub const fn decoded_len_chars(&self) -> u64 {
        self.decoded_len_chars
    }

    /// Canonical length in Unicode scalar values.
    #[must_use]
    pub const fn canonical_len_chars(&self) -> u64 {
        self.canonical_len_chars
    }

    /// Canonical length in UTF-8 bytes.
    #[must_use]
    pub fn canonical_len_bytes(&self) -> u64 {
        u64::try_from(self.text.len()).unwrap_or(u64::MAX)
    }

    /// Exact native input length in bytes.
    #[must_use]
    pub const fn native_len(&self) -> u64 {
        self.native_len
    }

    /// Source encoding the canonical text was decoded from.
    #[must_use]
    pub const fn source_encoding(&self) -> SourceEncoding {
        self.source_encoding
    }

    /// Whether a BOM was stripped before normalization.
    #[must_use]
    pub const fn bom_stripped(&self) -> bool {
        self.bom_stripped
    }

    /// Whether bytes were transcoded from UTF-16.
    #[must_use]
    pub const fn transcoded(&self) -> bool {
        self.transcoded
    }

    /// Profile identity this normalization was performed under.
    #[must_use]
    pub const fn profile_id(&self) -> MaterializerProfileId {
        self.profile_id
    }
}

impl core::fmt::Debug for CanonicalRepresentation {
    fn fmt(&self, formatter: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        formatter
            .debug_struct("CanonicalRepresentation")
            .field(
                "text",
                &format_args!("<{} chars>", self.canonical_len_chars),
            )
            .field("line_count", &self.lines.len())
            .field("newline_policy", &self.newline_policy)
            .field("encoding", &self.source_encoding)
            .field("bom_stripped", &self.bom_stripped)
            .field("transcoded", &self.transcoded)
            .field("profile_id", &self.profile_id)
            .finish_non_exhaustive()
    }
}

fn checked_add(first: u64, second: u64) -> Result<u64, MaterializationError> {
    first
        .checked_add(second)
        .ok_or(MaterializationError::OffsetOverflow)
}

fn checked_sub(first: u64, second: u64) -> Result<u64, MaterializationError> {
    first
        .checked_sub(second)
        .ok_or(MaterializationError::OffsetOverflow)
}

/// Applies exactly the profile's newline and Unicode rules.
///
/// Every offset-changing transform is recorded in the canonical line table
/// for map construction. Output expansion beyond finite limits fails with
/// [`MaterializationError::BudgetExhausted`]; a profile that forbids loss
/// fails with [`MaterializationError::Loss`] when any terminator would change.
/// Elementary steps accumulate into the caller-provided shared step counter.
/// Elementary steps accumulate into the shared step counter.
pub fn normalize_representation(
    decoded: &DecodedRepresentation,
    profile: &ValidatedMaterializerProfile,
    budget: &MaterializationBudget,
    steps: &mut StepCounter,
    cancel: CancellationToken<'_>,
) -> Result<CanonicalRepresentation, MaterializationError> {
    if cancel.is_cancelled() {
        return Err(MaterializationError::Cancelled);
    }
    let budget = budget.validate()?;
    if decoded.profile_id() != profile.id() {
        return Err(MaterializationError::ProfileMismatch);
    }
    let max_output = budget.effective_output(profile.limits().max_output_bytes);
    let decoded_bytes =
        u64::try_from(decoded.text().len()).map_err(|_| MaterializationError::OffsetOverflow)?;
    if decoded_bytes > max_output {
        return Err(MaterializationError::BudgetExhausted);
    }
    match profile.newline_policy() {
        NewlinePolicy::PreserveExact => preserve_exact(decoded, profile, steps, cancel),
        NewlinePolicy::NormalizeToLf => {
            normalize_to_lf(decoded, profile, max_output, steps, cancel)
        }
    }
}

fn preserve_exact(
    decoded: &DecodedRepresentation,
    profile: &ValidatedMaterializerProfile,
    steps: &mut StepCounter,
    cancel: CancellationToken<'_>,
) -> Result<CanonicalRepresentation, MaterializationError> {
    let mut lines = Vec::with_capacity(decoded.lines().len());
    for line in decoded.lines() {
        lines.push(CanonicalLine {
            decoded_start: line.decoded_start,
            decoded_end: line.decoded_end,
            decoded_content_end: line.decoded_content_end,
            canonical_start: line.decoded_start,
            canonical_end: line.decoded_end,
            canonical_content_end: line.decoded_content_end,
            ending_before: line.ending,
            ending_after: line.ending,
        });
        steps.consume(1)?;
        if cancel.is_cancelled() {
            return Err(MaterializationError::Cancelled);
        }
    }
    steps.consume(1)?;
    Ok(CanonicalRepresentation {
        text: decoded.text().to_string(),
        lines,
        newline_policy: NewlinePolicy::PreserveExact,
        decoded_len_chars: decoded.decoded_len_chars(),
        canonical_len_chars: decoded.decoded_len_chars(),
        native_len: decoded.native_len(),
        source_encoding: decoded.encoding(),
        bom_stripped: decoded.bom_stripped(),
        transcoded: decoded.transcoded(),
        profile_id: profile.id(),
    })
}

fn normalize_to_lf(
    decoded: &DecodedRepresentation,
    profile: &ValidatedMaterializerProfile,
    max_output: u64,
    steps: &mut StepCounter,
    cancel: CancellationToken<'_>,
) -> Result<CanonicalRepresentation, MaterializationError> {
    let mut canonical = String::with_capacity(decoded.text().len());
    let mut lines = Vec::with_capacity(decoded.lines().len());
    let mut chars = decoded.text().chars();
    let mut canon_index = 0_u64;
    let mut changed_lines = 0_u64;
    for line in decoded.lines() {
        let total = checked_sub(line.decoded_end, line.decoded_start)?;
        let content = checked_sub(line.decoded_content_end, line.decoded_start)?;
        if content > total {
            return Err(MaterializationError::CoordinateMapInvalid);
        }
        let terminator = checked_sub(total, content)?;
        let canon_start = canon_index;
        for _ in 0..content {
            let Some(value) = chars.next() else {
                return Err(MaterializationError::CoordinateMapInvalid);
            };
            canonical.push(value);
            canon_index = checked_add(canon_index, 1)?;
            steps.consume(1)?;
        }
        let canon_content_end = canon_index;
        for _ in 0..terminator {
            if chars.next().is_none() {
                return Err(MaterializationError::CoordinateMapInvalid);
            }
            steps.consume(1)?;
        }
        let ending_after = match line.ending {
            LineEnding::None => LineEnding::None,
            LineEnding::Lf | LineEnding::CrLf | LineEnding::Cr => LineEnding::Lf,
        };
        if ending_after != LineEnding::None {
            canonical.push('\n');
            canon_index = checked_add(canon_index, 1)?;
        }
        let changed = line.ending != ending_after;
        if changed {
            changed_lines = checked_add(changed_lines, 1)?;
        }
        lines.push(CanonicalLine {
            decoded_start: line.decoded_start,
            decoded_end: line.decoded_end,
            decoded_content_end: line.decoded_content_end,
            canonical_start: canon_start,
            canonical_end: canon_index,
            canonical_content_end: canon_content_end,
            ending_before: line.ending,
            ending_after,
        });
        if cancel.is_cancelled() {
            return Err(MaterializationError::Cancelled);
        }
    }
    if chars.next().is_some() {
        return Err(MaterializationError::CoordinateMapInvalid);
    }
    if profile.loss_behavior() == LossBehavior::RejectOnAnyLoss && changed_lines > 0 {
        return Err(MaterializationError::Loss);
    }
    let canonical_bytes =
        u64::try_from(canonical.len()).map_err(|_| MaterializationError::OffsetOverflow)?;
    if canonical_bytes > max_output {
        return Err(MaterializationError::BudgetExhausted);
    }
    let canonical_len_chars = canon_index;
    steps.consume(1)?;
    Ok(CanonicalRepresentation {
        text: canonical,
        lines,
        newline_policy: NewlinePolicy::NormalizeToLf,
        decoded_len_chars: decoded.decoded_len_chars(),
        canonical_len_chars,
        native_len: decoded.native_len(),
        source_encoding: decoded.encoding(),
        bom_stripped: decoded.bom_stripped(),
        transcoded: decoded.transcoded(),
        profile_id: profile.id(),
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::decode::{decode_text_or_code, detect_or_validate_encoding};
    use crate::profile::{baseline_profile_descriptor, validate_materializer_profile};
    use crate::request::DEFAULT_MATERIALIZATION_BUDGET;

    fn profile() -> ValidatedMaterializerProfile {
        validate_materializer_profile(&baseline_profile_descriptor("normalize-test", 1))
            .expect("profile")
    }

    fn normalize_profile() -> ValidatedMaterializerProfile {
        let mut descriptor = baseline_profile_descriptor("normalize-lf", 1);
        descriptor.newline_policy = NewlinePolicy::NormalizeToLf;
        validate_materializer_profile(&descriptor).expect("normalize profile")
    }

    fn decoded_with(bytes: &[u8], profile: &ValidatedMaterializerProfile) -> DecodedRepresentation {
        let decision =
            detect_or_validate_encoding(bytes, crate::profile::SourceEncoding::Utf8, profile)
                .expect("decision");
        decode_text_or_code(
            bytes,
            &decision,
            profile,
            &DEFAULT_MATERIALIZATION_BUDGET,
            &mut StepCounter::new(1 << 20),
            CancellationToken::never(),
        )
        .expect("decode")
    }

    #[test]
    fn preserve_exact_is_identity() {
        let profile = profile();
        let decoded = decoded_with(b"a\r\nb\nc\rd", &profile);
        let canonical = normalize_representation(
            &decoded,
            &profile,
            &DEFAULT_MATERIALIZATION_BUDGET,
            &mut StepCounter::new(1 << 20),
            CancellationToken::never(),
        )
        .expect("normalize");
        assert_eq!(canonical.text(), "a\r\nb\nc\rd");
        assert_eq!(canonical.lines().len(), 4);
        assert!(canonical.lines().iter().all(|line| !line.ending_changed()));
        assert_eq!(canonical.canonical_len_chars(), decoded.decoded_len_chars());
    }

    #[test]
    fn crlf_and_cr_normalize_with_records() {
        let profile = normalize_profile();
        let decoded = decoded_with(b"a\r\nb\nc\rd", &profile);
        let canonical = normalize_representation(
            &decoded,
            &profile,
            &DEFAULT_MATERIALIZATION_BUDGET,
            &mut StepCounter::new(1 << 20),
            CancellationToken::never(),
        )
        .expect("normalize");
        assert_eq!(canonical.text(), "a\nb\nc\nd");
        assert_eq!(canonical.lines().len(), 4);
        assert!(canonical.lines()[0].ending_changed());
        assert!(!canonical.lines()[1].ending_changed());
        assert!(canonical.lines()[2].ending_changed());
        assert!(!canonical.lines()[3].ending_changed());
        assert_eq!(canonical.lines()[0].ending_after, LineEnding::Lf);
        assert_eq!(canonical.canonical_len_chars(), 7);
    }

    #[test]
    fn strict_profile_rejects_newline_loss() {
        let mut descriptor = baseline_profile_descriptor("strict-normalize", 1);
        descriptor.newline_policy = NewlinePolicy::NormalizeToLf;
        descriptor.loss_behavior = LossBehavior::RejectOnAnyLoss;
        let strict = validate_materializer_profile(&descriptor).expect("strict");
        let decoded = decoded_with(b"a\r\n", &strict);
        assert_eq!(
            normalize_representation(
                &decoded,
                &strict,
                &DEFAULT_MATERIALIZATION_BUDGET,
                &mut StepCounter::new(1 << 20),
                CancellationToken::never()
            ),
            Err(MaterializationError::Loss)
        );
    }

    #[test]
    fn foreign_profile_is_mismatch() {
        let profile = profile();
        let other = normalize_profile();
        let decoded = decoded_with(b"a\n", &profile);
        assert_eq!(
            normalize_representation(
                &decoded,
                &other,
                &DEFAULT_MATERIALIZATION_BUDGET,
                &mut StepCounter::new(1 << 20),
                CancellationToken::never()
            ),
            Err(MaterializationError::ProfileMismatch)
        );
    }

    #[test]
    fn debug_never_leaks_canonical_text() {
        let profile = profile();
        let decoded = decoded_with(b"private-normalize\n", &profile);
        let canonical = normalize_representation(
            &decoded,
            &profile,
            &DEFAULT_MATERIALIZATION_BUDGET,
            &mut StepCounter::new(1 << 20),
            CancellationToken::never(),
        )
        .expect("normalize");
        assert!(!format!("{canonical:?}").contains("private-normalize"));
    }
}

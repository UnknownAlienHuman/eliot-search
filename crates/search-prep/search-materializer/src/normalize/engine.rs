//! Profile-bound newline normalization execution.

use super::model::{CanonicalLine, CanonicalRepresentation};
use crate::decode::{DecodedRepresentation, StepCounter};
use crate::profile::{
    LossBehavior, NewlinePolicy, ValidatedMaterializerProfile,
};
use crate::request::{CancellationToken, MaterializationBudget};
use crate::{LineEnding, MaterializationError};

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

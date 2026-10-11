//! Closed additive Markdown profile and cumulative operation budget.
//! Mirrors the manager's raw-preserving profile API; no legacy types are modified.
use super::identity::{IdentityBudget, digest_profile};
use super::model::HARD_LIMITS;
pub use super::model::{MarkdownError, MarkdownLimits};
pub(super) use super::parse::checkpoint;
use crate::api::{
    BomPolicy, CancellationToken, InvalidSequencePolicy, LossBehavior, MaterializerProfileId,
    NewlinePolicy, SourceEncoding, SourceKind, UnicodeNormalization, ValidatedMaterializerProfile,
};
use search_contracts::Blake3Digest32;
use std::time::Instant;

/// Closed maximum parser/profile ceilings, including all nine dimensions.
pub const MAX_MARKDOWN_LIMITS: MarkdownLimits = HARD_LIMITS;
/// Lower complete-preimage ceiling for each inventory frame.
pub const MAX_MARKDOWN_PREIMAGE_BYTES: usize = 2 * 1024 * 1024;
/// Lower aggregate complete nodes+inline+product preimage ceiling.
pub const MAX_MARKDOWN_TOTAL_CANONICAL_BYTES: usize = 4 * 1024 * 1024;

/// Additional positive lower whole-operation ceilings; work/storage remain in parser.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct MarkdownOperationLimits {
    /// Lower parser/work/storage ceilings, at most the selected profile's values.
    pub parser: MarkdownLimits,
    /// Maximum complete single nodes/inline/product canonical preimage.
    pub canonical_preimage_bytes: usize,
    /// Maximum cumulative complete bytes of all three canonical frames.
    pub total_canonical_bytes: usize,
}
impl MarkdownOperationLimits {
    /// Refuses zero or widening before any source/donor/canonical operation.
    pub fn validate(self) -> Result<Self, MarkdownError> {
        self.parser.validate()?;
        if self.canonical_preimage_bytes == 0
            || self.canonical_preimage_bytes > MAX_MARKDOWN_PREIMAGE_BYTES
            || self.total_canonical_bytes == 0
            || self.total_canonical_bytes > MAX_MARKDOWN_TOTAL_CANONICAL_BYTES
        {
            return Err(MarkdownError::InvalidLimits);
        }
        Ok(self)
    }
}

/// Purely constructed immutable Markdown profile; source/fixture identities are
/// internally computed by the sole shared identity producer, not caller supplied.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct MarkdownProfile {
    raw_profile: MaterializerProfileId,
    limits: MarkdownLimits,
    id: Blake3Digest32,
}
impl MarkdownProfile {
    /// Validates a genuine strict Text-only/UTF8-only raw profile and all nine
    /// profile limits, then calls bounded pure profile hashing. No clock is minted.
    pub fn new(
        raw: &ValidatedMaterializerProfile,
        limits: MarkdownLimits,
    ) -> Result<Self, MarkdownError> {
        validate_raw_profile(raw)?;
        let limits = limits.validate()?.get();
        let id = digest_profile(raw, limits)?;
        Ok(Self {
            raw_profile: raw.id(),
            limits,
            id,
        })
    }
    /// New closed owner-computed profile commitment, not a contract UUID.
    pub const fn id(&self) -> Blake3Digest32 {
        self.id
    }
    /// Unmodified raw transform profile used by request/context.
    pub const fn raw_profile_id(&self) -> MaterializerProfileId {
        self.raw_profile
    }
    /// Actual lower profile ceilings bound into its canonical descriptor.
    pub const fn limits(&self) -> MarkdownLimits {
        self.limits
    }
}

pub(super) fn validate_raw_profile(
    raw: &ValidatedMaterializerProfile,
) -> Result<(), MarkdownError> {
    if raw.source_kinds() != [SourceKind::Text]
        || raw.encodings() != [SourceEncoding::Utf8]
        || raw.bom_policy() != BomPolicy::RejectWhenPresent
        || raw.invalid_sequence_policy() != InvalidSequencePolicy::Reject
        || raw.newline_policy() != NewlinePolicy::PreserveExact
        || raw.unicode_normalization() != UnicodeNormalization::None
        || raw.loss_behavior() != LossBehavior::RejectOnAnyLoss
    {
        return Err(MarkdownError::ProfileMismatch);
    }
    Ok(())
}
pub(super) fn narrow_limits(
    operation: MarkdownLimits,
    profile: MarkdownLimits,
) -> Result<MarkdownLimits, MarkdownError> {
    operation.validate()?;
    let dimensions = [
        (operation.input_bytes, profile.input_bytes),
        (operation.next_attempts, profile.next_attempts),
        (operation.stack_depth, profile.stack_depth),
        (operation.nodes, profile.nodes),
        (operation.inline_records, profile.inline_records),
        (operation.metadata_bytes_each, profile.metadata_bytes_each),
        (operation.metadata_bytes_total, profile.metadata_bytes_total),
        (operation.owned_bytes, profile.owned_bytes),
        (operation.work_steps, profile.work_steps),
    ];
    if dimensions.iter().any(|&(value, accepted)| value > accepted) {
        return Err(MarkdownError::InvalidLimits);
    }
    Ok(operation)
}

/// Private same-call accounting; original/retained/row storage exclude raw/maps,
/// allocator spare capacity and donor internals. No externally supplied proof value.
pub(super) struct MarkdownMeter<'a> {
    limits: MarkdownOperationLimits,
    work: usize,
    canonical: usize,
    original: usize,
    retained: usize,
    row: usize,
    cancel: CancellationToken<'a>,
    deadline: Instant,
}
impl<'a> MarkdownMeter<'a> {
    pub(super) fn operation(
        limits: MarkdownOperationLimits,
        cancel: CancellationToken<'a>,
        deadline: Instant,
    ) -> Result<Self, MarkdownError> {
        let meter = Self {
            limits: limits.validate()?,
            work: 0,
            canonical: 0,
            original: 0,
            retained: 0,
            row: 0,
            cancel,
            deadline,
        };
        meter.poll()?;
        Ok(meter)
    }
    pub(super) fn poll(&self) -> Result<(), MarkdownError> {
        checkpoint(self.cancel, self.deadline)?;
        Ok(())
    }
    pub(super) const fn work_used(&self) -> usize {
        self.work
    }
    pub(super) const fn canonical_used(&self) -> usize {
        self.canonical
    }
    pub(super) const fn remaining_storage(&self) -> usize {
        self.limits.parser.owned_bytes - self.original - self.retained - self.row
    }
    pub(super) fn hold_retained(&mut self, amount: usize) -> Result<(), MarkdownError> {
        self.poll()?;
        if amount > self.remaining_storage() {
            return Err(MarkdownError::StorageLimit);
        }
        self.retained = self
            .retained
            .checked_add(amount)
            .ok_or(MarkdownError::ArithmeticOverflow)?;
        Ok(())
    }
    pub(super) fn hold_original_inventory(&mut self, bytes: usize) -> Result<(), MarkdownError> {
        self.poll()?;
        if self.original != 0 || bytes > self.remaining_storage() {
            return Err(MarkdownError::StorageLimit);
        }
        self.original = bytes;
        Ok(())
    }
    pub(super) fn parser_limits(&self) -> Result<MarkdownLimits, MarkdownError> {
        self.poll()?;
        let work = self.remaining_work();
        let storage = self.remaining_storage();
        if work == 0 {
            return Err(MarkdownError::WorkLimit);
        }
        if storage == 0 {
            return Err(MarkdownError::StorageLimit);
        }
        Ok(MarkdownLimits {
            work_steps: work,
            owned_bytes: storage,
            ..self.limits.parser
        })
    }
}
impl IdentityBudget for MarkdownMeter<'_> {
    fn remaining_work(&self) -> usize {
        self.limits.parser.work_steps - self.work
    }
    fn charge_work(&mut self, amount: usize) -> Result<(), MarkdownError> {
        self.poll()?;
        if amount > self.remaining_work() {
            return Err(MarkdownError::WorkLimit);
        }
        self.work = self
            .work
            .checked_add(amount)
            .ok_or(MarkdownError::ArithmeticOverflow)?;
        Ok(())
    }
    // Identity precharges exact row construction from actual lengths/counts.
    // This method admits transient storage only; it never charges prepaid work twice.
    fn hold_transient(&mut self, bytes: usize) -> Result<(), MarkdownError> {
        self.poll()?;
        if bytes > self.remaining_storage() {
            return Err(MarkdownError::StorageLimit);
        }
        self.row = self
            .row
            .checked_add(bytes)
            .ok_or(MarkdownError::ArithmeticOverflow)?;
        Ok(())
    }
    fn release_row(&mut self) {
        self.row = 0;
    }
    fn remaining_canonical_bytes(&self) -> usize {
        (self.limits.total_canonical_bytes - self.canonical)
            .min(self.limits.canonical_preimage_bytes)
    }
    fn charge_canonical_bytes(&mut self, actual: usize) -> Result<(), MarkdownError> {
        self.poll()?;
        if actual > self.remaining_canonical_bytes() {
            return Err(MarkdownError::CanonicalEncoding);
        }
        self.canonical = self
            .canonical
            .checked_add(actual)
            .ok_or(MarkdownError::ArithmeticOverflow)?;
        // The identity producer separately charges actual emitted-byte work once.
        Ok(())
    }
}

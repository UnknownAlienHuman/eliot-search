//! Single materialization ingress; no source I/O or second map producer.
use super::model::{MaterializerProvenance, V3SourceBinding};
use super::v3_profile::{V3RepresentationKind, ValidatedV3UnitizerProfile};
use crate::{SourceLineSpan, UnitizationError};
use search_contracts::Blake3Digest32;
use search_materializer::api::{
    AssuranceCeiling, MaterializationProduct, SourceEncoding, SourceKind,
    ValidatedMaterializationRequest, ValidatedMaterializerProfile, verify_materialization,
};
use std::sync::atomic::{AtomicBool, Ordering};
use std::time::Instant;

/// Finite per-call work/output bounds and live cancellation/deadline.
#[derive(Clone, Copy, Debug)]
pub struct UnitizationBudget<'a> {
    /// Maximum accounted work, narrowing the profile ceiling.
    pub max_steps: u64,
    /// Maximum encoded manifest bytes, narrowing the profile ceiling.
    pub max_encoded_bytes: usize,
    /// Absolute monotonic deadline; excluded from durable identities.
    pub deadline: Instant,
    /// Live cancellation flag; excluded from durable identities.
    pub cancelled: &'a AtomicBool,
}

impl UnitizationBudget<'_> {
    pub(super) fn check(&self, used: u64) -> Result<(), UnitizationError> {
        if self.cancelled.load(Ordering::Acquire) {
            return Err(UnitizationError::Cancelled);
        }
        if Instant::now() >= self.deadline {
            return Err(UnitizationError::DeadlineExceeded);
        }
        if used > self.max_steps {
            return Err(UnitizationError::WorkBudgetExceeded);
        }
        Ok(())
    }
    pub(super) fn validate(
        &self,
        profile: &ValidatedV3UnitizerProfile,
    ) -> Result<(), UnitizationError> {
        let descriptor = profile.descriptor();
        if self.max_steps == 0 || self.max_steps > descriptor.max_steps {
            return Err(UnitizationError::InvalidLimits);
        }
        if self.max_encoded_bytes == 0 || self.max_encoded_bytes > descriptor.max_manifest_bytes {
            return Err(UnitizationError::InvalidLimits);
        }
        self.check(0)
    }
}

/// Immutable materialization view accepted by the v3 unitizer.
/// Construction always calls the materializer's owning verifier.
pub struct UnitSetInput<'a> {
    pub(super) product: &'a MaterializationProduct,
    pub(super) provenance: MaterializerProvenance,
    pub(super) lines: Vec<SourceLineSpan>,
    pub(super) prep_steps: u64,
    pub(super) profile_id: search_contracts::ProfileId,
}

impl core::fmt::Debug for UnitSetInput<'_> {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        f.debug_struct("UnitSetInput")
            .field("provenance", &self.provenance)
            .field("line_count", &self.lines.len())
            .finish_non_exhaustive()
    }
}

/// Verify the incoming product and borrow its exact byte-identical UTF-8 text.
///
/// The typed binding belongs to admission; it is never cast from a digest.
/// Lossy, transcoded or ambiguous representations need another accepted profile.
pub fn prepare_unit_set_input<'a>(
    binding: V3SourceBinding,
    product: &'a MaterializationProduct,
    request: &ValidatedMaterializationRequest,
    materializer_profile: &ValidatedMaterializerProfile,
    profile: &ValidatedV3UnitizerProfile,
    budget: &UnitizationBudget<'_>,
) -> Result<UnitSetInput<'a>, UnitizationError> {
    budget.validate(profile)?;
    let text = product.canonical_text();
    if text.is_empty() {
        return Err(UnitizationError::EmptyInput);
    }
    let limits = profile.limits();
    if text.len() > limits.max_input_bytes || product.canonical().lines().len() > limits.max_lines {
        return Err(UnitizationError::InputTooLarge);
    }
    let input_bytes = u64::try_from(text.len()).map_err(|_| UnitizationError::OffsetOverflow)?;
    let line_count = u64::try_from(product.canonical().lines().len())
        .map_err(|_| UnitizationError::OffsetOverflow)?;
    // Charge bounded owner verification, byte checks and offset conversion
    // before their allocations/passes. Build carries these steps forward.
    let prep_steps = input_bytes
        .checked_mul(4)
        .and_then(|n| n.checked_add(line_count.checked_mul(4)?))
        .ok_or(UnitizationError::OffsetOverflow)?;
    budget.check(prep_steps)?;
    verify_materialization(product, request, materializer_profile)
        .map_err(|_| UnitizationError::UnitManifestIncomplete)?;
    let expected_kind = match profile.descriptor().representation_kind {
        V3RepresentationKind::Text => SourceKind::Text,
        V3RepresentationKind::Code => SourceKind::Code,
    };
    if request.source_id().as_str() != binding.source_id.to_string()
        || request.declared_kind() != expected_kind
        || product.encoding() != SourceEncoding::Utf8
        || product.assurance().ceiling() != AssuranceCeiling::ExactBytes
        || !product.maps().loss_map().records().is_empty()
        || product.canonical_digest() != product.input_digest()
        || product.resource_receipt().input_bytes != input_bytes
        || product.resource_receipt().output_bytes != input_bytes
        || request.byte_count() != input_bytes
    {
        return Err(UnitizationError::UnitManifestIncomplete);
    }
    // Existing maps have been verified above. The exact-bytes profile maps
    // canonical scalar line coordinates to the same authoritative source bytes.
    let mut offsets = text
        .char_indices()
        .map(|(offset, _)| offset)
        .chain(std::iter::once(text.len()))
        .enumerate()
        .peekable();
    let mut byte_offset = |scalar: u64| -> Result<u64, UnitizationError> {
        let target = usize::try_from(scalar).map_err(|_| UnitizationError::OffsetOverflow)?;
        while offsets.peek().is_some_and(|(index, _)| *index < target) {
            let (index, _) = offsets.next().ok_or(UnitizationError::InvalidLineSpan)?;
            if index % 256 == 0 {
                budget.check(prep_steps)?;
            }
        }
        let (index, offset) = offsets.peek().ok_or(UnitizationError::InvalidLineSpan)?;
        if *index != target {
            return Err(UnitizationError::InvalidLineSpan);
        }
        u64::try_from(*offset).map_err(|_| UnitizationError::OffsetOverflow)
    };
    let mut lines = Vec::with_capacity(product.canonical().lines().len());
    for (index, line) in product.canonical().lines().iter().enumerate() {
        budget.check(prep_steps)?;
        let source_start = byte_offset(line.canonical_start)?;
        let content_end = byte_offset(line.canonical_content_end)?;
        let source_end = byte_offset(line.canonical_end)?;
        lines.push(SourceLineSpan {
            line_index: u64::try_from(index).map_err(|_| UnitizationError::OffsetOverflow)?,
            source_start,
            source_end,
            content_end,
        });
    }
    budget.check(prep_steps)?;
    Ok(UnitSetInput {
        product,
        provenance: MaterializerProvenance {
            binding,
            materializer_commitment: product.representation_id(),
            materializer_profile_digest: Blake3Digest32::from_bytes(
                *materializer_profile.id().as_bytes(),
            ),
            canonical_digest: product.canonical_digest(),
            coordinate_digest: product.coordinate_digest(),
            loss_digest: product.loss_digest(),
            input_digest: product.input_digest(),
            native_bytes: input_bytes,
            canonical_bytes: input_bytes,
            legacy_revision_sequence: request.revision().get(),
        },
        lines,
        prep_steps,
        profile_id: profile.id().clone(),
    })
}

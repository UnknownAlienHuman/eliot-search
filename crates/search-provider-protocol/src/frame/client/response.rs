//! Typed provider-to-client messages using the same bounded schema kernel.

mod fences;
mod results;
mod types;

use search_contracts::{
    BoundedBytes, BoundedProgressCounts, CancelledBody, CompareImplementationsResult,
    EntityExplorationResult, EntityInspectionResult, ErrorBody, ProgressBody,
    ProtocolRange, ProviderEnvelope, RecipeResultV1, ResultBody, ResultFence, MAX_FRAME_BYTES,
};

use crate::config::ProtocolLimits;
use crate::error::ProtocolError;
use super::{ClientEnvelopeCodec, Direction};
use super::wire::{Decoder, Encoder, Result, Schema, record};

/// Strict P00 1.0 hello, progress, result, error and cancelled codec.
///
/// Every result family uses the existing shared contract types and bounded
/// fields, including exact reports, ambiguity, coverage and emission fences.
/// This encodes data, not authority: callers must check the live binding,
/// directional/event sequences, expected recipe and disclosure before using or
/// emitting it. A receipt reference is serialized, never issued or verified here.
/// Shell/grant-specific proof transcripts and live transport routing are unchanged.
pub struct ServerEnvelopeCodec;

impl ServerEnvelopeCodec {
    /// Encodes one provider message with the existing four-byte frame prefix.
    ///
    /// # Errors
    ///
    /// Rejects wrong direction/version, inconsistent tags/correlated identities,
    /// invalid nested contract values or exhausted frame/field/collection bounds.
    pub fn encode(
        envelope: &ProviderEnvelope,
        limits: ProtocolLimits,
        supported: ProtocolRange,
    ) -> Result<BoundedBytes<MAX_FRAME_BYTES>> {
        ClientEnvelopeCodec::encode_direction(envelope, limits, supported, Direction::Server)
    }

    /// Decodes one complete provider message; it never accepts a request/cancel.
    ///
    /// # Errors
    ///
    /// Rejects malformed framing, duplicate/unknown/missing fields, noncanonical
    /// scalars, wrong tags/direction/version and invalid nested contract shapes.
    /// Correlation with the live request and authorization remain caller-owned.
    pub fn decode(
        bytes: &[u8],
        limits: ProtocolLimits,
        supported: ProtocolRange,
    ) -> Result<ProviderEnvelope> {
        ClientEnvelopeCodec::decode_direction(bytes, limits, supported, Direction::Server)
    }
}

record!(BoundedProgressCounts {
    completed_legs, total_planned_legs, nominated_candidates, validated_candidates,
    omitted_or_failed_legs,
} => |value: &BoundedProgressCounts| {
    // An overflowing sum is not a valid count, even when a saturating sum
    // would equal the declared maximum. Do not normalize malformed progress.
    if value.completed_legs.checked_add(value.omitted_or_failed_legs)
        .is_none_or(|sum| sum > value.total_planned_legs)
        || value.validated_candidates > value.nominated_candidates
    {
        return Err(ProtocolError::InvalidBody);
    }
    Ok(())
});
record!(ProgressBody { event_sequence, phase, bounded_counts, degraded_reason_codes });
record!(ResultBody { event_sequence, result });
record!(ErrorBody { code, retryability, message_template_id, bounded_metadata });
record!(CancelledBody { target_request_id, terminal });

/// Check identities present in the result schema without inventing fields for
/// exact plans/reports. Those correlate by their enclosing request and plan_ref
/// through the request lifecycle, not through a synthetic inner request ID.
pub(super) fn validate_result_identity(
    result: &RecipeResultV1,
    envelope: &ProviderEnvelope,
) -> Result<()> {
    let identity = match result {
        RecipeResultV1::Locate(value) | RecipeResultV1::FindText(value) => {
            Some((value.request_id, &value.result_fence))
        }
        RecipeResultV1::InspectEntity(value) => {
            let header = match value {
                EntityInspectionResult::Resolved(value) => &value.header,
                EntityInspectionResult::Ambiguous(value) => &value.header,
            };
            Some((header.request_id, &header.result_fence))
        }
        RecipeResultV1::ExploreEntity(value) => {
            let header = match value {
                EntityExplorationResult::Resolved(value) => &value.header,
                EntityExplorationResult::Ambiguous(value) => &value.header,
            };
            Some((header.request_id, &header.result_fence))
        }
        RecipeResultV1::CompareImplementations(value) => {
            let header = match value {
                CompareImplementationsResult::Compared(value) => &value.header,
                CompareImplementationsResult::Ambiguous(value) => &value.header,
            };
            Some((header.request_id, &header.result_fence))
        }
        RecipeResultV1::CorpusProfile(value) => Some((value.header.request_id, &value.header.result_fence)),
        RecipeResultV1::CorpusDelta(value) => Some((value.header.request_id, &value.header.result_fence)),
        RecipeResultV1::Provenance(value) => Some((value.header.request_id, &value.header.result_fence)),
        RecipeResultV1::ExpandHandle(value) => Some((value.header.request_id, &value.header.result_fence)),
        RecipeResultV1::CompileExactScan(_) | RecipeResultV1::ExecuteExactScan(_) => None,
    };
    if let Some((request_id, fence)) = identity {
        if request_id != envelope.request_id || !same_incarnation(fence, envelope) {
            return Err(ProtocolError::InvalidBody);
        }
    }
    Ok(())
}

fn same_incarnation(fence: &ResultFence, envelope: &ProviderEnvelope) -> bool {
    fence.planned_snapshot.installation_incarnation_id == envelope.installation_incarnation_id
}

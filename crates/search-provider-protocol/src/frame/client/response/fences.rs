//! Planning/emission fences, coverage and exact reports remain typed data.

use search_contracts::{
    AuthorizedScopeRef, BehaviorConflict, BehaviorObservation, BoundedNonContentRankingTrace,
    CandidateValidationGap, ConfigurationObservation, Coverage, CoverageGap, CoverageUnknown,
    EmissionSecurityFence, ExactExecutionReport, ExactItemFailure, ExactMatch,
    ExactScanDenominator, ExactScanPlan, LegDescriptor, LegExecutionSummary, MembershipReadiness,
    ObservationFreshness, QuerySnapshotFence, RecipeResultHeader, ResultFence, SearchCandidateSet,
    SourceOwnerFence, SourceRevisionRef, ValidatedSearchCandidate,
};

use crate::error::ProtocolError;
use super::super::wire::{Decoder, Encoder, Result, Schema, record};

record!(SourceRevisionRef {
    source_namespace_id, source_id, revision_id, content_digest, byte_length,
});
record!(SourceOwnerFence { source_namespace_id, source_owner_generation });
record!(AuthorizedScopeRef { scope_domain_id, authorized_scope_digest });
record!(ObservationFreshness { state, observation_cursor_revision, observed_age_ms }
    => |value: &ObservationFreshness| value.validate().map_err(|_| ProtocolError::InvalidBody));
record!(QuerySnapshotFence {
    installation_incarnation_id, collection_generation_id, visible_epoch,
    collection_route_revision, catalog_revision, membership_revision, reference_portfolio_revision,
    access_policy_revision, shadow_fence_revision, purge_fence_revision, overlay_revision,
    observation_cursor_revision, observation_freshness, source_view, workspace_view_revision_ref,
    lexical_profile_ids, snapshot_fingerprint,
} => |value: &QuerySnapshotFence| value.validate().map_err(|_| ProtocolError::InvalidBody));
record!(EmissionSecurityFence {
    access_policy_revision, live_deny_generation, shadow_fence_revision, purge_fence_revision,
    checked_at, receipt_ref,
});
record!(ResultFence {
    planned_snapshot, emission_source_owner_fences, emission_security_fence, result_fingerprint,
});
record!(CandidateValidationGap {
    nominated_candidate_ref, source_revision_ref, reason, affected_leg_refs,
    contaminated_rank_leg, disposition,
} => |value: &CandidateValidationGap| value.validate().map_err(|_| ProtocolError::InvalidBody));
record!(LegDescriptor { leg_ref, leg_kind, scoring_partition_ref, profile_id });
record!(LegExecutionSummary { leg_ref, state, nominated_count, validated_count, reason_codes, receipt_ref });
record!(CoverageGap { gap_ref, kind, affected_scope_refs, reason_codes, retryability });
record!(CoverageUnknown { unknown_ref, description_template_id, bounded_metadata });
record!(Coverage {
    requested_legs, executed_legs, represented_memberships, represented_source_lineages,
    omitted_or_failed_legs, candidate_validation_gaps, observation_freshness, unknowns, denominator_kind,
} => |value: &Coverage| value.validate().map_err(|_| ProtocolError::InvalidBody));
record!(BoundedNonContentRankingTrace {
    fusion_profile_id, fused_rank, exact_or_entity_boost, evidence_role_priority,
    portfolio_priority, lineage_diversity_action, deterministic_tie_break_digest,
});
record!(ValidatedSearchCandidate {
    candidate_id, source_handle, evidence_role, entity_kind, assurance, freshness,
    ranking_trace, reason_codes, candidate_validation_receipt_ref,
} => |value: &ValidatedSearchCandidate| value.validate().map_err(|_| ProtocolError::InvalidBody));
record!(SearchCandidateSet {
    request_id, plan_id, plan_fingerprint, result_fence, candidates, coverage,
    continuation_handle, result_validation_receipt_ref,
} => |value: &SearchCandidateSet| value.validate().map_err(|_| ProtocolError::InvalidBody));
record!(RecipeResultHeader {
    request_id, plan_id, plan_fingerprint, result_fence, coverage, reason_codes,
});
record!(ConfigurationObservation { predicate, observation, evidence_handles, assurance });
record!(BehaviorObservation {
    axis, summary, evidence_handles, configuration_predicate, independent_lineage_count, assurance,
});
record!(BehaviorConflict { axis, left, right, conflict_summary, unresolved_reason_codes });
record!(MembershipReadiness {
    source_membership_id, direct_ready, lexical_ready, code_ready, semantic_ready,
    document_ready, visible_epoch, observation_freshness, degraded_reason_codes,
});
record!(ExactScanDenominator { source_revision_ids, inventory_revision });
record!(ExactScanPlan {
    plan_id, predicate, denominator, inclusion_policy_digest, unsaved_buffer_snapshot_ids,
    completeness_requirements, plan_fingerprint,
});
record!(ExactMatch {
    source_revision_ref, native_anchor, match_digest, matched_byte_length,
    predicate_profile_id, assurance, source_handle,
} => |value: &ExactMatch| value.validate().map_err(|_| ProtocolError::InvalidBody));
record!(ExactItemFailure { source_revision_id, failure_kind, reason_codes, bounded_metadata });
record!(ExactExecutionReport {
    plan_ref, matched_items, scanned_items, scanned_bytes, unreadable_items,
    changed_or_unavailable_items, timed_out, cancelled, scope_drifted, coverage, conclusion, receipt_ref,
} => |value: &ExactExecutionReport| value.validate().map_err(|_| ProtocolError::InvalidBody));

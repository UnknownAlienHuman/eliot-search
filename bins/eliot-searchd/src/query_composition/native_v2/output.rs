//! C26 projection into the actual prepared V2 envelope and existing MAC writer.

use super::ReadTurn;
use crate::provider_composition::{
    CanonicalServingError as Error, CanonicalTcpConnection, CanonicalTcpError,
};
use search_access::{AccessCheckpoint, AccessPermit};
use search_contracts::{
    self as c,
    evidence::{
        EvidenceCardV1, PublicCardSource, PublicCurrentnessState, RetrieveEvidenceResultV2,
    },
    evidence_frames::{
        EvidenceFrameHeader, EvidenceFrameKind, EvidenceFrameV2, EvidenceTerminalFrame,
        EvidenceTerminalKind,
    },
};
use search_handles::PendingHandleMints;
use search_provider_protocol::{
    AdmittedEvidenceProviderRequestV2, EvidenceResponseBodyV2, PreparedProviderDeliveryError,
    ServerEnvelopeCodec, frame::EvidenceRecipeResultV2,
};
use search_result_projector::{
    CandidateProjectionInput, CandidateSetEnvelope, ProjectionBudget, ProjectionError,
    project_candidate_set,
};
use std::collections::BTreeMap;

pub(super) struct CardSource {
    pub(super) source: PublicCardSource,
    pub(super) unit: c::UnitOccurrence,
}

pub(super) fn digest(
    domain: &str,
    values: Vec<c::CanonicalValue>,
) -> Result<c::Blake3Digest32, Error> {
    let domain = c::CanonicalDigestDomain::parse(domain).map_err(|_| Error::InvalidCompletion)?;
    let value = c::CanonicalValue::Array(
        c::BoundedList::new(values).map_err(|_| Error::InvalidCompletion)?,
    );
    c::blake3_canonical(
        &domain,
        &value,
        c::DigestInputLimit::new(c::MAX_CANONICAL_BYTES).map_err(|_| Error::InvalidCompletion)?,
    )
    .map_err(|_| Error::InvalidCompletion)
}

pub(super) fn bytes(value: &[u8]) -> Result<c::CanonicalValue, Error> {
    c::BoundedBytes::new(value.to_vec())
        .map(c::CanonicalValue::Bytes)
        .map_err(|_| Error::InvalidCompletion)
}

pub(super) fn text(value: &str) -> Result<c::CanonicalValue, Error> {
    c::CanonicalText::new(value)
        .map(c::CanonicalValue::Text)
        .map_err(|_| Error::InvalidCompletion)
}

/// Metadata describes the actual checkpoint performed while the same lock is
/// held. It is not a new permit: C26 still requires every private C24 permit.
pub(super) fn result_fence(
    turn: &ReadTurn<'_, '_>,
    permit: AccessPermit,
    plan: c::PlanFingerprint,
    inputs: &[CandidateProjectionInput],
) -> Result<c::ResultFence, Error> {
    turn.check()?;
    let snapshot = turn.population.snapshot_fence();
    let checked_at = crate::native_time::timestamp_from_system_time(std::time::SystemTime::now())
        .map_err(|_| Error::InvalidCompletion)?;
    let check = digest(
        "eliot/cbor/native-v2-emission-checkpoint/v1",
        vec![
            bytes(turn.request.guard().request_id().as_bytes())?,
            bytes(plan.as_bytes())?,
            bytes(snapshot.snapshot_fingerprint.as_bytes())?,
            bytes(permit.live_snapshot_digest.as_bytes())?,
            c::CanonicalValue::U64(permit.live_generation),
            text(checked_at.as_str())?,
        ],
    )?;
    let mut values = vec![bytes(plan.as_bytes())?, bytes(check.as_bytes())?];
    let mut owners = Vec::new();
    for input in inputs {
        values.push(bytes(input.source_backed.candidate_id().as_bytes())?);
        values.push(text(
            input
                .source_backed
                .candidate_validation_receipt_ref()
                .as_str(),
        )?);
        let owner = input.source_backed.source_owner_fence();
        if !owners.contains(&owner) {
            owners.push(owner);
        }
    }
    Ok(c::ResultFence {
        planned_snapshot: snapshot.clone(),
        emission_source_owner_fences: c::BoundedList::new(owners)
            .map_err(|_| Error::InvalidCompletion)?,
        emission_security_fence: c::EmissionSecurityFence {
            access_policy_revision: snapshot.access_policy_revision,
            live_deny_generation: permit.live_generation,
            shadow_fence_revision: snapshot.shadow_fence_revision,
            purge_fence_revision: snapshot.purge_fence_revision,
            checked_at,
            receipt_ref: c::ReceiptRef::new(format!("native-v2-emission-checkpoint:v1:{check}"))
                .map_err(|_| Error::InvalidCompletion)?,
        },
        result_fingerprint: digest("eliot/cbor/native-v2-source-backed-result/v1", values)?,
    })
}

fn evidence_result(
    set: &c::SearchCandidateSet,
    request: &c::evidence::RetrieveEvidenceRequestV2,
    sources: &BTreeMap<c::CandidateId, CardSource>,
) -> Result<RetrieveEvidenceResultV2, ProjectionError> {
    let mut cards = Vec::with_capacity(set.candidates.len());
    for candidate in &set.candidates {
        let source = sources
            .get(&candidate.candidate_id)
            .ok_or(ProjectionError::InvalidResultContract)?;
        cards.push(EvidenceCardV1 {
            card_id: candidate.candidate_id,
            evidence_role: candidate.evidence_role,
            source: source.source.clone(),
            unit: source.unit.clone(),
            handle: candidate.source_handle.clone(),
            bounded_preview: None,
            producer_assurance: candidate.assurance,
            freshness: candidate.freshness,
            // A retained revision is verified; no live worktree currentness was observed.
            currentness: PublicCurrentnessState::NotApplicable,
            lineage: None,
            relations: c::BoundedList::new(Vec::new())
                .map_err(|_| ProjectionError::InvalidResultContract)?,
            ambiguity_group: None,
            score_profile: request.profile_id.clone(),
            trace_ref: candidate.ranking_trace.clone(),
        });
    }
    let result = RetrieveEvidenceResultV2 {
        request_id: set.request_id,
        plan_id: set.plan_id,
        plan_fingerprint: set.plan_fingerprint,
        result_fence: set.result_fence.clone(),
        resolved_scope: request.scope.clone(),
        cards: c::BoundedList::new(cards).map_err(|_| ProjectionError::InvalidResultContract)?,
        groups: c::BoundedList::new(Vec::new())
            .map_err(|_| ProjectionError::InvalidResultContract)?,
        suggested_exact_checks: c::BoundedList::new(Vec::new())
            .map_err(|_| ProjectionError::InvalidResultContract)?,
        coverage: set.coverage.clone(),
        continuation: None,
    };
    result
        .validate()
        .map_err(|_| ProjectionError::InvalidResultContract)?;
    Ok(result)
}

#[allow(clippy::too_many_arguments)]
#[expect(
    clippy::too_many_lines,
    reason = "Keep the current permit, prepared frame and possible-delivery handle custody ordered"
)]
pub(super) fn emit_projected(
    transport: &mut CanonicalTcpConnection,
    admitted: &mut AdmittedEvidenceProviderRequestV2,
    issued: &crate::access_composition::VerifiedStandaloneGrant<'_>,
    population: &search_access::CompiledEligibilityPopulation,
    domain: &crate::access_composition::NativeSecurityDomain,
    fence: &search_access::RequestSecurityFence,
    budget: crate::provider_composition::CanonicalWorkBudget,
    mut envelope: CandidateSetEnvelope,
    inputs: Vec<CandidateProjectionInput>,
    sources: &BTreeMap<c::CandidateId, CardSource>,
    projection_budget: ProjectionBudget,
    pending: PendingHandleMints<'_>,
) -> Result<(), Error> {
    let mut pending = Some(pending);
    let mut final_coverage = None;
    domain
        .with_live_checkpoint(fence, AccessCheckpoint::BeforeResultEmission, |permit| {
            let turn = ReadTurn {
                request: admitted,
                issued,
                population,
                domain,
                fence,
                budget,
            };
            turn.check()?;
            envelope.result_fence =
                result_fence(&turn, permit, envelope.plan_fingerprint, &inputs)?;
            let request = match &admitted.envelope().recipe_request {
                search_provider_protocol::frame::EvidenceRecipeRequestV2::RetrieveEvidence(
                    request,
                ) => request.clone(),
                search_provider_protocol::frame::EvidenceRecipeRequestV2::OrientScope(_) => {
                    return Err(Error::InvalidCompletion);
                }
            };
            let delivery = transport.deliver_evidence_prepared_before_v2(
                admitted,
                budget.deadline().min(issued.valid_until()),
                |context| {
                    budget.check()?;
                    let mut body = None;
                    let projected = project_candidate_set(
                        envelope,
                        inputs,
                        projection_budget,
                        |set, ceiling| {
                            let result = evidence_result(set, &request, sources)?;
                            let response = EvidenceResponseBodyV2::Result(
                                EvidenceRecipeResultV2::RetrieveEvidence(result),
                            );
                            let frame = ServerEnvelopeCodec::encode_evidence_response_v2(
                                &context.envelope_v2(response.clone()),
                                context.limits(),
                                context.supported_versions(),
                            )
                            .map_err(|_| ProjectionError::OutputEncodingFailed)?;
                            if frame.len() > ceiling {
                                return Err(ProjectionError::SerializedOutputBudgetExceeded);
                            }
                            body = Some(response);
                            Ok(frame)
                        },
                    )
                    .map_err(|error| Error::Backend(error.code()))?;
                    budget.check()?;
                    final_coverage = Some(projected.result().coverage.clone());
                    let body = body.ok_or(Error::InvalidCompletion)?;
                    let frame = c::BoundedBytes::new(projected.serialized_output_bytes().to_vec())
                        .map_err(|_| Error::InvalidCompletion)?;
                    Ok((body, frame))
                },
            );
            if matches!(
                &delivery,
                Ok(())
                    | Err(PreparedProviderDeliveryError::Output(_)
                        | PreparedProviderDeliveryError::AfterOutput(_))
            ) && let Some(pending) = pending.take()
            {
                pending.preserve_after_possible_delivery();
            }
            match delivery {
                Ok(()) => Ok(()),
                Err(PreparedProviderDeliveryError::Preparation(error)) => Err(error),
                Err(PreparedProviderDeliveryError::Protocol(error)) => {
                    Err(Error::Transport(CanonicalTcpError::Protocol(error)))
                }
                Err(PreparedProviderDeliveryError::Output(error)) => Err(Error::Transport(error)),
                Err(PreparedProviderDeliveryError::AfterOutput(error)) => {
                    Err(Error::Transport(CanonicalTcpError::AfterOutput(error)))
                }
            }
        })
        .map_err(Error::Access)??;
    let coverage = final_coverage.ok_or(Error::InvalidCompletion)?;
    let has_gap = !coverage.omitted_or_failed_legs.is_empty()
        || !coverage.candidate_validation_gaps.is_empty()
        || !coverage.unknowns.is_empty();
    let profile = match &admitted.envelope().recipe_request {
        search_provider_protocol::frame::EvidenceRecipeRequestV2::RetrieveEvidence(value) => {
            value.profile_id.clone()
        }
        search_provider_protocol::frame::EvidenceRecipeRequestV2::OrientScope(_) => {
            return Err(Error::InvalidCompletion);
        }
    };
    let body = EvidenceResponseBodyV2::Frame(EvidenceFrameV2::Terminal(EvidenceTerminalFrame {
        header: EvidenceFrameHeader {
            request_id: *admitted.guard().request_id(),
            event_sequence: admitted
                .next_event_sequence()
                .ok_or(Error::InvalidCompletion)?,
            kind: EvidenceFrameKind::Terminal,
            profile_id: profile,
        },
        terminal: if has_gap {
            EvidenceTerminalKind::Partial
        } else {
            EvidenceTerminalKind::Success
        },
        coverage,
        reason_codes: c::BoundedSet::empty(),
        budget_exhausted: has_gap,
    }));
    domain
        .with_live_checkpoint(fence, AccessCheckpoint::BeforeResultEmission, |_| {
            budget.check()?;
            issued
                .verify_current_evidence_claims(&admitted.envelope().grant)
                .map_err(Error::GrantRefused)?;
            transport
                .deliver_evidence_event_before_v2(
                    admitted,
                    body,
                    budget.deadline().min(issued.valid_until()),
                )
                .map_err(Error::Transport)
        })
        .map_err(Error::Access)??;
    Ok(())
}

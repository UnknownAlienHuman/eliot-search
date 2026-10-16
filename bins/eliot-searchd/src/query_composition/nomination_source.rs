//! Pure source-metadata mapping for indexed nominations.
//!
//! Backend point and digest values remain hints. A result is formed only after
//! those hints bind to the exact planner manifest, the actual source controls,
//! the verified raw-v3 unit set, and the current request/leg/route pins.

use std::collections::{BTreeMap, BTreeSet};

use search_candidate_validator::{
    CandidateAssurance, CandidateNomination, ValidationContext, ValidationPrecheck, precheck,
};
use search_contracts::{
    Blake3Digest32, CollectionRouteRevision, NativeAnchor, ProfileId, SearchTaskPlan,
    SourceMembership, SourceRevision, SourceView, UnitId,
};
use search_epoch_pins::RouteIdentity;
use search_projection_planner::{PointSpec, ProjectionManifestEntry, ProjectionPlan};
use search_qdrant_bridge::CollectionRoute;
use search_retrieval_executor::{LegPinSet, LegTicket, indexed::IndexedNomination};
use search_unitizer::{
    V3AnchorPolicy, V3AttachmentPolicy, V3OmissionPolicy, V3OverlapPolicy, VerifiedUnitSet,
};

/// Closed refusal from the pure nomination mapper.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum NominationSourceError {
    ContextMismatch,
    SourceMismatch,
    RevisionMismatch,
    MembershipMismatch,
    UnitSetMismatch,
    ProjectionMismatch,
    UnsupportedRawProfile,
    UnknownPoint,
    DuplicateNomination,
    PointHintMismatch,
    InvalidScore,
    CandidateRejected,
}

/// Maps a complete backend batch to validator nominations without source I/O.
///
/// Membership comes from the control manifest and the actual control record;
/// the backend membership is checked only as a hint. `None` for the expected
/// excerpt digest is emitted only for a verified complete raw-v3 unit span.
#[allow(clippy::too_many_arguments)]
pub(super) fn map_indexed_nominations(
    nominations: &[IndexedNomination],
    task_plan: &SearchTaskPlan,
    ticket: &LegTicket,
    pins: &LegPinSet,
    route: &CollectionRoute,
    validation: &ValidationContext,
    membership: &SourceMembership,
    revision: &SourceRevision,
    unit_set: &VerifiedUnitSet,
    projection: &ProjectionPlan,
) -> Result<Vec<CandidateNomination>, NominationSourceError> {
    if nominations.len() > search_retrieval_executor::indexed::MAX_INDEXED_LIMIT {
        return Err(NominationSourceError::ContextMismatch);
    }
    let member = membership.source_membership_id;
    let source_binding = unit_set.manifest().provenance().binding();
    if membership.source_id != revision.source_id || source_binding.source_id != revision.source_id
    {
        return Err(NominationSourceError::SourceMismatch);
    }
    if source_binding.source_revision_id != revision.revision_id
        || unit_set.manifest().provenance().legacy_revision_sequence()
            != revision.occurrence_sequence
        || unit_set.manifest().provenance().input_digest() != revision.content_digest
        || unit_set.manifest().provenance().canonical_digest() != revision.content_digest
        || unit_set.manifest().provenance().native_bytes() != revision.byte_length
        || unit_set.manifest().provenance().canonical_bytes() != revision.byte_length
    {
        return Err(NominationSourceError::RevisionMismatch);
    }
    if unit_set.representation().unit_manifest_digest != unit_set.manifest_digest()
        || unit_set.representation().unitizer_profile_id != *unit_set.manifest().profile_id()
        || source_binding.representation_id != unit_set.representation().representation_id
    {
        return Err(NominationSourceError::UnitSetMismatch);
    }

    let profile = unit_set.manifest().profile();
    if profile.anchor_policy != V3AnchorPolicy::ExactTextBytes
        || profile.attachment_policy != V3AttachmentPolicy::Absent
        || profile.omission_policy != V3OmissionPolicy::Forbidden
        || profile.overlap_policy != V3OverlapPolicy::None
    {
        return Err(NominationSourceError::UnsupportedRawProfile);
    }
    let profile_id = unit_set.manifest().profile_id().clone();
    let profile_digest_hex = profile_id
        .as_str()
        .strip_prefix("unitizer-v3-")
        .ok_or(NominationSourceError::UnsupportedRawProfile)?;
    let profile_digest = Blake3Digest32::parse_hex(profile_digest_hex)
        .map_err(|_| NominationSourceError::UnsupportedRawProfile)?;

    let safe_leg = ticket
        .leg
        .safe_index_leg
        .as_ref()
        .ok_or(NominationSourceError::ContextMismatch)?;
    if ticket.request_id != task_plan.request_id
        || ticket.request_id != validation.request_id
        || ticket.plan_digest.0 != validation.plan_digest
        || safe_leg.leg_id != ticket.leg.leg_id
        || safe_leg.memberships != ticket.leg.memberships
        || !ticket.leg.memberships.contains(&member)
        || !validation.request_security.memberships.contains(&member)
        || !validation.allowed_memberships.contains(&member)
        || !task_plan
            .selected_membership_ids
            .iter()
            .any(|id| *id == member)
        || !task_plan
            .source_owner_fences
            .iter()
            .any(|fence| fence.source_namespace_id == source_binding.source_namespace_id)
        || task_plan.query_snapshot_fence.source_view
            != SourceView::RetainedRevision(revision.revision_id)
    {
        return Err(NominationSourceError::ContextMismatch);
    }
    let pins = pins.epoch_pins();
    if pins.len() != 1 {
        return Err(NominationSourceError::ContextMismatch);
    }
    let pin = &pins[0];
    let pin_route = pin.route();
    let indexed_route = RouteIdentity {
        collection_generation_id: safe_leg.route.collection_generation_id,
        route_revision: CollectionRouteRevision::new(safe_leg.route.route_generation),
    };
    let visible_epoch = pin.epoch();
    if pin_route != indexed_route
        || route.generation != pin_route.collection_generation_id
        || safe_leg.route.visible_epoch != visible_epoch
        || task_plan.query_snapshot_fence.installation_incarnation_id
            != projection
                .manifest
                .entries
                .first()
                .ok_or(NominationSourceError::ProjectionMismatch)?
                .installation_incarnation_id
        || task_plan.query_snapshot_fence.collection_generation_id != Some(route.generation)
        || task_plan.query_snapshot_fence.visible_epoch != Some(visible_epoch)
        || task_plan.query_snapshot_fence.collection_route_revision != pin_route.route_revision
        || validation.collection_generation_id != route.generation
        || validation.visible_epoch != visible_epoch
        || safe_leg.route.collection_generation_id != route.generation
    {
        return Err(NominationSourceError::ContextMismatch);
    }
    let valid_until_epoch_exclusive = visible_epoch
        .checked_next()
        .map_err(|_| NominationSourceError::ContextMismatch)?;

    if projection.source_membership_id != member
        || projection.points.len() != unit_set.units().len()
        || projection.manifest.entries.len() != projection.points.len()
    {
        return Err(NominationSourceError::ProjectionMismatch);
    }
    let mut units = BTreeMap::<UnitId, _>::new();
    for unit in unit_set.units() {
        if units.insert(unit.unit_id(), unit).is_some() {
            return Err(NominationSourceError::UnitSetMismatch);
        }
        let NativeAnchor::TextBytes(anchor) = &unit.occurrence().native_anchor else {
            return Err(NominationSourceError::UnsupportedRawProfile);
        };
        if anchor.content_digest != revision.content_digest
            || anchor.byte_start_0 != unit.source_start()
            || anchor.byte_end_exclusive_0 != unit.source_end()
            || unit.source_start() >= unit.source_end()
            || unit.source_end() > revision.byte_length
        {
            return Err(NominationSourceError::UnitSetMismatch);
        }
    }

    let incarnation = task_plan.query_snapshot_fence.installation_incarnation_id;
    let generation = route.generation;
    let mut entries = BTreeMap::<[u8; 16], &ProjectionManifestEntry>::new();
    for entry in &projection.manifest.entries {
        if entry.source_membership_id != member
            || entry.collection_generation_id != generation
            || entry.installation_incarnation_id != incarnation
            || !task_plan
                .profile_fence
                .projection_profile_set_ids
                .iter()
                .any(|id| id == &entry.projection_profile_set_id)
            || entries.insert(*entry.point_id.as_bytes(), entry).is_some()
        {
            return Err(NominationSourceError::ProjectionMismatch);
        }
    }
    let mut points = BTreeMap::<[u8; 16], &PointSpec>::new();
    let mut planned_units = BTreeSet::new();
    for point in &projection.points {
        let point_id = *point.point_id.as_bytes();
        let key = point.identity.key();
        if key.point_role.as_str() != "unit"
            || key.installation_incarnation_id != incarnation
            || key.collection_generation_id != generation
            || key.representation_id != unit_set.representation().representation_id
            || key.projection_membership_id != projection.projection_membership_id
            || !planned_units.insert(key.unit_id)
            || points.insert(point_id, point).is_some()
        {
            return Err(NominationSourceError::ProjectionMismatch);
        }
        let entry = entries
            .get(&point_id)
            .ok_or(NominationSourceError::ProjectionMismatch)?;
        let unit = units
            .get(&entry.unit_id)
            .ok_or(NominationSourceError::UnitSetMismatch)?;
        if point.identity.point_id() != entry.point_id
            || point.identity.full_digest() != entry.point_identity_digest_256
            || key.unit_id != entry.unit_id
            || key.projection_profile_set_id != entry.projection_profile_set_id
            || entry.projection_membership_id != projection.projection_membership_id
            || entry.unit_digest != unit.unit_content_digest()
            || entry.reference_digest != unit.reference_digest()
            || point.expected_readback.payload_identity != entry.payload_identity
            || point.expected_readback.unit_digest != entry.unit_digest
            || point.expected_readback.reference_digest != entry.reference_digest
            || point.expected_readback.vector_digests != entry.vector_digests
        {
            return Err(NominationSourceError::ProjectionMismatch);
        }
    }
    if entries.len() != points.len() || planned_units.len() != units.len() {
        return Err(NominationSourceError::ProjectionMismatch);
    }

    let mut seen_points = BTreeSet::new();
    let mut seen_units = BTreeSet::new();
    let mut mapped = Vec::with_capacity(nominations.len());
    for (ordinal, hint) in nominations.iter().enumerate() {
        if !seen_points.insert(hint.point_id) {
            return Err(NominationSourceError::DuplicateNomination);
        }
        let entry = entries
            .get(&hint.point_id)
            .ok_or(NominationSourceError::UnknownPoint)?;
        let point = points
            .get(&hint.point_id)
            .ok_or(NominationSourceError::UnknownPoint)?;
        let unit = units
            .get(&entry.unit_id)
            .ok_or(NominationSourceError::UnitSetMismatch)?;
        if entry.source_membership_id != membership.source_membership_id
            || hint.source_membership_id != entry.source_membership_id
        {
            return Err(NominationSourceError::MembershipMismatch);
        }
        if point.identity.point_id().as_bytes() != &hint.point_id
            || point.identity.full_digest() != hint.identity_digest
            || entry.point_identity_digest_256 != hint.identity_digest
            || point.expected_readback.payload_identity != hint.payload_digest
            || entry.payload_identity != hint.payload_digest
        {
            return Err(NominationSourceError::PointHintMismatch);
        }
        if !hint.raw_score.is_finite() {
            return Err(NominationSourceError::InvalidScore);
        }
        if !seen_units.insert(unit.unit_id()) {
            return Err(NominationSourceError::DuplicateNomination);
        }
        let candidate = CandidateNomination {
            request_id: ticket.request_id,
            plan_digest: ticket.plan_digest.0,
            leg_id: ticket.leg.leg_id,
            point_id: hint.point_id,
            source_membership_id: entry.source_membership_id,
            collection_generation_id: generation,
            valid_from_epoch: visible_epoch,
            valid_until_epoch_exclusive: Some(valid_until_epoch_exclusive),
            profile_id: profile_id.clone(),
            profile_digest,
            source_revision_id: revision.revision_id,
            representation_id: unit_set.representation().representation_id,
            unit_id: unit.unit_id(),
            unit_byte_start: unit.source_start(),
            unit_byte_end: unit.source_end(),
            expected_content_digest: revision.content_digest,
            expected_unit_digest: unit.unit_content_digest(),
            expected_excerpt_digest: None,
            expected_assurance: CandidateAssurance::Exact,
            expected_residency_digest: Blake3Digest32::from_bytes(
                *revision.object_residency_key_digest.as_bytes(),
            ),
            raw_score: hint.raw_score,
        };
        let mut candidate_context = validation.clone();
        candidate_context.candidate_ordinal = validation
            .candidate_ordinal
            .checked_add(ordinal)
            .ok_or(NominationSourceError::CandidateRejected)?;
        if precheck(&candidate, &candidate_context) != ValidationPrecheck::Proceed {
            return Err(NominationSourceError::CandidateRejected);
        }
        mapped.push(candidate);
    }
    Ok(mapped)
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::{BTreeMap, BTreeSet};
    use std::sync::atomic::AtomicBool;
    use std::time::{Duration, Instant};

    use search_access::{
        AccessCheckpoint, AccessPermit, IndexedRouteFence, LiveSecurityState, RequestSecurityFence,
        SafeRetrievalLeg,
    };
    use search_contracts::{
        AccessPartitionId, AccessPolicyBindingId, AccessPolicyRevision, BoundedList, BoundedName,
        CatalogRevision, CollectionGenerationId, CorpusId, Epoch, ExactnessRequirements,
        FusionProfileId, GrantFence, GrantId, InstallationIncarnationId, LegKind,
        MembershipRevision, MembershipRole, Modality, ObjectResidencyKeyDigest,
        ObservationCursorRevision, ObservationFreshness, ObservationFreshnessState, OpaqueId,
        OpaqueRef, OverlayRevision, PlanFingerprint, PlanId, PriorityClass, ProfileFence,
        ProjectionMembershipId, ProjectionProfileSetId, ProtocolVersion, PurgeFenceRevision,
        QueryExecutionBudget, QuerySnapshotFence, QuerySnapshotFingerprint, RequestId,
        RequiredDenominator, ResidencyPolicyBindingId, ScopeDomainId, ScoringPartitionId,
        SearchTaskPlan, ShadowFenceRevision, SourceId, SourceMembershipId, SourceNamespaceId,
        SourceOwnerFence, SourceOwnerGeneration, SourceRevisionId, SourceRevisionRef, SourceView,
        UtcTimestamp, WorkspaceId,
    };
    use search_epoch_pins::{PinLimits, PinRegistry};
    use search_projection_planner::{
        LineageObservation, PlannedUnit, PlannedVector, PlanningScope, ProjectionBudget,
        ProjectionMembershipBinding, plan_projection,
    };
    use search_query_planner::{CancellationBoundary, CompiledPlanDigest, LegBudget, PlannedLeg};
    use search_retrieval_executor::{LegTicket, acquire_leg_pins};

    const BODY: &[u8] = b"alpha\n";
    const MATERIALIZATION_BUDGET: search_materializer::api::MaterializationBudget =
        search_materializer::api::MaterializationBudget {
            max_input_bytes: 4096,
            max_output_bytes: 4096,
            max_lines: 512,
            max_map_segments: 512,
            max_loss_records: 512,
            max_steps: 65_536,
        };

    struct Fixture {
        nomination: IndexedNomination,
        task_plan: SearchTaskPlan,
        ticket: LegTicket,
        pins: LegPinSet,
        route: CollectionRoute,
        validation: ValidationContext,
        membership: SourceMembership,
        revision: SourceRevision,
        unit_set: VerifiedUnitSet,
        projection: ProjectionPlan,
    }

    impl Fixture {
        fn map(
            &self,
            nominations: &[IndexedNomination],
        ) -> Result<Vec<CandidateNomination>, NominationSourceError> {
            map_indexed_nominations(
                nominations,
                &self.task_plan,
                &self.ticket,
                &self.pins,
                &self.route,
                &self.validation,
                &self.membership,
                &self.revision,
                &self.unit_set,
                &self.projection,
            )
        }
    }

    fn fixture() -> Fixture {
        use search_contracts::{
            AcquisitionKind, Blake3Digest32, NonZeroRevision, ReceiptRef, RepresentationId,
            SourceMembership, SourceRevision, UnitKind,
        };
        use search_materializer::api::{
            AcceptedProfiles, BomPolicy, LossBehavior, MaterializationRequest, NewlinePolicy,
            SourceEncoding, SourceKind, baseline_profile_descriptor,
            validate_materialization_request, validate_materializer_profile,
        };
        use search_unitizer::{
            UnitizationBudget, UnitizationLimits, V3AnchorPolicy, V3AttachmentPolicy,
            V3EmptyPolicy, V3OmissionPolicy, V3OverlapPolicy, V3RepresentationKind,
            V3SourceBinding, V3UnitizerProfileDescriptor, validate_v3_unitizer_profile,
        };

        let source_namespace_id = SourceNamespaceId::from_bytes([1; 16]);
        let source_id = SourceId::from_bytes([2; 16]);
        let revision_id = SourceRevisionId::from_bytes([3; 16]);
        let materialization_id = search_contracts::MaterializationId::from_bytes([4; 16]);
        let representation_id = RepresentationId::from_bytes([5; 16]);
        let content_digest = search_contracts::blake3_exact_bytes(
            BODY,
            search_contracts::Blake3ExactInputLimit::new(BODY.len()).unwrap(),
        )
        .unwrap();
        let source_ref = SourceRevisionRef {
            source_namespace_id,
            source_id,
            revision_id,
            content_digest,
            byte_length: BODY.len() as u64,
        };
        let revision = SourceRevision {
            revision_id,
            source_id,
            occurrence_sequence: 7,
            content_digest,
            byte_length: BODY.len() as u64,
            observed_at: UtcTimestamp::parse("2026-10-16T12:00:00.000000Z").unwrap(),
            acquisition_kind: AcquisitionKind::Filesystem,
            stability_receipt_ref: ReceiptRef::new("nomination-test:stable-read").unwrap(),
            object_residency_key_digest: ObjectResidencyKeyDigest::from_bytes([6; 32]),
        };
        let membership = SourceMembership {
            source_membership_id: SourceMembershipId::from_bytes([7; 16]),
            corpus_id: CorpusId::from_bytes([8; 16]),
            source_id,
            workspace_id: WorkspaceId::from_bytes([9; 16]),
            role: MembershipRole::Source,
            preparation_profile_id: ProfileId::new("raw-v2-test-preparation").unwrap(),
            access_policy_binding_id: AccessPolicyBindingId::from_bytes([10; 16]),
            retention_policy_id: ProfileId::new("retention-test-v1").unwrap(),
            residency_policy_binding_id: ResidencyPolicyBindingId::from_bytes([11; 16]),
            membership_revision: MembershipRevision::new(1),
        };
        let binding = V3SourceBinding {
            source_namespace_id,
            source_id,
            source_revision_id: revision_id,
            materialization_id,
            representation_id,
        };
        let mut materializer_descriptor = baseline_profile_descriptor("raw-v2-test", 1);
        materializer_descriptor.source_kinds = vec![SourceKind::Text];
        materializer_descriptor.encodings = vec![SourceEncoding::Utf8];
        materializer_descriptor.bom_policy = BomPolicy::RejectWhenPresent;
        materializer_descriptor.newline_policy = NewlinePolicy::PreserveExact;
        materializer_descriptor.loss_behavior = LossBehavior::RecordAndLowerAssurance;
        let materializer = validate_materializer_profile(&materializer_descriptor).unwrap();
        let materialization_request = MaterializationRequest {
            source_id: OpaqueId::new(source_id.to_string()).unwrap(),
            revision: NonZeroRevision::new(7).unwrap(),
            residency: OpaqueId::new("nomination-test:retained-object").unwrap(),
            content_digest,
            byte_count: BODY.len() as u64,
            declared_kind: SourceKind::Text,
            declared_encoding: SourceEncoding::Utf8,
            profile_id: materializer.id(),
            operation_id: OpaqueId::new("nomination-test:materialize").unwrap(),
            from_unsaved_bytes: false,
            unsaved_snapshot_receipt: None,
        };
        let materialization_request = validate_materialization_request(
            &materialization_request,
            &AcceptedProfiles::new(vec![materializer.clone()]),
            &MATERIALIZATION_BUDGET,
        )
        .unwrap();
        let unitizer = validate_v3_unitizer_profile(&V3UnitizerProfileDescriptor {
            profile_name: ProfileId::new("nomination-test-unitizer-v3").unwrap(),
            profile_revision: 1,
            representation_kind: V3RepresentationKind::Text,
            unit_kind: UnitKind::File,
            boundary_revision: 1,
            overlap_policy: V3OverlapPolicy::None,
            anchor_policy: V3AnchorPolicy::ExactTextBytes,
            attachment_policy: V3AttachmentPolicy::Absent,
            omission_policy: V3OmissionPolicy::Forbidden,
            empty_policy: V3EmptyPolicy::Reject,
            limits: UnitizationLimits {
                max_input_bytes: 4096,
                preferred_unit_bytes: 64,
                max_unit_bytes: 128,
                max_lines: 512,
                max_units: 128,
            },
            min_unit_bytes: 1,
            max_unit_scalars: 128,
            max_unit_lines: 128,
            max_anchor_depth: 1,
            max_manifest_bytes: 65_536,
            max_steps: 65_536,
            fixture_digest: Blake3Digest32::from_bytes([12; 32]),
        })
        .unwrap();
        let cancelled = AtomicBool::new(false);
        let unitization_budget = UnitizationBudget {
            max_steps: 65_536,
            max_encoded_bytes: 65_536,
            deadline: Instant::now() + Duration::from_secs(30),
            cancelled: &cancelled,
        };
        let (_, unit_set) = crate::preparation_composition::raw_v2::prepare_raw_v2(
            &source_ref,
            revision.occurrence_sequence,
            BODY,
            &binding,
            &materialization_request,
            &materializer,
            &unitizer,
            MATERIALIZATION_BUDGET,
            &unitization_budget,
        )
        .unwrap();

        let epoch = Epoch::new(7).unwrap();
        let generation = CollectionGenerationId::from_bytes([13; 16]);
        let incarnation = InstallationIncarnationId::from_bytes([14; 16]);
        let projection_membership_id = ProjectionMembershipId::from_bytes([15; 16]);
        let profile_set_id = ProjectionProfileSetId::new("lexical-sparse-test-v1").unwrap();
        let projection_scope = PlanningScope {
            membership: ProjectionMembershipBinding {
                projection_membership_id,
                source_membership_id: membership.source_membership_id,
                source_id,
                source_revision_id: revision_id,
                representation_id: unit_set.representation().representation_id,
                projection_schema_id: ProfileId::new("lexical-sparse-test-v1").unwrap(),
                collection_generation_id: generation,
                installation_incarnation_id: incarnation,
                projection_profile_set_id: profile_set_id.clone(),
                access_partition_id: AccessPartitionId::from_bytes([16; 16]),
                scoring_partition_id: ScoringPartitionId::from_bytes([17; 16]),
            },
            valid_from_epoch: epoch,
            valid_until_epoch_exclusive: None,
            lineage: LineageObservation {
                repository_lineage_id: None,
            },
            identity_limits: search_point_identity::s11::DEFAULT_LIMITS,
        };
        let vector_name = search_contracts::indexed::VectorName::new("lexical-sparse").unwrap();
        let planned_units = unit_set
            .units()
            .iter()
            .map(|unit| PlannedUnit {
                unit_id: unit.unit_id(),
                unit_kind: unit.occurrence().unit_kind,
                modality: Modality::Text,
                language_or_format: BoundedName::new("text").unwrap(),
                entity_kind: None,
                normalized_symbol_key: None,
                unit_digest: unit.unit_content_digest(),
                reference_digest: unit.reference_digest(),
                vectors: vec![PlannedVector {
                    name: vector_name.clone(),
                    value: search_contracts::indexed::VectorValue::Sparse {
                        indices: vec![1],
                        values: vec![1.0],
                    },
                }],
            })
            .collect::<Vec<_>>();
        let requirements = BTreeMap::from([(
            vector_name.as_str().to_owned(),
            search_contracts::indexed::VectorRequirement::Sparse {
                index_ceiling: 8,
                idf_enabled: true,
            },
        )]);
        let projection = plan_projection(
            &unit_set,
            &projection_scope,
            &planned_units,
            &requirements,
            ProjectionBudget::BASELINE,
        )
        .unwrap();
        let entry = &projection.manifest.entries[0];

        let route_revision = CollectionRouteRevision::new(2);
        let source_owner_fence = SourceOwnerFence {
            source_namespace_id,
            source_owner_generation: SourceOwnerGeneration::from_bytes([18; 32]),
        };
        let task_plan = SearchTaskPlan {
            plan_id: PlanId::from_bytes([19; 16]),
            provider_protocol_version: ProtocolVersion { major: 1, minor: 0 },
            request_id: RequestId::from_bytes([20; 16]),
            recipe_request_digest: Blake3Digest32::from_bytes([21; 32]),
            grant_fence: GrantFence {
                grant_id: GrantId::from_bytes([22; 16]),
                revocation_generation: 1,
            },
            client_scope_fence: search_contracts::ClientScopeFence {
                client_scope_ref: search_contracts::OpaqueRef::new("nomination-test:scope")
                    .unwrap(),
                scope_domain_id: ScopeDomainId::from_bytes([23; 16]),
            },
            query_snapshot_fence: QuerySnapshotFence {
                installation_incarnation_id: incarnation,
                collection_generation_id: Some(generation),
                visible_epoch: Some(epoch),
                collection_route_revision: route_revision,
                catalog_revision: CatalogRevision::new(1),
                membership_revision: MembershipRevision::new(1),
                reference_portfolio_revision: None,
                access_policy_revision: AccessPolicyRevision::new(1),
                shadow_fence_revision: ShadowFenceRevision::new(1),
                purge_fence_revision: PurgeFenceRevision::new(1),
                overlay_revision: OverlayRevision::new(1),
                observation_cursor_revision: ObservationCursorRevision::new(1),
                observation_freshness: ObservationFreshness {
                    state: ObservationFreshnessState::CurrentConfirmed,
                    observation_cursor_revision: ObservationCursorRevision::new(1),
                    observed_age_ms: None,
                },
                source_view: SourceView::RetainedRevision(revision_id),
                workspace_view_revision_ref: None,
                lexical_profile_ids: BoundedList::empty(),
                snapshot_fingerprint: QuerySnapshotFingerprint::from_bytes([24; 32]),
            },
            source_owner_fences: BoundedList::new(vec![source_owner_fence]).unwrap(),
            selected_membership_ids: BoundedList::new(vec![membership.source_membership_id])
                .unwrap(),
            profile_fence: ProfileFence {
                fusion_profile_id: FusionProfileId::new("fusion-test-v1").unwrap(),
                projection_profile_set_ids: BoundedList::new(vec![profile_set_id]).unwrap(),
                optional_provider_profile_ids: BoundedList::empty(),
            },
            overlay_snapshot_refs: BoundedList::empty(),
            query_execution_budget: QueryExecutionBudget {
                priority_class: PriorityClass::Interactive,
                deadline_ms: 1000,
                max_scoring_legs: 1,
                max_prefetch_candidates_per_leg: 8,
                max_validated_candidates: 8,
                max_source_read_bytes: 4096,
                max_exact_scan_items: 0,
                max_exact_scan_bytes: 0,
                max_materialized_result_bytes: 1024,
                max_cpu_ms: 1000,
                max_memory_bytes: 1024 * 1024,
            },
            exactness_requirements: ExactnessRequirements {
                required_denominator: RequiredDenominator::CandidateScope,
                require_current_observation: true,
                allow_truthful_partial: true,
            },
            additional_state_dependencies: BoundedList::empty(),
            plan_fingerprint: PlanFingerprint::from_bytes([25; 32]),
            created_at: UtcTimestamp::parse("2026-10-16T12:00:00.000000Z").unwrap(),
            expires_at: UtcTimestamp::parse("2026-10-16T12:01:00.000000Z").unwrap(),
        };
        let route = CollectionRoute {
            generation,
            physical_name: OpaqueId::new("nomination-test:collection").unwrap(),
        };
        let member_set = BTreeSet::from([membership.source_membership_id]);
        let safe_leg = SafeRetrievalLeg {
            leg_id: 3,
            memberships: member_set.clone(),
            eligibility_plans: Vec::new(),
            route: IndexedRouteFence {
                collection_generation_id: generation,
                visible_epoch: epoch,
                route_generation: 2,
                owner_epoch: search_contracts::OwnerEpoch::new(1).unwrap(),
            },
            overlap_proof_digest: None,
        };
        let ticket = LegTicket {
            request_id: task_plan.request_id,
            plan_digest: CompiledPlanDigest([26; 32]),
            owner_id: OpaqueId::new("nomination-test:owner").unwrap(),
            leg: PlannedLeg {
                leg_id: 3,
                leg_kind: LegKind::Lexical,
                depends_on: Vec::new(),
                memberships: member_set.clone(),
                safe_index_leg: Some(safe_leg),
                budget: LegBudget {
                    deadline_ms: 1000,
                    max_candidates: 8,
                    max_source_read_bytes: 4096,
                    max_cpu_ms: 1000,
                    max_memory_bytes: 1024 * 1024,
                },
                cancellation_boundary: CancellationBoundary::BeforeEmission,
            },
            access_permit: AccessPermit {
                checkpoint: AccessCheckpoint::BeforeLegDispatch,
                live_generation: 2,
                live_snapshot_digest: Blake3Digest32::from_bytes([27; 32]),
            },
            issued_at_tick: 1,
        };
        let registry = PinRegistry::new(
            RouteIdentity {
                collection_generation_id: generation,
                route_revision,
            },
            epoch,
            PinLimits::BASELINE,
        )
        .unwrap();
        let pins = acquire_leg_pins(&ticket, &registry, 1).unwrap();
        let validation = ValidationContext {
            request_id: task_plan.request_id,
            plan_digest: ticket.plan_digest.0,
            collection_generation_id: generation,
            visible_epoch: epoch,
            allowed_memberships: member_set.clone(),
            shadowed_units: BTreeSet::new(),
            max_candidates: 8,
            candidate_ordinal: 0,
            request_security: RequestSecurityFence {
                planned_generation: 2,
                memberships: member_set.clone(),
            },
            live_security: LiveSecurityState {
                generation: 2,
                denied_memberships: BTreeSet::new(),
                purged_memberships: BTreeSet::new(),
                fail_closed: false,
                snapshot_digest: Blake3Digest32::from_bytes([28; 32]),
            },
            contaminated_legs: BTreeSet::new(),
        };
        let nomination = IndexedNomination {
            point_id: *entry.point_id.as_bytes(),
            source_membership_id: membership.source_membership_id,
            identity_digest: entry.point_identity_digest_256,
            payload_digest: entry.payload_identity,
            raw_score: 0.75,
        };
        Fixture {
            nomination,
            task_plan,
            ticket,
            pins,
            route,
            validation,
            membership,
            revision,
            unit_set,
            projection,
        }
    }

    #[test]
    fn maps_actual_raw_v3_unit_and_planner_control_manifest() {
        let fixture = fixture();
        let mapped = fixture.map(&[fixture.nomination]).unwrap();
        assert_eq!(mapped.len(), 1);
        let candidate = &mapped[0];
        let unit = &fixture.unit_set.units()[0];
        assert_eq!(candidate.request_id, fixture.ticket.request_id);
        assert_eq!(candidate.plan_digest, fixture.ticket.plan_digest.0);
        assert_eq!(
            candidate.source_membership_id,
            fixture.membership.source_membership_id
        );
        assert_eq!(candidate.source_revision_id, fixture.revision.revision_id);
        assert_eq!(
            candidate.representation_id,
            fixture.unit_set.representation().representation_id
        );
        assert_eq!(candidate.unit_id, unit.unit_id());
        assert_eq!(
            (candidate.unit_byte_start, candidate.unit_byte_end),
            (unit.source_start(), unit.source_end())
        );
        assert_eq!(
            candidate.expected_content_digest,
            fixture.revision.content_digest
        );
        assert_eq!(candidate.expected_unit_digest, unit.unit_content_digest());
        assert_eq!(candidate.expected_excerpt_digest, None);
        assert_eq!(candidate.expected_assurance, CandidateAssurance::Exact);
    }

    #[test]
    fn refuses_unknown_duplicate_crosssource_and_tampered_point_hints() {
        let fixture = fixture();
        let mut unknown = fixture.nomination;
        unknown.point_id = [0xEE; 16];
        assert_eq!(
            fixture.map(&[unknown]),
            Err(NominationSourceError::UnknownPoint)
        );

        assert_eq!(
            fixture.map(&[fixture.nomination, fixture.nomination]),
            Err(NominationSourceError::DuplicateNomination)
        );

        let mut wrong_member = fixture.nomination;
        wrong_member.source_membership_id = SourceMembershipId::from_bytes([0xEF; 16]);
        assert_eq!(
            fixture.map(&[wrong_member]),
            Err(NominationSourceError::MembershipMismatch)
        );

        let mut wrong_identity = fixture.nomination;
        wrong_identity.identity_digest = Blake3Digest32::from_bytes([0xF0; 32]);
        assert_eq!(
            fixture.map(&[wrong_identity]),
            Err(NominationSourceError::PointHintMismatch)
        );

        let mut wrong_payload = fixture.nomination;
        wrong_payload.payload_digest = Blake3Digest32::from_bytes([0xF1; 32]);
        assert_eq!(
            fixture.map(&[wrong_payload]),
            Err(NominationSourceError::PointHintMismatch)
        );
    }

    #[test]
    fn refuses_revision_route_incarnation_and_profile_drift() {
        let fixture = fixture();
        let mut foreign_revision = fixture.revision.clone();
        foreign_revision.source_id = SourceId::from_bytes([0xFA; 16]);
        assert_eq!(
            map_indexed_nominations(
                &[fixture.nomination],
                &fixture.task_plan,
                &fixture.ticket,
                &fixture.pins,
                &fixture.route,
                &fixture.validation,
                &fixture.membership,
                &foreign_revision,
                &fixture.unit_set,
                &fixture.projection,
            ),
            Err(NominationSourceError::SourceMismatch)
        );

        let mut stale_plan = fixture.task_plan.clone();
        stale_plan.query_snapshot_fence.installation_incarnation_id =
            InstallationIncarnationId::from_bytes([0xFB; 16]);
        assert_eq!(
            map_indexed_nominations(
                &[fixture.nomination],
                &stale_plan,
                &fixture.ticket,
                &fixture.pins,
                &fixture.route,
                &fixture.validation,
                &fixture.membership,
                &fixture.revision,
                &fixture.unit_set,
                &fixture.projection,
            ),
            Err(NominationSourceError::ContextMismatch)
        );

        let mut wrong_route = fixture.route.clone();
        wrong_route.generation = CollectionGenerationId::from_bytes([0xFC; 16]);
        assert_eq!(
            map_indexed_nominations(
                &[fixture.nomination],
                &fixture.task_plan,
                &fixture.ticket,
                &fixture.pins,
                &wrong_route,
                &fixture.validation,
                &fixture.membership,
                &fixture.revision,
                &fixture.unit_set,
                &fixture.projection,
            ),
            Err(NominationSourceError::ContextMismatch)
        );

        let mut wrong_profile = fixture.task_plan.clone();
        wrong_profile.profile_fence.projection_profile_set_ids = BoundedList::empty();
        assert_eq!(
            map_indexed_nominations(
                &[fixture.nomination],
                &wrong_profile,
                &fixture.ticket,
                &fixture.pins,
                &fixture.route,
                &fixture.validation,
                &fixture.membership,
                &fixture.revision,
                &fixture.unit_set,
                &fixture.projection,
            ),
            Err(NominationSourceError::ProjectionMismatch)
        );
    }
}

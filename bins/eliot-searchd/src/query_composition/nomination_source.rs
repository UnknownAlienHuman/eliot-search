//! Pure source-metadata mapping for indexed nominations.
//!
//! Backend point and digest values remain hints. A result is formed only after
//! those hints bind to the exact planner manifest, the actual source controls,
//! the verified raw-v3 unit set, and the current request/leg/route pins.

use std::collections::{BTreeMap, BTreeSet};

use search_access::{AuthorizedScope, SafeRetrievalLeg, compile_base_eligibility};
use search_candidate_validator::{
    CandidateAssurance, CandidateNomination, ValidationContext, ValidationPrecheck, precheck,
};
use search_contracts::indexed::{EligibilityPopulation, PointPayload};
use search_contracts::{
    Blake3Digest32, CollectionRouteRevision, NativeAnchor, SearchTaskPlan, SourceRevision,
    SourceRevisionRef, SourceView, UnitId,
};
use search_epoch_pins::RouteIdentity;
use search_projection_planner::{ProjectionManifestEntry, ProjectionPlan};
use search_qdrant_bridge::CollectionRoute;
use search_retrieval_executor::{LegPinSet, LegTicket, indexed::IndexedNomination};
use search_source_registry::SourceRegistry;
use search_unitizer::{
    V3AnchorPolicy, V3AttachmentPolicy, V3OmissionPolicy, V3OverlapPolicy, VerifiedUnitSet,
};

/// Closed refusal from the pure nomination mapper.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum NominationSourceError {
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
/// The caller must retain this same control-owned `expected_revision` for the
/// canonical Native readback; a nomination alone does not carry its namespace.
#[allow(clippy::too_many_arguments, clippy::too_many_lines)]
pub fn map_indexed_nominations(
    nominations: &[IndexedNomination],
    task_plan: &SearchTaskPlan,
    ticket: &LegTicket,
    pins: &LegPinSet,
    route: &CollectionRoute,
    validation: &ValidationContext,
    registry: &SourceRegistry,
    expected_revision: &SourceRevisionRef,
    scope: &AuthorizedScope,
    revision: &SourceRevision,
    unit_set: &VerifiedUnitSet,
    projection: &ProjectionPlan,
) -> Result<Vec<CandidateNomination>, NominationSourceError> {
    if nominations.len() > search_retrieval_executor::indexed::MAX_INDEXED_LIMIT {
        return Err(NominationSourceError::ContextMismatch);
    }
    let member = projection.source_membership_id;
    // The C03 owner checks permanent admission, Active lifecycle and every
    // current Root/namespace/source fence, including nonfenced ownership.
    let (membership, identity) = registry
        .active_source_membership_v2(member)
        .map_err(|_| NominationSourceError::MembershipMismatch)?;
    let namespace = registry
        .namespaces()
        .get(&identity.source_namespace_id)
        .ok_or(NominationSourceError::ContextMismatch)?;
    let mut owner_fences = task_plan
        .source_owner_fences
        .iter()
        .filter(|fence| fence.source_namespace_id == identity.source_namespace_id);
    let owner_fence = owner_fences
        .next()
        .ok_or(NominationSourceError::ContextMismatch)?;
    if owner_fences.next().is_some()
        || owner_fence.source_owner_generation != namespace.ownership.source_owner_generation
        || namespace.ownership.source_namespace_id != identity.source_namespace_id
        || scope.source_catalog_generation != registry.revision()
        || task_plan.query_snapshot_fence.catalog_revision.get() != registry.revision()
    {
        return Err(NominationSourceError::ContextMismatch);
    }
    let source_binding = unit_set.manifest().provenance().binding();
    if membership.source_id != revision.source_id
        || source_binding.source_id != identity.source_id
        || identity.source_id != revision.source_id
        || identity.source_namespace_id != source_binding.source_namespace_id
        || expected_revision.source_id != identity.source_id
        || expected_revision.source_namespace_id != identity.source_namespace_id
    {
        return Err(NominationSourceError::SourceMismatch);
    }
    let retained_revision = (
        revision.revision_id,
        revision.content_digest,
        revision.byte_length,
    );
    let expected_coordinates = (
        expected_revision.revision_id,
        expected_revision.content_digest,
        expected_revision.byte_length,
    );
    if source_binding.source_revision_id != expected_revision.revision_id {
        return Err(NominationSourceError::RevisionMismatch);
    }
    if expected_coordinates != retained_revision
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
        .safe_index_leg()
        .ok_or(NominationSourceError::ContextMismatch)?;
    validate_eligibility(safe_leg, scope, projection, task_plan, ticket)?;
    if ticket.request_id != task_plan.request_id
        || ticket.request_id != validation.request_id
        || *ticket.plan_digest.as_bytes() != validation.plan_digest
        || ticket.plan_digest.as_bytes() != task_plan.plan_fingerprint.as_bytes()
        || safe_leg.leg_id != ticket.leg.leg_id()
        || &safe_leg.memberships != ticket.leg.memberships()
        || !ticket.leg.memberships().contains(&member)
        || !validation.request_security.memberships.contains(&member)
        || !validation.allowed_memberships.contains(&member)
        || !task_plan
            .selected_membership_ids
            .iter()
            .any(|id| *id == member)
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
        || (route.generation, safe_leg.route.visible_epoch)
            != (pin_route.collection_generation_id, visible_epoch)
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
    if projection.points.len() != unit_set.units().len()
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
    let mut payloads = BTreeMap::<[u8; 16], (PointPayload, Blake3Digest32)>::new();
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
        {
            return Err(NominationSourceError::ProjectionMismatch);
        }
        let entry = entries
            .get(&point_id)
            .ok_or(NominationSourceError::ProjectionMismatch)?;
        let unit = units
            .get(&entry.unit_id)
            .ok_or(NominationSourceError::UnitSetMismatch)?;
        let payload =
            PointPayload::from_canonical_cbor(point.expected_readback.payload_cbor.as_slice())
                .map_err(|_| NominationSourceError::ProjectionMismatch)?;
        let payload_identity = payload
            .identity()
            .map_err(|_| NominationSourceError::ProjectionMismatch)?;
        // Shared C00 semantics compare actual UUID partitions, profile and
        // projection membership, then apply the stored half-open epoch range.
        let population = EligibilityPopulation::new(
            entry.installation_incarnation_id,
            entry.collection_generation_id,
            &[entry.projection_membership_id],
            entry.access_partition_id,
            entry.scoring_partition_id,
            entry.projection_profile_set_id.clone(),
            visible_epoch,
        )
        .map_err(|_| NominationSourceError::ProjectionMismatch)?;
        let payload_source = (
            payload.source_id,
            payload.source_revision_id,
            payload.representation_id,
            payload.unit_id,
            payload.unit_kind,
        );
        let expected_source = (
            expected_revision.source_id,
            expected_revision.revision_id,
            unit_set.representation().representation_id,
            unit.unit_id(),
            unit.occurrence().unit_kind,
        );
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
            || payload_identity != entry.payload_identity
            || payload_source != expected_source
            || payload.point_identity_digest_256 != point.identity.full_digest()
            || payload.scoring_document_id != entry.scoring_document_id
            || !population.matches(&payload)
            || payloads
                .insert(point_id, (payload, payload_identity))
                .is_some()
        {
            return Err(NominationSourceError::ProjectionMismatch);
        }
    }
    if entries.len() != payloads.len() || planned_units.len() != units.len() {
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
        let (payload, payload_identity) = payloads
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
        if payload.point_identity_digest_256 != hint.identity_digest
            || entry.point_identity_digest_256 != hint.identity_digest
            || *payload_identity != hint.payload_digest
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
            plan_digest: *ticket.plan_digest.as_bytes(),
            leg_id: ticket.leg.leg_id(),
            point_id: hint.point_id,
            source_membership_id: entry.source_membership_id,
            collection_generation_id: generation,
            valid_from_epoch: payload.valid_from_epoch,
            valid_until_epoch_exclusive: payload.valid_until_epoch_exclusive,
            profile_id: profile_id.clone(),
            profile_digest,
            source_revision_id: expected_revision.revision_id,
            representation_id: unit_set.representation().representation_id,
            unit_id: unit.unit_id(),
            unit_byte_start: unit.source_start(),
            unit_byte_end: unit.source_end(),
            expected_content_digest: expected_revision.content_digest,
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

fn validate_eligibility(
    leg: &SafeRetrievalLeg,
    scope: &AuthorizedScope,
    projection: &ProjectionPlan,
    task_plan: &SearchTaskPlan,
    ticket: &LegTicket,
) -> Result<(), NominationSourceError> {
    let member = projection.source_membership_id;
    let [eligibility] = leg.eligibility_plans.as_slice() else {
        return Err(NominationSourceError::ContextMismatch);
    };
    let binding = scope
        .memberships
        .get(&member)
        .ok_or(NominationSourceError::ContextMismatch)?;
    // Partition digests stay in the access owner's domain. Only the actual
    // projection UUID is compared with its lossless canonical opaque spelling.
    if binding.membership_id != member
        || leg.memberships.len() != 1
        || !leg.memberships.contains(&member)
        || binding.projection_membership_id.as_str()
            != projection.projection_membership_id.to_string()
        || *eligibility
            != compile_base_eligibility(
                binding,
                leg.route,
                ticket.access_permit.live_generation,
                task_plan.query_snapshot_fence.shadow_fence_revision.get(),
                task_plan.query_snapshot_fence.purge_fence_revision.get(),
            )
            .map_err(|_| NominationSourceError::ContextMismatch)?
    {
        return Err(NominationSourceError::ContextMismatch);
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::{BTreeMap, BTreeSet};
    use std::sync::atomic::AtomicBool;
    use std::time::{Duration, Instant};

    use search_access::{
        AccessCheckpoint, AccessPermit, AuthoritativeAccessSnapshot, IndexedRouteFence,
        LiveSecurityState, MembershipAccessBinding, RequestSecurityFence, RequestedMembershipScope,
        compile_safe_legs, intersect_scope,
    };
    use search_contracts::{
        AccessPartitionId, AccessPolicyBindingId, AccessPolicyRevision, BoundedList, BoundedName,
        CatalogRevision, CollectionGenerationId, CorpusId, Epoch, ExactnessRequirements,
        FusionProfileId, GrantFence, GrantId, InstallationIncarnationId, LegKind,
        MembershipRevision, MembershipRole, Modality, ObjectResidencyKeyDigest,
        ObservationCursorRevision, ObservationFreshness, ObservationFreshnessState, OpaqueId,
        OpaqueRef, OverlayRevision, PlanFingerprint, PlanId, PriorityClass, ProfileFence,
        ProfileId, ProjectionMembershipId, ProjectionProfileSetId, ProtocolVersion,
        PurgeFenceRevision, QueryExecutionBudget, QuerySnapshotFence, QuerySnapshotFingerprint,
        RequestId, RequiredDenominator, ResidencyPolicyBindingId, ScopeDomainId,
        ScoringPartitionId, SearchTaskPlan, ShadowFenceRevision, SourceId, SourceMembership,
        SourceMembershipId, SourceNamespaceId, SourceOwnerFence, SourceOwnerGeneration,
        SourceRevisionId, SourceRevisionRef, SourceView, UtcTimestamp, WorkspaceId,
    };
    use search_epoch_pins::{PinLimits, PinRegistry};
    use search_projection_planner::{
        LineageObservation, PlannedUnit, PlannedVector, PlanningScope, ProjectionBudget,
        ProjectionMembershipBinding, plan_projection,
    };
    use search_query_planner::{
        AdvancedCapabilities, CapabilitySet, RetrievalCapabilities, canonical_execution_dependency,
        compile_canonical_plan, normalize_recipe,
    };
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

    // Pure C03 owner fixture: real admission, identity and membership APIs over
    // the owner's bounded in-memory journal; no Native/durability qualification.
    mod c03 {
        use super::*;
        use search_contracts::{NonZeroRevision, OwnerEpoch, ReceiptRef, RootBindingId};
        use search_ports::{
            FakeCancellation, MutationIdentity, OperationContext, Port, PortReceipt,
        };
        use search_source_admission::policy_v2::{
            DEFAULT_COMPILE_LIMITS, POLICY_SCHEMA_VERSION, PolicyInputV2, PredicateV2, RuleInputV2,
            compile_policy_v2,
        };
        use search_source_admission::{
            AcceptedCapabilities, BASELINE_PROFILE, RuleEffect, SectionPlatform, SourceClass,
            UnvalidatedObservationInput, compiled_defaults, validate_section,
        };
        use search_source_identity::{
            CanonicalRelativePath, CaseBehavior, DEFAULT_IDENTITY_LIMITS,
            FilesystemIdentityProfile, IdentityObservation, LinkBehavior, ObservationConfidence,
            PathObservation, ReparseBehavior, SourceObservation, StableFieldPolicy,
            StableIdentityEvidence, StableIdentityKey, UnicodeBehavior, derive_canonical_path_key,
            validate_identity_observation,
        };
        use search_source_provenance::{OriginalAdmissionV2, RetainedPolicyV2};
        use search_source_registry::api::{
            self, SourceAdmissionPreparationV2, SourceAdmissionRequestV2,
            SourceNamespaceSelectionV2, SourceOwnerScopeV2,
        };
        use search_source_registry::error::{
            InMemoryRegistryJournal, RegistryControlPort, RegistryPortError,
        };
        use search_source_registry::recovery::{
            PreparedBatch, RegistryBatchLookup, RegistryBatchReadback,
        };
        use search_source_registry::source_ids::SourceIdentityIdPortV2;
        use std::sync::Arc;

        struct Control(InMemoryRegistryJournal<FakeCancellation>);
        impl Port for Control {
            type Error = RegistryPortError;
            type Cancellation = FakeCancellation;
        }
        impl RegistryControlPort for Control {
            fn commit_batch(
                &mut self,
                batch: &PreparedBatch,
                ctx: &OperationContext<FakeCancellation>,
                mutation: &MutationIdentity,
            ) -> Result<RegistryBatchReadback, Self::Error> {
                self.0.commit_batch(batch, ctx, mutation)
            }
            fn load_batch(
                &self,
                id: &OpaqueId,
                ctx: &OperationContext<FakeCancellation>,
            ) -> Result<RegistryBatchLookup, Self::Error> {
                self.0.load_batch(id, ctx)
            }
            fn persist_entry(
                &mut self,
                entry: &search_source_registry::RegistryJournalEntry,
                ctx: &OperationContext<FakeCancellation>,
                mutation: &MutationIdentity,
            ) -> Result<PortReceipt, Self::Error> {
                self.0.persist_entry(entry, ctx, mutation)
            }
            fn load_entry(
                &self,
                id: &OpaqueId,
                ctx: &OperationContext<FakeCancellation>,
            ) -> Result<Option<search_source_registry::RegistryJournalEntry>, Self::Error>
            {
                self.0.load_entry(id, ctx)
            }
        }
        fn uuid(value: u8) -> [u8; 16] {
            let mut bytes = [value; 16];
            bytes[6] = 0x40;
            bytes[8] = 0x80;
            bytes
        }
        impl SourceIdentityIdPortV2 for Control {
            fn fresh_source_id(
                &mut self,
                ctx: &OperationContext<FakeCancellation>,
            ) -> Result<SourceId, Self::Error> {
                ctx.preflight().unwrap();
                Ok(SourceId::from_bytes(uuid(2)))
            }
            fn fresh_source_namespace_id(
                &mut self,
                ctx: &OperationContext<FakeCancellation>,
            ) -> Result<SourceNamespaceId, Self::Error> {
                ctx.preflight().unwrap();
                Ok(SourceNamespaceId::from_bytes(uuid(1)))
            }
        }
        fn rev(value: u64) -> NonZeroRevision {
            NonZeroRevision::new(value).unwrap()
        }
        fn opaque(value: &str) -> OpaqueId {
            OpaqueId::new(value).unwrap()
        }
        fn receipt(value: &str) -> ReceiptRef {
            ReceiptRef::new(value).unwrap()
        }
        fn digest(value: u8) -> Blake3Digest32 {
            Blake3Digest32::from_bytes([value; 32])
        }

        pub(super) struct Admitted {
            pub registry: SourceRegistry,
            pub member: SourceMembership,
            control: Control,
            context: OperationContext<FakeCancellation>,
        }
        impl Admitted {
            pub fn restrict(&mut self) {
                api::transition_source_membership_v2(
                    &mut self.registry,
                    self.member.source_membership_id,
                    self.member.membership_revision,
                    search_source_registry::membership::MembershipCommand::Restrict,
                    &opaque("member:restrict"),
                    digest(94),
                    &receipt("member:restricted"),
                    &mut self.control,
                    &self.context,
                )
                .unwrap();
            }
        }
        pub(super) fn admit(content_digest: Blake3Digest32, byte_length: u64) -> Admitted {
            let platform = SectionPlatform {
                os: "windows".into(),
                arch: "x64".into(),
            };
            let capabilities = AcceptedCapabilities {
                profiles: vec![BASELINE_PROFILE.into()],
            };
            let section = validate_section(&compiled_defaults(), &platform, &capabilities).unwrap();
            let policy = compile_policy_v2(
                &PolicyInputV2 {
                    schema_version: POLICY_SCHEMA_VERSION,
                    revision: 1,
                    section,
                    rules: vec![RuleInputV2 {
                        id: "nomination-fixture".into(),
                        precedence: 0,
                        source_class: SourceClass::Regular,
                        predicate: PredicateV2::Always,
                        effect: RuleEffect::Allow,
                        unknown_fields: vec![],
                    }],
                    unknown_fields: vec![],
                },
                DEFAULT_COMPILE_LIMITS,
                &mut || Ok(()),
            )
            .unwrap();
            let policy = Arc::new(
                RetainedPolicyV2::capture(&policy, &platform, &capabilities, &mut || Ok(()))
                    .unwrap(),
            );
            let observation =
                search_source_admission::original_inputs_v2::reconstitute_observation_v2(
                    &UnvalidatedObservationInput {
                        locator_class: "normalized-file".into(),
                        location_class: "local-fixed".into(),
                        source_kind: "file".into(),
                        source_class: "regular".into(),
                        byte_size: Some(byte_length),
                        is_generated: Some(false),
                        is_vendor: Some(false),
                        is_binary: Some(false),
                        is_system: Some(false),
                        sensitivity: Some("public".into()),
                        detector_id: Some("nomination-fixture".into()),
                        profile_id: Some(BASELINE_PROFILE.into()),
                        unavailable_fields: vec![],
                        unknown_fields: vec![],
                    },
                    Some(b"src/lib.rs"),
                    policy.compiled(),
                    search_source_admission::observation_v2::MAX_OBSERVATION_PREIMAGE_BYTES,
                    &mut || Ok(()),
                )
                .unwrap();
            let original =
                OriginalAdmissionV2::retain(policy, &observation, &mut || Ok(())).unwrap();
            let context = OperationContext::new(
                RequestId::from_bytes([40; 16]),
                60_000,
                FakeCancellation::new(false),
                OpaqueRef::new("budget:nomination-fixture").unwrap(),
            )
            .unwrap();
            let mut control = Control(InMemoryRegistryJournal::new(32).unwrap());
            let mut registry = SourceRegistry::new(api::DEFAULT_REGISTRY_LIMITS).unwrap();
            let root = RootBindingId::from_bytes([30; 16]);
            let owner = SourceOwnerScopeV2 {
                system_id: opaque("owner:nomination-fixture"),
                installation_incarnation: InstallationIncarnationId::from_bytes([14; 16]),
                epoch: OwnerEpoch::new(1).unwrap(),
            };
            api::register_root(
                &mut registry,
                &api::RegisterRootRequest {
                    root_binding_id: root,
                    canonical_root_digest: digest(31),
                    policy_fingerprint: Blake3Digest32::from_bytes(
                        *original
                            .receipt()
                            .decision()
                            .policy_fingerprint()
                            .as_bytes(),
                    ),
                    policy_revision: original.receipt().decision().policy_revision(),
                    owner_epoch: owner.epoch,
                    owner_fence_receipt: receipt("root:owner"),
                    policy_receipt: receipt("root:policy"),
                },
                &opaque("root:register"),
                digest(32),
                &receipt("root:registered"),
                &mut control,
                &context,
            )
            .unwrap();
            let profile = FilesystemIdentityProfile::new(
                rev(1),
                digest(33),
                CaseBehavior::Sensitive,
                UnicodeBehavior::PreserveScalarValues,
                StableFieldPolicy::Required,
                StableFieldPolicy::Required,
                LinkBehavior::StablePhysicalIdentity,
                ReparseBehavior::FinalTargetIdentity,
            )
            .unwrap();
            let path_key = derive_canonical_path_key(
                &PathObservation {
                    root_binding_id: root,
                    root_relative_lookup_path: "src/lib.rs".into(),
                    profile_revision: profile.revision(),
                    profile_schema_digest: profile.schema_digest(),
                    normalization_attested: true,
                },
                profile,
            )
            .unwrap();
            let request = SourceAdmissionRequestV2 {
                expected_registry_revision: registry.revision(),
                root_binding_id: root,
                expected_root_revision: rev(1),
                namespace: SourceNamespaceSelectionV2::CreateNew,
                owner: owner.clone(),
                identity_profile: profile,
                identity_observation: validate_identity_observation(
                    IdentityObservation {
                        path_key,
                        stable_evidence: StableIdentityEvidence::Exact(
                            StableIdentityKey::Filesystem {
                                volume_identity: digest(34),
                                file_identity: digest(35),
                                generation: Some(1),
                            },
                        ),
                        content_digest_hint: Some(content_digest),
                        metadata_generation: Some(1),
                        byte_length_hint: Some(byte_length),
                        claimed_source_id: None,
                        candidate_catalog_revision: CatalogRevision::new(registry.revision()),
                        profile_revision: profile.revision(),
                        profile_schema_digest: profile.schema_digest(),
                        confidence: ObservationConfidence::Exact,
                        evidence_digest: digest(36),
                    },
                    profile,
                )
                .unwrap(),
                binding_observation: SourceObservation {
                    root_binding_id: root,
                    relative_path: CanonicalRelativePath::new(
                        "src/lib.rs",
                        DEFAULT_IDENTITY_LIMITS,
                    )
                    .unwrap(),
                    stable_file_identity_digest: Some(digest(35)),
                    content_digest,
                    content_bytes: byte_length,
                    observation_receipt: receipt("source:observed"),
                },
                original,
                expected_source_revision: None,
                operation_id: opaque("source:admit"),
                receipt: receipt("source:admitted"),
            };
            let SourceAdmissionPreparationV2::Prepared(prepared) =
                api::prepare_source_admission_v2(
                    &mut registry,
                    &request,
                    &mut control,
                    &context,
                    &mut || Ok(()),
                )
                .unwrap()
            else {
                panic!("fresh source");
            };
            let admitted = api::commit_source_admission_v2(
                &mut registry,
                &prepared,
                &mut control,
                &context,
                &mut || Ok(()),
            )
            .unwrap();
            let identity = admitted.source_identity;
            let namespace = &registry.namespaces()[&identity.source_namespace_id].ownership;
            let namespace = SourceNamespaceSelectionV2::Existing {
                id: identity.source_namespace_id,
                revision: namespace.ownership_record_revision,
                generation: namespace.source_owner_generation,
            };
            let member = SourceMembership {
                source_membership_id: SourceMembershipId::from_bytes([7; 16]),
                corpus_id: CorpusId::from_bytes([8; 16]),
                source_id: identity.source_id,
                workspace_id: WorkspaceId::from_bytes([9; 16]),
                role: MembershipRole::Source,
                preparation_profile_id: ProfileId::new("raw-v2-test-preparation").unwrap(),
                access_policy_binding_id: AccessPolicyBindingId::from_bytes([10; 16]),
                retention_policy_id: ProfileId::new("retention-test-v1").unwrap(),
                residency_policy_binding_id: ResidencyPolicyBindingId::from_bytes([11; 16]),
                membership_revision: MembershipRevision::new(1),
            };
            api::bind_source_membership_v2(
                &mut registry,
                &member,
                root,
                rev(1),
                namespace,
                &owner,
                admitted.source_revision,
                &opaque("member:bind"),
                digest(37),
                &receipt("member:bound"),
                &mut control,
                &context,
            )
            .unwrap();
            assert_eq!(
                registry
                    .active_source_membership_v2(member.source_membership_id)
                    .unwrap()
                    .1,
                &identity
            );
            Admitted {
                registry,
                member,
                control,
                context,
            }
        }
    }

    struct Fixture {
        nomination: IndexedNomination,
        task_plan: SearchTaskPlan,
        ticket: LegTicket,
        pins: LegPinSet,
        route: CollectionRoute,
        validation: ValidationContext,
        membership: SourceMembership,
        source: c03::Admitted,
        expected_revision: SourceRevisionRef,
        scope: AuthorizedScope,
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
                &self.source.registry,
                &self.expected_revision,
                &self.scope,
                &self.revision,
                &self.unit_set,
                &self.projection,
            )
        }

        fn stored_payload(&self) -> PointPayload {
            PointPayload::from_canonical_cbor(
                self.projection.points[0]
                    .expected_readback
                    .payload_cbor
                    .as_slice(),
            )
            .unwrap()
        }

        // Negative fixtures echo the owner's recomputed payload commitment so
        // rejection must also bind decoded fields to the real source/manifest.
        fn replace_payload(&mut self, payload: &PointPayload) {
            let identity = payload.identity().unwrap();
            let expected = &mut self.projection.points[0].expected_readback;
            expected.payload_cbor = payload.to_canonical_cbor().unwrap();
            expected.payload_identity = identity;
            self.projection.manifest.entries[0].payload_identity = identity;
            self.nomination.payload_digest = identity;
        }
    }

    fn canonical_ticket(
        task: &mut SearchTaskPlan,
        scope: &AuthorizedScope,
        safe_leg: SafeRetrievalLeg,
        projection: &ProjectionPlan,
    ) -> LegTicket {
        use search_contracts::{
            BindingId, BoundedSet, DisclosureCeiling, InstallationId, LocateRecipe,
            NormalizedNameSelector, RecipeBodyV1, RecipeIdV1, RequestedScope,
            SearchReadGrantClaims, SearchRecipeRequest, SensitivityClass, SubjectSelector,
        };
        let request = SearchRecipeRequest::new(
            task.request_id,
            RecipeIdV1::Locate,
            task.query_snapshot_fence.source_view.clone(),
            RequestedScope::ExplicitMemberships(task.selected_membership_ids.clone()),
            ProfileId::new("interactive").unwrap(),
            RecipeBodyV1::Locate(LocateRecipe {
                subject: SubjectSelector::NormalizedName(NormalizedNameSelector {
                    name: BoundedName::new("alpha").unwrap(),
                    entity_kinds: BoundedSet::empty(),
                }),
                evidence_roles: BoundedSet::empty(),
            }),
        )
        .unwrap();
        let grant = SearchReadGrantClaims {
            grant_id: task.grant_fence.grant_id,
            installation_id: InstallationId::from_bytes([40; 16]),
            installation_incarnation_id: task.query_snapshot_fence.installation_incarnation_id,
            binding_id: BindingId::from_bytes([41; 16]),
            principal_opaque_id: OpaqueId::new("principal:fixture").unwrap(),
            client_scope_ref: task.client_scope_fence.client_scope_ref.clone(),
            scope_domain_id: task.client_scope_fence.scope_domain_id,
            allowed_membership_ids: BoundedSet::from_items(scope.memberships.keys().copied())
                .unwrap(),
            allowed_corpus_or_portfolio_ids: BoundedSet::empty(),
            reference_portfolio_revision: None,
            allowed_access_partitions: BoundedSet::from_items([
                projection.manifest.entries[0].access_partition_id
            ])
            .unwrap(),
            allowed_modalities: BoundedSet::from_items([Modality::Text]).unwrap(),
            permitted_recipe_families: BoundedSet::from_items([RecipeIdV1::Locate]).unwrap(),
            maximum_budget_class: request.requested_budget_class.clone(),
            sensitivity_ceiling: SensitivityClass::Public,
            disclosure_ceiling: DisclosureCeiling::LocalOnly,
            source_read_permission: true,
            exact_scan_permission: false,
            issued_boot_id: OpaqueId::new("boot:fixture").unwrap(),
            issued_at: task.created_at.clone(),
            expires_at: task.expires_at.clone(),
            nonce: OpaqueId::new("nonce:fixture").unwrap(),
            revocation_generation: task.grant_fence.revocation_generation,
        };
        let capabilities = CapabilitySet {
            retrieval: RetrievalCapabilities {
                direct: false,
                lexical: true,
                exact: false,
            },
            advanced: AdvancedCapabilities {
                structural: false,
                semantic: false,
                rerank: false,
            },
        };
        let safe_legs = [safe_leg];
        task.recipe_request_digest = request.canonical_request_digest().unwrap();
        task.additional_state_dependencies = BoundedList::new(vec![
            canonical_execution_dependency(capabilities, scope, &safe_legs).unwrap(),
        ])
        .unwrap();
        let fingerprint = |bytes: &[u8]| {
            *search_contracts::blake3_exact_bytes(
                bytes,
                search_contracts::Blake3ExactInputLimit::new(bytes.len()).unwrap(),
            )
            .unwrap()
            .as_bytes()
        };
        task.query_snapshot_fence.snapshot_fingerprint =
            QuerySnapshotFingerprint::from_bytes(fingerprint(
                task.query_snapshot_fence
                    .canonical_fingerprint_input()
                    .unwrap()
                    .as_slice(),
            ));
        task.plan_fingerprint = PlanFingerprint::from_bytes(fingerprint(
            task.canonical_fingerprint_input().unwrap().as_slice(),
        ));
        let compiled = compile_canonical_plan(
            task,
            &grant,
            &normalize_recipe(request).unwrap(),
            scope.clone(),
            capabilities,
            &safe_legs,
        )
        .unwrap();
        LegTicket {
            request_id: compiled.request_id(),
            plan_digest: compiled.digest(),
            owner_id: OpaqueId::new("nomination-test:owner").unwrap(),
            issued_at_tick: 1,
            leg: compiled
                .legs()
                .iter()
                .find(|leg| leg.leg_kind() == LegKind::Lexical)
                .unwrap()
                .clone(),
            access_permit: AccessPermit {
                checkpoint: AccessCheckpoint::BeforeLegDispatch,
                live_generation: 2,
                live_snapshot_digest: scope.snapshot_digest,
            },
        }
    }

    fn fixture() -> Fixture {
        fixture_with_validity(Epoch::new(1).unwrap(), None)
    }

    fn fixture_with_validity(valid_from: Epoch, valid_until: Option<Epoch>) -> Fixture {
        use search_contracts::{
            AcquisitionKind, Blake3Digest32, NonZeroRevision, ReceiptRef, RepresentationId,
            SourceRevision, UnitKind,
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

        let revision_id = SourceRevisionId::from_bytes([3; 16]);
        let materialization_id = search_contracts::MaterializationId::from_bytes([4; 16]);
        let representation_id = RepresentationId::from_bytes([5; 16]);
        let content_digest = search_contracts::blake3_exact_bytes(
            BODY,
            search_contracts::Blake3ExactInputLimit::new(BODY.len()).unwrap(),
        )
        .unwrap();
        let source = c03::admit(content_digest, BODY.len() as u64);
        let membership = source.member.clone();
        let identity = source
            .registry
            .active_source_membership_v2(membership.source_membership_id)
            .unwrap()
            .1;
        let source_namespace_id = identity.source_namespace_id;
        let source_id = identity.source_id;
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

        let epoch = Epoch::new(5).unwrap();
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
                projection_schema_id: ProfileId::new("nomination-test-schema-v1").unwrap(),
                collection_generation_id: generation,
                installation_incarnation_id: incarnation,
                projection_profile_set_id: profile_set_id.clone(),
                access_partition_id: AccessPartitionId::from_bytes([16; 16]),
                scoring_partition_id: ScoringPartitionId::from_bytes([17; 16]),
            },
            valid_from_epoch: valid_from,
            valid_until_epoch_exclusive: valid_until,
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
            source_owner_generation: source.registry.namespaces()[&source_namespace_id]
                .ownership
                .source_owner_generation,
        };
        let mut task_plan = SearchTaskPlan {
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
                catalog_revision: CatalogRevision::new(source.registry.revision()),
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
        let scope = intersect_scope(
            &RequestedMembershipScope {
                memberships: member_set.clone(),
            },
            &member_set,
            &AuthoritativeAccessSnapshot {
                generation: 2,
                source_catalog_generation: source.registry.revision(),
                membership_generation: 1,
                bindings: BTreeMap::from([(
                    membership.source_membership_id,
                    MembershipAccessBinding {
                        membership_id: membership.source_membership_id,
                        access_partition_digest: Blake3Digest32::from_bytes([0x81; 32]),
                        scoring_partition_digest: Blake3Digest32::from_bytes([0x82; 32]),
                        projection_membership_id: OpaqueId::new(
                            projection_membership_id.to_string(),
                        )
                        .unwrap(),
                        active: true,
                    },
                )]),
                snapshot_digest: Blake3Digest32::from_bytes([27; 32]),
            },
        )
        .unwrap();
        let safe_leg = compile_safe_legs(
            &scope,
            IndexedRouteFence {
                collection_generation_id: generation,
                visible_epoch: epoch,
                route_generation: 2,
                owner_epoch: search_contracts::OwnerEpoch::new(1).unwrap(),
            },
            2,
            1,
            1,
            None,
            1,
        )
        .unwrap()
        .remove(0);
        let ticket = canonical_ticket(&mut task_plan, &scope, safe_leg, &projection);
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
            plan_digest: *ticket.plan_digest.as_bytes(),
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
            source,
            expected_revision: source_ref,
            scope,
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
        assert_eq!(
            &candidate.plan_digest,
            fixture.ticket.plan_digest.as_bytes()
        );
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
        assert_eq!(fixture.validation.visible_epoch, Epoch::new(5).unwrap());
        assert_eq!(candidate.valid_from_epoch, Epoch::new(1).unwrap());
        assert_eq!(candidate.valid_until_epoch_exclusive, None);
    }

    #[test]
    fn preserves_stored_epoch_bounds_and_refuses_points_outside_the_pinned_epoch() {
        let closed = fixture_with_validity(Epoch::new(1).unwrap(), Some(Epoch::new(9).unwrap()));
        let candidate = closed.map(&[closed.nomination]).unwrap().remove(0);
        assert_eq!(candidate.valid_from_epoch, Epoch::new(1).unwrap());
        assert_eq!(
            candidate.valid_until_epoch_exclusive,
            Some(Epoch::new(9).unwrap())
        );
        for (from, until) in [(6, None), (1, Some(5))] {
            let fixture = fixture_with_validity(
                Epoch::new(from).unwrap(),
                until.map(|value| Epoch::new(value).unwrap()),
            );
            assert_eq!(
                fixture.map(&[fixture.nomination]),
                Err(NominationSourceError::ProjectionMismatch)
            );
        }
    }

    #[test]
    fn refuses_wrong_stored_cbor_and_payload_body_commitment() {
        let mut fixture = fixture();
        let original = fixture.projection.points[0]
            .expected_readback
            .payload_cbor
            .clone();
        fixture.projection.points[0].expected_readback.payload_cbor =
            search_contracts::to_canonical_cbor(&search_contracts::CanonicalValue::Null).unwrap();
        assert_eq!(
            fixture.map(&[fixture.nomination]),
            Err(NominationSourceError::ProjectionMismatch)
        );
        fixture.projection.points[0].expected_readback.payload_cbor = original;
        let mut payload = fixture.stored_payload();
        payload.language_or_format = BoundedName::new("tampered-text").unwrap();
        fixture.projection.points[0].expected_readback.payload_cbor =
            payload.to_canonical_cbor().unwrap();
        assert_eq!(
            fixture.map(&[fixture.nomination]),
            Err(NominationSourceError::ProjectionMismatch)
        );
    }

    #[test]
    fn refuses_echoed_payload_digests_for_foreign_owner_and_manifest_fields() {
        let changes: &[fn(&mut PointPayload)] = &[
            |payload| payload.source_id = SourceId::from_bytes([0x81; 16]),
            |payload| payload.source_revision_id = SourceRevisionId::from_bytes([0x82; 16]),
            |payload| {
                payload.representation_id =
                    search_contracts::RepresentationId::from_bytes([0x83; 16])
            },
            |payload| payload.unit_id = UnitId::from_bytes([0x84; 16]),
            |payload| payload.point_identity_digest_256 = Blake3Digest32::from_bytes([0x85; 32]),
            |payload| {
                payload.installation_incarnation_id =
                    InstallationIncarnationId::from_bytes([0x86; 16])
            },
            |payload| {
                payload.collection_generation_id = CollectionGenerationId::from_bytes([0x87; 16])
            },
            |payload| {
                payload.projection_membership_id = ProjectionMembershipId::from_bytes([0x88; 16])
            },
            |payload| {
                payload.projection_profile_set_id =
                    ProjectionProfileSetId::new("foreign-profile-v1").unwrap()
            },
            |payload| payload.access_partition_id = AccessPartitionId::from_bytes([0x89; 16]),
            |payload| payload.scoring_partition_id = ScoringPartitionId::from_bytes([0x90; 16]),
            |payload| {
                payload.scoring_document_id =
                    search_contracts::ScoringDocumentId::from_bytes([0x91; 16])
            },
            |payload| payload.unit_kind = search_contracts::UnitKind::Doc,
        ];
        for change in changes {
            let mut fixture = fixture();
            let mut payload = fixture.stored_payload();
            change(&mut payload);
            fixture.replace_payload(&payload);
            assert_eq!(
                fixture.map(&[fixture.nomination]),
                Err(NominationSourceError::ProjectionMismatch)
            );
        }
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
                &fixture.source.registry,
                &fixture.expected_revision,
                &fixture.scope,
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
                &fixture.source.registry,
                &fixture.expected_revision,
                &fixture.scope,
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
                &fixture.source.registry,
                &fixture.expected_revision,
                &fixture.scope,
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
                &fixture.source.registry,
                &fixture.expected_revision,
                &fixture.scope,
                &fixture.revision,
                &fixture.unit_set,
                &fixture.projection,
            ),
            Err(NominationSourceError::ProjectionMismatch)
        );
    }

    #[test]
    fn requires_one_current_c03_namespace_owner_fence() {
        let mut fixture = fixture();
        let current = *fixture.task_plan.source_owner_fences.iter().next().unwrap();
        fixture.task_plan.source_owner_fences = BoundedList::empty();
        assert_eq!(
            fixture.map(&[fixture.nomination]),
            Err(NominationSourceError::ContextMismatch)
        );
        fixture.task_plan.source_owner_fences = BoundedList::new(vec![current, current]).unwrap();
        assert_eq!(
            fixture.map(&[fixture.nomination]),
            Err(NominationSourceError::ContextMismatch)
        );
        let mut stale = current;
        stale.source_owner_generation = SourceOwnerGeneration::from_bytes([0xEA; 32]);
        fixture.task_plan.source_owner_fences = BoundedList::new(vec![stale]).unwrap();
        assert_eq!(
            fixture.map(&[fixture.nomination]),
            Err(NominationSourceError::ContextMismatch)
        );
        stale.source_namespace_id = SourceNamespaceId::from_bytes([0xEB; 16]);
        fixture.task_plan.source_owner_fences = BoundedList::new(vec![stale]).unwrap();
        assert_eq!(
            fixture.map(&[fixture.nomination]),
            Err(NominationSourceError::ContextMismatch)
        );
        fixture.task_plan.source_owner_fences = BoundedList::new(vec![current]).unwrap();
        assert!(fixture.map(&[fixture.nomination]).is_ok());
        fixture.source.restrict();
        assert_eq!(
            fixture.map(&[fixture.nomination]),
            Err(NominationSourceError::MembershipMismatch)
        );
    }

    #[test]
    fn binds_the_same_expected_revision_ref_to_c03_identity_and_verified_bytes() {
        let mut fixture = fixture();
        let expected = fixture.expected_revision;
        fixture.expected_revision.source_namespace_id = SourceNamespaceId::from_bytes([0xEC; 16]);
        assert_eq!(
            fixture.map(&[fixture.nomination]),
            Err(NominationSourceError::SourceMismatch)
        );
        fixture.expected_revision = expected;
        fixture.expected_revision.source_id = SourceId::from_bytes([0xED; 16]);
        assert_eq!(
            fixture.map(&[fixture.nomination]),
            Err(NominationSourceError::SourceMismatch)
        );
        fixture.expected_revision = expected;
        fixture.expected_revision.revision_id = SourceRevisionId::from_bytes([0xEE; 16]);
        assert_eq!(
            fixture.map(&[fixture.nomination]),
            Err(NominationSourceError::RevisionMismatch)
        );
        fixture.expected_revision = expected;
        fixture.expected_revision.content_digest = Blake3Digest32::from_bytes([0xEF; 32]);
        assert_eq!(
            fixture.map(&[fixture.nomination]),
            Err(NominationSourceError::RevisionMismatch)
        );
        fixture.expected_revision = expected;
        fixture.expected_revision.byte_length += 1;
        assert_eq!(
            fixture.map(&[fixture.nomination]),
            Err(NominationSourceError::RevisionMismatch)
        );
    }

    #[test]
    fn refuses_empty_duplicate_foreign_and_route_diverged_eligibility_inputs() {
        let fixture = fixture();
        let leg = fixture.ticket.leg.safe_index_leg().unwrap();
        let check = |leg: &SafeRetrievalLeg| {
            validate_eligibility(
                leg,
                &fixture.scope,
                &fixture.projection,
                &fixture.task_plan,
                &fixture.ticket,
            )
        };
        assert_eq!(check(leg), Ok(()));
        let mut bad = leg.clone();
        bad.eligibility_plans.clear();
        assert_eq!(check(&bad), Err(NominationSourceError::ContextMismatch));
        bad = leg.clone();
        bad.eligibility_plans.push(bad.eligibility_plans[0].clone());
        assert_eq!(check(&bad), Err(NominationSourceError::ContextMismatch));
        bad = leg.clone();
        bad.eligibility_plans[0].membership_id = SourceMembershipId::from_bytes([0xF0; 16]);
        assert_eq!(check(&bad), Err(NominationSourceError::ContextMismatch));
        bad = leg.clone();
        bad.eligibility_plans[0].collection_generation_id =
            CollectionGenerationId::from_bytes([0xF1; 16]);
        assert_eq!(check(&bad), Err(NominationSourceError::ContextMismatch));
        bad = leg.clone();
        bad.eligibility_plans[0].visible_epoch = Epoch::new(8).unwrap();
        assert_eq!(check(&bad), Err(NominationSourceError::ContextMismatch));
        bad = leg.clone();
        bad.eligibility_plans[0].predicate_digest =
            search_access::EligibilityPlanDigest([0xF2; 32]);
        assert_eq!(check(&bad), Err(NominationSourceError::ContextMismatch));
    }

    #[test]
    fn requires_keyed_scope_digests_and_canonical_projection_uuid_spelling() {
        let mut fixture = fixture();
        let member = fixture.membership.source_membership_id;
        let binding = fixture.scope.memberships[&member].clone();
        fixture
            .scope
            .memberships
            .get_mut(&member)
            .unwrap()
            .access_partition_digest = Blake3Digest32::from_bytes([0xF3; 32]);
        assert_eq!(
            fixture.map(&[fixture.nomination]),
            Err(NominationSourceError::ContextMismatch)
        );
        fixture.scope.memberships.insert(member, binding.clone());
        fixture
            .scope
            .memberships
            .get_mut(&member)
            .unwrap()
            .scoring_partition_digest = Blake3Digest32::from_bytes([0xF4; 32]);
        assert_eq!(
            fixture.map(&[fixture.nomination]),
            Err(NominationSourceError::ContextMismatch)
        );
        fixture.scope.memberships.insert(member, binding.clone());
        fixture
            .scope
            .memberships
            .get_mut(&member)
            .unwrap()
            .membership_id = SourceMembershipId::from_bytes([0xF5; 16]);
        assert_eq!(
            fixture.map(&[fixture.nomination]),
            Err(NominationSourceError::ContextMismatch)
        );
        fixture.scope.memberships.insert(member, binding.clone());
        fixture.scope.memberships.get_mut(&member).unwrap().active = false;
        assert_eq!(
            fixture.map(&[fixture.nomination]),
            Err(NominationSourceError::ContextMismatch)
        );
        fixture.scope.memberships.insert(member, binding.clone());
        // These remain genuine canonical-compiler outputs: opaque spellings can
        // be internally coherent yet fail the actual projection UUID join.
        for spelling in [
            binding.projection_membership_id.as_str().to_uppercase(),
            fixture
                .task_plan
                .profile_fence
                .projection_profile_set_ids
                .iter()
                .next()
                .unwrap()
                .to_string(),
        ] {
            let mut foreign = binding.clone();
            foreign.projection_membership_id = OpaqueId::new(spelling).unwrap();
            fixture.scope.memberships.insert(member, foreign.clone());
            let mut leg = fixture.ticket.leg.safe_index_leg().unwrap().clone();
            leg.eligibility_plans =
                vec![compile_base_eligibility(&foreign, leg.route, 2, 1, 1).unwrap()];
            fixture.ticket = canonical_ticket(
                &mut fixture.task_plan,
                &fixture.scope,
                leg,
                &fixture.projection,
            );
            fixture.validation.plan_digest = *fixture.ticket.plan_digest.as_bytes();
            assert_eq!(
                fixture.map(&[fixture.nomination]),
                Err(NominationSourceError::ContextMismatch)
            );
        }
    }
}

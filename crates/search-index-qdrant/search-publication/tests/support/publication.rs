//! Canonical producer fixtures and synthetic owner observations for integration tests.
//!
//! Source bytes pass through the real C08 materializer, C09 unitizer and C13 planner.
//! Owner observations and receipt labels below are test-binary-only and make no native,
//! journal, Qdrant, or crash-recovery proof claim.

#![allow(dead_code)]

use core::fmt;
use std::fmt::Write as _;
use std::ops::{Deref, DerefMut};
use std::sync::{
    Arc,
    atomic::{AtomicBool, Ordering},
};

use search_contracts::{
    self as c, Epoch, OpaqueId, OpaqueRef, ReceiptRef, RequestId, SourceRevisionRef,
    indexed::{
        CollectionSchema, NamedVectorRequirement, VectorName, VectorRequirement, VectorValue,
    },
};
use search_point_identity::s11::PointId128;
use search_ports::{
    CancellationProbe, FakeCancellation, IdempotencyClass, MutationIdentity, OperationContext,
    PackageOpaque,
};
use search_projection_planner::{
    LineageObservation, PlannedUnit, PlannedVector, PlanningScope, ProjectionBudget,
    ProjectionManifest, ProjectionMembershipBinding, ProjectionPlan, manifest_canonical_bytes,
    plan_projection,
};
use search_publication::{
    AdmissionPurpose, AdmissionWork, ProjectionEvidence, ProjectionUpdate,
    PublicationAdmissionOwners, PublicationCoordinator, PublicationEffectBinding,
    PublicationEffectCommand, PublicationEffectKind, PublicationEffectOutcome,
    PublicationEffectPort, PublicationEffectPreparationOwners, PublicationError,
    PublicationMembershipState, PublicationMutationAttempt, PublicationOwnerObservation,
    PublicationPointExpectation, PublicationPointReadback, PublicationProposal,
    PublicationReadbackPort, PublicationRecoveryDecision, PublicationRestoreInput,
    PublicationScope,
};
use search_unitizer::{
    UnitizationLimits, V3AnchorPolicy, V3AttachmentPolicy, V3EmptyPolicy, V3OmissionPolicy,
    V3OverlapPolicy, V3RepresentationKind, V3SourceBinding, V3UnitizerProfileDescriptor,
    VerifiedUnitSet,
};

#[path = "../../src/admission_tests/support.rs"]
mod c08_c09;

fn epoch(value: i64) -> Epoch {
    Epoch::new(value).expect("fixture epoch is valid")
}

fn identity_bytes(domain: &[u8], text: &str) -> [u8; 16] {
    let mut input = Vec::with_capacity(domain.len() + text.len() + 1);
    input.extend_from_slice(domain);
    input.push(0);
    input.extend_from_slice(text.as_bytes());
    let digest = c::blake3_exact_bytes(
        &input,
        c::Blake3ExactInputLimit::new(4096).expect("bounded fixture input"),
    )
    .expect("fixture digest fits");
    let mut bytes = [0; 16];
    bytes.copy_from_slice(&digest.as_bytes()[..16]);
    bytes
}

fn unitizer_descriptor() -> V3UnitizerProfileDescriptor {
    V3UnitizerProfileDescriptor {
        profile_name: c::ProfileId::new("baseline-unitizer-v3").expect("profile name"),
        profile_revision: 1,
        representation_kind: V3RepresentationKind::Text,
        unit_kind: c::UnitKind::Section,
        boundary_revision: 1,
        overlap_policy: V3OverlapPolicy::None,
        anchor_policy: V3AnchorPolicy::ExactTextBytes,
        attachment_policy: V3AttachmentPolicy::Absent,
        omission_policy: V3OmissionPolicy::Forbidden,
        empty_policy: V3EmptyPolicy::Reject,
        limits: UnitizationLimits {
            max_input_bytes: 4096,
            preferred_unit_bytes: 8,
            max_unit_bytes: 16,
            max_lines: 512,
            max_units: 512,
        },
        min_unit_bytes: 1,
        max_unit_scalars: 16,
        max_unit_lines: 16,
        max_anchor_depth: 1,
        max_manifest_bytes: 512 * 1024,
        max_steps: 1_000_000,
        fixture_digest: c::Blake3Digest32::from_bytes([71; 32]),
    }
}

fn planned_units(units: &VerifiedUnitSet) -> Vec<PlannedUnit> {
    units
        .units()
        .iter()
        .map(|unit| PlannedUnit {
            unit_id: unit.unit_id(),
            unit_kind: unit.occurrence().unit_kind,
            modality: c::Modality::Text,
            language_or_format: c::BoundedName::new("text").expect("format"),
            entity_kind: None,
            normalized_symbol_key: None,
            unit_digest: unit.unit_content_digest(),
            reference_digest: unit.reference_digest(),
            vectors: vec![PlannedVector {
                name: VectorName::new("lexical-sparse").expect("vector name"),
                value: VectorValue::Sparse {
                    indices: vec![1, 4],
                    values: vec![1.0, 2.0],
                },
            }],
        })
        .collect()
}

fn projection_plan(
    units: &VerifiedUnitSet,
    scope: &PlanningScope,
    schema: &CollectionSchema,
    planned: &[PlannedUnit],
) -> ProjectionPlan {
    let requirement = VectorRequirement::Sparse {
        index_ceiling: 8,
        idf_enabled: true,
    };
    let requirements = [("lexical-sparse".to_owned(), requirement)].into();
    let plan = plan_projection(
        units,
        scope,
        planned,
        &requirements,
        ProjectionBudget::BASELINE,
    )
    .expect("C13 projection plan");
    search_projection_planner::validate_schema(schema, &plan, ProjectionBudget::BASELINE)
        .expect("C13 schema validation");
    plan
}

#[derive(Clone, Debug, PartialEq)]
pub struct Fixture {
    pub source: SourceRevisionRef,
    owner_source: SourceRevisionRef,
    pub units: VerifiedUnitSet,
    pub plan: ProjectionPlan,
    pub scope: PlanningScope,
    pub schema: CollectionSchema,
    planned_vectors: Vec<PlannedUnit>,
    sealed_plan: ProjectionPlan,
    sealed_scope: PlanningScope,
    retargetable: bool,
}

impl Fixture {
    fn from_text(text: &str, valid_from: Epoch, reject_as_empty: bool) -> Self {
        let mut binding: V3SourceBinding = c08_c09::binding();
        binding.source_revision_id =
            c::SourceRevisionId::from_bytes(identity_bytes(b"revision", text));
        binding.materialization_id =
            c::MaterializationId::from_bytes(identity_bytes(b"materialization", text));
        binding.representation_id =
            c::RepresentationId::from_bytes(identity_bytes(b"representation", text));
        let units = c08_c09::verified_unit_set(text, binding.clone());
        let membership = ProjectionMembershipBinding {
            projection_membership_id: c::ProjectionMembershipId::from_bytes([17; 16]),
            source_membership_id: c::SourceMembershipId::from_bytes([18; 16]),
            source_id: binding.source_id,
            source_revision_id: binding.source_revision_id,
            representation_id: binding.representation_id,
            projection_schema_id: c::ProfileId::new("publication-fixture-v1").expect("schema name"),
            collection_generation_id: c::CollectionGenerationId::from_bytes([19; 16]),
            installation_incarnation_id: c::InstallationIncarnationId::from_bytes([20; 16]),
            projection_profile_set_id: c::ProjectionProfileSetId::new("publication-fixture-v1")
                .expect("profile set"),
            access_partition_id: c::AccessPartitionId::from_bytes([21; 16]),
            scoring_partition_id: c::ScoringPartitionId::from_bytes([22; 16]),
        };
        let scope = PlanningScope {
            membership,
            valid_from_epoch: valid_from,
            valid_until_epoch_exclusive: None,
            lineage: LineageObservation {
                repository_lineage_id: None,
            },
            identity_limits: search_point_identity::s11::DEFAULT_LIMITS,
        };
        let vector_requirement = VectorRequirement::Sparse {
            index_ceiling: 8,
            idf_enabled: true,
        };
        let vectors = [NamedVectorRequirement::new(
            VectorName::new("lexical-sparse").expect("vector name"),
            vector_requirement,
        )
        .expect("named vector requirement")];
        let schema = CollectionSchema::initial(
            scope.membership.installation_incarnation_id,
            scope.membership.collection_generation_id,
            scope.membership.projection_profile_set_id.clone(),
            &vectors,
        )
        .expect("C13 collection schema");
        let planned_vectors = planned_units(&units);
        let mut plan = projection_plan(&units, &scope, &schema, &planned_vectors);
        if reject_as_empty {
            // Model an incomplete C13 product over a real nonempty C08/C09 set.
            // Canonical admission must reject this omission before creating a slot.
            plan.points.clear();
            plan.manifest.entries.clear();
            plan.manifest.canonical_bytes =
                manifest_canonical_bytes(&plan.manifest, ProjectionBudget::BASELINE)
                    .expect("canonical empty manifest bytes")
                    .as_slice()
                    .to_vec();
        }
        let source = SourceRevisionRef {
            source_namespace_id: binding.source_namespace_id,
            source_id: binding.source_id,
            revision_id: binding.source_revision_id,
            content_digest: units.manifest().provenance().input_digest(),
            byte_length: text.len() as u64,
        };
        let sealed_plan = plan.clone();
        let sealed_scope = scope.clone();
        Self {
            source,
            owner_source: source,
            units,
            plan,
            scope,
            schema,
            planned_vectors,
            sealed_plan,
            sealed_scope,
            retargetable: !reject_as_empty,
        }
    }

    pub const fn manifest(&self) -> &ProjectionManifest {
        &self.plan.manifest
    }

    pub fn at_epoch(&self, valid_from: Epoch) -> Self {
        if !self.retargetable || self.plan != self.sealed_plan || self.scope != self.sealed_scope {
            return self.clone();
        }
        let mut retargeted = self.clone();
        retargeted.scope.valid_from_epoch = valid_from;
        retargeted.plan = projection_plan(
            &retargeted.units,
            &retargeted.scope,
            &retargeted.schema,
            &retargeted.planned_vectors,
        );
        retargeted.sealed_plan = retargeted.plan.clone();
        retargeted.sealed_scope = retargeted.scope.clone();
        retargeted
    }

    /// Replan the real C13 product while preserving the complete source and point address.
    pub fn with_changed_vector_at(&self, unit_index: usize) -> Self {
        self.with_sparse_values_at(unit_index, vec![2.0, 1.0])
    }

    /// Rebuilds a complete real C13 product for exact vector-byte proof cases.
    pub fn with_sparse_values_at(&self, unit_index: usize, values: Vec<f32>) -> Self {
        let mut changed = self.clone();
        changed.planned_vectors[unit_index].vectors[0].value = VectorValue::Sparse {
            indices: vec![1, 4],
            values,
        };
        changed.plan = projection_plan(
            &changed.units,
            &changed.scope,
            &changed.schema,
            &changed.planned_vectors,
        );
        changed.sealed_plan = changed.plan.clone();
        changed.sealed_scope = changed.scope.clone();
        changed
    }
}

impl Deref for Fixture {
    type Target = ProjectionManifest;

    fn deref(&self) -> &Self::Target {
        self.manifest()
    }
}

impl DerefMut for Fixture {
    fn deref_mut(&mut self) -> &mut Self::Target {
        &mut self.plan.manifest
    }
}

pub fn projection_fixture(labels: &[u8]) -> Fixture {
    if labels.is_empty() {
        return Fixture::from_text("unsupported-empty-projection\n", epoch(1), true);
    }
    let mut text = String::new();
    for label in labels {
        writeln!(&mut text, "unit-{label}").expect("writing to a String is infallible");
    }
    Fixture::from_text(&text, epoch(1), false)
}

#[derive(Clone, Copy, Debug)]
pub struct OwnerSeed {
    pub previous_visible: Epoch,
    pub current_visible: Epoch,
    pub last_reserved: Epoch,
}

#[derive(Clone)]
pub enum ManifestExpectation {
    FromOwnerState,
    Exact(Option<ProjectionManifest>),
}

#[derive(Clone)]
pub struct PublicationCase {
    pub transaction_id: OpaqueId,
    pub old: Option<Fixture>,
    pub new: Fixture,
    pub seed: OwnerSeed,
    pub original_guards: c::PublicationGuards,
    pub current_guards: c::PublicationGuards,
    pub expected_manifest_override: ManifestExpectation,
    pub drift_after_first_observation: bool,
    pub retirement: bool,
}

pub fn proposal(old: Option<Fixture>, new: Fixture, name: &str) -> PublicationCase {
    let original_guards = guards();
    let (previous_visible, current_visible, last_reserved) = if old.is_some() {
        (epoch(1), epoch(1), epoch(1))
    } else {
        (epoch(0), epoch(0), epoch(0))
    };
    PublicationCase {
        transaction_id: OpaqueId::new(name).expect("transaction ID"),
        old,
        new,
        seed: OwnerSeed {
            previous_visible,
            current_visible,
            last_reserved,
        },
        original_guards,
        current_guards: original_guards,
        expected_manifest_override: ManifestExpectation::FromOwnerState,
        drift_after_first_observation: false,
        retirement: false,
    }
}

pub fn retirement_proposal(old: Fixture, name: &str) -> PublicationCase {
    let mut case = proposal(Some(old.clone()), old, name);
    case.retirement = true;
    case
}

pub fn guards() -> c::PublicationGuards {
    c::PublicationGuards {
        owner_epoch: c::OwnerEpoch::new(7).expect("owner epoch"),
        source_catalog_generation: 11,
        membership_generation: 13,
        access_generation: 17,
        shadow_generation: 19,
        purge_generation: 23,
        profile_digest: c::Blake3Digest32::from_bytes([29; 32]),
    }
}

fn owner_snapshot(
    case: &PublicationCase,
    previous: Epoch,
    visible: Epoch,
    floor: Epoch,
    expected_manifest: Option<ProjectionManifest>,
    current_guards: c::PublicationGuards,
) -> OwnerSnapshot {
    OwnerSnapshot {
        source: case
            .old
            .as_ref()
            .filter(|_| case.retirement)
            .unwrap_or(&case.new)
            .owner_source,
        expected_manifest,
        previous_visible_epoch: previous,
        current_visible_epoch: visible,
        last_reserved_epoch: floor,
        state: if case.retirement {
            PublicationMembershipState::Retired
        } else {
            PublicationMembershipState::Active
        },
        original_guards: case.original_guards,
        current_guards,
    }
}

struct OwnerSnapshot {
    source: SourceRevisionRef,
    expected_manifest: Option<ProjectionManifest>,
    previous_visible_epoch: Epoch,
    current_visible_epoch: Epoch,
    last_reserved_epoch: Epoch,
    state: PublicationMembershipState,
    original_guards: c::PublicationGuards,
    current_guards: c::PublicationGuards,
}

struct Owners {
    observations: usize,
    drift_after_first: bool,
}

impl PublicationAdmissionOwners for Owners {
    type Projection = Fixture;
    type Snapshot = OwnerSnapshot;

    fn projection<'a>(
        &self,
        token: &'a Fixture,
    ) -> Result<ProjectionEvidence<'a>, PublicationError> {
        Ok(ProjectionEvidence {
            source: &token.source,
            units: &token.units,
            plan: &token.plan,
            planning_scope: &token.scope,
            schema: &token.schema,
        })
    }

    fn observe(
        &mut self,
        snapshot: &OwnerSnapshot,
        _: &OpaqueId,
        scope: &PublicationScope,
        purpose: AdmissionPurpose,
    ) -> Result<PublicationOwnerObservation, PublicationError> {
        let binding = c08_c09::binding();
        if scope.source != snapshot.source
            || scope.source.source_namespace_id != binding.source_namespace_id
            || scope.source.source_id != binding.source_id
            || scope.source_membership_id != c::SourceMembershipId::from_bytes([18; 16])
            || scope.projection_membership_id != c::ProjectionMembershipId::from_bytes([17; 16])
            || scope.collection_generation_id != c::CollectionGenerationId::from_bytes([19; 16])
            || scope.installation_incarnation_id
                != c::InstallationIncarnationId::from_bytes([20; 16])
        {
            return Err(PublicationError::GuardMismatch);
        }
        self.observations += 1;
        let mut current_guards = snapshot.current_guards;
        if self.drift_after_first && self.observations > 1 {
            current_guards.access_generation += 1;
        }
        let target_epoch = match purpose {
            AdmissionPurpose::Prepare => snapshot
                .last_reserved_epoch
                .checked_next()
                .map_err(|_| PublicationError::ContractExhausted)?,
            AdmissionPurpose::Restore => snapshot.last_reserved_epoch,
        };
        Ok(PublicationOwnerObservation {
            original_guards: snapshot.original_guards,
            current_guards,
            previous_visible_epoch: snapshot.previous_visible_epoch,
            current_visible_epoch: snapshot.current_visible_epoch,
            last_reserved_epoch: snapshot.last_reserved_epoch,
            target_epoch,
            membership_state: snapshot.state,
        })
    }

    fn verify_current_manifest(
        &mut self,
        snapshot: &OwnerSnapshot,
        _: &PublicationScope,
        expected: Option<&ProjectionManifest>,
    ) -> Result<(), PublicationError> {
        if expected != snapshot.expected_manifest.as_ref() {
            return Err(PublicationError::ControlConflict);
        }
        Ok(())
    }

    fn work(&mut self, _: &OwnerSnapshot, _: AdmissionWork) -> Result<(), PublicationError> {
        Ok(())
    }
}

impl PublicationEffectPreparationOwners for Owners {
    fn planned_vectors<'a>(
        &self,
        token: &'a Fixture,
    ) -> Result<Option<&'a [PlannedUnit]>, PublicationError> {
        Ok(Some(&token.planned_vectors))
    }
}

/// Synthetic phase port for pure model tests. It returns complete typed point
/// observations and deliberately makes no Native, redb, Qdrant or crash-proof claim.
#[derive(Clone, Default)]
pub struct PhasePort {
    /// Mutation side of the synthetic split port.
    pub effects: PhaseEffects,
    /// Independent observation side of the synthetic split port.
    pub readback: PhaseReadback,
}

#[derive(Clone, Default)]
pub struct PhaseEffects {
    /// Mutation identities observed by the effect side.
    pub effect_ids: Vec<OpaqueId>,
    /// Effect kinds observed by the synthetic effect side.
    pub effect_kinds: Vec<PublicationEffectKind>,
    /// Full bindings observed at synthetic effect dispatch.
    pub effect_bindings: Vec<(PublicationEffectKind, PublicationEffectBinding)>,
    /// Optional cancellation triggered after one synthetic effect dispatch.
    pub cancel_after_apply: Option<(PublicationEffectKind, Arc<AtomicBool>)>,
}

#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub enum OptionalOverride<T> {
    /// Leave the synthetic port's value unchanged.
    #[default]
    Keep,
    /// Return the supplied value, including an explicit `None`.
    Set(T),
}

#[derive(Clone, Default)]
pub struct PhaseReadback {
    /// One-shot untrusted observation override for mismatch/unknown cases.
    pub override_readback: Option<search_publication::PublicationEffectReadback>,
    /// Outcome returned by the next readback; defaults to Applied.
    pub next_outcome: Option<PublicationEffectOutcome>,
    /// Durable phase returned by the synthetic intent journal readback.
    pub recorded_phase: Option<search_publication::PublicationPhase>,
    /// Original durable binding returned by the synthetic intent journal.
    pub intent_binding_override: Option<PublicationEffectBinding>,
    /// Exact owner binding echoed at top level for an intent restore readback.
    pub intent_effect_binding_override: Option<PublicationEffectBinding>,
    /// Persistent per-phase readback outcomes for recovery fixtures.
    pub outcomes: Vec<(PublicationEffectKind, PublicationEffectOutcome)>,
    /// Partial stage readback used to exercise compensation classification.
    pub staged_ids: Option<Vec<PointId128>>,
    /// Partial closure readback used to exercise compensation classification.
    pub closed_ids: Option<Vec<PointId128>>,
    /// Duplicate/foreign IDs injected into the untrusted readback response.
    pub unexpected_point_ids: Vec<PointId128>,
    /// Per-effect duplicate/foreign IDs for independent restore cases.
    pub unexpected_point_ids_by_effect: Vec<(PublicationEffectKind, Vec<PointId128>)>,
    /// Optional actual current visibility for stale/unrelated epoch scenarios.
    pub control_visible_epoch: Option<Epoch>,
    /// Corruption knobs for tests of full nested owner-record comparison.
    pub intent_operation_id_override: Option<OpaqueId>,
    pub intent_target_epoch_override: Option<Epoch>,
    pub intent_guards_override: Option<c::PublicationGuards>,
    pub intent_transaction_id_override: Option<OpaqueId>,
    pub intent_old_manifest_digest_override: OptionalOverride<Option<c::Blake3Digest32>>,
    pub intent_new_manifest_digest_override: Option<c::Blake3Digest32>,
    pub intent_route_generation_override: Option<c::CollectionGenerationId>,
    pub intent_schema_identity_override: Option<c::Blake3Digest32>,
    pub intent_request_id_override: Option<RequestId>,
    /// Context metadata returned by an untrusted durable-intent readback.
    pub intent_deadline_override: Option<u64>,
    /// Context metadata returned by an untrusted durable-intent readback.
    pub intent_budget_ref_override: Option<OpaqueRef>,
    /// C02 generation immediately before the synthetic visibility CAS/readback.
    pub control_before_generation_override: Option<u64>,
    /// C02 generation immediately after the synthetic visibility CAS/readback.
    pub control_generation_override: Option<u64>,
    pub control_state_digest_override: Option<c::Blake3Digest32>,
    pub snapshot_generation_override: Option<u64>,
    pub control_guards_override: Option<c::PublicationGuards>,
    pub control_after_epoch_override: Option<Epoch>,
    pub control_route_override: Option<c::CollectionGenerationId>,
    pub control_floor_override: Option<Epoch>,
    pub snapshot_visible_epoch_override: Option<Epoch>,
    pub snapshot_guards_override: Option<c::PublicationGuards>,
    pub snapshot_route_override: Option<c::CollectionGenerationId>,
    pub snapshot_manifest_override: OptionalOverride<Option<c::Blake3Digest32>>,
    pub snapshot_process_id_override: Option<OpaqueId>,
    pub snapshot_digest_override: Option<c::Blake3Digest32>,
    pub abort_request_override: Option<search_publication::AbortFinalizationRequest>,
    /// Original abort mutation binding returned from the durable control record.
    pub abort_binding_override: Option<PublicationEffectBinding>,
    pub abandonment_override: Option<search_publication::PublicationAbandonReadback>,
    pub abandon_point_ids_override: Option<Vec<PointId128>>,
    pub abandon_memberships_override: Option<Vec<c::ProjectionMembershipId>>,
    pub missing_point_ids_override: Option<(PublicationEffectKind, Vec<PointId128>)>,
    pub binding_transaction_override: Option<(PublicationEffectKind, OpaqueId)>,
    pub binding_target_epoch_override: Option<(PublicationEffectKind, Epoch)>,
    pub binding_new_manifest_override: Option<(PublicationEffectKind, c::Blake3Digest32)>,
    pub binding_old_manifest_override: Option<(PublicationEffectKind, Option<c::Blake3Digest32>)>,
    pub binding_schema_identity_override: Option<(PublicationEffectKind, c::Blake3Digest32)>,
    pub binding_request_id_override: Option<(PublicationEffectKind, RequestId)>,
    pub binding_difference_override: Option<(
        PublicationEffectKind,
        search_projection_planner::ManifestDiff,
    )>,
    pub corrupt_point_payload: Option<(PublicationEffectKind, PointId128)>,
    pub corrupt_vector: Option<(PublicationEffectKind, PointId128)>,
    pub corrupt_point_interval: Option<(PublicationEffectKind, PointId128)>,
    /// Mutation identities observed by the readback side.
    pub readback_ids: Vec<OpaqueId>,
    /// Effect kinds observed by the readback side.
    pub readback_kinds: Vec<PublicationEffectKind>,
}

impl<C: CancellationProbe> PublicationEffectPort<C> for PhaseEffects {
    fn apply(
        &mut self,
        command: &PublicationEffectCommand,
        identity: &MutationIdentity,
        _: &OperationContext<C>,
    ) -> PublicationMutationAttempt {
        self.effect_ids.push(identity.operation_id.clone());
        self.effect_kinds.push(command.effect());
        self.effect_bindings
            .push((command.effect(), command.binding().clone()));
        if let Some((effect, cancelled)) = &self.cancel_after_apply
            && *effect == command.effect()
        {
            cancelled.store(true, Ordering::SeqCst);
        }
        assert_eq!(identity.operation_id, command.binding().operation_id);
        assert_eq!(identity.idempotency, IdempotencyClass::RetrySameIdentity);
        PublicationMutationAttempt::Acknowledged
    }
}

impl<C: CancellationProbe> PublicationReadbackPort<C> for PhaseReadback {
    fn readback(
        &mut self,
        command: &PublicationEffectCommand,
        _: &OperationContext<C>,
    ) -> search_publication::PublicationEffectReadback {
        self.readback_ids
            .push(command.binding().operation_id.clone());
        self.readback_kinds.push(command.effect());
        if let Some(observation) = self.override_readback.take() {
            return observation;
        }
        let outcome = self.outcome_for(command.effect());
        let points = readback_points(self, command, outcome);
        let missing_point_ids = missing_point_ids(command, &points);
        let intent = intent_readback(self, command);
        let is_abort_record = matches!(
            command.effect(),
            PublicationEffectKind::PersistIntent
                | PublicationEffectKind::CommitAbortFinalization
                | PublicationEffectKind::PublishAbortSnapshot
        );
        let mut observation = search_publication::PublicationEffectReadback {
            effect: command.effect(),
            binding: if command.effect() == PublicationEffectKind::PersistIntent {
                self.intent_effect_binding_override
                    .clone()
                    .unwrap_or_else(|| command.binding().clone())
            } else {
                command.binding().clone()
            },
            outcome,
            points,
            missing_point_ids,
            unexpected_point_ids: self.unexpected_ids(command.effect()),
            intent: if outcome == PublicationEffectOutcome::Applied {
                intent
            } else {
                None
            },
            control: control_readback(self, command, outcome)
                .or_else(|| abort_control_readback(self, command)),
            snapshot: snapshot_readback(self, command, outcome),
            abandonment: abandonment_readback(self, command),
            abort_request: if is_abort_record {
                self.abort_request_override
                    .clone()
                    .or_else(|| command.abort_request().cloned())
            } else {
                None
            },
            abort_binding: match command.effect() {
                PublicationEffectKind::PersistIntent => self.abort_binding_override.clone(),
                PublicationEffectKind::CommitAbortFinalization => Some(command.binding().clone()),
                _ => None,
            },
        };
        apply_binding_overrides(self, command, &mut observation);
        observation
    }
}

impl PhaseReadback {
    fn outcome_for(&mut self, effect: PublicationEffectKind) -> PublicationEffectOutcome {
        self.next_outcome
            .take()
            .or_else(|| {
                self.outcomes
                    .iter()
                    .rev()
                    .find_map(|(candidate, outcome)| (*candidate == effect).then_some(*outcome))
            })
            .unwrap_or(PublicationEffectOutcome::Applied)
    }

    fn unexpected_ids(&self, effect: PublicationEffectKind) -> Vec<PointId128> {
        self.unexpected_point_ids_by_effect
            .iter()
            .rev()
            .find_map(|(candidate, ids)| (*candidate == effect).then_some(ids.clone()))
            .unwrap_or_else(|| self.unexpected_point_ids.clone())
    }
}

fn readback_points(
    readback: &PhaseReadback,
    command: &PublicationEffectCommand,
    outcome: PublicationEffectOutcome,
) -> Vec<PublicationPointReadback> {
    use PublicationEffectKind as K;
    use PublicationEffectOutcome as O;
    let mut points = if command.effect() == K::StageNewPoints
        && let Some(ids) = &readback.staged_ids
    {
        command
            .points()
            .iter()
            .filter(|point| ids.contains(&point.point_id()))
            .map(point_readback)
            .collect()
    } else if command.effect() == K::CloseOldPoints
        && let Some(ids) = &readback.closed_ids
    {
        let closed = command
            .points()
            .iter()
            .filter(|point| ids.contains(&point.point_id()))
            .map(point_readback);
        let original = command
            .original_points()
            .iter()
            .filter(|point| !ids.contains(&point.point_id()))
            .map(point_readback);
        let mut points = closed.chain(original).collect::<Vec<_>>();
        points.sort_by_key(|point| point.point_id);
        points
    } else {
        match (command.effect(), outcome) {
            (K::CloseOldPoints, O::NotApplied) => command
                .original_points()
                .iter()
                .map(point_readback)
                .collect(),
            (K::StageNewPoints, O::NotApplied)
            | (K::PersistIntent | K::CommitVisibleEpoch | K::PublishControlSnapshot, _) => {
                Vec::new()
            }
            _ => command.points().iter().map(point_readback).collect(),
        }
    };
    corrupt_point_payload(readback, command, &mut points);
    corrupt_vector(readback, command, &mut points);
    corrupt_point_interval(readback, command, &mut points);
    points
}

fn corrupt_point_payload(
    readback: &PhaseReadback,
    command: &PublicationEffectCommand,
    points: &mut [PublicationPointReadback],
) {
    if let Some((effect, point_id)) = readback.corrupt_point_payload
        && effect == command.effect()
        && let Some(point) = points.iter_mut().find(|point| point.point_id == point_id)
    {
        point.payload.unit_id = c::UnitId::from_bytes([0xfe; 16]);
    }
}

fn corrupt_vector(
    readback: &PhaseReadback,
    command: &PublicationEffectCommand,
    points: &mut [PublicationPointReadback],
) {
    if let Some((effect, point_id)) = readback.corrupt_vector
        && effect == command.effect()
        && let Some(point) = points.iter_mut().find(|point| point.point_id == point_id)
        && let Some(vector) = point.vectors.values_mut().next()
    {
        let values = match vector {
            VectorValue::Dense(values) | VectorValue::Sparse { values, .. } => values,
        };
        if let Some(value) = values.first_mut() {
            *value += 1.0;
        }
    }
}

fn corrupt_point_interval(
    readback: &PhaseReadback,
    command: &PublicationEffectCommand,
    points: &mut [PublicationPointReadback],
) {
    if let Some((effect, point_id)) = readback.corrupt_point_interval
        && effect == command.effect()
        && let Some(point) = points.iter_mut().find(|point| point.point_id == point_id)
    {
        point.payload.valid_until_epoch_exclusive = Some(
            Epoch::new(command.binding().target_epoch.get() + 1)
                .expect("synthetic point interval remains valid"),
        );
    }
}

fn missing_point_ids(
    command: &PublicationEffectCommand,
    points: &[PublicationPointReadback],
) -> Vec<PointId128> {
    match command.effect() {
        PublicationEffectKind::StageNewPoints => command
            .points()
            .iter()
            .map(PublicationPointExpectation::point_id)
            .filter(|id| !points.iter().any(|point| point.point_id == *id))
            .collect(),
        PublicationEffectKind::Compensate => command.required_absent_point_ids().to_vec(),
        _ => Vec::new(),
    }
}

fn intent_readback(
    readback: &PhaseReadback,
    command: &PublicationEffectCommand,
) -> Option<search_publication::PublicationIntentReadback> {
    if command.effect() != PublicationEffectKind::PersistIntent {
        return None;
    }
    let mut binding = readback
        .intent_binding_override
        .clone()
        .unwrap_or_else(|| command.binding().clone());
    if let Some(operation_id) = &readback.intent_operation_id_override {
        binding.operation_id.clone_from(operation_id);
    }
    if let Some(target_epoch) = readback.intent_target_epoch_override {
        binding.target_epoch = target_epoch;
    }
    if let Some(guards) = readback.intent_guards_override {
        binding.guards = guards;
    }
    if let Some(transaction_id) = &readback.intent_transaction_id_override {
        binding.transaction_id.clone_from(transaction_id);
    }
    if let OptionalOverride::Set(digest) = readback.intent_old_manifest_digest_override {
        binding.old_manifest_digest = digest;
    }
    if let Some(digest) = readback.intent_new_manifest_digest_override {
        binding.new_manifest_digest = digest;
    }
    if let Some(route) = readback.intent_route_generation_override {
        binding.scope.collection_generation_id = route;
    }
    if let Some(schema_identity) = readback.intent_schema_identity_override {
        binding.scope.schema_identity = schema_identity;
    }
    if let Some(request_id) = readback.intent_request_id_override {
        binding.request_id = request_id;
    }
    if let Some(deadline) = readback.intent_deadline_override {
        binding.relative_deadline_ms = deadline;
    }
    if let Some(budget_ref) = &readback.intent_budget_ref_override {
        binding.budget_ref.clone_from(budget_ref);
    }
    Some(search_publication::PublicationIntentReadback {
        binding,
        recorded_phase: readback
            .recorded_phase
            .unwrap_or(search_publication::PublicationPhase::IntentDurable),
        control_generation: 8,
        control_state_digest: c::Blake3Digest32::from_bytes([31; 32]),
    })
}

fn control_readback(
    readback: &PhaseReadback,
    command: &PublicationEffectCommand,
    outcome: PublicationEffectOutcome,
) -> Option<search_publication::PublicationControlReadback> {
    if command.effect() != PublicationEffectKind::CommitVisibleEpoch {
        return None;
    }
    let binding = command.binding();
    let after_visible_epoch = readback.control_visible_epoch.unwrap_or_else(|| {
        if outcome == PublicationEffectOutcome::Applied {
            binding.target_epoch
        } else {
            binding.previous_visible_epoch
        }
    });
    let committed = after_visible_epoch == binding.target_epoch;
    let visible_manifest_digest = if committed && !binding.new_manifest.entries.is_empty() {
        Some(binding.new_manifest_digest)
    } else if committed {
        None
    } else {
        binding.old_manifest_digest
    };
    Some(search_publication::PublicationControlReadback {
        before_visible_epoch: binding.previous_visible_epoch,
        after_visible_epoch: readback
            .control_after_epoch_override
            .unwrap_or(after_visible_epoch),
        last_reserved_epoch: readback
            .control_floor_override
            .unwrap_or(binding.last_reserved_epoch),
        guards: readback.control_guards_override.unwrap_or_else(|| {
            if committed {
                binding.guards
            } else {
                command.current_guards()
            }
        }),
        before_control_generation: readback.control_before_generation_override.unwrap_or(8),
        after_control_generation: readback
            .control_generation_override
            .unwrap_or(if committed { 9 } else { 8 }),
        control_state_digest: readback
            .control_state_digest_override
            .unwrap_or(c::Blake3Digest32::from_bytes([32; 32])),
        collection_generation_id: readback
            .control_route_override
            .unwrap_or(binding.scope.collection_generation_id),
        previous_visible_manifest_digest: binding.old_manifest_digest,
        visible_manifest_digest,
    })
}

fn abort_control_readback(
    readback: &PhaseReadback,
    command: &PublicationEffectCommand,
) -> Option<search_publication::PublicationControlReadback> {
    if command.effect() != PublicationEffectKind::CommitAbortFinalization {
        return None;
    }
    let request = command
        .abort_request()
        .expect("abort command carries latched request");
    Some(search_publication::PublicationControlReadback {
        before_visible_epoch: request.previous_visible_epoch,
        after_visible_epoch: readback
            .control_after_epoch_override
            .unwrap_or(request.previous_visible_epoch),
        last_reserved_epoch: readback
            .control_floor_override
            .unwrap_or_else(|| request.intent.target_epoch()),
        guards: readback
            .control_guards_override
            .unwrap_or(request.current_guards),
        before_control_generation: readback
            .control_before_generation_override
            .unwrap_or(request.expected_control_generation),
        after_control_generation: readback
            .control_generation_override
            .unwrap_or_else(|| request.expected_control_generation + 1),
        control_state_digest: readback
            .control_state_digest_override
            .unwrap_or(c::Blake3Digest32::from_bytes([32; 32])),
        collection_generation_id: readback
            .control_route_override
            .unwrap_or(request.collection_generation_id),
        previous_visible_manifest_digest: request.intent.old_manifest_digest(),
        visible_manifest_digest: request.intent.old_manifest_digest(),
    })
}

fn snapshot_readback(
    readback: &PhaseReadback,
    command: &PublicationEffectCommand,
    outcome: PublicationEffectOutcome,
) -> Option<search_publication::PublicationSnapshotReadback> {
    let is_snapshot = matches!(
        command.effect(),
        PublicationEffectKind::PublishControlSnapshot | PublicationEffectKind::PublishAbortSnapshot
    );
    if !is_snapshot || outcome != PublicationEffectOutcome::Applied {
        return None;
    }
    let abort_request = command.abort_request();
    let visible_epoch = readback.snapshot_visible_epoch_override.unwrap_or_else(|| {
        abort_request.map_or_else(
            || command.binding().target_epoch,
            |request| request.previous_visible_epoch,
        )
    });
    let control_generation = readback.snapshot_generation_override.unwrap_or_else(|| {
        abort_request.map_or_else(|| 9, |request| request.expected_control_generation + 1)
    });
    let guards = readback.snapshot_guards_override.unwrap_or_else(|| {
        abort_request.map_or_else(
            || command.binding().guards,
            |request| request.current_guards,
        )
    });
    let collection_generation_id = readback.snapshot_route_override.unwrap_or_else(|| {
        abort_request.map_or_else(
            || command.binding().scope.collection_generation_id,
            |request| request.collection_generation_id,
        )
    });
    let expected_manifest = if command.binding().new_manifest.entries.is_empty() {
        None
    } else {
        Some(command.binding().new_manifest_digest)
    };
    let default_manifest = abort_request.map_or(expected_manifest, |request| {
        request.intent.old_manifest_digest()
    });
    let visible_manifest_digest = match readback.snapshot_manifest_override {
        OptionalOverride::Keep => default_manifest,
        OptionalOverride::Set(value) => value,
    };
    Some(search_publication::PublicationSnapshotReadback {
        process_snapshot_id: readback
            .snapshot_process_id_override
            .clone()
            .unwrap_or_else(|| OpaqueId::new("fixture-current-process").unwrap()),
        visible_epoch,
        control_generation,
        guards,
        collection_generation_id,
        visible_manifest_digest,
        snapshot_digest: readback
            .snapshot_digest_override
            .unwrap_or(c::Blake3Digest32::from_bytes([33; 32])),
    })
}

fn abandonment_readback(
    readback: &PhaseReadback,
    command: &PublicationEffectCommand,
) -> Option<search_publication::PublicationAbandonReadback> {
    if command.effect() != PublicationEffectKind::Abandon {
        return None;
    }
    let difference = &command.binding().difference;
    let mut point_ids = difference
        .create
        .iter()
        .chain(&difference.retire)
        .map(|entry| entry.point_id)
        .collect::<Vec<_>>();
    point_ids.sort_unstable();
    let mut memberships = difference
        .create
        .iter()
        .chain(&difference.retire)
        .map(|entry| entry.projection_membership_id)
        .collect::<Vec<_>>();
    memberships.sort_unstable();
    memberships.dedup();
    Some(search_publication::PublicationAbandonReadback {
        binding: command.binding().clone(),
        excluded_point_ids: readback
            .abandon_point_ids_override
            .clone()
            .unwrap_or(point_ids),
        excluded_projection_memberships: readback
            .abandon_memberships_override
            .clone()
            .unwrap_or(memberships),
    })
}

fn apply_binding_overrides(
    readback: &PhaseReadback,
    command: &PublicationEffectCommand,
    observation: &mut search_publication::PublicationEffectReadback,
) {
    if let Some((effect, transaction_id)) = &readback.binding_transaction_override
        && *effect == command.effect()
    {
        observation
            .binding
            .transaction_id
            .clone_from(transaction_id);
    }
    if let Some((effect, target_epoch)) = readback.binding_target_epoch_override
        && effect == command.effect()
    {
        observation.binding.target_epoch = target_epoch;
    }
    if let Some((effect, digest)) = readback.binding_new_manifest_override
        && effect == command.effect()
    {
        observation.binding.new_manifest_digest = digest;
    }
    if let Some((effect, digest)) = readback.binding_old_manifest_override
        && effect == command.effect()
    {
        observation.binding.old_manifest_digest = digest;
    }
    if let Some((effect, schema_identity)) = readback.binding_schema_identity_override
        && effect == command.effect()
    {
        observation.binding.scope.schema_identity = schema_identity;
    }
    if let Some((effect, request_id)) = readback.binding_request_id_override
        && effect == command.effect()
    {
        observation.binding.request_id = request_id;
    }
    if let Some((effect, difference)) = &readback.binding_difference_override
        && *effect == command.effect()
    {
        observation.binding.difference.clone_from(difference);
    }
    if let Some((effect, missing)) = &readback.missing_point_ids_override
        && *effect == command.effect()
    {
        observation.missing_point_ids.clone_from(missing);
    }
}

fn point_readback(
    point: &search_publication::PublicationPointExpectation,
) -> PublicationPointReadback {
    PublicationPointReadback {
        point_id: point.point_id(),
        identity: point.identity().clone(),
        payload: point.payload().clone(),
        vectors: point.vectors().clone(),
    }
}

pub fn operation_context() -> OperationContext<FakeCancellation> {
    operation_context_with(FakeCancellation::new(false))
}

pub fn operation_context_with<C: CancellationProbe>(cancellation: C) -> OperationContext<C> {
    OperationContext::new(
        RequestId::from_bytes([51; 16]),
        60_000,
        cancellation,
        OpaqueRef::new("budget:publication-fixture").expect("budget reference"),
    )
    .expect("finite original context")
}

/// Shared cancellation state used to interrupt a synthetic effect call.
#[derive(Clone, Debug)]
pub struct SharedCancellation {
    cancelled: Arc<AtomicBool>,
}

impl SharedCancellation {
    /// Creates the shared test cancellation state.
    pub fn new(cancelled: bool) -> Self {
        Self {
            cancelled: Arc::new(AtomicBool::new(cancelled)),
        }
    }

    /// Shares the flag with a synthetic effect port.
    pub fn shared_flag(&self) -> Arc<AtomicBool> {
        Arc::clone(&self.cancelled)
    }
}

impl PackageOpaque for SharedCancellation {
    fn owner_package(&self) -> &'static str {
        "search-publication-test-fixture"
    }
}

impl CancellationProbe for SharedCancellation {
    fn is_cancelled(&self) -> bool {
        self.cancelled.load(Ordering::SeqCst)
    }
}

pub fn persist_intent(
    machine: &mut PublicationCoordinator,
    port: &mut PhasePort,
) -> Result<search_publication::DurableIntent, PublicationError> {
    machine.persist_intent(&mut port.effects, &mut port.readback, &operation_context())
}

pub fn stage_new_points(
    machine: &mut PublicationCoordinator,
    port: &mut PhasePort,
) -> Result<(), PublicationError> {
    machine.stage_new_points(&mut port.effects, &mut port.readback, &operation_context())
}

pub fn close_old_points(
    machine: &mut PublicationCoordinator,
    port: &mut PhasePort,
) -> Result<(), PublicationError> {
    machine.close_old_points(&mut port.effects, &mut port.readback, &operation_context())
}

pub fn verify_readback(
    machine: &mut PublicationCoordinator,
    port: &mut PhasePort,
) -> Result<search_publication::ReadbackVerified, PublicationError> {
    machine.verify_readback(&mut port.effects, &mut port.readback, &operation_context())
}

pub fn commit_visible_epoch(
    machine: &mut PublicationCoordinator,
    port: &mut PhasePort,
) -> Result<search_publication::VisibleCommitReceipt, PublicationError> {
    machine.commit_visible_epoch(&mut port.effects, &mut port.readback, &operation_context())
}

pub fn publish_control_snapshot(
    machine: &mut PublicationCoordinator,
    port: &mut PhasePort,
) -> Result<(), PublicationError> {
    machine.publish_control_snapshot(&mut port.effects, &mut port.readback, &operation_context())
}

pub fn compensate_exact(
    machine: &mut PublicationCoordinator,
    port: &mut PhasePort,
) -> Result<(), PublicationError> {
    machine.compensate_exact(&mut port.effects, &mut port.readback, &operation_context())
}

pub fn compensate_and_restore(
    machine: &mut PublicationCoordinator,
    port: &mut PhasePort,
) -> Result<(), PublicationError> {
    machine.compensate_and_restore(&mut port.effects, &mut port.readback, &operation_context())
}

pub fn abandon(
    machine: &mut PublicationCoordinator,
    port: &mut PhasePort,
) -> Result<(), PublicationError> {
    machine.abandon(&mut port.effects, &mut port.readback, &operation_context())
}

pub fn recover(
    machine: &mut PublicationCoordinator,
    port: &mut PhasePort,
) -> Result<PublicationRecoveryDecision, PublicationError> {
    machine.recover(&mut port.readback, &operation_context())
}

/// Test-only shorthand used to script exact phase observations; production APIs
/// accept only full point/control/snapshot readbacks returned by owner ports.
#[derive(Clone, Debug)]
pub struct PublicationRecoveryObservation {
    pub intent_durable: bool,
    pub staged_ids: Vec<PointId128>,
    pub closed_ids: Vec<PointId128>,
    pub control_visible_epoch: Epoch,
    pub snapshot_published: bool,
    pub abandon_fence_durable: bool,
    pub missing_point_ids: Vec<PointId128>,
    pub unexpected_point_ids: Vec<PointId128>,
}

pub const fn recovery_observation(
    machine: &PublicationCoordinator,
) -> PublicationRecoveryObservation {
    let tx = machine.active().expect("active fixture transaction");
    PublicationRecoveryObservation {
        intent_durable: tx.durable_intent().is_some(),
        staged_ids: Vec::new(),
        closed_ids: Vec::new(),
        control_visible_epoch: tx.previous_visible_epoch(),
        snapshot_published: false,
        abandon_fence_durable: false,
        missing_point_ids: Vec::new(),
        unexpected_point_ids: Vec::new(),
    }
}

pub fn recover_observation(
    transaction: &search_publication::PublicationTransaction,
    observed: &PublicationRecoveryObservation,
) -> Result<PublicationRecoveryDecision, PublicationError> {
    let mut port = recovery_port(transaction, observed);
    recover_readback(transaction, &mut port)
}

pub fn recovery_port(
    transaction: &search_publication::PublicationTransaction,
    observed: &PublicationRecoveryObservation,
) -> PhaseReadback {
    let difference = transaction.prepared().difference();
    let mut expected_stage = difference
        .create
        .iter()
        .map(|entry| entry.point_id)
        .collect::<Vec<_>>();
    let mut expected_closed = difference
        .retire
        .iter()
        .map(|entry| entry.point_id)
        .collect::<Vec<_>>();
    expected_stage.sort_unstable();
    expected_closed.sort_unstable();
    let stage_full = observed.staged_ids == expected_stage;
    let close_full = observed.closed_ids == expected_closed;
    let mut stage_unexpected = observed.unexpected_point_ids.clone();
    stage_unexpected.extend(unexpected_ids(&observed.staged_ids, &expected_stage));
    let mut closure_unexpected = observed.unexpected_point_ids.clone();
    closure_unexpected.extend(unexpected_ids(&observed.closed_ids, &expected_closed));
    PhaseReadback {
        recorded_phase: Some(transaction.phase()),
        staged_ids: Some(observed.staged_ids.clone()),
        closed_ids: Some(observed.closed_ids.clone()),
        control_visible_epoch: Some(observed.control_visible_epoch),
        missing_point_ids_override: (!observed.missing_point_ids.is_empty()).then(|| {
            (
                PublicationEffectKind::StageNewPoints,
                observed.missing_point_ids.clone(),
            )
        }),
        unexpected_point_ids_by_effect: vec![
            (PublicationEffectKind::StageNewPoints, stage_unexpected),
            (PublicationEffectKind::CloseOldPoints, closure_unexpected),
        ],
        outcomes: recovery_outcomes(transaction, observed, stage_full, close_full),
        ..PhaseReadback::default()
    }
}

fn unexpected_ids(ids: &[PointId128], expected: &[PointId128]) -> Vec<PointId128> {
    let mut sorted = ids.to_vec();
    sorted.sort_unstable();
    sorted
        .iter()
        .copied()
        .filter(|id| !expected.contains(id))
        .chain(
            sorted
                .windows(2)
                .filter_map(|pair| (pair[0] == pair[1]).then_some(pair[0])),
        )
        .collect()
}

fn recovery_outcomes(
    transaction: &search_publication::PublicationTransaction,
    observed: &PublicationRecoveryObservation,
    stage_full: bool,
    close_full: bool,
) -> Vec<(PublicationEffectKind, PublicationEffectOutcome)> {
    vec![
        (
            PublicationEffectKind::PersistIntent,
            if observed.intent_durable {
                PublicationEffectOutcome::Applied
            } else {
                PublicationEffectOutcome::NotApplied
            },
        ),
        (
            PublicationEffectKind::StageNewPoints,
            if stage_full {
                PublicationEffectOutcome::Applied
            } else if observed.staged_ids.is_empty() {
                PublicationEffectOutcome::NotApplied
            } else {
                PublicationEffectOutcome::Applied
            },
        ),
        (
            PublicationEffectKind::CloseOldPoints,
            if close_full {
                PublicationEffectOutcome::Applied
            } else if observed.closed_ids.is_empty() {
                PublicationEffectOutcome::NotApplied
            } else {
                PublicationEffectOutcome::Applied
            },
        ),
        (
            PublicationEffectKind::VerifyReadback,
            if stage_full && close_full {
                PublicationEffectOutcome::Applied
            } else {
                PublicationEffectOutcome::NotApplied
            },
        ),
        (
            PublicationEffectKind::CommitVisibleEpoch,
            if observed.abandon_fence_durable {
                PublicationEffectOutcome::Conflict
            } else if observed.control_visible_epoch == transaction.target_epoch() {
                PublicationEffectOutcome::Applied
            } else if observed.control_visible_epoch == transaction.previous_visible_epoch() {
                PublicationEffectOutcome::NotApplied
            } else {
                PublicationEffectOutcome::Conflict
            },
        ),
        (
            PublicationEffectKind::PublishControlSnapshot,
            if observed.snapshot_published {
                PublicationEffectOutcome::Applied
            } else {
                PublicationEffectOutcome::NotApplied
            },
        ),
    ]
}

pub fn recover_readback(
    transaction: &search_publication::PublicationTransaction,
    port: &mut PhaseReadback,
) -> Result<PublicationRecoveryDecision, PublicationError> {
    search_publication::recover(transaction, port, &operation_context())
}

fn retargeted_tokens(
    case: &PublicationCase,
    previous: Epoch,
    target: Epoch,
) -> (Option<Fixture>, Fixture) {
    let old = case.old.as_ref().map(|fixture| fixture.at_epoch(previous));
    let unchanged = case.old.as_ref().is_some_and(|old| {
        old.owner_source == case.new.owner_source && old.units == case.new.units
    });
    let new = case.new.at_epoch(if unchanged { previous } else { target });
    (old, new)
}

pub trait SubmitFixture {
    fn submit_fixture(&mut self, case: PublicationCase) -> Result<Epoch, PublicationError>;
}

impl SubmitFixture for PublicationCoordinator {
    fn submit_fixture(&mut self, case: PublicationCase) -> Result<Epoch, PublicationError> {
        let initialized = self.visible_epoch();
        let previous = initialized.unwrap_or(case.seed.previous_visible);
        let visible = initialized.unwrap_or(case.seed.current_visible);
        let floor = self
            .last_reserved_epoch()
            .unwrap_or(case.seed.last_reserved);
        let (old, new) = retargeted_tokens(&case, previous, floor.checked_next().unwrap_or(floor));
        let expected_manifest = if initialized.is_some() {
            self.current_manifest().cloned()
        } else {
            old.as_ref().map(|fixture| fixture.plan.manifest.clone())
        };
        let expected_manifest = match &case.expected_manifest_override {
            ManifestExpectation::FromOwnerState => expected_manifest,
            ManifestExpectation::Exact(manifest) => manifest.clone(),
        };
        let snapshot = owner_snapshot(
            &case,
            previous,
            visible,
            floor,
            expected_manifest,
            case.current_guards,
        );
        let update = if case.retirement {
            ProjectionUpdate::Retirement
        } else {
            ProjectionUpdate::Publish(&new)
        };
        let proposal = PublicationProposal {
            transaction_id: case.transaction_id,
            old: old.as_ref(),
            update,
        };
        let mut owners = Owners {
            observations: 0,
            drift_after_first: case.drift_after_first_observation,
        };
        Self::submit_with_effect_preparation(
            self,
            &proposal,
            &mut owners,
            &snapshot,
            ProjectionBudget::BASELINE,
        )
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct SyntheticIntent {
    pub transaction_id: OpaqueId,
    pub target_epoch: Epoch,
    pub old_manifest_digest: Option<c::Blake3Digest32>,
    pub new_manifest_digest: c::Blake3Digest32,
    pub guards: c::PublicationGuards,
    pub persist_operation_id: OpaqueId,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct SyntheticStageReceipt {
    pub transaction_id: OpaqueId,
    pub target_epoch: Epoch,
    pub staged_ids: Vec<PointId128>,
    pub missing_ids: Vec<PointId128>,
    pub unexpected_ids: Vec<PointId128>,
    pub readback_digest: c::Blake3Digest32,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct SyntheticClosureReceipt {
    pub transaction_id: OpaqueId,
    pub target_epoch: Epoch,
    pub closed_ids: Vec<PointId128>,
    pub missing_ids: Vec<PointId128>,
    pub unexpected_ids: Vec<PointId128>,
    pub readback_digest: c::Blake3Digest32,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct SyntheticVerified {
    pub transaction_id: OpaqueId,
    pub target_epoch: Epoch,
    pub staged_readback_digest: c::Blake3Digest32,
    pub closure_readback_digest: c::Blake3Digest32,
    pub new_manifest_digest: c::Blake3Digest32,
    pub retired_manifest_digest: Option<c::Blake3Digest32>,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct SyntheticControlCommit {
    pub before_visible_epoch: Epoch,
    pub after_visible_epoch: Epoch,
    pub observed_guards: c::PublicationGuards,
    pub control_generation: u64,
    pub control_state_digest: c::Blake3Digest32,
}

#[derive(Clone)]
pub struct RestoreScenario {
    pub case: PublicationCase,
    pub previous_visible_epoch: Epoch,
    pub last_reserved_epoch: Epoch,
    pub collection_generation_id: c::CollectionGenerationId,
    pub current_guards: c::PublicationGuards,
    pub phase: search_publication::PublicationPhase,
    pub intent: SyntheticIntent,
    /// Full original intent binding returned by the synthetic owner port.
    pub intent_binding: PublicationEffectBinding,
    pub stage_receipt: Option<SyntheticStageReceipt>,
    pub closure_receipt: Option<SyntheticClosureReceipt>,
    pub verified: Option<SyntheticVerified>,
    pub control_commit: Option<SyntheticControlCommit>,
    pub observation: PublicationRecoveryObservation,
    /// Optional test-only owner readback mutations for adversarial cases.
    pub readback_script: PhaseReadback,
}

impl fmt::Debug for RestoreScenario {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("RestoreScenario")
            .field("previous_visible_epoch", &self.previous_visible_epoch)
            .field("last_reserved_epoch", &self.last_reserved_epoch)
            .field("phase", &self.phase)
            .field("has_stage_observation", &self.stage_receipt.is_some())
            .field("has_closure_observation", &self.closure_receipt.is_some())
            .field("has_verified_observation", &self.verified.is_some())
            .field("has_control_observation", &self.control_commit.is_some())
            .finish_non_exhaustive()
    }
}

impl RestoreScenario {
    pub fn restore(
        &self,
        max_points: usize,
    ) -> Result<(PublicationCoordinator, PublicationRecoveryDecision), PublicationError> {
        self.restore_with_context(max_points, &operation_context())
    }

    pub fn restore_with_context<C: CancellationProbe>(
        &self,
        max_points: usize,
        context: &OperationContext<C>,
    ) -> Result<(PublicationCoordinator, PublicationRecoveryDecision), PublicationError> {
        let target = self.intent.target_epoch;
        let (old, new) = retargeted_tokens(&self.case, self.previous_visible_epoch, target);
        let visible = self.observation.control_visible_epoch;
        let expected_manifest = if visible == target {
            (!self.case.retirement).then(|| new.plan.manifest.clone())
        } else {
            old.as_ref().map(|fixture| fixture.plan.manifest.clone())
        };
        let snapshot = owner_snapshot(
            &self.case,
            self.previous_visible_epoch,
            visible,
            self.last_reserved_epoch,
            expected_manifest,
            self.current_guards,
        );
        let update = if self.case.retirement {
            ProjectionUpdate::Retirement
        } else {
            ProjectionUpdate::Publish(&new)
        };
        let proposal = PublicationProposal {
            transaction_id: self.case.transaction_id.clone(),
            old: old.as_ref(),
            update,
        };
        let input = PublicationRestoreInput::new(proposal);
        let mut owners = Owners {
            observations: 0,
            drift_after_first: self.case.drift_after_first_observation,
        };
        let mut port = PhasePort {
            effects: PhaseEffects::default(),
            readback: self.readback_script.clone(),
        };
        self.configure_readback(&mut port, old.as_ref(), &new, target);
        if port.readback.intent_effect_binding_override.is_none() {
            port.readback.intent_effect_binding_override = Some(self.intent_binding.clone());
        }
        PublicationCoordinator::restore_inflight(
            &input,
            &mut owners,
            &snapshot,
            ProjectionBudget {
                max_points,
                ..ProjectionBudget::BASELINE
            },
            &mut port.readback,
            context,
        )
    }

    fn configure_readback(
        &self,
        port: &mut PhasePort,
        old: Option<&Fixture>,
        new: &Fixture,
        target: Epoch,
    ) {
        let old_ids = old.map_or_else(Vec::new, |fixture| {
            fixture
                .plan
                .manifest
                .entries
                .iter()
                .map(|entry| entry.point_id)
                .collect()
        });
        let new_ids = if self.case.retirement {
            Vec::new()
        } else {
            new.plan
                .manifest
                .entries
                .iter()
                .map(|entry| entry.point_id)
                .collect()
        };
        let expected_stage = new_ids
            .iter()
            .copied()
            .filter(|id| !old_ids.contains(id))
            .collect::<Vec<_>>();
        let expected_closed = old_ids
            .iter()
            .copied()
            .filter(|id| !new_ids.contains(id))
            .collect::<Vec<_>>();
        self.configure_effect_state(port, &expected_stage, &expected_closed);
        self.configure_intent_readback(port, new);
        self.configure_stage_readback(port, &expected_stage, target);
        self.configure_closure_readback(port, &expected_closed, target);
        self.configure_verified_readback(port, &expected_stage, &expected_closed, target);
        self.configure_misc_readback(port);
    }

    fn configure_effect_state(
        &self,
        port: &mut PhasePort,
        expected_stage: &[PointId128],
        expected_closed: &[PointId128],
    ) {
        if port.readback.recorded_phase.is_none() {
            port.readback.recorded_phase = Some(self.phase);
        }
        if port.readback.staged_ids.is_none() {
            port.readback.staged_ids = Some(self.observation.staged_ids.clone());
        }
        if port.readback.closed_ids.is_none() {
            port.readback.closed_ids = Some(self.observation.closed_ids.clone());
        }
        port.readback.control_visible_epoch = Some(self.observation.control_visible_epoch);
        for (effect, observed_ids, expected_ids) in [
            (
                PublicationEffectKind::StageNewPoints,
                &self.observation.staged_ids,
                expected_stage,
            ),
            (
                PublicationEffectKind::CloseOldPoints,
                &self.observation.closed_ids,
                expected_closed,
            ),
        ] {
            if !port
                .readback
                .unexpected_point_ids_by_effect
                .iter()
                .any(|(existing, _)| *existing == effect)
            {
                let unexpected = unexpected_ids(observed_ids, expected_ids);
                if !unexpected.is_empty() {
                    port.readback
                        .unexpected_point_ids_by_effect
                        .push((effect, unexpected));
                }
            }
        }

        self.configure_effect_outcomes(
            port,
            self.observation.staged_ids == expected_stage,
            self.observation.closed_ids == expected_closed,
        );
    }

    fn configure_effect_outcomes(
        &self,
        port: &mut PhasePort,
        stage_full: bool,
        closure_full: bool,
    ) {
        let scripted = port.readback.outcomes.clone();
        port.readback.outcomes = vec![
            (
                PublicationEffectKind::PersistIntent,
                if self.observation.intent_durable {
                    PublicationEffectOutcome::Applied
                } else {
                    PublicationEffectOutcome::NotApplied
                },
            ),
            (
                PublicationEffectKind::StageNewPoints,
                if stage_full || !self.observation.staged_ids.is_empty() {
                    PublicationEffectOutcome::Applied
                } else {
                    PublicationEffectOutcome::NotApplied
                },
            ),
            (
                PublicationEffectKind::CloseOldPoints,
                if closure_full && stage_full || !self.observation.closed_ids.is_empty() {
                    PublicationEffectOutcome::Applied
                } else {
                    PublicationEffectOutcome::NotApplied
                },
            ),
            (
                PublicationEffectKind::VerifyReadback,
                if stage_full && closure_full {
                    PublicationEffectOutcome::Applied
                } else {
                    PublicationEffectOutcome::NotApplied
                },
            ),
            (
                PublicationEffectKind::CommitVisibleEpoch,
                if self.observation.control_visible_epoch == self.intent.target_epoch {
                    PublicationEffectOutcome::Applied
                } else if self.observation.control_visible_epoch == self.previous_visible_epoch {
                    PublicationEffectOutcome::NotApplied
                } else {
                    PublicationEffectOutcome::Conflict
                },
            ),
            // A prior-process snapshot flag is not a current-process acknowledgement.
            (
                PublicationEffectKind::PublishControlSnapshot,
                PublicationEffectOutcome::NotApplied,
            ),
        ];
        for (effect, outcome) in scripted {
            if let Some(existing) = port
                .readback
                .outcomes
                .iter_mut()
                .find(|(current, _)| *current == effect)
            {
                existing.1 = outcome;
            } else {
                port.readback.outcomes.push((effect, outcome));
            }
        }
    }

    fn configure_intent_readback(&self, port: &mut PhasePort, new: &Fixture) {
        let mut binding = self.intent_binding.clone();
        binding
            .transaction_id
            .clone_from(&self.intent.transaction_id);
        binding
            .operation_id
            .clone_from(&self.intent.persist_operation_id);
        binding.target_epoch = self.intent.target_epoch;
        binding.old_manifest_digest = self.intent.old_manifest_digest;
        binding.new_manifest_digest = self.intent.new_manifest_digest;
        binding.guards = self.intent.guards;
        if port.readback.intent_binding_override.is_none() {
            port.readback.intent_binding_override = Some(binding);
        }
        if port.readback.intent_operation_id_override.is_none() {
            port.readback.intent_operation_id_override =
                Some(self.intent.persist_operation_id.clone());
        }
        if port.readback.intent_transaction_id_override.is_none() {
            port.readback.intent_transaction_id_override = Some(self.intent.transaction_id.clone());
        }
        if port.readback.intent_target_epoch_override.is_none() {
            port.readback.intent_target_epoch_override = Some(self.intent.target_epoch);
        }
        if port.readback.intent_old_manifest_digest_override == OptionalOverride::Keep {
            port.readback.intent_old_manifest_digest_override =
                OptionalOverride::Set(self.intent.old_manifest_digest);
        }
        if port.readback.intent_new_manifest_digest_override.is_none() {
            port.readback.intent_new_manifest_digest_override =
                Some(self.intent.new_manifest_digest);
        }
        if port.readback.intent_guards_override.is_none() {
            port.readback.intent_guards_override = Some(self.intent.guards);
        }
        if port.readback.intent_route_generation_override.is_none()
            && self.collection_generation_id != new.scope.membership.collection_generation_id
        {
            port.readback.intent_route_generation_override = Some(self.collection_generation_id);
        }
    }

    fn configure_stage_readback(
        &self,
        port: &mut PhasePort,
        expected_stage: &[PointId128],
        target: Epoch,
    ) {
        let Some(stage) = &self.stage_receipt else {
            return;
        };
        if stage.transaction_id != self.case.transaction_id
            && port.readback.binding_transaction_override.is_none()
        {
            port.readback.binding_transaction_override = Some((
                PublicationEffectKind::StageNewPoints,
                stage.transaction_id.clone(),
            ));
        }
        if stage.target_epoch != target && port.readback.binding_target_epoch_override.is_none() {
            port.readback.binding_target_epoch_override =
                Some((PublicationEffectKind::StageNewPoints, stage.target_epoch));
        }
        if !stage.unexpected_ids.is_empty()
            && port.readback.unexpected_point_ids_by_effect.is_empty()
        {
            port.readback.unexpected_point_ids_by_effect.push((
                PublicationEffectKind::StageNewPoints,
                stage.unexpected_ids.clone(),
            ));
        }
        if !stage.missing_ids.is_empty() && port.readback.missing_point_ids_override.is_none() {
            port.readback.missing_point_ids_override = Some((
                PublicationEffectKind::StageNewPoints,
                stage.missing_ids.clone(),
            ));
        }
        let unexpected = unexpected_ids(&stage.staged_ids, expected_stage);
        if !unexpected.is_empty()
            && !port
                .readback
                .unexpected_point_ids_by_effect
                .iter()
                .any(|(effect, _)| *effect == PublicationEffectKind::StageNewPoints)
        {
            port.readback
                .unexpected_point_ids_by_effect
                .push((PublicationEffectKind::StageNewPoints, unexpected));
        }
        let missing = expected_stage
            .iter()
            .copied()
            .filter(|id| !stage.staged_ids.contains(id))
            .collect::<Vec<_>>();
        if !missing.is_empty() && port.readback.missing_point_ids_override.is_none() {
            port.readback.missing_point_ids_override =
                Some((PublicationEffectKind::StageNewPoints, missing));
        }
        if stage.readback_digest != c::Blake3Digest32::from_bytes([3; 32])
            && port.readback.corrupt_vector.is_none()
            && let Some(id) = expected_stage.first()
        {
            port.readback.corrupt_vector = Some((PublicationEffectKind::StageNewPoints, *id));
        }
    }

    fn configure_closure_readback(
        &self,
        port: &mut PhasePort,
        expected_closed: &[PointId128],
        target: Epoch,
    ) {
        let Some(closure) = &self.closure_receipt else {
            return;
        };
        if closure.transaction_id != self.case.transaction_id
            && port.readback.binding_transaction_override.is_none()
        {
            port.readback.binding_transaction_override = Some((
                PublicationEffectKind::CloseOldPoints,
                closure.transaction_id.clone(),
            ));
        }
        if closure.target_epoch != target && port.readback.binding_target_epoch_override.is_none() {
            port.readback.binding_target_epoch_override =
                Some((PublicationEffectKind::CloseOldPoints, closure.target_epoch));
        }
        if !closure.unexpected_ids.is_empty()
            && port.readback.unexpected_point_ids_by_effect.is_empty()
        {
            port.readback.unexpected_point_ids_by_effect.push((
                PublicationEffectKind::CloseOldPoints,
                closure.unexpected_ids.clone(),
            ));
        }
        if !closure.missing_ids.is_empty() && port.readback.missing_point_ids_override.is_none() {
            port.readback.missing_point_ids_override = Some((
                PublicationEffectKind::CloseOldPoints,
                closure.missing_ids.clone(),
            ));
        }
        let unexpected = unexpected_ids(&closure.closed_ids, expected_closed);
        if !unexpected.is_empty()
            && !port
                .readback
                .unexpected_point_ids_by_effect
                .iter()
                .any(|(effect, _)| *effect == PublicationEffectKind::CloseOldPoints)
        {
            port.readback
                .unexpected_point_ids_by_effect
                .push((PublicationEffectKind::CloseOldPoints, unexpected));
        }
        let missing = expected_closed
            .iter()
            .copied()
            .filter(|id| !closure.closed_ids.contains(id))
            .collect::<Vec<_>>();
        if !missing.is_empty() && port.readback.missing_point_ids_override.is_none() {
            port.readback.missing_point_ids_override =
                Some((PublicationEffectKind::CloseOldPoints, missing));
        }
        if closure.readback_digest != c::Blake3Digest32::from_bytes([4; 32])
            && port.readback.corrupt_point_payload.is_none()
            && let Some(id) = expected_closed.first()
        {
            port.readback.corrupt_point_payload =
                Some((PublicationEffectKind::CloseOldPoints, *id));
        }
    }

    fn configure_verified_readback(
        &self,
        port: &mut PhasePort,
        expected_stage: &[PointId128],
        expected_closed: &[PointId128],
        target: Epoch,
    ) {
        let Some(verified) = &self.verified else {
            return;
        };
        if verified.transaction_id != self.case.transaction_id
            && port.readback.binding_transaction_override.is_none()
        {
            port.readback.binding_transaction_override = Some((
                PublicationEffectKind::VerifyReadback,
                verified.transaction_id.clone(),
            ));
        }
        if verified.target_epoch != target && port.readback.binding_target_epoch_override.is_none()
        {
            port.readback.binding_target_epoch_override =
                Some((PublicationEffectKind::VerifyReadback, verified.target_epoch));
        }
        if verified.new_manifest_digest != self.intent.new_manifest_digest
            && port.readback.binding_new_manifest_override.is_none()
        {
            port.readback.binding_new_manifest_override = Some((
                PublicationEffectKind::VerifyReadback,
                verified.new_manifest_digest,
            ));
        }
        if verified.staged_readback_digest != c::Blake3Digest32::from_bytes([3; 32])
            && port.readback.corrupt_vector.is_none()
            && let Some(id) = expected_stage.first()
        {
            port.readback.corrupt_vector = Some((PublicationEffectKind::VerifyReadback, *id));
        }
        if verified.closure_readback_digest != c::Blake3Digest32::from_bytes([4; 32])
            && port.readback.corrupt_point_payload.is_none()
            && let Some(id) = expected_closed.first()
        {
            port.readback.corrupt_point_payload =
                Some((PublicationEffectKind::VerifyReadback, *id));
        }
        if verified.retired_manifest_digest != self.intent.old_manifest_digest
            && port.readback.binding_old_manifest_override.is_none()
        {
            port.readback.binding_old_manifest_override = Some((
                PublicationEffectKind::VerifyReadback,
                verified.retired_manifest_digest,
            ));
        }
    }

    fn configure_misc_readback(&self, port: &mut PhasePort) {
        if !self.observation.unexpected_point_ids.is_empty()
            && port.readback.unexpected_point_ids_by_effect.is_empty()
        {
            port.readback.unexpected_point_ids_by_effect.extend([
                (
                    PublicationEffectKind::StageNewPoints,
                    self.observation.unexpected_point_ids.clone(),
                ),
                (
                    PublicationEffectKind::CloseOldPoints,
                    self.observation.unexpected_point_ids.clone(),
                ),
            ]);
        }
        if !self.observation.missing_point_ids.is_empty()
            && port.readback.missing_point_ids_override.is_none()
        {
            port.readback.missing_point_ids_override = Some((
                PublicationEffectKind::StageNewPoints,
                self.observation.missing_point_ids.clone(),
            ));
        }
    }
}

pub fn manifest_fixture(points: &[u8]) -> Fixture {
    projection_fixture(points)
}

pub fn projection_fixture_at(labels: &[u8], valid_from: Epoch) -> Fixture {
    projection_fixture(labels).at_epoch(valid_from)
}

pub fn receipt_ref(value: &str) -> ReceiptRef {
    ReceiptRef::new(format!("fixture:{value}")).expect("receipt reference")
}

//! Canonical producer fixtures and synthetic owner observations for integration tests.
//!
//! Source bytes pass through the real C08 materializer, C09 unitizer and C13 planner.
//! Owner observations and receipt labels below are test-binary-only and make no native,
//! journal, Qdrant, or crash-recovery proof claim.

#![allow(dead_code)]

use core::fmt;
use std::fmt::Write as _;
use std::ops::{Deref, DerefMut};

use search_contracts::{
    self as c, Epoch, OpaqueId, ReceiptRef, SourceRevisionRef,
    indexed::{
        CollectionSchema, NamedVectorRequirement, VectorName, VectorRequirement, VectorValue,
    },
};
use search_projection_planner::{
    LineageObservation, PlannedUnit, PlannedVector, PlanningScope, ProjectionBudget,
    ProjectionManifest, ProjectionMembershipBinding, ProjectionPlan, manifest_canonical_bytes,
    plan_projection,
};
use search_publication::{
    AdmissionPurpose, AdmissionWork, ProjectionEvidence, ProjectionUpdate,
    PublicationAdmissionOwners, PublicationCoordinator, PublicationError,
    PublicationMembershipState, PublicationOwnerObservation, PublicationProposal,
    PublicationRecoveryDecision, PublicationRestoreInput, PublicationScope,
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

fn projection_plan(
    units: &VerifiedUnitSet,
    scope: &PlanningScope,
    schema: &CollectionSchema,
    changed_vector_at: Option<usize>,
) -> ProjectionPlan {
    let requirement = VectorRequirement::Sparse {
        index_ceiling: 8,
        idf_enabled: true,
    };
    let planned = units
        .units()
        .iter()
        .enumerate()
        .map(|(index, unit)| PlannedUnit {
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
                    values: if changed_vector_at == Some(index) {
                        vec![2.0, 1.0]
                    } else {
                        vec![1.0, 2.0]
                    },
                },
            }],
        })
        .collect::<Vec<_>>();
    let requirements = [("lexical-sparse".to_owned(), requirement)].into();
    let plan = plan_projection(
        units,
        scope,
        &planned,
        &requirements,
        ProjectionBudget::BASELINE,
    )
    .expect("C13 projection plan");
    search_projection_planner::validate_schema(schema, &plan, ProjectionBudget::BASELINE)
        .expect("C13 schema validation");
    plan
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct Fixture {
    pub source: SourceRevisionRef,
    owner_source: SourceRevisionRef,
    pub units: VerifiedUnitSet,
    pub plan: ProjectionPlan,
    pub scope: PlanningScope,
    pub schema: CollectionSchema,
    sealed_plan: ProjectionPlan,
    sealed_scope: PlanningScope,
    retargetable: bool,
    changed_vector_at: Option<usize>,
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
        let mut plan = projection_plan(&units, &scope, &schema, None);
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
            sealed_plan,
            sealed_scope,
            retargetable: !reject_as_empty,
            changed_vector_at: None,
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
            retargeted.changed_vector_at,
        );
        retargeted.sealed_plan = retargeted.plan.clone();
        retargeted.sealed_scope = retargeted.scope.clone();
        retargeted
    }

    /// Replans one actual C13 point with a different vector while preserving
    /// the same verified C08/C09 source, representation, and point address.
    pub fn with_changed_vector_at(&self, unit_index: usize) -> Self {
        assert!(unit_index < self.units.units().len());
        let mut changed = self.clone();
        changed.changed_vector_at = Some(unit_index);
        changed.plan = projection_plan(
            &changed.units,
            &changed.scope,
            &changed.schema,
            changed.changed_vector_at,
        );
        changed.sealed_plan = changed.plan.clone();
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
        Self::submit(
            self,
            &proposal,
            &mut owners,
            &snapshot,
            ProjectionBudget::BASELINE,
        )
    }
}

#[derive(Clone)]
pub struct RestoreScenario {
    pub case: PublicationCase,
    pub previous_visible_epoch: Epoch,
    pub last_reserved_epoch: Epoch,
    pub collection_generation_id: c::CollectionGenerationId,
    pub current_guards: c::PublicationGuards,
    pub phase: search_publication::PublicationPhase,
    pub intent: search_publication::DurableIntent,
    pub stage_receipt: Option<search_publication::StageReceipt>,
    pub closure_receipt: Option<search_publication::ClosureReceipt>,
    pub verified: Option<search_publication::ReadbackVerified>,
    pub control_commit: Option<search_publication::ControlCommitObservation>,
    pub observation: search_publication::PublicationRecoveryObservation,
}

impl fmt::Debug for RestoreScenario {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("RestoreScenario")
            .field("previous_visible_epoch", &self.previous_visible_epoch)
            .field("last_reserved_epoch", &self.last_reserved_epoch)
            .field("phase", &self.phase)
            .field("has_stage_receipt", &self.stage_receipt.is_some())
            .field("has_closure_receipt", &self.closure_receipt.is_some())
            .field("has_verified_readback", &self.verified.is_some())
            .field("has_control_commit", &self.control_commit.is_some())
            .finish_non_exhaustive()
    }
}

impl RestoreScenario {
    pub fn restore(
        &self,
        max_points: usize,
    ) -> Result<(PublicationCoordinator, PublicationRecoveryDecision), PublicationError> {
        let target = self.intent.target_epoch;
        let (old, new) = retargeted_tokens(&self.case, self.previous_visible_epoch, target);
        let expected_manifest = if self.observation.control_visible_epoch == target {
            (!self.case.retirement).then(|| new.plan.manifest.clone())
        } else {
            old.as_ref().map(|fixture| fixture.plan.manifest.clone())
        };
        let snapshot = owner_snapshot(
            &self.case,
            self.previous_visible_epoch,
            self.observation.control_visible_epoch,
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
        let input = PublicationRestoreInput {
            prepared: proposal,
            previous_visible_epoch: self.previous_visible_epoch,
            last_reserved_epoch: self.last_reserved_epoch,
            collection_generation_id: self.collection_generation_id,
            current_guards: self.current_guards,
            phase: self.phase,
            intent: self.intent.clone(),
            stage_receipt: self.stage_receipt.clone(),
            closure_receipt: self.closure_receipt.clone(),
            verified: self.verified.clone(),
            control_commit: self.control_commit,
            observation: self.observation.clone(),
        };
        let mut owners = Owners {
            observations: 0,
            drift_after_first: self.case.drift_after_first_observation,
        };
        PublicationCoordinator::restore_inflight(
            input,
            &mut owners,
            &snapshot,
            ProjectionBudget {
                max_points,
                ..ProjectionBudget::BASELINE
            },
        )
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

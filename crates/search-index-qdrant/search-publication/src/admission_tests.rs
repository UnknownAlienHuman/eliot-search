//! Actual C08/C09/C13 data with test-only owner observations; no effect proof.

mod support;
use crate::*;
use search_contracts::{self as c, indexed::*};
use search_projection_planner::{self as planner, *};
use search_unitizer::VerifiedUnitSet;
use std::collections::BTreeMap;

fn epoch(value: i64) -> c::Epoch {
    c::Epoch::new(value).unwrap()
}
fn guards() -> PublicationGuards {
    PublicationGuards {
        owner_epoch: c::OwnerEpoch::new(1).unwrap(),
        source_catalog_generation: 1,
        membership_generation: 1,
        access_generation: 1,
        shadow_generation: 1,
        purge_generation: 1,
        profile_digest: c::Blake3Digest32::from_bytes([1; 32]),
    }
}

struct Fixture {
    source: c::SourceRevisionRef,
    units: VerifiedUnitSet,
    plan: ProjectionPlan,
    scope: PlanningScope,
    schema: CollectionSchema,
}
impl Fixture {
    fn new(text: &str, revision: u8, from: i64) -> Self {
        let mut binding = support::binding();
        binding.source_revision_id = c::SourceRevisionId::from_bytes([revision; 16]);
        binding.materialization_id = c::MaterializationId::from_bytes([revision + 40; 16]);
        binding.representation_id = c::RepresentationId::from_bytes([revision + 50; 16]);
        let units = support::verified_unit_set(text, binding.clone());
        let scope = PlanningScope {
            membership: ProjectionMembershipBinding {
                projection_membership_id: c::ProjectionMembershipId::from_bytes([17; 16]),
                source_membership_id: c::SourceMembershipId::from_bytes([18; 16]),
                source_id: binding.source_id,
                source_revision_id: binding.source_revision_id,
                representation_id: binding.representation_id,
                projection_schema_id: c::ProfileId::new("publication-fixture-v1").unwrap(),
                collection_generation_id: c::CollectionGenerationId::from_bytes([19; 16]),
                installation_incarnation_id: c::InstallationIncarnationId::from_bytes([20; 16]),
                projection_profile_set_id: c::ProjectionProfileSetId::new("publication-fixture-v1")
                    .unwrap(),
                access_partition_id: c::AccessPartitionId::from_bytes([21; 16]),
                scoring_partition_id: c::ScoringPartitionId::from_bytes([22; 16]),
            },
            valid_from_epoch: epoch(from),
            valid_until_epoch_exclusive: None,
            lineage: LineageObservation {
                repository_lineage_id: None,
            },
            identity_limits: search_point_identity::s11::DEFAULT_LIMITS,
        };
        let requirement = VectorRequirement::Sparse {
            index_ceiling: 8,
            idf_enabled: true,
        };
        let vectors = vec![
            NamedVectorRequirement::new(VectorName::new("lexical-sparse").unwrap(), requirement)
                .unwrap(),
        ];
        let schema = CollectionSchema::initial(
            scope.membership.installation_incarnation_id,
            scope.membership.collection_generation_id,
            scope.membership.projection_profile_set_id.clone(),
            &vectors,
        )
        .unwrap();
        let planned = units
            .units()
            .iter()
            .map(|unit| PlannedUnit {
                unit_id: unit.unit_id(),
                unit_kind: unit.occurrence().unit_kind,
                modality: c::Modality::Text,
                language_or_format: c::BoundedName::new("text").unwrap(),
                entity_kind: None,
                normalized_symbol_key: None,
                unit_digest: unit.unit_content_digest(),
                reference_digest: unit.reference_digest(),
                vectors: vec![PlannedVector {
                    name: vectors[0].name().clone(),
                    value: VectorValue::Sparse {
                        indices: vec![1, 4],
                        values: vec![1.0, 2.0],
                    },
                }],
            })
            .collect::<Vec<_>>();
        let requirements = BTreeMap::from([("lexical-sparse".to_owned(), requirement)]);
        let plan = plan_projection(
            &units,
            &scope,
            &planned,
            &requirements,
            ProjectionBudget::BASELINE,
        )
        .unwrap();
        let source = c::SourceRevisionRef {
            source_namespace_id: binding.source_namespace_id,
            source_id: binding.source_id,
            revision_id: binding.source_revision_id,
            content_digest: units.manifest().provenance().input_digest(),
            byte_length: text.len() as u64,
        };
        Self {
            source,
            units,
            plan,
            scope,
            schema,
        }
    }
}

struct Snapshot {
    previous: c::Epoch,
    visible: c::Epoch,
    floor: c::Epoch,
    expected: Option<ProjectionManifest>,
    state: PublicationMembershipState,
    source: c::SourceRevisionRef,
}
struct Owners {
    observations: usize,
    drift: bool,
}
impl PublicationAdmissionOwners for Owners {
    type Projection = Fixture;
    type Snapshot = Snapshot;
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
        snapshot: &Snapshot,
        _: &c::OpaqueId,
        scope: &PublicationScope,
        purpose: AdmissionPurpose,
    ) -> Result<PublicationOwnerObservation, PublicationError> {
        if scope.source != snapshot.source
            || scope.source_membership_id != c::SourceMembershipId::from_bytes([18; 16])
        {
            return Err(PublicationError::GuardMismatch);
        }
        self.observations += 1;
        let mut current = guards();
        if self.drift && self.observations > 1 {
            current.access_generation += 1;
        }
        Ok(PublicationOwnerObservation {
            original_guards: guards(),
            current_guards: current,
            previous_visible_epoch: snapshot.previous,
            current_visible_epoch: snapshot.visible,
            last_reserved_epoch: snapshot.floor,
            target_epoch: if purpose == AdmissionPurpose::Prepare {
                snapshot.floor.checked_next().unwrap()
            } else {
                snapshot.floor
            },
            membership_state: snapshot.state,
        })
    }
    fn verify_current_manifest(
        &mut self,
        snapshot: &Snapshot,
        _: &PublicationScope,
        expected: Option<&ProjectionManifest>,
    ) -> Result<(), PublicationError> {
        if expected != snapshot.expected.as_ref() {
            return Err(PublicationError::ControlConflict);
        }
        Ok(())
    }
    fn work(&mut self, _: &Snapshot, _: AdmissionWork) -> Result<(), PublicationError> {
        Ok(())
    }
}
fn owners() -> Owners {
    Owners {
        observations: 0,
        drift: false,
    }
}
fn snapshot(
    fixture: &Fixture,
    old: Option<&Fixture>,
    previous: i64,
    floor: i64,
    state: PublicationMembershipState,
) -> Snapshot {
    Snapshot {
        previous: epoch(previous),
        visible: epoch(previous),
        floor: epoch(floor),
        expected: old.map(|f| f.plan.manifest.clone()),
        source: fixture.source,
        state,
    }
}
fn proposal<'a>(old: Option<&'a Fixture>, new: &'a Fixture) -> PublicationProposal<'a, Fixture> {
    PublicationProposal {
        transaction_id: c::OpaqueId::new("canonical-publication-fixture").unwrap(),
        old,
        update: ProjectionUpdate::Publish(new),
    }
}

#[test]
fn canonical_prepare_and_restore_recompute_the_same_frozen_input() {
    let new = Fixture::new("alpha\nbeta\ngamma\n", 3, 1);
    let proposal = proposal(None, &new);
    let prepare = snapshot(&new, None, 0, 0, PublicationMembershipState::Active);
    let normal = validate_prepared_generation(
        &mut owners(),
        &prepare,
        &proposal,
        AdmissionPurpose::Prepare,
        ProjectionBudget::BASELINE,
    )
    .unwrap();
    let restore = snapshot(&new, None, 0, 1, PublicationMembershipState::Active);
    let restored = validate_prepared_generation(
        &mut owners(),
        &restore,
        &proposal,
        AdmissionPurpose::Restore,
        ProjectionBudget::BASELINE,
    )
    .unwrap();
    assert_eq!(
        normal.prepared_input_digest(),
        restored.prepared_input_digest()
    );
    assert_eq!(normal.new_manifest(), restored.new_manifest());
    assert_eq!(normal.difference(), restored.difference());
    let mut coordinator = PublicationCoordinator::new(64).unwrap();
    assert_eq!(
        coordinator.submit(
            &proposal,
            &mut owners(),
            &prepare,
            ProjectionBudget::BASELINE
        ),
        Ok(epoch(1))
    );
    let intent = coordinator
        .persist_intent(
            c::OpaqueId::new("fixture-intent").unwrap(),
            c::ReceiptRef::new("fixture-correlation").unwrap(),
        )
        .unwrap();
    let input = PublicationRestoreInput {
        prepared: proposal,
        previous_visible_epoch: epoch(0),
        last_reserved_epoch: epoch(1),
        collection_generation_id: new.scope.membership.collection_generation_id,
        current_guards: guards(),
        phase: PublicationPhase::IntentDurable,
        intent,
        stage_receipt: None,
        closure_receipt: None,
        verified: None,
        control_commit: None,
        observation: PublicationRecoveryObservation {
            intent_durable: true,
            staged_ids: Vec::new(),
            closed_ids: Vec::new(),
            control_visible_epoch: epoch(0),
            snapshot_published: false,
            abandon_fence_durable: false,
        },
    };
    let (restored, decision) = PublicationCoordinator::restore_inflight(
        input,
        &mut owners(),
        &restore,
        ProjectionBudget::BASELINE,
    )
    .unwrap();
    assert_eq!(decision, PublicationRecoveryDecision::Continue);
    assert_eq!(
        restored.active().unwrap().prepared.prepared_input_digest(),
        normal.prepared_input_digest()
    );
}

#[test]
fn canonical_invalid_input_never_consumes_the_slot_or_epoch() {
    let mut fixture = Fixture::new("alpha\nbeta\ngamma\n", 3, 1);
    fixture.plan.manifest.canonical_bytes[0] ^= 1;
    let context = snapshot(&fixture, None, 0, 0, PublicationMembershipState::Active);
    let mut coordinator = PublicationCoordinator::new(64).unwrap();
    assert!(
        coordinator
            .submit(
                &proposal(None, &fixture),
                &mut owners(),
                &context,
                ProjectionBudget::BASELINE
            )
            .is_err()
    );
    assert!(coordinator.active().is_none());
    assert_eq!(coordinator.last_reserved_epoch(), None);
    assert_eq!(coordinator.visible_epoch(), None);
    let mut fixture = Fixture::new("alpha\nbeta\ngamma\n", 3, 1);
    fixture.plan.points.pop();
    fixture.plan.manifest.entries.pop();
    fixture.plan.manifest.canonical_bytes =
        planner::manifest_canonical_bytes(&fixture.plan.manifest, ProjectionBudget::BASELINE)
            .unwrap()
            .as_slice()
            .to_vec();
    let context = snapshot(&fixture, None, 0, 0, PublicationMembershipState::Active);
    assert!(
        coordinator
            .submit(
                &proposal(None, &fixture),
                &mut owners(),
                &context,
                ProjectionBudget::BASELINE
            )
            .is_err()
    );
    assert!(coordinator.active().is_none());
    assert_eq!(coordinator.last_reserved_epoch(), None);
}

#[test]
fn canonical_retirement_is_explicit_and_retained_bounds_are_unchanged() {
    let old = Fixture::new("alpha\nbeta\ngamma\n", 3, 1);
    let context = snapshot(&old, Some(&old), 1, 1, PublicationMembershipState::Retired);
    let retirement = PublicationProposal {
        transaction_id: c::OpaqueId::new("fixture-retirement").unwrap(),
        old: Some(&old),
        update: ProjectionUpdate::Retirement,
    };
    let retired = validate_prepared_generation(
        &mut owners(),
        &context,
        &retirement,
        AdmissionPurpose::Prepare,
        ProjectionBudget::BASELINE,
    )
    .unwrap();
    assert!(matches!(retired.target(), PublicationTarget::Retirement(_)));
    assert!(retired.new_manifest().entries.is_empty());
    assert_eq!(retired.difference().retire.len(), old.units.units().len());
    let context = snapshot(&old, Some(&old), 1, 1, PublicationMembershipState::Active);
    assert!(
        validate_prepared_generation(
            &mut owners(),
            &context,
            &retirement,
            AdmissionPurpose::Prepare,
            ProjectionBudget::BASELINE
        )
        .is_err()
    );
    let unchanged = validate_prepared_generation(
        &mut owners(),
        &context,
        &proposal(Some(&old), &old),
        AdmissionPurpose::Prepare,
        ProjectionBudget::BASELINE,
    )
    .unwrap();
    assert!(unchanged.difference().create.is_empty());
    assert!(unchanged.difference().retire.is_empty());
    assert_eq!(unchanged.difference().retain.len(), old.units.units().len());
}

#[test]
fn canonical_replacement_checks_current_owner_after_validation() {
    let old = Fixture::new("alpha\nbeta\ngamma\n", 3, 1);
    let new = Fixture::new("delta\nepsilon\nzeta\n", 4, 2);
    let context = snapshot(&new, Some(&old), 1, 1, PublicationMembershipState::Active);
    let replacement = proposal(Some(&old), &new);
    let admitted = validate_prepared_generation(
        &mut owners(),
        &context,
        &replacement,
        AdmissionPurpose::Prepare,
        ProjectionBudget::BASELINE,
    )
    .unwrap();
    assert_eq!(admitted.difference().create.len(), new.units.units().len());
    assert_eq!(admitted.difference().retire.len(), old.units.units().len());
    let mut drift = owners();
    drift.drift = true;
    let mut coordinator = PublicationCoordinator::new(64).unwrap();
    assert!(
        coordinator
            .submit(
                &replacement,
                &mut drift,
                &context,
                ProjectionBudget::BASELINE
            )
            .is_err()
    );
    assert!(coordinator.active().is_none());
    assert_eq!(coordinator.last_reserved_epoch(), None);
}

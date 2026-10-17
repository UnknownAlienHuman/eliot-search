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

fn c13_plan(
    units: &VerifiedUnitSet,
    scope: &PlanningScope,
    schema: &CollectionSchema,
    changed_vector_at: Option<usize>,
) -> ProjectionPlan {
    let requirement = VectorRequirement::Sparse {
        index_ceiling: 8,
        idf_enabled: true,
    };
    let vector_name = VectorName::new("lexical-sparse").unwrap();
    let planned = units
        .units()
        .iter()
        .enumerate()
        .map(|(index, unit)| PlannedUnit {
            unit_id: unit.unit_id(),
            unit_kind: unit.occurrence().unit_kind,
            modality: c::Modality::Text,
            language_or_format: c::BoundedName::new("text").unwrap(),
            entity_kind: None,
            normalized_symbol_key: None,
            unit_digest: unit.unit_content_digest(),
            reference_digest: unit.reference_digest(),
            vectors: vec![PlannedVector {
                name: vector_name.clone(),
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
    let requirements = BTreeMap::from([("lexical-sparse".to_owned(), requirement)]);
    let plan = plan_projection(
        units,
        scope,
        &planned,
        &requirements,
        ProjectionBudget::BASELINE,
    )
    .unwrap();
    planner::validate_schema(schema, &plan, ProjectionBudget::BASELINE).unwrap();
    plan
}

#[derive(Clone)]
struct Fixture {
    source: c::SourceRevisionRef,
    units: VerifiedUnitSet,
    plan: ProjectionPlan,
    scope: PlanningScope,
    schema: CollectionSchema,
    changed_vector_at: Option<usize>,
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
        let plan = c13_plan(&units, &scope, &schema, None);
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
            changed_vector_at: None,
        }
    }

    fn with_projection_membership(mut self, id: c::ProjectionMembershipId) -> Self {
        self.scope.membership.projection_membership_id = id;
        self.plan = c13_plan(
            &self.units,
            &self.scope,
            &self.schema,
            self.changed_vector_at,
        );
        self
    }

    fn with_changed_vector_at(mut self, unit_index: usize) -> Self {
        assert!(unit_index < self.units.units().len());
        self.changed_vector_at = Some(unit_index);
        self.plan = c13_plan(
            &self.units,
            &self.scope,
            &self.schema,
            self.changed_vector_at,
        );
        self
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

fn assert_uninitialized(coordinator: &PublicationCoordinator) {
    assert!(coordinator.active().is_none());
    assert_eq!(coordinator.visible_epoch(), None);
    assert_eq!(coordinator.last_reserved_epoch(), None);
    assert_eq!(coordinator.current_manifest(), None);
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
    assert_eq!(
        coordinator.submit(
            &proposal(None, &fixture),
            &mut owners(),
            &context,
            ProjectionBudget::BASELINE
        ),
        Err(PublicationError::InvalidPreparedPublication)
    );
    assert_uninitialized(&coordinator);
    let mut fixture = Fixture::new("alpha\nbeta\ngamma\n", 3, 1);
    fixture.plan.points.pop();
    fixture.plan.manifest.entries.pop();
    fixture.plan.manifest.canonical_bytes =
        planner::manifest_canonical_bytes(&fixture.plan.manifest, ProjectionBudget::BASELINE)
            .unwrap()
            .as_slice()
            .to_vec();
    let context = snapshot(&fixture, None, 0, 0, PublicationMembershipState::Active);
    assert_eq!(
        coordinator.submit(
            &proposal(None, &fixture),
            &mut owners(),
            &context,
            ProjectionBudget::BASELINE
        ),
        Err(PublicationError::InvalidPreparedPublication)
    );
    assert_uninitialized(&coordinator);
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
    assert_eq!(
        validate_prepared_generation(
            &mut owners(),
            &context,
            &retirement,
            AdmissionPurpose::Prepare,
            ProjectionBudget::BASELINE
        ),
        Err(PublicationError::InvalidPreparedPublication)
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
    assert_eq!(unchanged.difference().retain, old.plan.manifest.entries);
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
    assert_eq!(
        coordinator.submit(
            &replacement,
            &mut drift,
            &context,
            ProjectionBudget::BASELINE
        ),
        Err(PublicationError::GuardMismatch)
    );
    assert_uninitialized(&coordinator);
}

fn assert_old_manifest_corruption_is_rejected() {
    let mut corrupted_old = Fixture::new("alpha\nbeta\ngamma\n", 3, 1);
    let new = Fixture::new("delta\nepsilon\nzeta\n", 4, 2);
    corrupted_old.plan.manifest.canonical_bytes[0] ^= 1;
    let old_corruption_snapshot = snapshot(
        &new,
        Some(&corrupted_old),
        1,
        1,
        PublicationMembershipState::Active,
    );
    let mut coordinator = PublicationCoordinator::new(64).unwrap();
    assert_eq!(
        coordinator.submit(
            &proposal(Some(&corrupted_old), &new),
            &mut owners(),
            &old_corruption_snapshot,
            ProjectionBudget::BASELINE
        ),
        Err(PublicationError::InvalidPreparedPublication)
    );
    assert_uninitialized(&coordinator);
}

fn assert_mixed_scope_is_rejected() {
    let old = Fixture::new("alpha\nbeta\ngamma\n", 3, 1);
    let new = Fixture::new("delta\nepsilon\nzeta\n", 4, 2);
    let mixed_scope =
        new.with_projection_membership(c::ProjectionMembershipId::from_bytes([99; 16]));
    let mixed_snapshot = snapshot(
        &mixed_scope,
        Some(&old),
        1,
        1,
        PublicationMembershipState::Active,
    );
    let mut coordinator = PublicationCoordinator::new(64).unwrap();
    assert_eq!(
        coordinator.submit(
            &proposal(Some(&old), &mixed_scope),
            &mut owners(),
            &mixed_snapshot,
            ProjectionBudget::BASELINE
        ),
        Err(PublicationError::InvalidPreparedPublication)
    );
    assert_uninitialized(&coordinator);
}

fn assert_foreign_unit_set_is_rejected() {
    let foreign = Fixture::new("foreign\nsource\n", 9, 2);
    let mut foreign_units = Fixture::new("delta\nepsilon\nzeta\n", 4, 2);
    foreign_units.units = foreign.units;
    let foreign_snapshot = snapshot(
        &foreign_units,
        None,
        0,
        0,
        PublicationMembershipState::Active,
    );
    let mut coordinator = PublicationCoordinator::new(64).unwrap();
    assert_eq!(
        coordinator.submit(
            &proposal(None, &foreign_units),
            &mut owners(),
            &foreign_snapshot,
            ProjectionBudget::BASELINE
        ),
        Err(PublicationError::InvalidPreparedPublication)
    );
    assert_uninitialized(&coordinator);
}

fn assert_admission_budgets_precede_reservation() {
    let bounded = Fixture::new("alpha\nbeta\ngamma\n", 5, 1);
    let bounded_snapshot = snapshot(&bounded, None, 0, 0, PublicationMembershipState::Active);
    for too_small in [
        ProjectionBudget {
            max_points: bounded.plan.points.len().saturating_sub(1),
            ..ProjectionBudget::BASELINE
        },
        ProjectionBudget {
            max_manifest_bytes: bounded
                .plan
                .manifest
                .canonical_bytes
                .len()
                .saturating_sub(1),
            ..ProjectionBudget::BASELINE
        },
    ] {
        let mut coordinator = PublicationCoordinator::new(64).unwrap();
        assert_eq!(
            coordinator.submit(
                &proposal(None, &bounded),
                &mut owners(),
                &bounded_snapshot,
                too_small
            ),
            Err(PublicationError::BudgetExceeded)
        );
        assert_uninitialized(&coordinator);
    }
}

fn assert_input_permutation_is_stable() {
    let canonical = Fixture::new("alpha\nbeta\ngamma\n", 6, 1);
    let mut permuted = Fixture::new("alpha\nbeta\ngamma\n", 6, 1);
    permuted.plan.points.reverse();
    let canonical_snapshot = snapshot(&canonical, None, 0, 0, PublicationMembershipState::Active);
    let permuted_snapshot = snapshot(&permuted, None, 0, 0, PublicationMembershipState::Active);
    let canonical_prepared = validate_prepared_generation(
        &mut owners(),
        &canonical_snapshot,
        &proposal(None, &canonical),
        AdmissionPurpose::Prepare,
        ProjectionBudget::BASELINE,
    )
    .unwrap();
    let permuted_prepared = validate_prepared_generation(
        &mut owners(),
        &permuted_snapshot,
        &proposal(None, &permuted),
        AdmissionPurpose::Prepare,
        ProjectionBudget::BASELINE,
    )
    .unwrap();
    assert_eq!(canonical_prepared, permuted_prepared);
}

#[test]
fn canonical_admission_negatives_and_permutation_stability() {
    assert_old_manifest_corruption_is_rejected();
    assert_mixed_scope_is_rejected();
    assert_foreign_unit_set_is_rejected();
    assert_admission_budgets_precede_reservation();
    assert_input_permutation_is_stable();
}

#[test]
fn canonical_point_id_conflicts_and_disjoint_changes_use_real_c13_products() {
    let original = Fixture::new("alpha\nbeta\ngamma\n", 7, 1);
    let changed = original.clone().with_changed_vector_at(0);
    for fixture in [&original, &changed] {
        planner::verify_manifest_reconstruction(
            &fixture.plan.manifest,
            &fixture.plan.points,
            &fixture.scope,
            ProjectionBudget::BASELINE,
        )
        .expect("both C13 products reconstruct from their actual point specs");
    }
    assert_eq!(
        planner::diff_manifests(&original.plan.manifest, &changed.plan.manifest),
        Err(ProjectionError::ManifestMismatch)
    );
    let context = snapshot(
        &changed,
        Some(&original),
        1,
        1,
        PublicationMembershipState::Active,
    );
    let mut coordinator = PublicationCoordinator::new(64).unwrap();
    assert_eq!(
        coordinator.submit(
            &proposal(Some(&original), &changed),
            &mut owners(),
            &context,
            ProjectionBudget::BASELINE
        ),
        Err(PublicationError::InvalidPreparedPublication)
    );
    assert_uninitialized(&coordinator);

    let disjoint = Fixture::new("delta\nepsilon\nzeta\n", 8, 2).with_changed_vector_at(0);
    assert_ne!(original.source.revision_id, disjoint.source.revision_id);
    assert_ne!(
        original.scope.membership.representation_id,
        disjoint.scope.membership.representation_id
    );
    assert!(original.plan.manifest.entries.iter().all(|old_entry| {
        disjoint
            .plan
            .manifest
            .entries
            .iter()
            .all(|new_entry| old_entry.point_id != new_entry.point_id)
    }));
    let disjoint_context = snapshot(
        &disjoint,
        Some(&original),
        1,
        1,
        PublicationMembershipState::Active,
    );
    let mut coordinator = PublicationCoordinator::new(64).unwrap();
    assert_eq!(
        coordinator.submit(
            &proposal(Some(&original), &disjoint),
            &mut owners(),
            &disjoint_context,
            ProjectionBudget::BASELINE
        ),
        Ok(epoch(2))
    );
    let difference = coordinator.active().unwrap().prepared.difference();
    assert_eq!(difference.create, disjoint.plan.manifest.entries);
    assert_eq!(difference.retain, Vec::new());
    assert_eq!(difference.retire, original.plan.manifest.entries);
}

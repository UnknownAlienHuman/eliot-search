//! Focused causal fixtures for the pure projection planner.
//!
//! Every fixture below builds a real `VerifiedUnitSet` through the public
//! materializer and unitizer owners, then plans one exact membership-scoped
//! candidate projection. Scope coordinates are not registry authority; the
//! cases check exact unitizer facts without introducing a second planner.

mod support;

use std::collections::BTreeMap;

use search_contracts::indexed::{VectorName, VectorRequirement, VectorValue};
use search_contracts::{
    AccessPartitionId, Blake3ExactInputLimit, BoundedName, CollectionGenerationId, Epoch,
    InstallationIncarnationId, Modality, ProjectionMembershipId, ProjectionProfileSetId,
    ScoringPartitionId, SourceMembershipId, UnitId, blake3_exact_bytes,
};
use search_point_identity::s11::{DEFAULT_LIMITS, PointRole};
use search_projection_planner::{
    LineageObservation, PlannedUnit, PlannedVector, PlanningScope, ProjectionBudget,
    ProjectionError, ProjectionMembershipBinding, ProjectionPlan, diff_manifests, plan_projection,
    require_single_membership, validate_membership_isolation, validate_scope_binding,
    verified_projection_manifest_semantic_digest, verify_manifest_reconstruction,
};
use support::{Blake3Digest32, ProfileId, verified_unit_set};

/// The one sparse lexical requirement accepted by every fixture profile.
fn lexical_requirement() -> BTreeMap<String, VectorRequirement> {
    let name = VectorName::new("lexical-sparse").expect("closed vector name");
    BTreeMap::from([(
        name.as_str().to_owned(),
        VectorRequirement::Sparse {
            index_ceiling: 8,
            idf_enabled: true,
        },
    )])
}

/// One exact sparse encoder product for one unit occurrence.
fn sparse_vector() -> VectorValue {
    VectorValue::Sparse {
        indices: vec![1, 4],
        values: vec![1.0, 2.0],
    }
}

/// The exact membership binding every fixture plans.
fn membership() -> ProjectionMembershipBinding {
    ProjectionMembershipBinding {
        projection_membership_id: ProjectionMembershipId::from_bytes([0x11; 16]),
        source_membership_id: SourceMembershipId::from_bytes([0x22; 16]),
        source_id: support::SourceId::from_bytes([0x02; 16]),
        source_revision_id: support::SourceRevisionId::from_bytes([0x03; 16]),
        representation_id: support::RepresentationId::from_bytes([0x04; 16]),
        projection_schema_id: ProfileId::new("lexical-sparse-v1").expect("schema id"),
        collection_generation_id: CollectionGenerationId::from_bytes([0x33; 16]),
        installation_incarnation_id: InstallationIncarnationId::from_bytes([0x44; 16]),
        projection_profile_set_id: ProjectionProfileSetId::new("lexical-sparse-v1")
            .expect("profile set"),
        access_partition_id: AccessPartitionId::from_bytes([0x55; 16]),
        scoring_partition_id: ScoringPartitionId::from_bytes([0x66; 16]),
    }
}

/// The complete planning scope for one plan.
fn scope() -> PlanningScope {
    PlanningScope {
        membership: membership(),
        valid_from_epoch: Epoch::new(7).expect("epoch"),
        valid_until_epoch_exclusive: None,
        lineage: LineageObservation {
            repository_lineage_id: None,
        },
        identity_limits: DEFAULT_LIMITS,
    }
}

/// One exact planned unit for every verified unit in the set.
fn planned_units(text: &str) -> Vec<PlannedUnit> {
    verified_unit_set(text)
        .units()
        .iter()
        .map(|unit| PlannedUnit {
            unit_id: unit.unit_id(),
            unit_kind: unit.occurrence().unit_kind,
            modality: Modality::Text,
            language_or_format: BoundedName::new("text").expect("language"),
            entity_kind: None,
            normalized_symbol_key: None,
            unit_digest: unit.unit_content_digest(),
            reference_digest: unit.reference_digest(),
            vectors: vec![PlannedVector {
                name: VectorName::new("lexical-sparse").expect("name"),
                value: sparse_vector(),
            }],
        })
        .collect()
}

#[test]
fn complete_verified_unit_set_yields_exact_deterministic_plan() {
    let text = "alpha\nbeta\ngamma\n";
    let set = verified_unit_set(text);
    let units = planned_units(text);
    let requirements = lexical_requirement();
    let plan = plan_projection(
        &set,
        &scope(),
        &units,
        &requirements,
        ProjectionBudget::BASELINE,
    )
    .expect("complete plan");
    assert_eq!(plan.points.len(), set.units().len());
    assert_eq!(plan.manifest.entries.len(), set.units().len());
    validate_membership_isolation(&plan).expect("one membership per point");
}

#[test]
fn input_permutation_yields_identical_canonical_bytes() {
    let text = "one\ntwo\nthree\n";
    let set = verified_unit_set(text);
    let requirements = lexical_requirement();
    let budget = ProjectionBudget::BASELINE;
    let mut forward = planned_units(text);
    let baseline =
        plan_projection(&set, &scope(), &forward, &requirements, budget).expect("forward");
    forward.reverse();
    let reversed =
        plan_projection(&set, &scope(), &forward, &requirements, budget).expect("reversed");
    assert_eq!(
        baseline.manifest.canonical_bytes,
        reversed.manifest.canonical_bytes
    );
    assert_eq!(baseline.manifest.entries, reversed.manifest.entries);
}

#[test]
fn missing_verified_unit_fails_closed() {
    let text = "alpha\nbeta\ngamma\n";
    let set = verified_unit_set(text);
    let units = planned_units(text);
    let requirements = lexical_requirement();
    let dropped = units.iter().skip(1).cloned().collect::<Vec<_>>();
    assert_eq!(
        plan_projection(
            &set,
            &scope(),
            &dropped,
            &requirements,
            ProjectionBudget::BASELINE
        ),
        Err(ProjectionError::UnitSetTooLarge)
    );
    let mut foreign = units.clone();
    foreign.pop();
    foreign.push(PlannedUnit {
        unit_id: UnitId::from_bytes([0xAB; 16]),
        ..units[0].clone()
    });
    assert_eq!(
        plan_projection(
            &set,
            &scope(),
            &foreign,
            &requirements,
            ProjectionBudget::BASELINE
        ),
        Err(ProjectionError::ScopeMismatch)
    );
}

#[test]
fn duplicate_unit_refused_and_representation_binding_enforced() {
    let text = "alpha\nbeta\ngamma\n";
    let set = verified_unit_set(text);
    let units = planned_units(text);
    let requirements = lexical_requirement();
    let mut duplicated = units.clone();
    duplicated.push(units[0].clone());
    assert_eq!(
        plan_projection(
            &set,
            &scope(),
            &duplicated,
            &requirements,
            ProjectionBudget::BASELINE
        ),
        Err(ProjectionError::UnitSetTooLarge)
    );
    let mut same_count_duplicate = units;
    assert!(same_count_duplicate.len() > 1);
    same_count_duplicate[1] = same_count_duplicate[0].clone();
    assert_eq!(
        plan_projection(
            &set,
            &scope(),
            &same_count_duplicate,
            &requirements,
            ProjectionBudget::BASELINE
        ),
        Err(ProjectionError::DuplicateUnit)
    );
    let mut wrong = scope();
    wrong.membership.representation_id = search_contracts::RepresentationId::from_bytes([0x99; 16]);
    assert_eq!(
        validate_scope_binding(&set, &wrong, ProjectionBudget::BASELINE),
        Err(ProjectionError::RepresentationMismatch)
    );
}

#[test]
fn forged_unit_digests_and_kind_are_refused_before_point_construction() {
    let text = "alpha\nbeta\ngamma\n";
    let set = verified_unit_set(text);
    let units = planned_units(text);
    let requirements = lexical_requirement();
    let replacement = search_contracts::Blake3Digest32::from_bytes([0xee; 32]);
    assert_ne!(replacement, units[0].unit_digest);
    assert_ne!(replacement, units[0].reference_digest);
    for field in 0..3 {
        let mut forged = units.clone();
        match field {
            0 => forged[0].unit_digest = replacement,
            1 => forged[0].reference_digest = replacement,
            2 => forged[0].unit_kind = search_contracts::UnitKind::ImageRegion,
            _ => unreachable!(),
        }
        assert_eq!(
            plan_projection(
                &set,
                &scope(),
                &forged,
                &requirements,
                ProjectionBudget::BASELINE
            ),
            Err(ProjectionError::ScopeMismatch)
        );
    }

    let plan = plan_projection(
        &set,
        &scope(),
        &units,
        &requirements,
        ProjectionBudget::BASELINE,
    )
    .expect("owner facts accepted");
    let owner_by_id: BTreeMap<_, _> = set
        .units()
        .iter()
        .map(|unit| (unit.unit_id(), unit))
        .collect();
    for point in &plan.points {
        let descriptor = owner_by_id[&point.identity.key().unit_id];
        assert_eq!(
            point.expected_readback.unit_digest,
            descriptor.unit_content_digest()
        );
        assert_eq!(
            point.expected_readback.reference_digest,
            descriptor.reference_digest()
        );
        let payload = search_contracts::indexed::PointPayload::from_canonical_cbor(
            point.expected_readback.payload_cbor.as_slice(),
        )
        .expect("shared payload codec");
        assert_eq!(payload.unit_kind, descriptor.occurrence().unit_kind);
    }
    for entry in &plan.manifest.entries {
        let descriptor = owner_by_id[&entry.unit_id];
        assert_eq!(entry.unit_digest, descriptor.unit_content_digest());
        assert_eq!(entry.reference_digest, descriptor.reference_digest());
    }
}

#[test]
fn matching_representation_cannot_hide_a_wrong_source_or_revision() {
    let text = "alpha\nbeta\ngamma\n";
    let set = verified_unit_set(text);
    let units = planned_units(text);
    let binding = set.manifest().provenance().binding();
    assert_eq!(
        binding.materialization_id,
        set.representation().materialization_id
    );
    assert_eq!(
        binding.representation_id,
        scope().membership.representation_id
    );
    for wrong_revision in [false, true] {
        let mut wrong = scope();
        if wrong_revision {
            wrong.membership.source_revision_id = support::SourceRevisionId::from_bytes([0xff; 16]);
        } else {
            wrong.membership.source_id = support::SourceId::from_bytes([0xff; 16]);
        }
        assert_eq!(
            validate_scope_binding(&set, &wrong, ProjectionBudget::BASELINE),
            Err(ProjectionError::ScopeMismatch)
        );
        assert_eq!(
            plan_projection(
                &set,
                &wrong,
                &units,
                &lexical_requirement(),
                ProjectionBudget::BASELINE
            ),
            Err(ProjectionError::ScopeMismatch)
        );
    }
}

#[test]
fn membership_array_is_forbidden_and_manifest_binds_one_membership() {
    assert_eq!(
        require_single_membership(&[]),
        Err(ProjectionError::MembershipArrayForbidden)
    );
    assert_eq!(
        require_single_membership(&[
            ProjectionMembershipId::from_bytes([0x11; 16]),
            ProjectionMembershipId::from_bytes([0x12; 16]),
        ]),
        Err(ProjectionError::MembershipArrayForbidden)
    );

    let text = "alpha\nbeta\ngamma\n";
    let set = verified_unit_set(text);
    let units = planned_units(text);
    let requirements = lexical_requirement();
    let plan = plan_projection(
        &set,
        &scope(),
        &units,
        &requirements,
        ProjectionBudget::BASELINE,
    )
    .expect("plan");
    assert_eq!(
        plan.source_membership_id,
        SourceMembershipId::from_bytes([0x22; 16])
    );
    for entry in &plan.manifest.entries {
        assert_eq!(entry.source_membership_id, plan.source_membership_id);
        assert_eq!(
            entry.projection_membership_id,
            plan.projection_membership_id
        );
    }
}

#[test]
fn missing_duplicate_and_foreign_vector_refused() {
    let text = "alpha\nbeta\ngamma\n";
    let set = verified_unit_set(text);
    let units = planned_units(text);
    let requirements = lexical_requirement();

    let subset = units
        .iter()
        .map(|unit| PlannedUnit {
            vectors: Vec::new(),
            ..unit.clone()
        })
        .collect::<Vec<_>>();
    assert_eq!(
        plan_projection(
            &set,
            &scope(),
            &subset,
            &requirements,
            ProjectionBudget::BASELINE
        ),
        Err(ProjectionError::MissingVector)
    );

    let duplicate = units
        .iter()
        .map(|unit| PlannedUnit {
            vectors: vec![unit.vectors[0].clone(), unit.vectors[0].clone()],
            ..unit.clone()
        })
        .collect::<Vec<_>>();
    assert_eq!(
        plan_projection(
            &set,
            &scope(),
            &duplicate,
            &requirements,
            ProjectionBudget::BASELINE
        ),
        Err(ProjectionError::MissingVector)
    );

    let foreign = units
        .iter()
        .map(|unit| PlannedUnit {
            vectors: vec![PlannedVector {
                name: VectorName::new("dense-other").expect("name"),
                value: sparse_vector(),
            }],
            ..unit.clone()
        })
        .collect::<Vec<_>>();
    assert_eq!(
        plan_projection(
            &set,
            &scope(),
            &foreign,
            &requirements,
            ProjectionBudget::BASELINE
        ),
        Err(ProjectionError::VectorNameMismatch)
    );
}

#[test]
fn nonfinite_vector_refused() {
    let text = "alpha\nbeta\ngamma\n";
    let set = verified_unit_set(text);
    let units = planned_units(text);
    let requirements = lexical_requirement();
    let nonfinite = units
        .iter()
        .map(|unit| PlannedUnit {
            vectors: vec![PlannedVector {
                name: unit.vectors[0].name.clone(),
                value: VectorValue::Sparse {
                    indices: vec![1],
                    values: vec![f32::NAN],
                },
            }],
            ..unit.clone()
        })
        .collect::<Vec<_>>();
    assert_eq!(
        plan_projection(
            &set,
            &scope(),
            &nonfinite,
            &requirements,
            ProjectionBudget::BASELINE
        ),
        Err(ProjectionError::VectorShapeMismatch)
    );
}

#[test]
fn manifest_diff_separates_exact_create_retain_and_retire() {
    let text = "alpha\nbeta\ngamma\n";
    let set = verified_unit_set(text);
    let units = planned_units(text);
    let requirements = lexical_requirement();
    let budget = ProjectionBudget::BASELINE;
    let baseline =
        plan_projection(&set, &scope(), &units, &requirements, budget).expect("baseline");
    let again = plan_projection(&set, &scope(), &units, &requirements, budget).expect("again");
    let noop = diff_manifests(&baseline.manifest, &again.manifest).expect("noop diff");
    assert_eq!(noop.create.len(), 0);
    assert_eq!(noop.retain.len(), baseline.points.len());
    assert_eq!(noop.retire.len(), 0);

    let mut next = scope();
    next.membership.collection_generation_id = CollectionGenerationId::from_bytes([0x34; 16]);
    let changed = plan_projection(&set, &next, &units, &requirements, budget).expect("next");
    let diff = diff_manifests(&baseline.manifest, &changed.manifest).expect("diff");
    assert_eq!(diff.create.len(), baseline.points.len());
    assert_eq!(diff.retain.len(), 0);
    assert_eq!(diff.retire.len(), baseline.points.len());
}

#[test]
fn manifest_reconstruction_proves_rebuilt_bytes_equal_the_plan() {
    let text = "alpha\nbeta\ngamma\n";
    let set = verified_unit_set(text);
    let units = planned_units(text);
    let requirements = lexical_requirement();
    let budget = ProjectionBudget::BASELINE;
    let plan = plan_projection(&set, &scope(), &units, &requirements, budget).expect("plan");
    verify_manifest_reconstruction(&plan.manifest, &plan.points, &scope(), budget)
        .expect("reconstruction");

    let mut corrupted = plan.manifest.clone();
    corrupted.canonical_bytes.push(0);
    assert_eq!(
        verify_manifest_reconstruction(&corrupted, &plan.points, &scope(), budget),
        Err(ProjectionError::ManifestMismatch)
    );

    let mut swapped = plan.manifest.clone();
    swapped.entries.swap(0, 1);
    assert_eq!(
        verify_manifest_reconstruction(&swapped, &plan.points, &scope(), budget),
        Err(ProjectionError::ManifestMismatch)
    );
}

#[test]
fn verified_manifest_semantic_digest_rejects_stale_fields_and_is_domain_separated() {
    let text = "alpha\nbeta\ngamma\n";
    let set = verified_unit_set(text);
    let units = planned_units(text);
    let requirements = lexical_requirement();
    let budget = ProjectionBudget::BASELINE;
    let scope = scope();
    let plan = plan_projection(&set, &scope, &units, &requirements, budget).expect("plan");

    let semantic =
        verified_projection_manifest_semantic_digest(&plan.manifest, &plan.points, &scope, budget)
            .expect("verified semantic digest");
    let raw = blake3_exact_bytes(
        &plan.manifest.canonical_bytes,
        Blake3ExactInputLimit::new(plan.manifest.canonical_bytes.len()).expect("raw digest limit"),
    )
    .expect("raw body digest");
    assert_ne!(semantic, raw);

    let mut field_tampered = plan.manifest.clone();
    field_tampered.entries[0].unit_digest = Blake3Digest32::from_bytes([0xee; 32]);
    assert_eq!(
        verified_projection_manifest_semantic_digest(&field_tampered, &plan.points, &scope, budget,),
        Err(ProjectionError::ManifestMismatch)
    );

    let mut bytes_tampered = plan.manifest.clone();
    bytes_tampered.canonical_bytes[0] ^= 1;
    assert_eq!(
        verified_projection_manifest_semantic_digest(&bytes_tampered, &plan.points, &scope, budget,),
        Err(ProjectionError::ManifestMismatch)
    );
}

#[test]
fn zero_epoch_and_sub_budget_refused() {
    assert_eq!(
        ProjectionBudget {
            max_points: 0,
            max_manifest_bytes: 64,
        }
        .validate(),
        Err(ProjectionError::InvalidLimits)
    );
    let mut bad = scope();
    bad.valid_from_epoch = Epoch::new(0).expect("zero epoch");
    bad.valid_until_epoch_exclusive = Some(Epoch::new(1).expect("one"));
    let text = "alpha\n";
    let set = verified_unit_set(text);
    let units = planned_units(text);
    assert_eq!(
        plan_projection(
            &set,
            &bad,
            &units,
            &lexical_requirement(),
            ProjectionBudget::BASELINE
        ),
        Err(ProjectionError::ScopeMismatch)
    );

    // A budget below the verified set size cannot plan a complete set.
    let narrow_text = "alpha\nbeta\ngamma\n";
    let narrow_set = verified_unit_set(narrow_text);
    let narrow_units = planned_units(narrow_text);
    let narrow = ProjectionBudget {
        max_points: 1,
        max_manifest_bytes: 1024,
    };
    assert_eq!(
        plan_projection(
            &narrow_set,
            &scope(),
            &narrow_units,
            &lexical_requirement(),
            narrow
        ),
        Err(ProjectionError::UnitSetTooLarge)
    );
}

#[test]
fn plan_exposes_full_s11_identity_every_point() {
    let text = "alpha\nbeta\n";
    let set = verified_unit_set(text);
    let units = planned_units(text);
    let requirements = lexical_requirement();
    let plan: ProjectionPlan = plan_projection(
        &set,
        &scope(),
        &units,
        &requirements,
        ProjectionBudget::BASELINE,
    )
    .expect("plan");
    for point in &plan.points {
        assert_eq!(point.identity.key().point_role, PointRole::Unit);
        let observed: &[u8; 16] = point.point_id.as_bytes();
        assert_eq!(observed.len(), 16);
    }
}

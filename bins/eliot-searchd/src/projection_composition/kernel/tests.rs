use super::spec::{REFERENCE_BYTES, REFERENCE_MAGIC};
use super::*;

use std::collections::BTreeMap;
use std::fs;
use std::path::PathBuf;

use search_contracts::{
    AccessPartitionId, Blake3Digest32, CollectionGenerationId, Epoch,
    InstallationIncarnationId, Modality, ProfileId, ProjectionMembershipId,
    ProjectionProfileSetId, RepresentationId, ScoringDocumentId,
    ScoringPartitionId, SourceId, SourceMembershipId, SourceRevisionId, UnitId,
    UnitKind,
};
use search_point_identity::DEFAULT_POINT_IDENTITY_LIMITS;
use search_projection_planner::{
    ExpectedUnit, NamedVector, ProjectionBudget, ProjectionProfiles,
    ScopeExpectation, VectorRequirement, VectorValue,
};

const TEST_BUDGET: ProjectionBudget = ProjectionBudget {
    max_points: 16,
    max_vectors_per_point: 4,
    max_vector_name_bytes: 64,
    max_stored_vector_values_per_point: 1_024,
    max_manifest_bytes: 1_048_576,
};

fn digest(byte: u8) -> Blake3Digest32 {
    Blake3Digest32::from_bytes([byte; 32])
}

fn profile_set() -> ProjectionProfileSetId {
    ProjectionProfileSetId::new("profile-set-v1").expect("profile set")
}

fn test_profiles() -> ProjectionProfiles {
    ProjectionProfiles {
        profile_set_id: profile_set(),
        profile_set_digest: digest(0xa0),
        vectors: BTreeMap::from([(
            "lex_code_v1".to_owned(),
            VectorRequirement {
                dimensions: 8,
                sparse: true,
            },
        )]),
    }
}

fn test_vectors() -> Vec<NamedVector> {
    let mut vector = NamedVector {
        name: "lex_code_v1".to_owned(),
        dimensions: 8,
        value: VectorValue::Sparse {
            indices: vec![1, 4],
            values: vec![1.0, 2.0],
        },
        digest: digest(0),
    };
    vector.digest = vector.canonical_digest();
    vec![vector]
}

fn test_scope(generation: u8) -> ScopeExpectation {
    ScopeExpectation {
        installation_incarnation_id:
            InstallationIncarnationId::from_bytes([1; 16]),
        collection_generation_id:
            CollectionGenerationId::from_bytes([generation; 16]),
        source_membership_id: SourceMembershipId::from_bytes([3; 16]),
        projection_membership_id:
            ProjectionMembershipId::from_bytes([4; 16]),
        access_partition_id: AccessPartitionId::from_bytes([5; 16]),
        scoring_partition_id: ScoringPartitionId::from_bytes([6; 16]),
        source_id: SourceId::from_bytes([7; 16]),
        source_revision_id: SourceRevisionId::from_bytes([8; 16]),
        representation_id: RepresentationId::from_bytes([9; 16]),
        projection_profile_set_id: profile_set(),
        profile_set_digest: digest(0xa0),
        residency_digest: digest(0xb4),
    }
}

fn test_unit(ordinal: u64) -> ComposingUnit {
    let byte = u8::try_from(ordinal).unwrap_or(u8::MAX);
    ComposingUnit {
        receipt: AdmittedUnitReceipt {
            unit_id: UnitId::from_bytes([byte; 16]),
            scoring_document_id: ScoringDocumentId::from_bytes([
                byte.wrapping_add(1);
                16
            ]),
            unit_ordinal: ordinal,
            source_byte_start: ordinal * 100,
            source_byte_end: ordinal * 100 + 50,
            unit_digest: Blake3Digest32::from_bytes({
                let mut bytes = [0xc0; 32];
                bytes[0] = byte;
                bytes
            }),
            reference_digest: digest(0xc1),
            residency_digest: digest(0xb4),
            unit_kind: UnitKind::File,
            modality: Modality::Code,
            language_or_format: ProfileId::new("rust-v1")
                .expect("language profile"),
            entity_kind: None,
            normalized_symbol_key: None,
            repository_lineage_id: None,
        },
        vectors: test_vectors(),
    }
}

fn test_request(generation: u8, ordinals: &[u64]) -> CompositionRequest {
    let scope = test_scope(generation);
    let units = ordinals
        .iter()
        .map(|ordinal| test_unit(*ordinal))
        .collect::<Vec<_>>();
    let expected_units = units
        .iter()
        .map(|unit| ExpectedUnit {
            unit_id: unit.receipt.unit_id,
            unit_ordinal: unit.receipt.unit_ordinal,
            unit_digest: unit.receipt.unit_digest,
        })
        .collect::<Vec<_>>();
    CompositionRequest {
        scope,
        visible_epoch: Epoch::new(7).expect("epoch"),
        membership: MembershipReceipt {
            source_membership_id: SourceMembershipId::from_bytes([3; 16]),
            projection_membership_id:
                ProjectionMembershipId::from_bytes([4; 16]),
        },
        units,
        expected_units,
    }
}

fn test_root(name: &str) -> PathBuf {
    let root = std::env::temp_dir().join(format!(
        "eliot-search-t26-{}-{name}",
        std::process::id()
    ));
    let _ = fs::remove_dir_all(&root);
    fs::create_dir_all(&root).expect("test root");
    root
}

#[test]
fn compose_persists_reloads_and_recomposes_identical_bytes() {
    let root = test_root("roundtrip");
    let profiles = test_profiles();
    let request = test_request(0xc0, &[0, 1]);
    let plan = compose_scoped_projection(
        &request,
        &profiles,
        TEST_BUDGET,
        DEFAULT_POINT_IDENTITY_LIMITS,
    )
    .expect("compose");
    assert_eq!(plan.points.len(), 2);
    let stored = store_projection_manifest(
        &root,
        &plan,
        &request.scope,
        TEST_BUDGET,
    )
    .expect("store");
    let loaded = load_projection_manifest_bytes(
        &root,
        &stored.reference,
        TEST_BUDGET,
    )
    .expect("load");
    assert_eq!(loaded, plan.manifest.canonical_bytes);
    let recomposed = compose_scoped_projection(
        &request,
        &profiles,
        TEST_BUDGET,
        DEFAULT_POINT_IDENTITY_LIMITS,
    )
    .expect("recompose");
    assert_eq!(
        recomposed.manifest.canonical_bytes,
        plan.manifest.canonical_bytes
    );
    verify_stored_projection(&root, &stored, &recomposed, TEST_BUDGET)
        .expect("verify");
    let _ = fs::remove_dir_all(&root);
}

#[test]
fn persist_is_idempotent_for_same_bytes_and_conflicts_on_divergence() {
    let root = test_root("immutable");
    let profiles = test_profiles();
    let request = test_request(0xc0, &[0]);
    let plan = compose_scoped_projection(
        &request,
        &profiles,
        TEST_BUDGET,
        DEFAULT_POINT_IDENTITY_LIMITS,
    )
    .expect("compose");
    let first = store_projection_manifest(
        &root,
        &plan,
        &request.scope,
        TEST_BUDGET,
    )
    .expect("first store");
    let second = store_projection_manifest(
        &root,
        &plan,
        &request.scope,
        TEST_BUDGET,
    )
    .expect("replay store");
    assert_eq!(first, second);

    let mut diverged = request;
    diverged.units[0].receipt.unit_digest = digest(0xee);
    diverged.expected_units[0].unit_digest = digest(0xee);
    let diverged_plan = compose_scoped_projection(
        &diverged,
        &profiles,
        TEST_BUDGET,
        DEFAULT_POINT_IDENTITY_LIMITS,
    )
    .expect("diverged compose");
    assert_eq!(
        store_projection_manifest(
            &root,
            &diverged_plan,
            &diverged.scope,
            TEST_BUDGET,
        ),
        Err(ProjectionCompositionError::ReferenceConflict)
    );
    let _ = fs::remove_dir_all(&root);
}

#[test]
fn missing_wrong_residency_duplicate_propagate_typed_errors() {
    let profiles = test_profiles();
    let mut missing = test_request(0xc0, &[0]);
    missing.expected_units.push(ExpectedUnit {
        unit_id: UnitId::from_bytes([1; 16]),
        unit_ordinal: 1,
        unit_digest: digest(0xe0),
    });
    assert_eq!(
        compose_scoped_projection(
            &missing,
            &profiles,
            TEST_BUDGET,
            DEFAULT_POINT_IDENTITY_LIMITS,
        ),
        Err(ProjectionCompositionError::MissingUnitReceipt)
    );

    let mut residency = test_request(0xc0, &[0]);
    residency.units[0].receipt.residency_digest = digest(0xff);
    assert_eq!(
        compose_scoped_projection(
            &residency,
            &profiles,
            TEST_BUDGET,
            DEFAULT_POINT_IDENTITY_LIMITS,
        ),
        Err(ProjectionCompositionError::ResidencyMismatch)
    );

    let mut duplicate = test_request(0xc0, &[0]);
    duplicate.units.push(test_unit(0));
    assert_eq!(
        compose_scoped_projection(
            &duplicate,
            &profiles,
            TEST_BUDGET,
            DEFAULT_POINT_IDENTITY_LIMITS,
        ),
        Err(ProjectionCompositionError::DuplicatePoint)
    );

    let mut drifted = test_request(0xc0, &[0]);
    drifted.membership.projection_membership_id =
        ProjectionMembershipId::from_bytes([99; 16]);
    assert_eq!(
        compose_scoped_projection(
            &drifted,
            &profiles,
            TEST_BUDGET,
            DEFAULT_POINT_IDENTITY_LIMITS,
        ),
        Err(ProjectionCompositionError::MembershipMismatch)
    );
}

#[test]
fn one_source_two_memberships_yield_distinct_manifests() {
    let profiles = test_profiles();
    let first_request = test_request(0xc0, &[0]);
    let first = compose_scoped_projection(
        &first_request,
        &profiles,
        TEST_BUDGET,
        DEFAULT_POINT_IDENTITY_LIMITS,
    )
    .expect("first membership");

    let mut second_request = test_request(0xc0, &[0]);
    second_request.scope.source_membership_id =
        SourceMembershipId::from_bytes([30; 16]);
    second_request.scope.projection_membership_id =
        ProjectionMembershipId::from_bytes([40; 16]);
    second_request.membership.source_membership_id =
        second_request.scope.source_membership_id;
    second_request.membership.projection_membership_id =
        second_request.scope.projection_membership_id;
    let second = compose_scoped_projection(
        &second_request,
        &profiles,
        TEST_BUDGET,
        DEFAULT_POINT_IDENTITY_LIMITS,
    )
    .expect("second membership");
    assert_ne!(first.points[0].point_id, second.points[0].point_id);
    assert_ne!(
        first.manifest.canonical_bytes,
        second.manifest.canonical_bytes
    );
}

#[test]
fn generation_change_replaces_manifest_and_reference_scope() {
    let root = test_root("generation");
    let profiles = test_profiles();
    let baseline_request = test_request(0xc0, &[0]);
    let baseline = compose_scoped_projection(
        &baseline_request,
        &profiles,
        TEST_BUDGET,
        DEFAULT_POINT_IDENTITY_LIMITS,
    )
    .expect("baseline");
    let stored = store_projection_manifest(
        &root,
        &baseline,
        &baseline_request.scope,
        TEST_BUDGET,
    )
    .expect("store baseline");

    let rotated_request = test_request(0xc1, &[0]);
    let rotated = compose_scoped_projection(
        &rotated_request,
        &profiles,
        TEST_BUDGET,
        DEFAULT_POINT_IDENTITY_LIMITS,
    )
    .expect("rotated");
    assert_ne!(baseline.points[0].point_id, rotated.points[0].point_id);
    let rotated_stored = store_projection_manifest(
        &root,
        &rotated,
        &rotated_request.scope,
        TEST_BUDGET,
    )
    .expect("store rotated");
    assert_ne!(stored.reference.scope_key, rotated_stored.reference.scope_key);
    verify_stored_projection(&root, &stored, &baseline, TEST_BUDGET)
        .expect("baseline kept");
    let _ = fs::remove_dir_all(&root);
}

#[test]
fn reordered_units_yield_identical_manifest_bytes() {
    let profiles = test_profiles();
    let mut reversed = test_request(0xc0, &[0, 1]);
    reversed.units.reverse();
    let reversed_plan = compose_scoped_projection(
        &reversed,
        &profiles,
        TEST_BUDGET,
        DEFAULT_POINT_IDENTITY_LIMITS,
    )
    .expect("reversed");
    let ordered_plan = compose_scoped_projection(
        &test_request(0xc0, &[0, 1]),
        &profiles,
        TEST_BUDGET,
        DEFAULT_POINT_IDENTITY_LIMITS,
    )
    .expect("ordered");
    assert_eq!(
        reversed_plan.manifest.canonical_bytes,
        ordered_plan.manifest.canonical_bytes
    );
}

#[test]
fn reference_carries_no_source_bodies() {
    let root = test_root("reference");
    let profiles = test_profiles();
    let request = test_request(0xc0, &[0]);
    let plan = compose_scoped_projection(
        &request,
        &profiles,
        TEST_BUDGET,
        DEFAULT_POINT_IDENTITY_LIMITS,
    )
    .expect("compose");
    let stored = store_projection_manifest(
        &root,
        &plan,
        &request.scope,
        TEST_BUDGET,
    )
    .expect("store");
    let record = stored.reference.to_bytes();
    assert_eq!(record.len(), REFERENCE_BYTES);
    assert_eq!(&record[..8], REFERENCE_MAGIC);
    let parsed = ProjectionReference::from_bytes(&record).expect("parse");
    assert_eq!(parsed, stored.reference);
    assert_eq!(
        ProjectionReference::from_bytes(b"short"),
        Err(ProjectionCompositionError::ManifestInvalid)
    );
    let _ = fs::remove_dir_all(&root);
}

#[test]
fn payload_indexes_for_t24_are_exact_and_complete() {
    let indexes = expected_payload_indexes_for_bridge();
    assert_eq!(indexes.len(), 19);
    for field in [
        "installation_incarnation_id",
        "collection_generation_id",
        "projection_membership_id",
        "access_partition_id",
        "scoring_partition_id",
        "source_id",
        "source_revision_id",
        "representation_id",
        "unit_id",
        "scoring_document_id",
        "projection_profile_set_id",
        "unit_kind",
        "modality",
        "language_or_format",
        "entity_kind",
        "normalized_symbol_key",
        "repository_lineage_id",
        "valid_from_epoch",
        "valid_until_epoch_exclusive",
    ] {
        assert!(indexes.contains(&field), "missing payload index {field}");
    }
    for forbidden in [
        "source_membership_id",
        "payload_digest",
        "vector_digest",
        "raw_source_text",
    ] {
        assert!(!indexes.contains(&forbidden));
    }
}

#[test]
fn composition_error_codes_are_stable() {
    assert_eq!(
        ProjectionCompositionError::ScopeMismatch.code(),
        "PROJECTION_COMPOSITION_SCOPE_MISMATCH"
    );
    assert_eq!(
        ProjectionCompositionError::MissingUnitReceipt.code(),
        "PROJECTION_COMPOSITION_MISSING_UNIT_RECEIPT"
    );
    assert_eq!(
        ProjectionCompositionError::ReferenceConflict.code(),
        "PROJECTION_COMPOSITION_REFERENCE_CONFLICT"
    );
}

//! Disk, reopen, exact-recovery and conflicting-input proof for the C02 input binding.
use super::*;
use crate::{
    CommitRecoveryDecision, ControlError, ControlSnapshotPublisher, JournalIdentity,
    JournalProfileInitialization, MigrationBootstrapWork, MutationId, PersistentControlJournal,
    PreparationInputMetadataV1, PreparationInputRecordV1,
};
use search_contracts::indexed::{
    CollectionSchema, NamedVectorRequirement, VectorName, VectorRequirement,
};
use search_contracts::{
    self as c, AccessPartitionId, AccessPolicyBindingId, Blake3Digest32, CollectionGenerationId,
    CorpusId, InstallationIncarnationId, Materialization, MaterializationId, MembershipRevision,
    MembershipRole, ObjectResidencyKeyDigest, OpaqueId, ProfileId, ProjectionMembership,
    ProjectionMembershipId, ProjectionProfileSetId, Representation, RepresentationId,
    ResidencyPolicyBindingId, ScoringPartitionId, SourceId, SourceMembership, SourceMembershipId,
    SourceNamespaceId, SourceRevisionId, SourceRevisionRef, WorkspaceId,
};
use std::cell::Cell;
use std::time::{Duration, Instant};

fn collection_schema() -> CollectionSchema {
    let vectors = [NamedVectorRequirement::new(
        VectorName::new("unit-sparse").expect("valid vector name"),
        VectorRequirement::Sparse {
            index_ceiling: 1024,
            idf_enabled: true,
        },
    )
    .expect("valid sparse requirement")];
    CollectionSchema::initial(
        InstallationIncarnationId::from_bytes([1; 16]),
        CollectionGenerationId::from_bytes([5; 16]),
        ProjectionProfileSetId::new("projection:sparse-v1").expect("valid profile set"),
        &vectors,
    )
    .expect("valid collection schema")
}

fn canonical_u64(value: u64) -> Vec<u8> {
    c::to_canonical_cbor(&c::CanonicalValue::U64(value))
        .expect("canonical value")
        .as_slice()
        .to_vec()
}

fn metadata(schema: &CollectionSchema, policy_value: u64) -> PreparationInputMetadataV1 {
    let source_revision = SourceRevisionRef {
        source_namespace_id: SourceNamespaceId::from_bytes([41; 16]),
        source_id: SourceId::from_bytes([42; 16]),
        revision_id: SourceRevisionId::from_bytes([43; 16]),
        content_digest: Blake3Digest32::from_bytes([44; 32]),
        byte_length: 123,
    };
    let source_membership = SourceMembership {
        source_membership_id: SourceMembershipId::from_bytes([45; 16]),
        corpus_id: CorpusId::from_bytes([46; 16]),
        source_id: source_revision.source_id,
        workspace_id: WorkspaceId::from_bytes([47; 16]),
        role: MembershipRole::Source,
        preparation_profile_id: ProfileId::new("profile:prepared-text-v2")
            .expect("valid preparation profile"),
        access_policy_binding_id: AccessPolicyBindingId::from_bytes([48; 16]),
        retention_policy_id: ProfileId::new("policy:retention-v1")
            .expect("valid retention profile"),
        residency_policy_binding_id: ResidencyPolicyBindingId::from_bytes([49; 16]),
        membership_revision: MembershipRevision::new(3),
    };
    let materialization = Materialization {
        materialization_id: MaterializationId::from_bytes([50; 16]),
        source_revision_id: source_revision.revision_id,
        materializer_profile_id: ProfileId::new("profile:materializer-blake3-v1")
            .expect("valid materializer profile"),
        canonical_object_digest: Blake3Digest32::from_bytes([51; 32]),
        object_residency_key_digest: ObjectResidencyKeyDigest::from_bytes([52; 32]),
        native_coordinate_map_digest: Blake3Digest32::from_bytes([53; 32]),
        loss_map_digest: Blake3Digest32::from_bytes([54; 32]),
        assurance_ceiling: search_contracts::schema::AssuranceClass::MappedText,
    };
    let representation = Representation {
        representation_id: RepresentationId::from_bytes([55; 16]),
        materialization_id: materialization.materialization_id,
        unitizer_profile_id: ProfileId::new("profile:code-unitizer-v1")
            .expect("valid unitizer profile"),
        enrichment_profile_ids: c::BoundedList::new(vec![
            ProfileId::new("profile:syntax-enrichment-v1").expect("valid enrichment profile"),
        ])
        .expect("bounded enrichment profiles"),
        unit_manifest_digest: Blake3Digest32::from_bytes([56; 32]),
    };
    let projection_membership = ProjectionMembership {
        projection_membership_id: ProjectionMembershipId::from_bytes([57; 16]),
        source_membership_id: source_membership.source_membership_id,
        representation_id: representation.representation_id,
        access_partition_id: AccessPartitionId::from_bytes([58; 16]),
        scoring_partition_id: ScoringPartitionId::from_bytes([59; 16]),
        projection_schema_id: ProfileId::new("profile:projection-schema-v1")
            .expect("valid projection schema profile"),
    };
    PreparationInputMetadataV1 {
        original_operation_id: OpaqueId::new("root:prepare-source-42")
            .expect("valid original operation"),
        c02_mutation_id: MutationId([60; 32]),
        expected_generation: 1,
        source_revision,
        source_membership,
        materialization,
        representation,
        projection_membership,
        collection_schema: schema.clone(),
        residency_canonical: canonical_u64(61),
        policy_canonical: canonical_u64(policy_value),
    }
}

#[test]
fn preparation_input_is_operation_keyed_durable_and_exactly_recoverable() {
    let scratch = Scratch::new();
    let schema = collection_schema();
    let mut visibility = state();
    visibility.schema_identity_digest = *schema
        .identity()
        .expect("schema identity")
        .as_digest();

    let mut journal = scratch.empty(crate::REGISTRY_JOURNAL_SCHEMA_VERSION);
    journal
        .initialize_operating_profile(
            JournalProfileInitialization::Indexed(visibility),
            MutationId([1; 32]),
            digest(),
            &context(false),
        )
        .expect("initialize indexed profile");

    let deadline = Instant::now() + Duration::from_secs(30);
    let work = |_: MigrationBootstrapWork| Ok::<(), &'static str>(());
    let record = PreparationInputRecordV1::prepare_with_original_context(
        metadata(&schema, 62),
        LIMITS,
        deadline,
        &work,
    )
    .expect("prepare complete immutable input");

    let mut publisher = ControlSnapshotPublisher::new();
    let mut bound = journal
        .bind_control_port::<Cancel>(&mut publisher)
        .expect("bind existing journal");
    assert!(
        bound
            .read_preparation_input_with_original_context(
                record.original_operation_id(),
                deadline,
                &work,
            )
            .expect("exact pre-ID lookup")
            .is_none()
    );

    // Refuse the first exact post-commit read. The caller receives UNKNOWN and
    // must recover with this same record, including its original C02 mutation ID.
    let write_boundaries = Cell::new(0_u8);
    let refuse_after_commit = |unit| {
        if unit == MigrationBootstrapWork::Write(0) {
            write_boundaries.set(write_boundaries.get() + 1);
        }
        if write_boundaries.get() >= 2 && matches!(unit, MigrationBootstrapWork::Read(_)) {
            Err("post-commit-read")
        } else {
            Ok(())
        }
    };
    let unknown = bound
        .transact_preparation_input_with_original_context(
            &record,
            deadline,
            &refuse_after_commit,
        )
        .expect_err("post-commit refusal is unknown");
    assert_eq!(unknown.control, ControlError::CommitOutcomeUnknown);

    let decision = bound
        .recover_preparation_input_with_original_context(&record, deadline, &work)
        .expect("recover exact original operation");
    let CommitRecoveryDecision::Committed(receipt) = decision else {
        panic!("expected committed exact recovery");
    };
    assert!(receipt.replayed);
    bound
        .publish_committed_snapshot_with_original_context(&receipt, deadline, &work)
        .expect("publish the actual recovered journal snapshot");

    drop(bound);
    drop(publisher);
    drop(journal);

    let identity = JournalIdentity {
        schema_version: crate::REGISTRY_JOURNAL_SCHEMA_VERSION,
        ..identity()
    };
    let mut reopened = PersistentControlJournal::open(scratch.file(), identity, LIMITS)
        .expect("reopen same redb journal");
    let mut reopened_publisher = ControlSnapshotPublisher::new();
    let mut rebound = reopened
        .bind_control_port::<Cancel>(&mut reopened_publisher)
        .expect("bind reopened journal");

    let readback = rebound
        .read_preparation_input_with_original_context(
            record.original_operation_id(),
            deadline,
            &work,
        )
        .expect("read exact operation key")
        .expect("committed row exists");
    assert_eq!(readback.generation(), receipt.after_generation);
    assert_eq!(readback.record(), &record);
    assert_eq!(readback.record().c02_mutation_id(), MutationId([60; 32]));

    let replay = rebound
        .transact_preparation_input_with_original_context(&record, deadline, &work)
        .expect("same original command replays");
    assert!(replay.replayed);

    let mut changed_metadata = record.metadata().clone();
    changed_metadata.policy_canonical = canonical_u64(999);
    let conflicting = PreparationInputRecordV1::prepare_with_original_context(
        changed_metadata,
        LIMITS,
        deadline,
        &work,
    )
    .expect("encode different input under same operation identity");
    assert_eq!(
        rebound
            .recover_preparation_input_with_original_context(
                &conflicting,
                deadline,
                &work,
            )
            .expect("conflicting operation is classified"),
        CommitRecoveryDecision::ConflictingInput
    );

    let retained = rebound
        .read_preparation_input_with_original_context(
            record.original_operation_id(),
            deadline,
            &work,
        )
        .expect("read after conflicting attempt")
        .expect("original row remains present");
    assert_eq!(retained.record(), &record);
}

use super::*;

pub(crate) const VECTOR: &str = "lexical";

pub(crate) const fn digest(byte: u8) -> Blake3Digest32 {
    Blake3Digest32::from_bytes([byte; 32])
}

pub(crate) fn opaque(text: &str) -> OpaqueId {
    OpaqueId::new(text).expect("fixture identifier")
}

pub(crate) fn epoch(value: i64) -> Epoch {
    Epoch::new(value).expect("fixture epoch")
}

pub(crate) const fn id(byte: u8) -> QdrantPointId {
    QdrantPointId([byte; 16])
}

pub(crate) fn mutation(tag: &str, byte: u8) -> BridgeMutation {
    BridgeMutation {
        operation_id: opaque(tag),
        canonical_input_digest: digest(byte),
    }
}

pub(crate) const fn collection_generation() -> CollectionGenerationId {
    CollectionGenerationId::from_bytes([1; 16])
}

pub(crate) const fn access_partition() -> AccessPartitionId {
    AccessPartitionId::from_bytes([0x41; 16])
}

pub(crate) const fn scoring_partition() -> ScoringPartitionId {
    ScoringPartitionId::from_bytes([0x51; 16])
}

pub(crate) const fn projection_membership() -> ProjectionMembershipId {
    ProjectionMembershipId::from_bytes([0x31; 16])
}

fn profile_set() -> ProjectionProfileSetId {
    ProjectionProfileSetId::new("oracle-profile-set-v1")
        .expect("projection profile set")
}

pub(crate) fn point(byte: u8, weight: f32) -> PointRecord {
    PointRecord {
        point_id: id(byte),
        payload: PointPayload {
            installation_incarnation_id:
                InstallationIncarnationId::from_bytes([0x11; 16]),
            collection_generation_id: collection_generation(),
            projection_membership_id: projection_membership(),
            access_partition_id: access_partition(),
            scoring_partition_id: scoring_partition(),
            source_id: SourceId::from_bytes([0x61; 16]),
            source_revision_id: SourceRevisionId::from_bytes([byte; 16]),
            representation_id: RepresentationId::from_bytes([
                byte.wrapping_add(1);
                16
            ]),
            unit_id: UnitId::from_bytes([byte; 16]),
            point_identity_digest_256: digest(byte),
            scoring_document_id: ScoringDocumentId::from_bytes([
                byte.wrapping_add(2);
                16
            ]),
            projection_profile_set_id: profile_set(),
            unit_kind: UnitKind::File,
            modality: Modality::Code,
            language_or_format: ProfileId::new("rust-v1")
                .expect("language profile"),
            entity_kind: None,
            normalized_symbol_key: None,
            repository_lineage_id: None,
            valid_from_epoch: epoch(10),
            valid_until_epoch_exclusive: None,
        },
        vectors: BTreeMap::from([(
            VECTOR.to_owned(),
            StoredVector {
                dimensions: 8,
                sparse: true,
                values: vec![(0, weight)],
            },
        )]),
    }
}

pub(crate) fn filter() -> EligibilityFilter {
    EligibilityFilter {
        installation_incarnation_id:
            InstallationIncarnationId::from_bytes([0x11; 16]),
        collection_generation_id: collection_generation(),
        allowed_projection_memberships: BTreeSet::from([
            projection_membership(),
        ]),
        access_partition_id: access_partition(),
        scoring_partition_id: scoring_partition(),
        projection_profile_set_id: profile_set(),
        visible_epoch: epoch(42),
    }
}

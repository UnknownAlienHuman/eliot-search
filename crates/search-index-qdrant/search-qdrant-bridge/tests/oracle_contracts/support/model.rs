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

pub(crate) const fn installation() -> InstallationIncarnationId {
    InstallationIncarnationId::from_bytes([0x10; 16])
}

pub(crate) const fn generation() -> CollectionGenerationId {
    CollectionGenerationId::from_bytes([0x11; 16])
}

pub(crate) const fn projection(byte: u8) -> ProjectionMembershipId {
    ProjectionMembershipId::from_bytes([byte; 16])
}

pub(crate) const fn access_partition(byte: u8) -> AccessPartitionId {
    AccessPartitionId::from_bytes([byte; 16])
}

pub(crate) const fn scoring_partition(byte: u8) -> ScoringPartitionId {
    ScoringPartitionId::from_bytes([byte; 16])
}

pub(crate) fn profile_set() -> ProjectionProfileSetId {
    ProjectionProfileSetId::new("oracle-profile-v1").expect("profile set")
}

pub(crate) fn language_profile() -> ProfileId {
    ProfileId::new("text-neutral-v1").expect("language profile")
}

pub(crate) fn mutation(tag: &str, byte: u8) -> BridgeMutation {
    BridgeMutation {
        operation_id: opaque(tag),
        canonical_input_digest: digest(byte),
    }
}

pub(crate) fn point(byte: u8, weight: f32) -> PointRecord {
    PointRecord {
        point_id: id(byte),
        payload: PointPayload {
            installation_incarnation_id: installation(),
            collection_generation_id: generation(),
            projection_membership_id: projection(0x21),
            access_partition_id: access_partition(0x31),
            scoring_partition_id: scoring_partition(0x41),
            source_id: SourceId::from_bytes([0x51; 16]),
            source_revision_id: SourceRevisionId::from_bytes([0x52; 16]),
            representation_id: RepresentationId::from_bytes([0x53; 16]),
            unit_id: UnitId::from_bytes([byte; 16]),
            point_identity_digest_256: digest(byte),
            scoring_document_id: ScoringDocumentId::from_bytes([byte; 16]),
            projection_profile_set_id: profile_set(),
            unit_kind: UnitKind::File,
            modality: Modality::Text,
            language_or_format: language_profile(),
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
        installation_incarnation_id: installation(),
        collection_generation_id: generation(),
        allowed_projection_memberships: BTreeSet::from([projection(0x21)]),
        access_partition_id: access_partition(0x31),
        scoring_partition_id: scoring_partition(0x41),
        projection_profile_set_id: profile_set(),
        visible_epoch: epoch(42),
    }
}

use super::*;

pub(crate) const VECTOR_NAME: &str = "lex_code_v1";
const VECTOR_DIMS: u32 = 256;
const PARTITION_PERMITTED: u8 = 0xA1;
const PARTITION_DENIED: u8 = 0xB2;
const PROJECTION_PERMITTED: u8 = 0xC1;
const PROJECTION_DENIED: u8 = 0xC2;

pub(crate) const fn ctx() -> OpContext {
    OpContext::new(Duration::from_secs(20))
}

pub(crate) fn schema() -> CollectionSchema {
    let mut named_vectors = BTreeMap::new();
    named_vectors.insert(
        VECTOR_NAME.to_owned(),
        VectorSchema {
            dimensions: VECTOR_DIMS,
            sparse: true,
            idf_enabled: true,
        },
    );
    CollectionSchema {
        named_vectors,
        indexed_payload_fields: PointPayload::INDEXED_FIELDS
            .into_iter()
            .map(str::to_owned)
            .collect(),
        one_shard: true,
        floors: StrictnessFloors {
            strict_mode: true,
            wait_for_mutations: true,
            strong_ordering: true,
        },
        schema_digest: Blake3Digest32::from_bytes([0x77; 32]),
    }
}

pub(crate) fn route() -> CollectionRoute {
    CollectionRoute {
        generation: CollectionGenerationId::from_bytes([0xC4; 16]),
        physical_name: OpaqueId::new("t24_t20_parity").expect("physical name"),
    }
}

const fn installation() -> InstallationIncarnationId {
    InstallationIncarnationId::from_bytes([0xD1; 16])
}

const fn projection(byte: u8) -> ProjectionMembershipId {
    ProjectionMembershipId::from_bytes([byte; 16])
}

const fn access_partition(byte: u8) -> AccessPartitionId {
    AccessPartitionId::from_bytes([byte; 16])
}

const fn scoring_partition(byte: u8) -> ScoringPartitionId {
    ScoringPartitionId::from_bytes([byte; 16])
}

fn projection_profile_set() -> ProjectionProfileSetId {
    ProjectionProfileSetId::new("t20-code-v1").expect("profile set")
}

fn language_profile() -> ProfileId {
    ProfileId::new("code-neutral-v1").expect("language profile")
}

fn filter(partition_byte: u8, projection_byte: u8) -> EligibilityFilter {
    EligibilityFilter {
        installation_incarnation_id: installation(),
        collection_generation_id: route().generation,
        allowed_projection_memberships: BTreeSet::from([projection(projection_byte)]),
        access_partition_id: access_partition(partition_byte),
        scoring_partition_id: scoring_partition(partition_byte),
        projection_profile_set_id: projection_profile_set(),
        visible_epoch: Epoch::new(42).expect("epoch"),
    }
}

pub(crate) fn permitted_filter() -> EligibilityFilter {
    filter(PARTITION_PERMITTED, PROJECTION_PERMITTED)
}

pub(crate) fn denied_filter() -> EligibilityFilter {
    filter(PARTITION_DENIED, PROJECTION_DENIED)
}

pub(crate) const fn point_id(number: u8) -> QdrantPointId {
    let mut bytes = [0_u8; 16];
    bytes[15] = number;
    QdrantPointId(bytes)
}

fn point(
    number: u8,
    partition_byte: u8,
    projection_byte: u8,
    terms: Vec<(u32, f32)>,
) -> PointRecord {
    let mut vectors = BTreeMap::new();
    vectors.insert(
        VECTOR_NAME.to_owned(),
        StoredVector {
            dimensions: VECTOR_DIMS,
            sparse: true,
            values: terms,
        },
    );
    PointRecord {
        point_id: point_id(number),
        payload: PointPayload {
            installation_incarnation_id: installation(),
            collection_generation_id: route().generation,
            projection_membership_id: projection(projection_byte),
            access_partition_id: access_partition(partition_byte),
            scoring_partition_id: scoring_partition(partition_byte),
            source_id: SourceId::from_bytes([0xD2; 16]),
            source_revision_id: SourceRevisionId::from_bytes([number; 16]),
            representation_id: RepresentationId::from_bytes([0xD3; 16]),
            unit_id: UnitId::from_bytes([number; 16]),
            point_identity_digest_256: Blake3Digest32::from_bytes([
                number.wrapping_add(200);
                32
            ]),
            scoring_document_id: ScoringDocumentId::from_bytes([number; 16]),
            projection_profile_set_id: projection_profile_set(),
            unit_kind: UnitKind::File,
            modality: Modality::Code,
            language_or_format: language_profile(),
            entity_kind: None,
            normalized_symbol_key: None,
            repository_lineage_id: None,
            valid_from_epoch: Epoch::new(10).expect("from"),
            valid_until_epoch_exclusive: None,
        },
        vectors,
    }
}

pub(crate) fn permitted_point(number: u8, terms: Vec<(u32, f32)>) -> PointRecord {
    point(number, PARTITION_PERMITTED, PROJECTION_PERMITTED, terms)
}

pub(crate) fn denied_point(number: u8) -> PointRecord {
    point(
        number,
        PARTITION_DENIED,
        PROJECTION_DENIED,
        vec![(0, 1.0)],
    )
}

pub(crate) fn denied_ids() -> Vec<QdrantPointId> {
    (10..=15).map(point_id).collect()
}

pub(crate) fn mutation(tag: &str, byte: u8) -> BridgeMutation {
    BridgeMutation {
        operation_id: OpaqueId::new(tag).expect("operation id"),
        canonical_input_digest: Blake3Digest32::from_bytes([byte; 32]),
    }
}

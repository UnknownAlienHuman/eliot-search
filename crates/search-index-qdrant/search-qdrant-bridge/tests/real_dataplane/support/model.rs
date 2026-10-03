use super::*;

pub(crate) const VECTOR_NAME: &str = "lex_code_v1";
const VECTOR_DIMS: u32 = 256;
const PRIMARY_PROJECTION: u8 = 0xB1;
const SCORING_PARTITION: u8 = 0xB2;

pub(crate) const fn limits() -> BridgeLimits {
    BridgeLimits::BASELINE
}

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
    let indexed = PointPayload::INDEXED_FIELDS
        .into_iter()
        .map(str::to_owned)
        .collect();
    CollectionSchema {
        named_vectors,
        indexed_payload_fields: indexed,
        one_shard: true,
        floors: StrictnessFloors {
            strict_mode: true,
            wait_for_mutations: true,
            strong_ordering: true,
        },
        schema_digest: Blake3Digest32::from_bytes([0x51; 32]),
    }
}

pub(crate) fn make_route(name: &str, generation_byte: u8) -> CollectionRoute {
    CollectionRoute {
        generation: CollectionGenerationId::from_bytes([generation_byte; 16]),
        physical_name: OpaqueId::new(name).expect("physical name"),
    }
}

pub(crate) const fn installation() -> InstallationIncarnationId {
    InstallationIncarnationId::from_bytes([0x91; 16])
}

pub(crate) const fn projection(byte: u8) -> ProjectionMembershipId {
    ProjectionMembershipId::from_bytes([byte; 16])
}

pub(crate) const fn access_partition(byte: u8) -> AccessPartitionId {
    AccessPartitionId::from_bytes([byte; 16])
}

pub(crate) const fn scoring_partition() -> ScoringPartitionId {
    ScoringPartitionId::from_bytes([SCORING_PARTITION; 16])
}

pub(crate) fn projection_profile_set() -> ProjectionProfileSetId {
    ProjectionProfileSetId::new("t24-lex-code-v1").expect("profile set")
}

pub(crate) fn language_profile() -> ProfileId {
    ProfileId::new("code-neutral-v1").expect("language profile")
}

pub(crate) fn permitted_filter(route: &CollectionRoute) -> EligibilityFilter {
    EligibilityFilter {
        installation_incarnation_id: installation(),
        collection_generation_id: route.generation,
        allowed_projection_memberships: BTreeSet::from([
            projection(PRIMARY_PROJECTION),
        ]),
        access_partition_id: access_partition(0xA1),
        scoring_partition_id: scoring_partition(),
        projection_profile_set_id: projection_profile_set(),
        visible_epoch: Epoch::new(42).expect("epoch"),
    }
}

pub(crate) const fn point_id(number: u8) -> QdrantPointId {
    let mut bytes = [0_u8; 16];
    bytes[15] = number;
    QdrantPointId(bytes)
}

pub(crate) fn point(
    route: &CollectionRoute,
    number: u8,
    partition_byte: u8,
    projection_byte: u8,
    from: i64,
    until: Option<i64>,
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
            collection_generation_id: route.generation,
            projection_membership_id: projection(projection_byte),
            access_partition_id: access_partition(partition_byte),
            scoring_partition_id: scoring_partition(),
            source_id: SourceId::from_bytes([0x92; 16]),
            source_revision_id: SourceRevisionId::from_bytes([number; 16]),
            representation_id: RepresentationId::from_bytes([0x93; 16]),
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
            valid_from_epoch: Epoch::new(from).expect("from"),
            valid_until_epoch_exclusive: until
                .map(|value| Epoch::new(value).expect("until")),
        },
        vectors,
    }
}

pub(crate) fn mutation(tag: &str, byte: u8) -> BridgeMutation {
    BridgeMutation {
        operation_id: OpaqueId::new(tag).expect("operation id"),
        canonical_input_digest: Blake3Digest32::from_bytes([byte; 32]),
    }
}

pub(crate) fn parity_points(route: &CollectionRoute) -> Vec<PointRecord> {
    // Distinct term-0 weights: single-term IDF scaling is monotone, so the
    // live ranking must equal the oracle TF order exactly.
    vec![
        point(
            route,
            1,
            0xA1,
            PRIMARY_PROJECTION,
            10,
            None,
            vec![(0, 2.0), (1, 1.0)],
        ),
        point(
            route,
            2,
            0xA1,
            PRIMARY_PROJECTION,
            10,
            Some(50),
            vec![(0, 1.0)],
        ),
        point(
            route,
            3,
            0xA1,
            PRIMARY_PROJECTION,
            10,
            None,
            vec![(2, 1.0)],
        ),
        point(
            route,
            4,
            0xA1,
            PRIMARY_PROJECTION,
            10,
            None,
            vec![(0, 3.0), (2, 1.0)],
        ),
    ]
}

pub(crate) fn oversize_batch(route: &CollectionRoute) -> Vec<PointRecord> {
    let mut batch = Vec::new();
    for index in 0..=limits().max_points_per_mutation {
        let id_byte = u8::try_from(index % 251).expect("byte") + 1;
        batch.push(point(
            route,
            id_byte,
            0xA1,
            PRIMARY_PROJECTION,
            10,
            None,
            vec![(0, 1.0)],
        ));
    }
    batch
}

use super::*;

pub(crate) const VECTOR_NAME: &str = "lex_code_v1";
const VECTOR_DIMS: u32 = 256;

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
    let mut indexed = BTreeSet::new();
    for field in EligibilityFilter::INDEXED_FIELDS {
        indexed.insert((*field).to_owned());
    }
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

const fn partition(byte: u8) -> Blake3Digest32 {
    Blake3Digest32::from_bytes([byte; 32])
}

fn member(name: &str) -> OpaqueId {
    OpaqueId::new(name).expect("member")
}

pub(crate) fn permitted_filter() -> EligibilityFilter {
    let mut allowed = BTreeSet::new();
    allowed.insert(member("t24-member-a"));
    EligibilityFilter {
        access_partition_digest: partition(0xA1),
        allowed_source_memberships: allowed,
        visible_epoch: Epoch::new(42).expect("epoch"),
    }
}

pub(crate) const fn point_id(number: u8) -> QdrantPointId {
    let mut bytes = [0_u8; 16];
    bytes[15] = number;
    QdrantPointId(bytes)
}

pub(crate) fn point(
    number: u8,
    partition_byte: u8,
    membership: &str,
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
            digest: Blake3Digest32::from_bytes([number; 32]),
        },
    );
    PointRecord {
        point_id: point_id(number),
        payload: PointPayload {
            source_membership_id: member(membership),
            projection_membership_id: member("t24-proj-a"),
            access_partition_digest: partition(partition_byte),
            source_revision: u64::from(number),
            unit_ordinal: u64::from(number),
            valid_from_epoch: Epoch::new(from).expect("from"),
            valid_until_epoch_exclusive: until
                .map(|value| Epoch::new(value).expect("until")),
            payload_digest: Blake3Digest32::from_bytes([
                number.wrapping_add(100);
                32
            ]),
            identity_digest: Blake3Digest32::from_bytes([
                number.wrapping_add(200);
                32
            ]),
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

pub(crate) fn parity_points() -> Vec<PointRecord> {
    // Distinct term-0 weights: single-term IDF scaling is monotone, so the
    // live ranking must equal the oracle TF order exactly.
    vec![
        point(
            1,
            0xA1,
            "t24-member-a",
            10,
            None,
            vec![(0, 2.0), (1, 1.0)],
        ),
        point(
            2,
            0xA1,
            "t24-member-a",
            10,
            Some(50),
            vec![(0, 1.0)],
        ),
        point(
            3,
            0xA1,
            "t24-member-a",
            10,
            None,
            vec![(2, 1.0)],
        ),
        point(
            4,
            0xA1,
            "t24-member-a",
            10,
            None,
            vec![(0, 3.0), (2, 1.0)],
        ),
    ]
}

pub(crate) fn oversize_batch() -> Vec<PointRecord> {
    let mut batch = Vec::new();
    for index in 0..=limits().max_points_per_mutation {
        let id_byte = u8::try_from(index % 251).expect("byte") + 1;
        batch.push(point(
            id_byte,
            0xA1,
            "t24-member-a",
            10,
            None,
            vec![(0, 1.0)],
        ));
    }
    batch
}

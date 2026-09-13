use super::*;

pub(crate) const VECTOR_NAME: &str = "lex_code_v1";
const VECTOR_DIMS: u32 = 256;
const PARTITION_PERMITTED: u8 = 0xA1;
const PARTITION_DENIED: u8 = 0xB2;

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
        schema_digest: Blake3Digest32::from_bytes([0x77; 32]),
    }
}

pub(crate) fn route() -> CollectionRoute {
    CollectionRoute {
        generation: CollectionGenerationId::from_bytes([0xC4; 16]),
        physical_name: OpaqueId::new("t24_t20_parity")
            .expect("physical name"),
    }
}

fn filter(partition_byte: u8, membership: &str) -> EligibilityFilter {
    let mut allowed = BTreeSet::new();
    allowed.insert(OpaqueId::new(membership).expect("member"));
    EligibilityFilter {
        access_partition_digest: Blake3Digest32::from_bytes([
            partition_byte;
            32
        ]),
        allowed_source_memberships: allowed,
        visible_epoch: Epoch::new(42).expect("epoch"),
    }
}

pub(crate) fn permitted_filter() -> EligibilityFilter {
    filter(PARTITION_PERMITTED, "t20-member-a")
}

pub(crate) fn denied_filter() -> EligibilityFilter {
    filter(PARTITION_DENIED, "t20-member-denied")
}

pub(crate) const fn point_id(number: u8) -> QdrantPointId {
    let mut bytes = [0_u8; 16];
    bytes[15] = number;
    QdrantPointId(bytes)
}

fn point(
    number: u8,
    partition_byte: u8,
    membership: &str,
    projection: &str,
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
            source_membership_id: OpaqueId::new(membership).expect("member"),
            projection_membership_id: OpaqueId::new(projection).expect("proj"),
            access_partition_digest: Blake3Digest32::from_bytes([
                partition_byte;
                32
            ]),
            source_revision: u64::from(number),
            unit_ordinal: u64::from(number),
            valid_from_epoch: Epoch::new(10).expect("from"),
            valid_until_epoch_exclusive: None,
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

pub(crate) fn permitted_point(
    number: u8,
    terms: Vec<(u32, f32)>,
) -> PointRecord {
    point(
        number,
        PARTITION_PERMITTED,
        "t20-member-a",
        "t20-proj-a",
        terms,
    )
}

pub(crate) fn denied_point(number: u8) -> PointRecord {
    point(
        number,
        PARTITION_DENIED,
        "t20-member-denied",
        "t20-proj-denied",
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

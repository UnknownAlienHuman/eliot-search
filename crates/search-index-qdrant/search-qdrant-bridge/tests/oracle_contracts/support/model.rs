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

pub(crate) fn point(byte: u8, weight: f32) -> PointRecord {
    PointRecord {
        point_id: id(byte),
        payload: PointPayload {
            source_membership_id: opaque("member"),
            projection_membership_id: opaque("projection"),
            access_partition_digest: digest(1),
            source_revision: 1,
            unit_ordinal: u64::from(byte),
            valid_from_epoch: epoch(10),
            valid_until_epoch_exclusive: None,
            payload_digest: digest(byte),
            identity_digest: digest(byte),
        },
        vectors: BTreeMap::from([(
            VECTOR.to_owned(),
            StoredVector {
                dimensions: 8,
                sparse: true,
                values: vec![(0, weight)],
                digest: digest(byte),
            },
        )]),
    }
}

pub(crate) fn filter() -> EligibilityFilter {
    EligibilityFilter {
        access_partition_digest: digest(1),
        allowed_source_memberships: BTreeSet::from([opaque("member")]),
        visible_epoch: epoch(42),
    }
}

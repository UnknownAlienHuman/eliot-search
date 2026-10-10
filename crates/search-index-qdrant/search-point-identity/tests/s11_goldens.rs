//! Independently encoded CBOR and direct BLAKE3 oracle vectors for S11.

use std::fmt::Write as _;

use search_contracts::{
    BoundedMap, CanonicalValue, CollectionGenerationId, InstallationIncarnationId,
    ProjectionMembershipId, ProjectionProfileSetId, RepresentationId, UnitId, to_canonical_cbor,
};
use search_point_identity::s11::{
    DEFAULT_LIMITS, IDENTITY_DOMAIN, PointIdError, PointRole, ProjectionPointKey,
    derive_point_identity,
};

fn key() -> ProjectionPointKey {
    ProjectionPointKey {
        schema_version: 1,
        installation_incarnation_id: InstallationIncarnationId::from_bytes([1; 16]),
        collection_generation_id: CollectionGenerationId::from_bytes([2; 16]),
        projection_membership_id: ProjectionMembershipId::from_bytes([3; 16]),
        representation_id: RepresentationId::from_bytes([4; 16]),
        unit_id: UnitId::from_bytes([5; 16]),
        projection_profile_set_id: ProjectionProfileSetId::new("lexical-v1").unwrap(),
        point_role: PointRole::Unit,
    }
}

fn hex_bytes(bytes: &[u8]) -> String {
    let mut text = String::with_capacity(bytes.len() * 2);
    for byte in bytes {
        write!(&mut text, "{byte:02x}").unwrap();
    }
    text
}

#[test]
fn exact_eight_field_cbor_full_digest_and_address() {
    // CBOR was independently assembled from the eight fixed scalar fields.
    // Digests were computed by a standalone direct BLAKE3 oracle over the
    // literal domain/NUL/bytes, without calling this package or digest helper.
    let key = key();
    let encoded = key.canonical_bytes(DEFAULT_LIMITS).unwrap();
    assert_eq!(encoded.len(), 263);
    assert_eq!(
        hex_bytes(encoded.as_slice()),
        "a867756e69745f696450050505050505050505050505050505056a706f696e745f726f6c6564756e69746e736368656d615f76657273696f6e0171726570726573656e746174696f6e5f696450040404040404040404040404040404047818636f6c6c656374696f6e5f67656e65726174696f6e5f69645002020202020202020202020202020202781870726f6a656374696f6e5f6d656d626572736869705f69645003030303030303030303030303030303781970726f6a656374696f6e5f70726f66696c655f7365745f69646a6c65786963616c2d7631781b696e7374616c6c6174696f6e5f696e6361726e6174696f6e5f69645001010101010101010101010101010101"
    );
    let identity = derive_point_identity(&key, DEFAULT_LIMITS).unwrap();
    assert_eq!(
        identity.full_digest().to_string(),
        "395a97a9b48700cfef5811d51367bc94e89bb899871e2d3c103afccebc429e92"
    );
    assert_eq!(
        identity.point_id().to_string(),
        "df8b4260-1ee4-decf-a59f-6028e5d0cecc"
    );
    assert_eq!(
        ProjectionPointKey::from_canonical_bytes(encoded.as_slice(), DEFAULT_LIMITS).unwrap(),
        key
    );
}

#[test]
fn map_input_order_does_not_change_bytes_or_identity() {
    let key = key();
    let CanonicalValue::Object(fields) = key.to_canonical_value(DEFAULT_LIMITS).unwrap() else {
        panic!("closed object");
    };
    let mut entries: Vec<_> = fields.iter().map(|(k, v)| (k.clone(), v.clone())).collect();
    entries.reverse();
    let reordered = CanonicalValue::Object(BoundedMap::from_entries(entries).unwrap());
    assert_eq!(
        to_canonical_cbor(&reordered).unwrap(),
        key.canonical_bytes(DEFAULT_LIMITS).unwrap()
    );
}

#[test]
fn complete_identity_preimage_ceiling_counts_domain_and_nul() {
    let key = key();
    let complete = IDENTITY_DOMAIN.len() + 1 + key.canonical_bytes(DEFAULT_LIMITS).unwrap().len();
    assert_eq!(complete, 292);
    let exact = search_point_identity::s11::PointIdentityLimits {
        max_digest_preimage_bytes: complete,
        ..DEFAULT_LIMITS
    };
    assert_eq!(
        derive_point_identity(&key, exact).unwrap(),
        derive_point_identity(&key, DEFAULT_LIMITS).unwrap()
    );
    let insufficient = search_point_identity::s11::PointIdentityLimits {
        max_digest_preimage_bytes: complete - 1,
        ..DEFAULT_LIMITS
    };
    assert_eq!(
        derive_point_identity(&key, insufficient),
        Err(PointIdError::DigestPreimageExceeded)
    );
}

use core::fmt::Write as _;

use search_contracts::{
    Blake3Digest32, CollectionGenerationId, InstallationIncarnationId,
    ProjectionMembershipId, ProjectionProfileSetId, RepresentationId, UnitId,
};

use super::*;

fn key() -> ProjectionPointKey {
    ProjectionPointKey {
        schema_version: POINT_IDENTITY_SCHEMA_VERSION,
        installation_incarnation_id: InstallationIncarnationId::from_bytes([1; 16]),
        collection_generation_id: CollectionGenerationId::from_bytes([2; 16]),
        projection_membership_id: ProjectionMembershipId::from_bytes([3; 16]),
        representation_id: RepresentationId::from_bytes([4; 16]),
        unit_id: UnitId::from_bytes([5; 16]),
        projection_profile_set_id: ProjectionProfileSetId::new("profile-set:test")
            .expect("profile set"),
        point_role: PointRole::Unit,
    }
}

fn hex(bytes: &[u8]) -> String {
    let mut output = String::with_capacity(bytes.len() * 2);
    for byte in bytes {
        write!(&mut output, "{byte:02x}").expect("String writes cannot fail");
    }
    output
}

fn identity() -> PointIdentity {
    derive_point_identity(key(), DEFAULT_POINT_IDENTITY_LIMITS)
        .expect("identity")
}

fn assert_collision(expected: &PointIdentity, observed: &PointIdentityPayload) {
    assert_eq!(
        validate_identity_payload(expected, observed),
        Err(PointIdentityError::DigestCollision)
    );
    assert_eq!(
        compare_existing_identity(expected, Some(observed)),
        CollisionDecision::CollisionBlock
    );
}

#[test]
fn canonical_bytes_and_digest_golden() {
    let canonical = encode_canonical_key(&key(), DEFAULT_POINT_IDENTITY_LIMITS)
        .expect("canonical CBOR");
    assert_eq!(
        hex(canonical.as_slice()),
        concat!(
            "a867756e69745f696450050505050505050505050505050505056a706f696e745f726f6c6564756e69746e736368656d615f",
            "76657273696f6e0171726570726573656e746174696f6e5f696450040404040404040404040404040404047818636f6c6c65",
            "6374696f6e5f67656e65726174696f6e5f69645002020202020202020202020202020202781870726f6a656374696f6e5f6d",
            "656d626572736869705f69645003030303030303030303030303030303781970726f6a656374696f6e5f70726f66696c655f",
            "7365745f69647070726f66696c652d7365743a74657374781b696e7374616c6c6174696f6e5f696e6361726e6174696f6e5f",
            "69645001010101010101010101010101010101"
        )
    );
    let digest = full_digest(&canonical);
    assert_eq!(
        hex(digest.as_bytes()),
        "73eb0eeced878b3b19897840ad3b2c2153b32a5ae24f78ed2bfcefee98c457fe"
    );
    assert_eq!(
        derive_qdrant_uuid(digest).to_hyphenated(),
        "0e10e0f5-a101-7890-198d-3947e337ffff"
    );
}

#[test]
fn same_key_same_identity_and_no_json_stringification() {
    let first = identity();
    let second = identity();
    assert_eq!(first, second);
    let canonical = canonical_point_key_bytes(&first.key, DEFAULT_POINT_IDENTITY_LIMITS)
        .expect("canonical key");
    assert_eq!(canonical.as_slice().first(), Some(&0xa8));
    assert!(!canonical.as_slice().starts_with(b"{"));
    assert!(!canonical.as_slice().contains(&b'='));
}

#[test]
fn every_s11_identity_coordinate_changes_the_identity() {
    let baseline = identity();

    let mut changed = key();
    changed.installation_incarnation_id =
        InstallationIncarnationId::from_bytes([9; 16]);
    assert_ne!(
        baseline.point_id,
        derive_point_identity(changed, DEFAULT_POINT_IDENTITY_LIMITS)
            .expect("installation")
            .point_id
    );

    let mut changed = key();
    changed.collection_generation_id =
        CollectionGenerationId::from_bytes([9; 16]);
    assert_ne!(
        baseline.point_id,
        derive_point_identity(changed, DEFAULT_POINT_IDENTITY_LIMITS)
            .expect("generation")
            .point_id
    );

    let mut changed = key();
    changed.projection_membership_id =
        ProjectionMembershipId::from_bytes([9; 16]);
    assert_ne!(
        baseline.point_id,
        derive_point_identity(changed, DEFAULT_POINT_IDENTITY_LIMITS)
            .expect("membership")
            .point_id
    );

    let mut changed = key();
    changed.representation_id = RepresentationId::from_bytes([9; 16]);
    assert_ne!(
        baseline.point_id,
        derive_point_identity(changed, DEFAULT_POINT_IDENTITY_LIMITS)
            .expect("representation")
            .point_id
    );

    let mut changed = key();
    changed.unit_id = UnitId::from_bytes([9; 16]);
    assert_ne!(
        baseline.point_id,
        derive_point_identity(changed, DEFAULT_POINT_IDENTITY_LIMITS)
            .expect("unit")
            .point_id
    );

    let mut changed = key();
    changed.projection_profile_set_id =
        ProjectionProfileSetId::new("profile-set:other").expect("profile set");
    assert_ne!(
        baseline.point_id,
        derive_point_identity(changed, DEFAULT_POINT_IDENTITY_LIMITS)
            .expect("profile")
            .point_id
    );

    let mut changed = key();
    changed.point_role = PointRole::Relation;
    assert_ne!(
        baseline.point_id,
        derive_point_identity(changed, DEFAULT_POINT_IDENTITY_LIMITS)
            .expect("role")
            .point_id
    );
}

#[test]
fn fake_projected_uuid_collision_never_authorizes_overwrite() {
    let expected = identity();
    let exact = PointIdentityPayload::from_identity(&expected);
    assert_eq!(
        compare_existing_identity(&expected, None),
        CollisionDecision::Vacant
    );
    assert_eq!(
        compare_existing_identity(&expected, Some(&exact)),
        CollisionDecision::SameFullIdentity
    );

    let mut other_key = key();
    other_key.unit_id = UnitId::from_bytes([0x55; 16]);
    let other = derive_point_identity(other_key, DEFAULT_POINT_IDENTITY_LIMITS)
        .expect("other");
    let mut forged = PointIdentityPayload::from_identity(&other);
    forged.point_id = expected.point_id;
    assert_collision(&expected, &forged);
    assert_eq!(PointIdentityError::DigestCollision.code(), "POINT_ID_COLLISION");
}

#[test]
fn payload_validation_checks_every_independently_stored_field() {
    let expected = identity();
    let exact = PointIdentityPayload::from_identity(&expected);
    validate_identity_payload(&expected, &exact).expect("exact payload");

    let mut changed = exact.clone();
    changed.point_identity_digest_256 = Blake3Digest32::from_bytes([0x99; 32]);
    assert_collision(&expected, &changed);

    let mut changed = exact.clone();
    changed.installation_incarnation_id =
        InstallationIncarnationId::from_bytes([0x66; 16]);
    assert_collision(&expected, &changed);

    let mut changed = exact.clone();
    changed.collection_generation_id =
        CollectionGenerationId::from_bytes([0x67; 16]);
    assert_collision(&expected, &changed);

    let mut changed = exact.clone();
    changed.projection_membership_id =
        ProjectionMembershipId::from_bytes([0x68; 16]);
    assert_collision(&expected, &changed);

    let mut changed = exact.clone();
    changed.representation_id = RepresentationId::from_bytes([0x69; 16]);
    assert_collision(&expected, &changed);

    let mut changed = exact.clone();
    changed.unit_id = UnitId::from_bytes([0x6a; 16]);
    assert_collision(&expected, &changed);

    let mut changed = exact;
    changed.projection_profile_set_id =
        ProjectionProfileSetId::new("profile-set:foreign").expect("profile set");
    assert_collision(&expected, &changed);
}

#[test]
fn payload_for_another_uuid_is_an_identity_routing_mismatch() {
    let expected = identity();
    let mut observed = PointIdentityPayload::from_identity(&expected);
    observed.point_id = PointId128::from_bytes([0x44; 16]);
    assert_eq!(
        validate_identity_fields(&expected, &observed),
        Err(PointIdentityError::IdentityMismatch)
    );
    assert_eq!(
        compare_existing_identity(&expected, Some(&observed)),
        CollisionDecision::CollisionBlock
    );
}

#[test]
fn unsupported_version_and_finite_limits_fail_closed() {
    let mut unsupported = key();
    unsupported.schema_version = 2;
    assert_eq!(
        derive_point_identity(unsupported, DEFAULT_POINT_IDENTITY_LIMITS),
        Err(PointIdentityError::PointKeyVersionUnsupported)
    );

    let tiny_cbor = PointIdentityLimits {
        max_canonical_bytes: 8,
        ..DEFAULT_POINT_IDENTITY_LIMITS
    };
    assert_eq!(
        derive_point_identity(key(), tiny_cbor),
        Err(PointIdentityError::CanonicalBytesExceeded)
    );

    let tiny_identifier = PointIdentityLimits {
        max_identifier_bytes: 4,
        ..DEFAULT_POINT_IDENTITY_LIMITS
    };
    assert_eq!(
        derive_point_identity(key(), tiny_identifier),
        Err(PointIdentityError::IdentifierTooLong)
    );

    assert_eq!(
        PointIdentityLimits {
            max_identifier_bytes: 0,
            ..DEFAULT_POINT_IDENTITY_LIMITS
        }
        .validate(),
        Err(PointIdentityError::InvalidLimits)
    );
}

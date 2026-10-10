//! Causal identity, readback and admission fixtures for the S11 cutover.

use search_contracts::{
    Blake3Digest32, BoundedMap, CanonicalKey, CanonicalText, CanonicalValue,
    CollectionGenerationId, InstallationIncarnationId, NonZeroRevision, OpaqueId,
    ProjectionMembershipId, ProjectionProfileSetId, RepresentationId, UnitId, to_canonical_cbor,
};
use search_point_identity::s11::{
    CollisionDecision, DEFAULT_LIMITS, ObservedPointIdentity, PointId128, PointIdError,
    PointIdentity, PointRole, ProjectionPointKey, compare_existing_identity, derive_point_identity,
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

fn observed(identity: &PointIdentity) -> ObservedPointIdentity {
    ObservedPointIdentity {
        key: identity.key().clone(),
        full_digest: identity.full_digest(),
        point_id: identity.point_id(),
    }
}

#[test]
fn every_admitted_identity_coordinate_changes_full_digest_and_address() {
    let base = key();
    let expected = derive_point_identity(&base, DEFAULT_LIMITS).unwrap();
    let mut variants = vec![base.clone(); 8];
    variants[0].installation_incarnation_id = InstallationIncarnationId::from_bytes([9; 16]);
    variants[1].collection_generation_id = CollectionGenerationId::from_bytes([9; 16]);
    variants[2].projection_membership_id = ProjectionMembershipId::from_bytes([9; 16]);
    variants[3].representation_id = RepresentationId::from_bytes([9; 16]);
    variants[4].unit_id = UnitId::from_bytes([9; 16]);
    variants[5].projection_profile_set_id = ProjectionProfileSetId::new("lexical-v2").unwrap();
    variants[6].point_role = PointRole::Relation;
    variants[7].point_role = PointRole::Auxiliary;
    let identities: Vec<_> = variants
        .iter()
        .map(|variant| derive_point_identity(variant, DEFAULT_LIMITS).unwrap())
        .collect();
    for variant in &variants {
        let changed = derive_point_identity(variant, DEFAULT_LIMITS).unwrap();
        assert_ne!(changed.full_digest(), expected.full_digest());
        assert_ne!(changed.point_id(), expected.point_id());
        let forged = ObservedPointIdentity {
            key: variant.clone(),
            ..observed(&expected)
        };
        assert_eq!(
            compare_existing_identity(&expected, Some(&forged), DEFAULT_LIMITS).unwrap(),
            CollisionDecision::CollisionBlock
        );
    }
    // Reverse evaluation order cannot change any computation.
    for (variant, previously_computed) in variants.iter().rev().zip(identities.iter().rev()) {
        assert_eq!(
            derive_point_identity(variant, DEFAULT_LIMITS).unwrap(),
            *previously_computed
        );
    }
    assert_eq!(
        derive_point_identity(&base, DEFAULT_LIMITS).unwrap(),
        expected
    );
}

#[test]
fn vacant_and_same_full_identity_are_the_only_successful_readback_cases() {
    let expected = derive_point_identity(&key(), DEFAULT_LIMITS).unwrap();
    assert_eq!(
        compare_existing_identity(&expected, None, DEFAULT_LIMITS).unwrap(),
        CollisionDecision::Vacant
    );
    assert_eq!(
        compare_existing_identity(&expected, Some(&observed(&expected)), DEFAULT_LIMITS).unwrap(),
        CollisionDecision::SameFullIdentity
    );
}

#[test]
fn matching_address_with_other_full_digest_refuses_overwrite() {
    let expected = derive_point_identity(&key(), DEFAULT_LIMITS).unwrap();
    let mut stored = observed(&expected);
    let mut bytes = *stored.full_digest.as_bytes();
    bytes[31] ^= 1;
    stored.full_digest = Blake3Digest32::from_stored_bytes(bytes);
    let decision = compare_existing_identity(&expected, Some(&stored), DEFAULT_LIMITS).unwrap();
    assert_eq!(decision, CollisionDecision::CollisionBlock);
    assert_eq!(decision.code(), "POINT_ID_COLLISION");
}

#[test]
fn matching_digest_with_unsupported_key_version_is_a_collision_refusal() {
    let expected = derive_point_identity(&key(), DEFAULT_LIMITS).unwrap();
    let stored = ObservedPointIdentity {
        key: ProjectionPointKey {
            schema_version: 2,
            ..key()
        },
        ..observed(&expected)
    };
    assert_eq!(
        compare_existing_identity(&expected, Some(&stored), DEFAULT_LIMITS).unwrap(),
        CollisionDecision::CollisionBlock
    );
}

#[test]
fn foreign_address_is_an_explicit_error() {
    let expected = derive_point_identity(&key(), DEFAULT_LIMITS).unwrap();
    let mut stored = observed(&expected);
    let mut bytes = *stored.point_id.as_bytes();
    bytes[0] ^= 1;
    stored.point_id = PointId128::from_stored_bytes(bytes);
    assert_eq!(
        compare_existing_identity(&expected, Some(&stored), DEFAULT_LIMITS),
        Err(PointIdError::ForeignAddress)
    );
}

#[test]
fn uuid_round_trip_preserves_version_and_variant_bits() {
    for bytes in [[0xff; 16], [0; 16], [0x7f; 16]] {
        let address = PointId128::from_stored_bytes(bytes);
        assert_eq!(
            *PointId128::parse(&address.to_string()).unwrap().as_bytes(),
            bytes
        );
    }
    assert_eq!(
        PointId128::parse("FFFFFFFF-FFFF-FFFF-FFFF-FFFFFFFFFFFF"),
        Err(PointIdError::InvalidUuid)
    );
    assert_eq!(
        PointId128::parse("not-a-uuid"),
        Err(PointIdError::InvalidUuid)
    );
}

#[test]
fn schema_versions_and_roles_are_closed() {
    for version in [0, 2, u16::MAX] {
        assert_eq!(
            derive_point_identity(
                &ProjectionPointKey {
                    schema_version: version,
                    ..key()
                },
                DEFAULT_LIMITS
            ),
            Err(PointIdError::PointKeyVersionUnsupported)
        );
    }
    for role in ["", "Unit", "unit ", "relation-or-unit", "unknown"] {
        assert_eq!(PointRole::parse(role), Err(PointIdError::InvalidPointRole));
    }
    for role in [PointRole::Unit, PointRole::Relation, PointRole::Auxiliary] {
        assert_eq!(PointRole::parse(role.as_str()).unwrap(), role);
    }
}

#[test]
fn budgets_cannot_be_zero_or_widen_the_frozen_profile() {
    let invalid = [
        search_point_identity::s11::PointIdentityLimits {
            max_profile_id_bytes: 0,
            ..DEFAULT_LIMITS
        },
        search_point_identity::s11::PointIdentityLimits {
            max_profile_id_bytes: 257,
            ..DEFAULT_LIMITS
        },
        search_point_identity::s11::PointIdentityLimits {
            max_canonical_bytes: 0,
            ..DEFAULT_LIMITS
        },
        search_point_identity::s11::PointIdentityLimits {
            max_canonical_bytes: 1025,
            ..DEFAULT_LIMITS
        },
        search_point_identity::s11::PointIdentityLimits {
            max_digest_preimage_bytes: 0,
            ..DEFAULT_LIMITS
        },
        search_point_identity::s11::PointIdentityLimits {
            max_digest_preimage_bytes: 1089,
            ..DEFAULT_LIMITS
        },
    ];
    for limits in invalid {
        assert_eq!(
            derive_point_identity(&key(), limits),
            Err(PointIdError::InvalidLimits)
        );
    }
}

#[test]
fn narrow_identifier_cbor_and_preimage_budgets_fail_closed() {
    let identifier = search_point_identity::s11::PointIdentityLimits {
        max_profile_id_bytes: 1,
        ..DEFAULT_LIMITS
    };
    assert_eq!(
        derive_point_identity(&key(), identifier),
        Err(PointIdError::IdentifierTooLong)
    );
    let cbor = search_point_identity::s11::PointIdentityLimits {
        max_canonical_bytes: 1,
        ..DEFAULT_LIMITS
    };
    assert_eq!(
        derive_point_identity(&key(), cbor),
        Err(PointIdError::CanonicalBytesExceeded)
    );
    let preimage = search_point_identity::s11::PointIdentityLimits {
        max_digest_preimage_bytes: 1,
        ..DEFAULT_LIMITS
    };
    let expected = derive_point_identity(&key(), DEFAULT_LIMITS).unwrap();
    assert_eq!(
        compare_existing_identity(&expected, None, preimage),
        Err(PointIdError::DigestPreimageExceeded)
    );
}

fn changed_wire_field(name: &str, value: CanonicalValue) -> Vec<u8> {
    let CanonicalValue::Object(fields) = key().to_canonical_value(DEFAULT_LIMITS).unwrap() else {
        panic!("object")
    };
    let mut entries: Vec<_> = fields.iter().map(|(k, v)| (k.clone(), v.clone())).collect();
    entries.retain(|(k, _)| k.as_str() != name);
    entries.push((CanonicalKey::new_non_empty(name).unwrap(), value));
    to_canonical_cbor(&CanonicalValue::Object(
        BoundedMap::from_entries(entries).unwrap(),
    ))
    .unwrap()
    .as_slice()
    .to_vec()
}

#[test]
fn decoder_rejects_unknown_role_field_version_and_noncanonical_bytes() {
    let invalid_role = changed_wire_field(
        "point_role",
        CanonicalValue::Text(CanonicalText::new("unknown").unwrap()),
    );
    assert_eq!(
        ProjectionPointKey::from_canonical_bytes(&invalid_role, DEFAULT_LIMITS),
        Err(PointIdError::InvalidPointRole)
    );
    let unknown = changed_wire_field("unexpected", CanonicalValue::U64(1));
    assert_eq!(
        ProjectionPointKey::from_canonical_bytes(&unknown, DEFAULT_LIMITS),
        Err(PointIdError::CanonicalEncodingMismatch)
    );
    let version = changed_wire_field("schema_version", CanonicalValue::U64(65_536));
    assert_eq!(
        ProjectionPointKey::from_canonical_bytes(&version, DEFAULT_LIMITS),
        Err(PointIdError::PointKeyVersionUnsupported)
    );
    let mut noncanonical = key()
        .canonical_bytes(DEFAULT_LIMITS)
        .unwrap()
        .as_slice()
        .to_vec();
    noncanonical.splice(0..1, [0xb8, 8]); // non-shortest map length
    assert_eq!(
        ProjectionPointKey::from_canonical_bytes(&noncanonical, DEFAULT_LIMITS),
        Err(PointIdError::CanonicalEncodingMismatch)
    );
}

#[test]
fn actual_legacy_key_bytes_cannot_decode_as_s11() {
    // This is the existing producer used by current consumers, not a new
    // compatibility facade or a relabelled S11 key.
    let id = || OpaqueId::new("legacy-fixture").unwrap();
    let digest = Blake3Digest32::from_stored_bytes([7; 32]);
    let legacy = search_point_identity::PointIdentityKey {
        namespace_id: id(),
        source_id: id(),
        source_revision: NonZeroRevision::new(1).unwrap(),
        unit_ordinal: 0,
        source_byte_start: 0,
        source_byte_end: 1,
        projection_kind: search_point_identity::ProjectionKind::Lexical,
        projection_fingerprint: digest,
        projection_schema_revision: NonZeroRevision::new(1).unwrap(),
        source_membership_id: id(),
        projection_membership_id: id(),
        representation_digest: digest,
        unit_digest: digest,
        scoring_partition_digest: digest,
        collection_generation_digest: digest,
    };
    let bytes = legacy
        .canonical_bytes(search_point_identity::DEFAULT_POINT_IDENTITY_LIMITS)
        .unwrap();
    assert_eq!(
        ProjectionPointKey::from_canonical_bytes(&bytes, DEFAULT_LIMITS),
        Err(PointIdError::CanonicalEncodingMismatch)
    );
}

use super::*;
use crate::batch::{decode_registry_state, decode_registry_state_v5};
use crate::membership::{
    MembershipKey, MembershipLifecycle, MembershipRecord, PermanentMembershipProvenanceV2,
};
use search_contracts::{
    AccessPolicyBindingId, CorpusId, MembershipRevision, ProfileId, ResidencyPolicyBindingId,
    RootBindingId, SourceId, SourceIdentity, SourceIdentityKind, SourceMembership,
    SourceMembershipId, SourceNamespaceId, SourceOwnerGeneration, WorkspaceId,
};

fn rev(value: u64) -> NonZeroRevision {
    NonZeroRevision::new(value).expect("revision")
}

fn typed_member() -> SourceMembership {
    SourceMembership {
        source_membership_id: SourceMembershipId::from_bytes([1; 16]),
        corpus_id: CorpusId::from_bytes([2; 16]),
        source_id: SourceId::from_bytes([3; 16]),
        workspace_id: WorkspaceId::from_bytes([4; 16]),
        role: c::MembershipRole::Documentation,
        preparation_profile_id: ProfileId::new("profile:preparation-v2").expect("profile"),
        access_policy_binding_id: AccessPolicyBindingId::from_bytes([5; 16]),
        retention_policy_id: ProfileId::new("profile:retention-v4").expect("profile"),
        residency_policy_binding_id: ResidencyPolicyBindingId::from_bytes([6; 16]),
        membership_revision: MembershipRevision::new(1),
    }
}

fn source_identity(member: &SourceMembership) -> SourceIdentity {
    SourceIdentity {
        source_namespace_id: SourceNamespaceId::from_bytes([7; 16]),
        source_id: member.source_id,
        identity_kind: SourceIdentityKind::NtfsFile,
        stable_identity_components: c::OpaqueCanonicalBytes::from_validated(
            b"membership-v5-fixture".to_vec(),
        )
        .expect("components"),
    }
}

fn permanent_record() -> MembershipRecord {
    let member = typed_member();
    let identity = source_identity(&member);
    let provenance = PermanentMembershipProvenanceV2::from_stored_parts(
        identity.clone(),
        rev(13),
        RootBindingId::from_bytes([8; 16]),
        rev(11),
        identity.source_namespace_id,
        rev(12),
        SourceOwnerGeneration::from_bytes([9; 32]),
        b"policy-reference-v2".to_vec(),
        b"admission-reference-v2".to_vec(),
        rev(7),
    )
    .expect("provenance refs");
    MembershipRecord::new_permanent_v2(
        MembershipKey {
            corpus_id: OpaqueId::new(member.corpus_id.to_string()).expect("corpus key"),
            source_identity: identity,
        },
        rev(1),
        member,
        provenance,
        MembershipLifecycle::Active,
        14,
        ReceiptRef::new("membership:commit:v2").expect("receipt"),
    )
    .expect("record")
}

#[test]
fn v5_state_roundtrips_full_typed_member_and_permanent_provenance_refs() {
    let expected = RegistryStateRecord::Membership(permanent_record());
    let bytes = expected.encode().expect("v5 state");
    let restored = decode_registry_state_v5(&bytes, &mut || Ok(())).expect("restore v5");
    assert_eq!(restored, expected);
    let RegistryStateRecord::Membership(record) = restored else {
        panic!("membership state")
    };
    let permanent = record.permanent_v2().expect("typed member");
    assert_eq!(
        permanent.member().source_membership_id,
        SourceMembershipId::from_bytes([1; 16])
    );
    assert_eq!(permanent.member().role, c::MembershipRole::Documentation);
    assert_eq!(
        permanent.member().preparation_profile_id.as_str(),
        "profile:preparation-v2"
    );
    assert_eq!(
        permanent.member().retention_policy_id.as_str(),
        "profile:retention-v4"
    );
    assert_eq!(
        permanent.member().access_policy_binding_id.as_bytes(),
        &[5; 16]
    );
    assert_eq!(
        permanent.member().residency_policy_binding_id.as_bytes(),
        &[6; 16]
    );
    assert_eq!(
        permanent.provenance().policy_reference(),
        b"policy-reference-v2"
    );
    assert_eq!(
        permanent.provenance().admission_reference(),
        b"admission-reference-v2"
    );
}

#[test]
fn v2_membership_is_refused_by_frozen_state_profiles_and_legacy_bytes_still_restore() {
    let permanent = RegistryStateRecord::Membership(permanent_record());
    assert!(
        permanent
            .encode_with_profile("eliot.registry-state.v3")
            .is_err()
    );

    let key = MembershipKey {
        corpus_id: OpaqueId::new("legacy-corpus").expect("corpus"),
        source_identity: source_identity(&typed_member()),
    };
    let legacy_id = crate::membership::derive_membership_id(&key);
    let legacy = RegistryStateRecord::Membership(MembershipRecord::new(
        key,
        legacy_id,
        rev(1),
        rev(1),
        MembershipLifecycle::Active,
        4,
        ReceiptRef::new("legacy-membership-receipt").expect("receipt"),
    ));
    let bytes_before = legacy
        .encode_with_profile("eliot.registry-state.v3")
        .expect("legacy bytes");
    let actual_hex = bytes_before
        .iter()
        .map(|byte| format!("{byte:02x}"))
        .collect::<String>();
    assert_eq!(
        actual_hex,
        "a2657374617465a86269645005f3cdee2edd032afa0c3211d122fcd5636b6579a266636f727075736d6c65676163792d636f72707573686964656e74697479a4646b696e64646e74667366736f757263655003030303030303030303030303030303696e616d65737061636550070707070707070707070707070707076a636f6d706f6e656e7473556d656d626572736869702d76352d666978747572656666616d696c796a6d656d62657273686970677265636569707478196c65676163792d6d656d626572736869702d72656365697074687265766973696f6e01696c6966656379636c65666163746976656a67656e65726174696f6e017172656769737472795f7265766973696f6e046770726f66696c6577656c696f742e72656769737472792d73746174652e7633"
    );
    assert_eq!(
        decode_registry_state(&bytes_before).expect("legacy decode"),
        legacy
    );
    let bytes_after = legacy
        .encode_with_profile("eliot.registry-state.v3")
        .expect("legacy re-encode");
    assert_eq!(bytes_after, bytes_before);
}

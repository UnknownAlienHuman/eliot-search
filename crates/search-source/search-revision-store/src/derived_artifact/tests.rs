//! One pure encoded-byte/profile test; no I/O, native, credential or crypto proof.
use super::*;
use search_contracts::{self as c, CanonicalValue as V};

#[test]
fn encoded_identity_roundtrip_and_mismatch_remain_unqualified() {
    fn digest(bytes: &[u8]) -> Blake3Digest32 {
        codec::hash_bytes(bytes, 4096).unwrap()
    }
    let domain_ids = [
        "62fa46de-6c86-4e73-94d3-c3e99b511001",
        "62fa46de-6c86-4e73-94d3-c3e99b511002",
        "62fa46de-6c86-4e73-94d3-c3e99b511003",
        "62fa46de-6c86-4e73-94d3-c3e99b511004",
        "62fa46de-6c86-4e73-94d3-c3e99b511005",
        "62fa46de-6c86-4e73-94d3-c3e99b511006",
    ];
    let domains = std::array::from_fn(|index| {
        DomainBinding::new(
            crate::DomainKind::ALL[index],
            *c::UuidBytes::parse(domain_ids[index]).unwrap().as_bytes(),
            c::to_canonical_cbor(&V::U64(index as u64))
                .unwrap()
                .into_vec(),
        )
        .unwrap()
    });
    let source = SourceRevisionRef {
        source_namespace_id: c::SourceNamespaceId::parse("7429c61b-1352-4a89-a45c-9547552e0011")
            .unwrap(),
        source_id: c::SourceId::parse("7429c61b-1352-4a89-a45c-9547552e0012").unwrap(),
        revision_id: c::SourceRevisionId::parse("7429c61b-1352-4a89-a45c-9547552e0013").unwrap(),
        content_digest: digest(b"original source bytes"),
        byte_length: 21,
    };
    let mat = MaterializationId::parse("7429c61b-1352-4a89-a45c-9547552e0014").unwrap();
    let rep = RepresentationId::parse("7429c61b-1352-4a89-a45c-9547552e0015").unwrap();
    let profile = DerivedOwnerProfileV1 {
        id: ProfileId::new("fixture-materializer-v2").unwrap(),
        revision: NonZeroRevision::new(2).unwrap(),
        identity_digest: digest(b"actual owner profile identity"),
    };
    let make = |kind, body: &[u8]| {
        DescriptorFixture::make(kind, body, &source, mat, rep, &profile, &domains)
    };
    let body = b"canonical representation";
    let descriptor = make(DerivedArtifactKindV1::RepresentationBody, body);
    assert_eq!(descriptor.encoded_body_digest(), digest(body));
    assert_ne!(
        descriptor.encoded_body_digest(),
        descriptor.source().content_digest
    );
    assert_ne!(
        descriptor.encoded_body_digest(),
        descriptor.owner_semantic_digest()
    );
    assert_eq!(
        DerivedArtifactDescriptorV1::decode(&descriptor.encode().unwrap()).unwrap(),
        descriptor
    );
    assert!(matches!(
        DerivedArtifactPayloadCandidateV1::metadata(descriptor.clone(), body.to_vec(), 4096),
        Err(DerivedArtifactErrorV1::PlaintextBodyForbidden)
    ));
    let binding = DerivedArtifactSealBindingV1::new(
        descriptor.clone(),
        &c::to_canonical_cbor(&V::U64(7)).unwrap().into_vec(),
        crate::EncryptionBinding {
            key_reference: OpaqueId::new("fixture-key-reference").unwrap(),
            key_version: NonZeroRevision::new(1).unwrap(),
            cipher_suite: crate::CipherSuite::AuthenticatedEncryptionV1,
        },
        0,
        [0; 12],
    )
    .unwrap();
    // Deliberately not encryption: framing can only produce an unqualified candidate.
    let candidate =
        DerivedArtifactPayloadCandidateV1::sealed(&binding, vec![0xa5; body.len() + 16], 32768)
            .unwrap();
    assert_eq!(
        candidate.qualification(),
        DerivedArtifactQualificationV1::Unqualified
    );
    assert_ne!(
        candidate.object_ref().address().unwrap().stored_digest(),
        descriptor.encoded_body_digest()
    );
    let mut impossible = candidate.object_ref().clone();
    impossible.stored_bytes = 0;
    assert!(matches!(
        DerivedArtifactObjectRefV1::decode(&impossible.encode().unwrap()),
        Err(DerivedArtifactErrorV1::Mismatch)
    ));
    let (decoded_binding, _) = payload::open_frame(candidate.encoded_object()).unwrap();
    assert_eq!(decoded_binding, binding);
    let metadata = make(
        DerivedArtifactKindV1::MaterializationMetadata,
        b"technical metadata",
    );
    assert!(matches!(
        DerivedArtifactPayloadCandidateV1::metadata(
            metadata.clone(),
            b"different bytes".to_vec(),
            4096
        ),
        Err(DerivedArtifactErrorV1::Mismatch)
    ));
    let metadata_candidate = DerivedArtifactPayloadCandidateV1::metadata(
        metadata.clone(),
        b"technical metadata".to_vec(),
        4096,
    )
    .unwrap();
    let unit = DerivedArtifactPayloadCandidateV1::metadata(
        make(DerivedArtifactKindV1::UnitManifest, b"unit envelope"),
        b"unit envelope".to_vec(),
        4096,
    )
    .unwrap();
    let projection = DerivedArtifactPayloadCandidateV1::metadata(
        make(
            DerivedArtifactKindV1::ProjectionManifest,
            b"projection envelope",
        ),
        b"projection envelope".to_vec(),
        4096,
    )
    .unwrap();
    let set = DerivedArtifactSetDescriptorV1::new([
        candidate.object_ref().clone(),
        metadata_candidate.object_ref().clone(),
        unit.object_ref().clone(),
        projection.object_ref().clone(),
    ])
    .unwrap();
    assert_eq!(
        DerivedArtifactSetDescriptorV1::decode(&set.encode().unwrap()).unwrap(),
        set
    );
    let set_candidate = set
        .candidate(OpaqueId::new("fixture-set-operation").unwrap())
        .unwrap();
    assert_eq!(
        set_candidate.qualification(),
        DerivedArtifactQualificationV1::Unqualified
    );
    let mut changed = metadata.clone();
    changed.domains[0] = DomainBinding::new(
        crate::DomainKind::Scope,
        changed.domains[0].uuid(),
        c::to_canonical_cbor(&V::U64(99)).unwrap().into_vec(),
    )
    .unwrap();
    assert_eq!(
        metadata.require_same_residency(&changed),
        Err(DerivedArtifactErrorV1::ResidencyMismatch)
    );
    let changed_candidate =
        DerivedArtifactPayloadCandidateV1::metadata(changed, b"technical metadata".to_vec(), 4096)
            .unwrap();
    assert_ne!(
        metadata_candidate.object_ref().address().unwrap(),
        changed_candidate.object_ref().address().unwrap()
    );
    let mut reordered = set.members().clone();
    reordered.swap(1, 2);
    assert!(matches!(
        DerivedArtifactSetDescriptorV1::new(reordered),
        Err(DerivedArtifactErrorV1::Mismatch)
    ));
}

struct DescriptorFixture;
impl DescriptorFixture {
    #[allow(clippy::too_many_arguments)]
    fn make(
        kind: DerivedArtifactKindV1,
        body: &[u8],
        source: &SourceRevisionRef,
        mat: MaterializationId,
        rep: RepresentationId,
        profile: &DerivedOwnerProfileV1,
        domains: &[DomainBinding; 6],
    ) -> DerivedArtifactDescriptorV1 {
        DerivedArtifactDescriptorV1::from_encoded(
            kind,
            *source,
            mat,
            rep,
            profile.clone(),
            codec::hash_bytes(b"owner semantic commitment", 4096).unwrap(),
            domains.clone(),
            OpaqueId::new(format!("fixture-operation:{}", kind.as_str())).unwrap(),
            body,
            4096,
        )
        .unwrap()
    }
}

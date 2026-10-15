#[allow(clippy::wildcard_imports)]
use super::*;
use crate::batch::{
    BatchCodecV4Error, DecodedBatch, OriginalInputResolver, RegistryStateDelta, RegistryStateKey, decode_batch_record_v4, decode_registry_state, decode_registry_state_key,
    decode_registry_state_key_v4, decode_registry_state_v4,
};
use crate::error::{DEFAULT_REGISTRY_LIMITS, JournalEntryKind, RegistryJournalEntry};
use crate::namespace::NamespaceCutoverState;
use search_contracts::{
    BoundedMap, CanonicalKey, CanonicalValue, InstallationIncarnationId, NamespaceOwnershipStatus, OpaqueId, OwnerEpoch, PolicyRevision, RootBindingId, SourceNamespaceId, SourceNamespaceOwnership,
    SourceOwnerGeneration, parse_canonical_cbor, to_canonical_cbor,
};
use search_source_admission::policy_v2::CheckpointFailureV2;
use search_source_identity::{CaseBehavior, FilesystemIdentityProfile, LinkBehavior, ReparseBehavior, StableFieldPolicy, UnicodeBehavior};
use std::sync::Arc;

struct Resolver {
    policy_bytes: Vec<u8>,
    admission_bytes: Vec<u8>,
    policy_reference: Vec<u8>,
    admission_reference: Vec<u8>,
}

impl Resolver {
    fn new(original: &OriginalAdmissionV2, permanent: &PermanentSourceAdmissionV2) -> Self {
        Self {
            policy_bytes: original.policy().encoded_inputs().to_vec(),
            admission_bytes: original.encoded_source_evidence().to_vec(),
            policy_reference: permanent.policy_reference().to_vec(),
            admission_reference: permanent.admission_reference().to_vec(),
        }
    }
}

impl OriginalInputResolver for Resolver {
    fn resolve_policy(&mut self, reference: &[u8], probe: &mut dyn FnMut() -> Result<(), CheckpointFailureV2>) -> Result<Arc<RetainedPolicyV2>, BatchCodecV4Error> {
        if reference != self.policy_reference {
            return Err(BatchCodecV4Error::Admission(AdmissionError::ReceiptMismatch));
        }
        let mut forward = || probe();
        RetainedPolicyV2::restore(
            &self.policy_bytes,
            &SectionPlatform {
                os: "windows".into(),
                arch: "x64".into(),
            },
            &AcceptedCapabilities {
                profiles: vec![BASELINE_PROFILE.into()],
            },
            &mut forward,
        )
        .map(Arc::new)
        .map_err(BatchCodecV4Error::Admission)
    }

    fn resolve_admission(&mut self, reference: &[u8], policy: Arc<RetainedPolicyV2>, probe: &mut dyn FnMut() -> Result<(), CheckpointFailureV2>) -> Result<OriginalAdmissionV2, BatchCodecV4Error> {
        if reference != self.admission_reference {
            return Err(BatchCodecV4Error::Admission(AdmissionError::ReceiptMismatch));
        }
        let mut forward = || probe();
        OriginalAdmissionV2::restore(policy, &self.admission_bytes, &mut forward).map_err(BatchCodecV4Error::Admission)
    }
}

fn profile() -> FilesystemIdentityProfile {
    FilesystemIdentityProfile::new(
        rev(3),
        Blake3Digest32::from_bytes([6; 32]),
        CaseBehavior::InsensitiveAscii,
        UnicodeBehavior::PreNormalizedNfc,
        StableFieldPolicy::Required,
        StableFieldPolicy::Required,
        LinkBehavior::StablePhysicalIdentity,
        ReparseBehavior::FinalTargetIdentity,
    )
    .expect("qualified profile")
}

fn ownership() -> SourceNamespaceOwnership {
    let mut value = SourceNamespaceOwnership {
        source_namespace_id: SourceNamespaceId::from_bytes([8; 16]),
        owner_system_id: OpaqueId::new("records-v4-owner").expect("owner"),
        owner_installation_incarnation_id: InstallationIncarnationId::from_bytes([9; 16]),
        owner_epoch: OwnerEpoch::new(2).expect("epoch"),
        ownership_record_revision: rev(3),
        source_owner_generation: SourceOwnerGeneration::from_bytes([0; 32]),
        source_admission_policy_revision: PolicyRevision::new(7),
        status: NamespaceOwnershipStatus::Active,
        cutover_receipt_ref: None,
    };
    value.source_owner_generation = search_domain::source_ownership_v2::source_owner_generation_v2(&value).expect("generation");
    value
}

fn namespace_state(original: &OriginalAdmissionV2) -> NamespaceCutoverState {
    let reference = OriginalInputsLayoutV2::policy_reference(original.policy(), &mut || Ok(())).expect("policy reference");
    NamespaceCutoverState::new_v2_metadata(
        ownership(),
        RootBindingId::from_bytes([3; 16]),
        profile(),
        Arc::clone(original.policy()),
        reference,
        5,
        receipt("namespace:register"),
        &mut || Ok(()),
    )
    .expect("namespace")
}

fn replace_ascii(mut bytes: Vec<u8>, from: &[u8], to: &[u8]) -> Vec<u8> {
    let start = bytes.windows(from.len()).position(|window| window == from).expect("marker");
    bytes[start..start + to.len()].copy_from_slice(to);
    bytes
}

fn add_unknown_field(bytes: &[u8]) -> Vec<u8> {
    let CanonicalValue::Object(map) = parse_canonical_cbor(bytes).expect("canonical") else {
        panic!("object")
    };
    let mut entries = (&map).into_iter().map(|(key, value)| (key.clone(), value.clone())).collect::<Vec<_>>();
    entries.push((CanonicalKey::new_non_empty("unknown").expect("key"), CanonicalValue::Null));
    to_canonical_cbor(&CanonicalValue::Object(BoundedMap::from_entries(entries).expect("map")))
        .expect("canonical")
        .as_slice()
        .to_vec()
}

#[test]
fn source_permanent_v4_roundtrips_through_real_original_owner() {
    let original = original();
    let source = registered(original.clone());
    let permanent = source.permanent_admission().expect("permanent").clone();
    let mut resolver = Resolver::new(&original, &permanent);
    let bytes = RegistryStateRecord::Source(source.clone()).encode_with_profile("eliot.registry-state.v4").expect("v4 source");
    let restored = decode_registry_state_v4(&bytes, &mut resolver, &mut || Ok(())).expect("restore");
    assert_eq!(restored, RegistryStateRecord::Source(source));
    assert!(decode_registry_state(&bytes).is_err());
}

#[test]
fn namespace_v4_roundtrips_profile_generation_and_key_profile() {
    let original = original();
    let state = namespace_state(&original);
    let expected_generation = state.ownership.source_owner_generation;
    let expected_profile = profile();
    let permanent = permanent(original.clone(), 9);
    let mut resolver = Resolver::new(&original, &permanent);
    let bytes = RegistryStateRecord::Namespace(state.clone()).encode_with_profile("eliot.registry-state.v4").expect("v4 namespace");
    let restored = decode_registry_state_v4(&bytes, &mut resolver, &mut || Ok(())).expect("restore");
    let RegistryStateRecord::Namespace(restored) = restored else { panic!("namespace") };
    assert_eq!(restored, state);
    assert_eq!(restored.permanent_metadata().expect("metadata").identity_profile(), expected_profile);
    assert_eq!(
        search_domain::source_ownership_v2::source_owner_generation_v2(&restored.ownership).expect("generation"),
        expected_generation
    );

    let key = RegistryStateKey::Namespace(SourceNamespaceId::from_bytes([8; 16]));
    let key_bytes = key.encode().expect("key");
    assert!(key_bytes.windows(b"eliot.registry-state-key.v2".len()).any(|w| w == b"eliot.registry-state-key.v2"));
    assert_eq!(decode_registry_state_key_v4(&key_bytes).expect("v4 key"), key);
    assert!(decode_registry_state_key(&key_bytes).is_err());
}

#[test]
fn v4_rejects_unknown_noncanonical_profile_and_origin_bytes() {
    let original = original();
    let source = registered(original.clone());
    let permanent = source.permanent_admission().expect("permanent").clone();
    let bytes = RegistryStateRecord::Source(source).encode_with_profile("eliot.registry-state.v4").expect("v4 source");
    let mut resolver = Resolver::new(&original, &permanent);
    assert!(decode_registry_state_v4(&add_unknown_field(&bytes), &mut resolver, &mut || Ok(())).is_err());
    let mut noncanonical = bytes.clone();
    noncanonical.push(0);
    assert!(decode_registry_state_v4(&noncanonical, &mut resolver, &mut || Ok(())).is_err());
    let profile = replace_ascii(bytes.clone(), b"eliot.registry-state.v4", b"eliot.registry-state.v9");
    assert!(decode_registry_state_v4(&profile, &mut resolver, &mut || Ok(())).is_err());
    resolver.policy_bytes.clear();
    assert!(decode_registry_state_v4(&bytes, &mut resolver, &mut || Ok(())).is_err());
    let mut foreign = Resolver::new(&original, &permanent);
    foreign.admission_bytes = vec![0];
    assert!(decode_registry_state_v4(&bytes, &mut foreign, &mut || Ok(())).is_err());
}

#[test]
fn v4_cancellation_before_and_during_restore_returns_no_state() {
    let original = original();
    let source = registered(original.clone());
    let permanent = source.permanent_admission().expect("permanent").clone();
    let bytes = RegistryStateRecord::Source(source).encode_with_profile("eliot.registry-state.v4").expect("v4 source");
    let mut before = Resolver::new(&original, &permanent);
    let result = decode_registry_state_v4(&bytes, &mut before, &mut || Err(CheckpointFailureV2::Cancelled));
    assert_eq!(result, Err(BatchCodecV4Error::Admission(AdmissionError::Cancelled)));

    let mut healthy = Resolver::new(&original, &permanent);
    let mut healthy_calls = 0;
    decode_registry_state_v4(&bytes, &mut healthy, &mut || {
        healthy_calls += 1;
        Ok(())
    })
    .expect("healthy");
    let mut during = Resolver::new(&original, &permanent);
    let fail_at = (healthy_calls / 2).max(2);
    let mut calls = 0;
    let result = decode_registry_state_v4(&bytes, &mut during, &mut || {
        calls += 1;
        if calls == fail_at { Err(CheckpointFailureV2::Cancelled) } else { Ok(()) }
    });
    assert_eq!(result, Err(BatchCodecV4Error::Admission(AdmissionError::Cancelled)));
}

#[test]
fn immutable_batch_v4_roundtrips_from_observed_parts_with_full_refs() {
    let original = original();
    let state = RegistryStateRecord::Source(registered(original.clone()));
    let permanent = match &state {
        RegistryStateRecord::Source(source) => source.permanent_admission().expect("permanent").clone(),
        _ => unreachable!(),
    };
    let delta = RegistryStateDelta::new(state.key(), None, state).expect("delta");
    let entry = RegistryJournalEntry::new(
        OpaqueId::new("batch:v4").expect("operation"),
        Blake3Digest32::from_bytes([7; 32]),
        0,
        1,
        JournalEntryKind::BatchApply,
        receipt("batch:v4"),
    );
    let mut resolver = Resolver::new(&original, &permanent);
    let batch = DecodedBatch::from_observed_parts_v4(entry, Blake3Digest32::from_bytes([8; 32]), 1, vec![delta], DEFAULT_REGISTRY_LIMITS, &mut resolver, &mut || Ok(())).expect("v4 batch");
    assert_eq!(batch.encoding_profile(), "eliot.registry-batch.v4");
    assert_eq!(batch.state_encoding_profile(), "eliot.registry-state.v4");
    let bytes = batch.encode().expect("encode");
    assert_eq!(decode_batch_record_v4(&bytes, DEFAULT_REGISTRY_LIMITS, &mut resolver, &mut || Ok(())).expect("decode"), batch);
}

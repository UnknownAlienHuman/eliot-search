//! Access-policy barrier codec round-trip (T20).
//!
//! The codec preserves bytes only: it performs no authority check and
//! assigns no grant. Enforcement stays owned by `search-access`.

use search_contracts::{
    AccessPolicyRevision, Blake3Digest32, PurgeFenceRevision, ShadowFenceRevision,
    SourceNamespaceId, SourceOwnerGeneration,
};
use search_control_redb::policy_codec::{
    ACCESS_POLICY_RECORD_LEN, ACCESS_POLICY_RECORD_MAGIC,
    ACCESS_POLICY_RECORD_VERSION, AccessPolicyRecord, PolicyCodecError,
    decode_access_policy, encode_access_policy,
};

const fn fixture() -> AccessPolicyRecord {
    AccessPolicyRecord {
        namespace_id: SourceNamespaceId::from_bytes([1; 16]),
        owner_generation: SourceOwnerGeneration::from_bytes([2; 32]),
        policy_revision: AccessPolicyRevision::new(3),
        live_deny_generation: 4,
        shadow_fence_revision: ShadowFenceRevision::new(5),
        purge_fence_revision: PurgeFenceRevision::new(6),
        policy_digest: Blake3Digest32::from_bytes([7; 32]),
    }
}

#[test]
fn round_trip_preserves_every_field() {
    let record = fixture();
    let bytes = encode_access_policy(&record);
    assert_eq!(bytes.len(), ACCESS_POLICY_RECORD_LEN);
    assert_eq!(
        &bytes[..ACCESS_POLICY_RECORD_MAGIC.len()],
        ACCESS_POLICY_RECORD_MAGIC
    );
    assert_eq!(decode_access_policy(&bytes), Ok(record));
}

#[test]
fn encoding_is_deterministic() {
    assert_eq!(
        encode_access_policy(&fixture()),
        encode_access_policy(&fixture())
    );
}

#[test]
fn magic_mismatch_fails_closed() {
    let mut bytes = encode_access_policy(&fixture());
    bytes[0] ^= 0xFF;
    assert_eq!(
        decode_access_policy(&bytes),
        Err(PolicyCodecError::MagicMismatch)
    );
}

#[test]
fn version_mismatch_fails_closed() {
    let mut bytes = encode_access_policy(&fixture());
    let version_at = ACCESS_POLICY_RECORD_MAGIC.len();
    bytes[version_at + 3] ^= 0x01;
    assert_eq!(
        decode_access_policy(&bytes),
        Err(PolicyCodecError::VersionUnsupported)
    );
}

#[test]
fn truncated_input_fails_closed() {
    let bytes = encode_access_policy(&fixture());
    assert_eq!(
        decode_access_policy(&bytes[..bytes.len() - 1]),
        Err(PolicyCodecError::Truncated)
    );
}

#[test]
fn trailing_bytes_fail_closed() {
    let mut bytes = encode_access_policy(&fixture());
    bytes.push(0);
    assert_eq!(
        decode_access_policy(&bytes),
        Err(PolicyCodecError::TrailingBytes)
    );
}

#[test]
fn version_constant_is_one() {
    assert_eq!(ACCESS_POLICY_RECORD_VERSION, 1);
}

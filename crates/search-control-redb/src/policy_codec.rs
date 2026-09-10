//! Access-policy barrier persistence codec (T20).
//!
//! Pure, deterministic byte codec for the barrier policy record: namespace,
//! owner generation, policy revision, live-deny generation, shadow/purge
//! fence revisions and the policy digest. The record is persisted as data by
//! the control journal; this codec never decides access, never validates a
//! grant and never consults live state. Enforcement stays owned by
//! `search-access` (`compile_pre_retrieval` and the live-barrier rechecks).
//!
//! Encoding is fixed-size and fail-closed: any magic, version, length or
//! trailing-byte mismatch decodes to [`PolicyCodecError`], never to a
//! defaulted or widened record.
//!
//! Wiring (integration owner): add to `lib.rs`
//!
//! ```text
//! mod policy_codec;
//! ```
//!
//! and store the encoded bytes through the existing journal tables. No new
//! redb table, schema migration or dependency is introduced here.

use search_contracts::{
    AccessPolicyRevision, Blake3Digest32, PurgeFenceRevision, ShadowFenceRevision,
    SourceNamespaceId, SourceOwnerGeneration,
};

/// Magic prefix for one encoded access-policy barrier record.
pub const ACCESS_POLICY_RECORD_MAGIC: &[u8; 8] = b"ELACCP01";
/// Exact codec version accepted by [`decode_access_policy`].
pub const ACCESS_POLICY_RECORD_VERSION: u32 = 1;
/// Maximum encoded record bytes (fixed-size format is far smaller).
pub const MAX_POLICY_RECORD_BYTES: usize = 256;
/// Exact encoded record bytes for [`ACCESS_POLICY_RECORD_VERSION`].
pub const ACCESS_POLICY_RECORD_LEN: usize = 124;

/// Barrier policy persisted as data: which namespace/owner/policy revision
/// the live-deny generation and fence revisions belong to.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct AccessPolicyRecord {
    /// Source namespace this barrier row governs.
    pub namespace_id: SourceNamespaceId,
    /// Owner generation this barrier row was fenced under.
    pub owner_generation: SourceOwnerGeneration,
    /// Access-policy revision this barrier row belongs to.
    pub policy_revision: AccessPolicyRevision,
    /// Live-deny generation published with this policy.
    pub live_deny_generation: u64,
    /// Shadow fence revision bound into predicate digests.
    pub shadow_fence_revision: ShadowFenceRevision,
    /// Purge fence revision bound into predicate digests.
    pub purge_fence_revision: PurgeFenceRevision,
    /// Digest of the canonical policy this row was derived from.
    pub policy_digest: Blake3Digest32,
}

/// Closed codec failure: every malformed input fails, never defaults.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum PolicyCodecError {
    /// Magic prefix mismatch: not an access-policy record.
    MagicMismatch,
    /// Record version other than [`ACCESS_POLICY_RECORD_VERSION`].
    VersionUnsupported,
    /// Input shorter than [`ACCESS_POLICY_RECORD_LEN`].
    Truncated,
    /// Input longer than [`ACCESS_POLICY_RECORD_LEN`].
    TrailingBytes,
}

impl PolicyCodecError {
    /// Stable machine-readable reason code.
    #[must_use]
    pub const fn code(self) -> &'static str {
        match self {
            Self::MagicMismatch => "ACCESS_POLICY_CODEC_MAGIC_MISMATCH",
            Self::VersionUnsupported => "ACCESS_POLICY_CODEC_VERSION_UNSUPPORTED",
            Self::Truncated => "ACCESS_POLICY_CODEC_TRUNCATED",
            Self::TrailingBytes => "ACCESS_POLICY_CODEC_TRAILING_BYTES",
        }
    }
}

impl core::fmt::Display for PolicyCodecError {
    fn fmt(&self, formatter: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        formatter.write_str(self.code())
    }
}

impl std::error::Error for PolicyCodecError {}

/// Encodes one barrier policy record to its canonical bytes.
///
/// Deterministic and bounded: always exactly [`ACCESS_POLICY_RECORD_LEN`]
/// bytes, well under [`MAX_POLICY_RECORD_BYTES`].
#[must_use]
pub fn encode_access_policy(record: &AccessPolicyRecord) -> Vec<u8> {
    let mut out = Vec::with_capacity(ACCESS_POLICY_RECORD_LEN);
    out.extend_from_slice(ACCESS_POLICY_RECORD_MAGIC);
    out.extend_from_slice(&ACCESS_POLICY_RECORD_VERSION.to_be_bytes());
    out.extend_from_slice(record.namespace_id.as_bytes());
    out.extend_from_slice(record.owner_generation.as_bytes());
    out.extend_from_slice(&record.policy_revision.get().to_be_bytes());
    out.extend_from_slice(&record.live_deny_generation.to_be_bytes());
    out.extend_from_slice(&record.shadow_fence_revision.get().to_be_bytes());
    out.extend_from_slice(&record.purge_fence_revision.get().to_be_bytes());
    out.extend_from_slice(record.policy_digest.as_bytes());
    debug_assert_eq!(out.len(), ACCESS_POLICY_RECORD_LEN);
    out
}

/// Decodes one barrier policy record, failing closed on any mismatch.
///
/// The codec preserves bytes only: it performs no authority check and
/// assigns no grant. A successfully decoded record is still only data until
/// `search-access` fences it against live state.
///
/// # Errors
///
/// Returns [`PolicyCodecError`] on magic, version, length or trailing-byte
/// mismatch. Never defaults or widens.
pub fn decode_access_policy(bytes: &[u8]) -> Result<AccessPolicyRecord, PolicyCodecError> {
    if bytes.len() != ACCESS_POLICY_RECORD_LEN {
        if bytes.len() >= ACCESS_POLICY_RECORD_MAGIC.len()
            && bytes[..ACCESS_POLICY_RECORD_MAGIC.len()] != *ACCESS_POLICY_RECORD_MAGIC
        {
            return Err(PolicyCodecError::MagicMismatch);
        }
        if bytes.len() < ACCESS_POLICY_RECORD_LEN {
            return Err(PolicyCodecError::Truncated);
        }
        return Err(PolicyCodecError::TrailingBytes);
    }
    let magic = &bytes[..8];
    if magic != ACCESS_POLICY_RECORD_MAGIC.as_slice() {
        return Err(PolicyCodecError::MagicMismatch);
    }
    let version = u32::from_be_bytes(bytes[8..12].try_into().unwrap_or([0xFF; 4]));
    if version != ACCESS_POLICY_RECORD_VERSION {
        return Err(PolicyCodecError::VersionUnsupported);
    }
    let mut namespace_bytes = [0_u8; 16];
    namespace_bytes.copy_from_slice(&bytes[12..28]);
    let mut owner_bytes = [0_u8; 32];
    owner_bytes.copy_from_slice(&bytes[28..60]);
    let mut digest_bytes = [0_u8; 32];
    digest_bytes.copy_from_slice(&bytes[92..124]);
    Ok(AccessPolicyRecord {
        namespace_id: SourceNamespaceId::from_bytes(namespace_bytes),
        owner_generation: SourceOwnerGeneration::from_bytes(owner_bytes),
        policy_revision: AccessPolicyRevision::new(u64::from_be_bytes(
            bytes[60..68].try_into().unwrap_or([0; 8]),
        )),
        live_deny_generation: u64::from_be_bytes(bytes[68..76].try_into().unwrap_or([0; 8])),
        shadow_fence_revision: ShadowFenceRevision::new(u64::from_be_bytes(
            bytes[76..84].try_into().unwrap_or([0; 8]),
        )),
        purge_fence_revision: PurgeFenceRevision::new(u64::from_be_bytes(
            bytes[84..92].try_into().unwrap_or([0; 8]),
        )),
        policy_digest: Blake3Digest32::from_bytes(digest_bytes),
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    fn record() -> AccessPolicyRecord {
        AccessPolicyRecord {
            namespace_id: SourceNamespaceId::from_bytes([0x20; 16]),
            owner_generation: SourceOwnerGeneration::from_bytes([0x21; 32]),
            policy_revision: AccessPolicyRevision::new(11),
            live_deny_generation: 7,
            shadow_fence_revision: ShadowFenceRevision::new(6),
            purge_fence_revision: PurgeFenceRevision::new(6),
            policy_digest: Blake3Digest32::from_bytes([0x22; 32]),
        }
    }

    #[test]
    fn round_trip_preserves_every_field() {
        let bytes = encode_access_policy(&record());
        assert_eq!(bytes.len(), ACCESS_POLICY_RECORD_LEN);
        assert!(bytes.len() <= MAX_POLICY_RECORD_BYTES);
        assert_eq!(decode_access_policy(&bytes), Ok(record()));
    }

    #[test]
    fn foreign_magic_fails_closed() {
        let mut bytes = encode_access_policy(&record());
        bytes[0] ^= 0xFF;
        assert_eq!(
            decode_access_policy(&bytes),
            Err(PolicyCodecError::MagicMismatch)
        );
    }

    #[test]
    fn foreign_version_fails_closed() {
        let mut bytes = encode_access_policy(&record());
        bytes[11] = bytes[11].wrapping_add(1);
        assert_eq!(
            decode_access_policy(&bytes),
            Err(PolicyCodecError::VersionUnsupported)
        );
    }

    #[test]
    fn truncated_input_fails_closed() {
        let bytes = encode_access_policy(&record());
        for len in [0, 7, 8, ACCESS_POLICY_RECORD_LEN - 1] {
            assert_eq!(
                decode_access_policy(&bytes[..len]),
                Err(PolicyCodecError::Truncated),
                "length {len} must fail closed"
            );
        }
    }

    #[test]
    fn trailing_bytes_fail_closed() {
        let mut bytes = encode_access_policy(&record());
        bytes.push(0);
        assert_eq!(
            decode_access_policy(&bytes),
            Err(PolicyCodecError::TrailingBytes)
        );
    }

    #[test]
    fn codec_persists_data_without_deciding() {
        // Even an all-maximal record round-trips byte-identical: the codec
        // assigns no authority, widens nothing and narrows nothing.
        let extreme = AccessPolicyRecord {
            namespace_id: SourceNamespaceId::from_bytes([0xFF; 16]),
            owner_generation: SourceOwnerGeneration::from_bytes([0xFF; 32]),
            policy_revision: AccessPolicyRevision::new(u64::MAX),
            live_deny_generation: u64::MAX,
            shadow_fence_revision: ShadowFenceRevision::new(u64::MAX),
            purge_fence_revision: PurgeFenceRevision::new(u64::MAX),
            policy_digest: Blake3Digest32::from_bytes([0xFF; 32]),
        };
        let bytes = encode_access_policy(&extreme);
        assert_eq!(decode_access_policy(&bytes), Ok(extreme));
    }
}

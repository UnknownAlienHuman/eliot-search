//! Deterministic typed scope-key framing and lower-case hex.

use search_projection_planner::ScopeExpectation;

use super::spec::SCOPE_KEY_DOMAIN;

/// Computes the scope-binding key identifying one exact projection plan scope.
///
/// This key is content-free control identity. It binds every typed coordinate
/// whose drift requires another manifest/reference and never includes a raw
/// source body, path, display name or Qdrant collection name.
#[must_use]
pub fn compute_scope_key(scope: &ScopeExpectation) -> [u8; 32] {
    let mut hasher = blake3::Hasher::new();
    hasher.update(SCOPE_KEY_DOMAIN);
    hasher.update(&[0]);
    hasher.update(scope.installation_incarnation_id.as_bytes());
    hasher.update(scope.collection_generation_id.as_bytes());
    hasher.update(scope.source_membership_id.as_bytes());
    hasher.update(scope.projection_membership_id.as_bytes());
    hasher.update(scope.access_partition_id.as_bytes());
    hasher.update(scope.scoring_partition_id.as_bytes());
    hasher.update(scope.source_id.as_bytes());
    hasher.update(scope.source_revision_id.as_bytes());
    hasher.update(scope.representation_id.as_bytes());
    append_text_hash(
        &mut hasher,
        scope.projection_profile_set_id.as_str(),
    );
    hasher.update(scope.profile_set_digest.as_bytes());
    hasher.update(scope.residency_digest.as_bytes());
    *hasher.finalize().as_bytes()
}

fn append_text_hash(hasher: &mut blake3::Hasher, value: &str) {
    let bytes = value.as_bytes();
    hasher.update(&u64::try_from(bytes.len()).unwrap_or(u64::MAX).to_be_bytes());
    hasher.update(bytes);
}

pub(super) fn hex(bytes: &[u8]) -> String {
    const TABLE: &[u8; 16] = b"0123456789abcdef";
    let mut output = String::with_capacity(bytes.len() * 2);
    for byte in bytes {
        output.push(char::from(TABLE[usize::from(byte >> 4)]));
        output.push(char::from(TABLE[usize::from(byte & 0x0f)]));
    }
    output
}

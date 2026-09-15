//! Deterministic payload/scope digest framing and lowercase hex.

use search_contracts::{Blake3Digest32, Epoch};
use search_projection_planner::ScopeExpectation;

use super::model::AdmittedUnitReceipt;
use super::spec::{PAYLOAD_DIGEST_DOMAIN, SCOPE_KEY_DOMAIN};

/// Computes the minimal-payload digest for one composed point.
#[must_use]
pub fn compute_payload_digest(
    scope: &ScopeExpectation,
    visible_epoch: Epoch,
    receipt: &AdmittedUnitReceipt,
) -> Blake3Digest32 {
    let mut hasher = blake3::Hasher::new();
    hasher.update(PAYLOAD_DIGEST_DOMAIN);
    hasher.update(&[0]);
    append_text_hash(&mut hasher, scope.source_membership_id.as_str());
    append_text_hash(&mut hasher, scope.projection_membership_id.as_str());
    hasher.update(&scope.source_revision.get().to_be_bytes());
    hasher.update(&receipt.unit_ordinal.to_be_bytes());
    hasher.update(&visible_epoch.get().to_be_bytes());
    hasher.update(receipt.access_partition_digest.as_bytes());
    hasher.update(receipt.unit_digest.as_bytes());
    hasher.update(receipt.representation_digest.as_bytes());
    Blake3Digest32::from_bytes(*hasher.finalize().as_bytes())
}

/// Computes the scope-binding key identifying one exact plan scope.
#[must_use]
pub fn compute_scope_key(scope: &ScopeExpectation) -> [u8; 32] {
    let mut hasher = blake3::Hasher::new();
    hasher.update(SCOPE_KEY_DOMAIN);
    hasher.update(&[0]);
    append_text_hash(&mut hasher, scope.namespace_id.as_str());
    append_text_hash(&mut hasher, scope.source_id.as_str());
    hasher.update(&scope.source_revision.get().to_be_bytes());
    append_text_hash(&mut hasher, scope.source_membership_id.as_str());
    append_text_hash(&mut hasher, scope.projection_membership_id.as_str());
    hasher.update(scope.projection_fingerprint.as_bytes());
    hasher.update(&scope.projection_schema_revision.get().to_be_bytes());
    hasher.update(scope.representation_digest.as_bytes());
    hasher.update(scope.scoring_partition_digest.as_bytes());
    hasher.update(scope.collection_generation_digest.as_bytes());
    hasher.update(scope.residency_digest.as_bytes());
    *hasher.finalize().as_bytes()
}

pub(super) fn append_text_hash(hasher: &mut blake3::Hasher, value: &str) {
    let bytes = value.as_bytes();
    hasher.update(&u64::try_from(bytes.len()).unwrap_or(u64::MAX).to_be_bytes());
    hasher.update(bytes);
}

pub(super) fn hex(bytes: &[u8]) -> String {
    const TABLE: &[u8; 16] = b"0123456789abcdef";
    let mut output = String::with_capacity(bytes.len() * 2);
    for byte in bytes {
        output.push(char::from(TABLE[usize::from(byte >> 4)]));
        output.push(char::from(TABLE[usize::from(byte & 0x0F)]));
    }
    output
}

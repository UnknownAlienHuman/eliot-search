//! Candidate identity, metadata digest and authority ceiling.

use serde_json::Value;
use sha2::{Digest, Sha256};

use super::spec::{
    AUTHORITY_FIELDS, CANDIDATE_ID_DOMAIN, CANDIDATE_METADATA_DOMAIN,
};

/// SHA-256 over the candidate domain plus bundle bytes.
#[must_use]
pub fn candidate_id(bundle_bytes: &[u8]) -> String {
    let mut hasher = Sha256::new();
    hasher.update(CANDIDATE_ID_DOMAIN);
    hasher.update(bundle_bytes);
    format!("{:x}", hasher.finalize())
}

/// SHA-256 over the metadata domain plus canonical metadata bytes.
#[must_use]
pub fn candidate_metadata_digest(candidate_without_digest: &Value) -> String {
    let mut hasher = Sha256::new();
    hasher.update(CANDIDATE_METADATA_DOMAIN);
    hasher.update(crate::ticket_planner::canonical_json_bytes(
        candidate_without_digest,
    ));
    format!("{:x}", hasher.finalize())
}

/// Recomputes `candidate_sha256` over the payload without that field.
#[must_use]
pub fn assert_candidate_digest(candidate: &Value) -> bool {
    let Value::Object(map) = candidate else {
        return false;
    };
    let Some(Value::String(digest)) = map.get("candidate_sha256") else {
        return false;
    };
    let mut payload = map.clone();
    payload.remove("candidate_sha256");
    &candidate_metadata_digest(&Value::Object(payload)) == digest
}

/// Returns the all-false authority ceiling object.
#[must_use]
pub fn authority_map() -> Value {
    let mut map = serde_json::Map::new();
    for field in AUTHORITY_FIELDS {
        map.insert(field.to_owned(), Value::Bool(false));
    }
    Value::Object(map)
}

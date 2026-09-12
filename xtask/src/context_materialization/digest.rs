//! Authority ceiling and domain-separated materialization digests.

use serde_json::Value;
use sha2::{Digest, Sha256};

use super::spec::{AUTHORITY_FIELDS, OPERATION_DOMAIN, PLAN_DOMAIN};

/// Returns the all-false authority ceiling object.
#[must_use]
pub fn authority_map() -> Value {
    let mut map = serde_json::Map::new();
    for field in AUTHORITY_FIELDS {
        map.insert(field.to_owned(), Value::Bool(false));
    }
    Value::Object(map)
}

/// SHA-256 over the plan domain plus canonical plan bytes.
#[must_use]
pub fn plan_digest(value_without_digest: &Value) -> String {
    let mut hasher = Sha256::new();
    hasher.update(PLAN_DOMAIN);
    hasher.update(crate::ticket_planner::canonical_json_bytes(
        value_without_digest,
    ));
    format!("{:x}", hasher.finalize())
}

/// SHA-256 over the operation domain plus canonical manifest bytes.
#[must_use]
pub fn operation_id(input_manifest: &Value) -> String {
    let mut hasher = Sha256::new();
    hasher.update(OPERATION_DOMAIN);
    hasher.update(crate::ticket_planner::canonical_json_bytes(input_manifest));
    format!("{:x}", hasher.finalize())
}

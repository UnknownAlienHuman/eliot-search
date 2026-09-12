//! Bundle block model and canonical header projection.

use serde_json::Value;

use super::super::error::ContextArtifactError;

/// One canonical context-artifact bundle block.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct BundleBlock {
    /// Block kind: `source`, `registry_fragment` or `accepted_handoff`.
    pub kind: String,
    /// Exact canonical header line without terminal LF.
    pub header: String,
    /// Block metadata without framing fields.
    pub metadata: Value,
    /// Raw block content bytes.
    pub content: Vec<u8>,
}

fn metadata_string(metadata: &Value, key: &str) -> String {
    metadata
        .get(key)
        .and_then(Value::as_str)
        .unwrap_or_default()
        .to_owned()
}

/// Projects the canonical header for one closed block kind.
///
/// # Errors
///
/// Returns `BUNDLE_FORMAT_INVALID` for an unknown block kind.
pub fn expected_header(
    kind: &str,
    metadata: &Value,
) -> Result<String, ContextArtifactError> {
    match kind {
        "source" => Ok(format!(
            "--- repository-path: {} ---",
            metadata_string(metadata, "repository_path")
        )),
        "registry_fragment" => Ok(format!(
            "--- registry-selector: {}::{} ---",
            metadata_string(metadata, "registry_path"),
            metadata_string(metadata, "selector")
        )),
        "accepted_handoff" => Ok(format!(
            "--- accepted-handoff: {} ---",
            metadata_string(metadata, "package")
        )),
        _ => Err(ContextArtifactError::new(
            "BUNDLE_FORMAT_INVALID",
            format!("unknown bundle block kind: {kind}"),
        )),
    }
}

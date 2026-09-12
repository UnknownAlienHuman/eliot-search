//! Canonical bundle rendering.

use serde_json::Value;

use super::bundle_error;
use super::model::{BundleBlock, expected_header};
use super::super::error::ContextArtifactError;
use super::super::spec::{BUNDLE_END, BUNDLE_MAGIC, MAX_BUNDLE_BYTES};

/// Renders canonical bundle bytes for a preamble and ordered blocks.
///
/// # Errors
///
/// Returns `BUNDLE_FORMAT_INVALID` when metadata/header framing differs from
/// the closed codec, or `BUNDLE_SIZE_EXCEEDED` above the 20 MiB ceiling.
pub fn render_bundle(
    preamble: &Value,
    blocks: &[BundleBlock],
) -> Result<Vec<u8>, ContextArtifactError> {
    let mut output = Vec::new();
    output.extend_from_slice(BUNDLE_MAGIC);
    output.extend_from_slice(&crate::ticket_planner::canonical_json_bytes(
        preamble,
    ));
    for block in blocks {
        let mut metadata = match &block.metadata {
            Value::Object(map) => map.clone(),
            _ => return Err(bundle_error("block metadata must be an object")),
        };
        metadata.insert("block_kind".to_owned(), Value::String(block.kind.clone()));
        metadata.insert(
            "content_bytes".to_owned(),
            Value::Number(serde_json::Number::from(
                u64::try_from(block.content.len())
                    .map_err(|_| bundle_error("block content too large"))?,
            )),
        );
        metadata.insert(
            "content_sha256".to_owned(),
            Value::String(crate::ticket_planner::exact_sha256_hex(
                &block.content,
            )),
        );
        let full = Value::Object(metadata);
        let header = expected_header(&block.kind, &full)?;
        if block.header != header {
            return Err(bundle_error(format!(
                "bundle header differs from canonical header: {}",
                block.header
            )));
        }
        output.extend_from_slice(header.as_bytes());
        output.push(b'\n');
        output.extend_from_slice(&crate::ticket_planner::canonical_json_bytes(&full));
        output.extend_from_slice(&block.content);
        output.push(b'\n');
    }
    output.extend_from_slice(BUNDLE_END);
    if output.len() > MAX_BUNDLE_BYTES {
        return Err(ContextArtifactError::new(
            "BUNDLE_SIZE_EXCEEDED",
            format!(
                "context artifact candidate exceeds {MAX_BUNDLE_BYTES} bytes"
            ),
        ));
    }
    Ok(output)
}

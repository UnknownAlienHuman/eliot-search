//! Strict inverse of the canonical context-artifact bundle renderer.

use serde_json::Value;

use super::bundle_error;
use super::model::{BundleBlock, expected_header};
use super::super::error::ContextArtifactError;
use super::super::spec::{
    ARTIFACT_FORMAT, BUNDLE_END, BUNDLE_MAGIC, MAX_BUNDLE_BYTES,
};

fn read_line(
    data: &[u8],
    offset: usize,
) -> Result<(Vec<u8>, usize), ContextArtifactError> {
    let rest = data
        .get(offset..)
        .ok_or_else(|| bundle_error("unterminated bundle line"))?;
    let end = rest
        .iter()
        .position(|byte| *byte == b'\n')
        .ok_or_else(|| bundle_error("unterminated bundle line"))?;
    Ok((rest[..end].to_vec(), offset + end + 1))
}

fn parse_canonical_line(raw: &[u8]) -> Result<Value, ContextArtifactError> {
    let text = std::str::from_utf8(raw)
        .map_err(|_| bundle_error("invalid JSON line encoding"))?;
    let value: Value = serde_json::from_str(text)
        .map_err(|_| bundle_error("invalid JSON line"))?;
    let Value::Object(_) = value else {
        return Err(bundle_error("JSON line must be an object"));
    };
    let mut canonical = crate::ticket_planner::canonical_json_bytes(&value);
    canonical.pop();
    if canonical != raw {
        return Err(bundle_error("JSON line is not canonical"));
    }
    Ok(value)
}

fn block_size(metadata: &Value) -> Result<usize, ContextArtifactError> {
    match metadata.get("content_bytes") {
        Some(Value::Number(number)) if number.is_u64() => {
            usize::try_from(number.as_u64().unwrap_or(0))
                .map_err(|_| bundle_error("invalid block framing metadata"))
        }
        Some(Value::Number(number))
            if number.is_i64() && number.as_i64().unwrap_or(-1) >= 0 =>
        {
            usize::try_from(number.as_i64().unwrap_or(0))
                .map_err(|_| bundle_error("invalid block framing metadata"))
        }
        _ => Err(bundle_error("invalid block framing metadata")),
    }
}

fn preamble_count(preamble: &Value, key: &str) -> Option<i64> {
    let value = preamble.get(key)?;
    value.as_i64().map_or_else(
        || value.as_u64().and_then(|number| i64::try_from(number).ok()),
        Some,
    )
}

fn check_bundle_counts(
    preamble: &Value,
    blocks: usize,
) -> Result<(), ContextArtifactError> {
    let (Some(sources), Some(fragments), Some(handoffs)) = (
        preamble_count(preamble, "source_count"),
        preamble_count(preamble, "registry_fragment_count"),
        preamble_count(preamble, "accepted_handoff_count"),
    ) else {
        return Err(bundle_error("invalid bundle counts"));
    };
    if sources < 0 || fragments < 0 || handoffs < 0 {
        return Err(bundle_error("invalid bundle counts"));
    }
    if usize::try_from(sources + fragments + handoffs)
        .is_ok_and(|total| total != blocks)
    {
        return Err(bundle_error("bundle count mismatch"));
    }
    Ok(())
}

fn parse_one_block(
    data: &[u8],
    offset: &mut usize,
    header_line: &[u8],
) -> Result<Option<BundleBlock>, ContextArtifactError> {
    let mut with_end = header_line.to_owned();
    with_end.push(b'\n');
    if with_end == BUNDLE_END {
        return Ok(None);
    }
    let header = std::str::from_utf8(header_line)
        .map_err(|_| bundle_error("non-UTF-8 block header"))?
        .to_owned();
    let (metadata_raw, next) = read_line(data, *offset)?;
    *offset = next;
    let metadata = parse_canonical_line(&metadata_raw)?;
    let kind = metadata
        .get("block_kind")
        .and_then(Value::as_str)
        .ok_or_else(|| bundle_error("invalid block framing metadata"))?;
    let size = block_size(&metadata)?;
    let digest = metadata
        .get("content_sha256")
        .and_then(Value::as_str)
        .unwrap_or_default()
        .to_owned();
    if offset.checked_add(size).is_none_or(|end| end >= data.len()) {
        return Err(bundle_error("truncated block content"));
    }
    let content = data[*offset..*offset + size].to_vec();
    *offset += size;
    if data.get(*offset) != Some(&b'\n') {
        return Err(bundle_error("missing block delimiter"));
    }
    *offset += 1;
    if crate::ticket_planner::exact_sha256_hex(&content) != digest {
        return Err(bundle_error("block digest mismatch"));
    }
    if header != expected_header(kind, &metadata)? {
        return Err(bundle_error("block header mismatch"));
    }
    let mut clean = metadata.as_object().cloned().unwrap_or_default();
    clean.remove("block_kind");
    clean.remove("content_bytes");
    clean.remove("content_sha256");
    Ok(Some(BundleBlock {
        kind: kind.to_owned(),
        header,
        metadata: Value::Object(clean),
        content,
    }))
}

/// Strict inverse of [`super::render_bundle`].
///
/// # Errors
///
/// Returns `BUNDLE_FORMAT_INVALID` for any magic, framing, digest, header,
/// count or trailing-byte violation.
pub fn parse_bundle(
    data: &[u8],
) -> Result<(Value, Vec<BundleBlock>), ContextArtifactError> {
    if data.len() > MAX_BUNDLE_BYTES || !data.starts_with(BUNDLE_MAGIC) {
        return Err(bundle_error("invalid bundle magic or size"));
    }
    let mut offset = BUNDLE_MAGIC.len();
    let (preamble_raw, next) = read_line(data, offset)?;
    offset = next;
    let preamble = parse_canonical_line(&preamble_raw)?;
    if preamble.get("artifact_format").and_then(Value::as_str)
        != Some(ARTIFACT_FORMAT)
    {
        return Err(bundle_error("artifact format mismatch"));
    }
    let mut blocks = Vec::new();
    loop {
        let (line, next) = read_line(data, offset)?;
        offset = next;
        match parse_one_block(data, &mut offset, &line)? {
            Some(block) => blocks.push(block),
            None => break,
        }
    }
    if offset != data.len() {
        return Err(bundle_error("trailing bundle bytes"));
    }
    check_bundle_counts(&preamble, blocks.len())?;
    Ok((preamble, blocks))
}

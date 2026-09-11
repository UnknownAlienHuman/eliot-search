//! Bounded port of pure `tools/context_artifact_builder_v1` helpers (T41, context-artifact slice).
//!
//! Covers only deterministic, IO-free helpers from `core.py` / `bundle.py`:
//! closed constants, UTF-8/LF normalization, JSON-value fence, domain-separated
//! digests, authority ceiling, advisory output-root grammar and exact bundle
//! framing (`render_bundle` / `parse_bundle`).
//!
//! Git-tree reads (`GitView`), draft preflight/extraction (`extract.py`),
//! candidate assembly and idempotent writes (`build.py`, `core.write_exact_idempotent`)
//! plus both `build`/`validate` entrypoints remain Python-owned (see report).

use serde_json::Value;
use sha2::{Digest, Sha256};
use std::error::Error;
use std::fmt::{Display, Formatter, Result as FmtResult};

/// `SCHEMA_VERSION`: candidate metadata schema version.
pub const SCHEMA_VERSION: i64 = 1;
/// `RECORD_KIND`: candidate metadata record kind.
pub const RECORD_KIND: &str = "context_artifact_candidate_v1";
/// `STATUS`: candidates are never stored/signed by the builder.
pub const STATUS: &str = "ARTIFACT_CANDIDATE_NOT_STORED_NOT_SIGNED";
/// `ARTIFACT_FORMAT`: bundle magic format token.
pub const ARTIFACT_FORMAT: &str = "ELIOT_SWARM_CONTEXT_1";
/// `ARTIFACT_ROOT`: sole writable candidate artifact directory.
pub const ARTIFACT_ROOT: &str = "artifacts/context-artifact-candidates";
/// Bundle magic prefix (includes trailing newline).
pub const BUNDLE_MAGIC: &[u8] = b"ELIOT_SWARM_CONTEXT_1\n";
/// Bundle end marker (includes trailing newline).
pub const BUNDLE_END: &[u8] = b"--- end-context-artifact ---\n";
/// Domain separator for `candidate_id`.
pub const CANDIDATE_ID_DOMAIN: &[u8] = b"eliot-search/context-artifact-candidate/v1\0";
/// Domain separator for `candidate_sha256`.
pub const CANDIDATE_METADATA_DOMAIN: &[u8] =
    b"eliot-search/context-artifact-candidate-metadata/v1\0";
/// `MAX_BUNDLE_BYTES`: bundle size ceiling (20 MiB).
pub const MAX_BUNDLE_BYTES: usize = 20 * 1024 * 1024;

/// Additional failure codes registered by the candidate schema (order-pinned).
pub const ADDITIONAL_FAILURE_CODES: [&str; 9] = [
    "CONTEXT_ARTIFACT_BUILDER_CONTRACT_MISMATCH",
    "CONTEXT_SOURCE_CONTAINS_NUL",
    "REGISTRY_FRAGMENT_NONCANONICAL",
    "BUNDLE_FORMAT_INVALID",
    "BUNDLE_SIZE_EXCEEDED",
    "OUTPUT_PATH_OUTSIDE_ARTIFACT_ROOT",
    "OUTPUT_PATH_SYMLINK",
    "CANDIDATE_OUTPUT_CONFLICT",
    "CANDIDATE_OUTPUT_WRITE_FAILED",
];

/// Authority ceiling fields (order-pinned, always `false`).
pub const AUTHORITY_FIELDS: [&str; 9] = [
    "materializes_authoritative_context",
    "creates_context_manifest_record",
    "creates_immutable_artifact_ref",
    "creates_assignment_ticket",
    "creates_writer_lease",
    "authorizes_implementation",
    "publishes_package_handoff",
    "accepts_gate_or_wave",
    "advances_launch_state",
];

/// Manifest projection unresolved fields (order-pinned).
pub const UNRESOLVED_MANIFEST_FIELDS: [&str; 14] = [
    "identity.context_id",
    "identity.operation_id",
    "artifact.ref",
    "verification.readback_verified",
    "signature.created_at",
    "signature.materializer_identity",
    "signature.reviewer_identity",
    "signature.record_sha256",
    "signature.materializer_signature_ref",
    "signature.reviewer_signature_ref",
    "record_path.context_record_sha256",
    "record_path.git_commit",
    "record_path.git_blob_id",
    "record_path.exact_record_file_sha256",
];

/// Typed failure carrying the machine `reason_code` (`CandidateFailure` in Python).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ContextArtifactError {
    reason: &'static str,
    message: String,
}

impl ContextArtifactError {
    fn new(reason: &'static str, message: impl Into<String>) -> Self {
        Self {
            reason,
            message: message.into(),
        }
    }

    /// Machine reason code (closed registry).
    #[must_use]
    pub const fn reason(&self) -> &'static str {
        self.reason
    }
}

impl Display for ContextArtifactError {
    fn fmt(&self, f: &mut Formatter<'_>) -> FmtResult {
        write!(f, "{}: {}", self.reason, self.message)
    }
}

impl Error for ContextArtifactError {}

/// `normalize_utf8_lf`: strict UTF-8 decode, NUL fence, CRLF/CR to LF.
///
/// # Errors
///
/// Returns `CONTEXT_SOURCE_NOT_UTF8` when `raw` is not strict UTF-8, or
/// `CONTEXT_SOURCE_CONTAINS_NUL` when the decoded text holds NUL.
pub fn normalize_utf8_lf(raw: &[u8]) -> Result<Vec<u8>, ContextArtifactError> {
    let text = std::str::from_utf8(raw).map_err(|_| {
        ContextArtifactError::new("CONTEXT_SOURCE_NOT_UTF8", "source is not strict UTF-8")
    })?;
    if text.contains('\0') {
        return Err(ContextArtifactError::new(
            "CONTEXT_SOURCE_CONTAINS_NUL",
            "source contains NUL",
        ));
    }
    Ok(text.replace("\r\n", "\n").replace('\r', "\n").into_bytes())
}

fn require_json_inner(value: &Value) -> bool {
    match value {
        Value::Null => false,
        Value::Bool(_) | Value::String(_) => true,
        Value::Number(number) => number.is_i64() || number.is_u64(),
        Value::Array(items) => items.iter().all(require_json_inner),
        Value::Object(map) => map.values().all(require_json_inner),
    }
}

/// `require_json_value`: forbid null and floating-point values recursively.
///
/// # Errors
///
/// Returns `REGISTRY_FRAGMENT_NONCANONICAL` on null, float, or unsupported shapes.
pub fn require_json_value(value: &Value) -> Result<(), ContextArtifactError> {
    if require_json_inner(value) {
        Ok(())
    } else {
        Err(ContextArtifactError::new(
            "REGISTRY_FRAGMENT_NONCANONICAL",
            "value contains a forbidden null or floating-point value",
        ))
    }
}

/// `candidate_id`: SHA-256 over the candidate domain separator plus bundle bytes.
#[must_use]
pub fn candidate_id(bundle_bytes: &[u8]) -> String {
    let mut hasher = Sha256::new();
    hasher.update(CANDIDATE_ID_DOMAIN);
    hasher.update(bundle_bytes);
    format!("{:x}", hasher.finalize())
}

/// `candidate_metadata_digest`: SHA-256 over the metadata domain plus canonical bytes.
#[must_use]
pub fn candidate_metadata_digest(candidate_without_digest: &Value) -> String {
    let mut hasher = Sha256::new();
    hasher.update(CANDIDATE_METADATA_DOMAIN);
    hasher.update(crate::ticket_planner::canonical_json_bytes(
        candidate_without_digest,
    ));
    format!("{:x}", hasher.finalize())
}

/// `assert_candidate_digest`: recompute `candidate_sha256` over the payload minus itself.
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

/// `authority_map`: all-false authority ceiling object.
#[must_use]
pub fn authority_map() -> Value {
    let mut map = serde_json::Map::new();
    for field in AUTHORITY_FIELDS {
        map.insert(field.to_owned(), Value::Bool(false));
    }
    Value::Object(map)
}

/// Advisory output-root grammar (pure prefix of `validate_output_root`).
///
/// Mirrors the `safe_path` + `under(ARTIFACT_ROOT)` + trailing `/.` fence;
/// symlink and parent-directory creation checks stay Python-owned.
///
/// Returns the slash-normalized target on success.
///
/// # Errors
///
/// Returns `OUTPUT_PATH_OUTSIDE_ARTIFACT_ROOT` when the normalized path is
/// not `ARTIFACT_ROOT` or a descendant.
pub fn advisory_output_target(relative: &str) -> Result<String, ContextArtifactError> {
    let normalized = relative.replace('\\', "/");
    let normalized = normalized.trim_end_matches('/').to_owned();
    if crate::ticket_planner::safe_path(&normalized)
        && crate::ticket_planner::under(&normalized, ARTIFACT_ROOT)
        && normalized != format!("{ARTIFACT_ROOT}/.")
    {
        Ok(normalized)
    } else {
        Err(ContextArtifactError::new(
            "OUTPUT_PATH_OUTSIDE_ARTIFACT_ROOT",
            format!("output root must be {ARTIFACT_ROOT} or a descendant"),
        ))
    }
}

fn meta_str(metadata: &Value, key: &str) -> String {
    metadata
        .get(key)
        .and_then(Value::as_str)
        .unwrap_or_default()
        .to_owned()
}

/// `expected_header`: canonical bundle block header per kind.
///
/// # Errors
///
/// Returns `BUNDLE_FORMAT_INVALID` for unknown block kinds.
pub fn expected_header(kind: &str, metadata: &Value) -> Result<String, ContextArtifactError> {
    match kind {
        "source" => Ok(format!(
            "--- repository-path: {} ---",
            meta_str(metadata, "repository_path")
        )),
        "registry_fragment" => Ok(format!(
            "--- registry-selector: {}::{} ---",
            meta_str(metadata, "registry_path"),
            meta_str(metadata, "selector")
        )),
        "accepted_handoff" => Ok(format!(
            "--- accepted-handoff: {} ---",
            meta_str(metadata, "package")
        )),
        _ => Err(ContextArtifactError::new(
            "BUNDLE_FORMAT_INVALID",
            format!("unknown bundle block kind: {kind}"),
        )),
    }
}

/// One bundle block (header is the exact canonical header string).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct BundleBlock {
    /// Block kind (`source` / `registry_fragment` / `accepted_handoff`).
    pub kind: String,
    /// Exact canonical header line (without trailing newline).
    pub header: String,
    /// Block metadata object (without framing fields).
    pub metadata: Value,
    /// Raw block content bytes.
    pub content: Vec<u8>,
}

fn bundle_error(message: impl Into<String>) -> ContextArtifactError {
    ContextArtifactError::new("BUNDLE_FORMAT_INVALID", message)
}

/// `render_bundle`: canonical bundle bytes for a preamble plus ordered blocks.
///
/// # Errors
///
/// Returns `BUNDLE_FORMAT_INVALID` when a header differs from canonical framing,
/// or `BUNDLE_SIZE_EXCEEDED` when output passes the 20 MiB ceiling.
pub fn render_bundle(
    preamble: &Value,
    blocks: &[BundleBlock],
) -> Result<Vec<u8>, ContextArtifactError> {
    let mut out = Vec::new();
    out.extend_from_slice(BUNDLE_MAGIC);
    out.extend_from_slice(&crate::ticket_planner::canonical_json_bytes(preamble));
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
            Value::String(crate::ticket_planner::exact_sha256_hex(&block.content)),
        );
        let full = Value::Object(metadata);
        let header = expected_header(&block.kind, &full)?;
        if block.header != header {
            return Err(bundle_error(format!(
                "bundle header differs from canonical header: {}",
                block.header
            )));
        }
        out.extend_from_slice(header.as_bytes());
        out.push(b'\n');
        out.extend_from_slice(&crate::ticket_planner::canonical_json_bytes(&full));
        out.extend_from_slice(&block.content);
        out.push(b'\n');
    }
    out.extend_from_slice(BUNDLE_END);
    if out.len() > MAX_BUNDLE_BYTES {
        return Err(ContextArtifactError::new(
            "BUNDLE_SIZE_EXCEEDED",
            format!("context artifact candidate exceeds {MAX_BUNDLE_BYTES} bytes"),
        ));
    }
    Ok(out)
}

fn read_line(data: &[u8], offset: usize) -> Result<(Vec<u8>, usize), ContextArtifactError> {
    let rest = data
        .get(offset..)
        .ok_or_else(|| bundle_error("unterminated bundle line"))?;
    let end = rest
        .iter()
        .position(|b| *b == b'\n')
        .ok_or_else(|| bundle_error("unterminated bundle line"))?;
    Ok((rest[..end].to_vec(), offset + end + 1))
}

fn parse_canonical_line(raw: &[u8]) -> Result<Value, ContextArtifactError> {
    let text = std::str::from_utf8(raw).map_err(|_| bundle_error("invalid JSON line encoding"))?;
    let value: Value = serde_json::from_str(text).map_err(|_| bundle_error("invalid JSON line"))?;
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
        Some(Value::Number(number)) if number.is_i64() && number.as_i64().unwrap_or(-1) >= 0 => {
            usize::try_from(number.as_i64().unwrap_or(0))
                .map_err(|_| bundle_error("invalid block framing metadata"))
        }
        _ => Err(bundle_error("invalid block framing metadata")),
    }
}

fn preamble_count(preamble: &Value, key: &str) -> Option<i64> {
    let value = preamble.get(key)?;
    value
        .as_i64()
        .map_or_else(|| value.as_u64().and_then(|u| i64::try_from(u).ok()), Some)
}

fn check_bundle_counts(preamble: &Value, blocks: usize) -> Result<(), ContextArtifactError> {
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
    if usize::try_from(sources + fragments + handoffs).is_ok_and(|total| total != blocks) {
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
    let (meta_raw, next) = read_line(data, *offset)?;
    *offset = next;
    let metadata = parse_canonical_line(&meta_raw)?;
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

/// `parse_bundle`: strict inverse of `render_bundle`.
///
/// # Errors
///
/// Returns `BUNDLE_FORMAT_INVALID` on any magic, framing, digest, header,
/// count, or trailing-byte violation.
pub fn parse_bundle(data: &[u8]) -> Result<(Value, Vec<BundleBlock>), ContextArtifactError> {
    if data.len() > MAX_BUNDLE_BYTES || !data.starts_with(BUNDLE_MAGIC) {
        return Err(bundle_error("invalid bundle magic or size"));
    }
    let mut offset = BUNDLE_MAGIC.len();
    let (preamble_raw, next) = read_line(data, offset)?;
    offset = next;
    let preamble = parse_canonical_line(&preamble_raw)?;
    if preamble.get("artifact_format").and_then(Value::as_str) != Some(ARTIFACT_FORMAT) {
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

//! Port of `tools/accepted_evidence_digest_v1.py` (T41, family E).
//!
//! Byte-exact canonical JSON (`sort_keys`, `separators=(",", ":")`,
//! `ensure_ascii=False`, trailing LF) and SHA-256 manifest behavior.

use std::error::Error;
use std::fmt::{Display, Formatter, Result as FmtResult};

/// Manifest schema version (`SCHEMA_VERSION`).
pub const SCHEMA_VERSION: u32 = 1;
/// Digest profile name (`PROFILE`).
pub const PROFILE: &str = "accepted_evidence_digest_v1";
/// Manifest magic first line including LF (`MAGIC`).
pub const MAGIC: &[u8] = b"ELIOT_ACCEPTED_EVIDENCE_MANIFEST_1\n";
/// Maximum evidence items (`MAX_EVIDENCE`).
pub const MAX_EVIDENCE: usize = 256;
/// Maximum manifest bytes (`MAX_MANIFEST_BYTES`).
pub const MAX_MANIFEST_BYTES: usize = 4 * 1024 * 1024;

const EXPECTED_FIELDS: [&str; 6] = [
    "requirement_id",
    "evidence_class",
    "artifact_ref",
    "artifact_sha256",
    "raw_outcome_digest",
    "availability",
];

const ARTIFACT_FIELDS: [&str; 4] = ["store_profile_ref", "artifact_id", "bytes", "sha256"];

const EXPECTED_FIELDS_REPR: &str = "['requirement_id', 'evidence_class', 'artifact_ref', \
     'artifact_sha256', 'raw_outcome_digest', 'availability']";
const ARTIFACT_FIELDS_REPR: &str = "['store_profile_ref', 'artifact_id', 'bytes', 'sha256']";

/// Typed validation failure (`EvidenceDigestError`).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct EvidenceDigestError(String);

impl EvidenceDigestError {
    pub(crate) fn new(message: impl Into<String>) -> Self {
        Self(message.into())
    }
}

impl Display for EvidenceDigestError {
    fn fmt(&self, f: &mut Formatter<'_>) -> FmtResult {
        f.write_str(&self.0)
    }
}

impl Error for EvidenceDigestError {}

/// Validated artifact reference.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ArtifactRef {
    /// Storage profile identifier.
    pub store_profile_ref: String,
    /// Artifact identifier.
    pub artifact_id: String,
    /// Artifact size in bytes.
    pub bytes: u64,
    /// Artifact SHA-256 hex digest.
    pub sha256: String,
}

/// Validated evidence record.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct EvidenceRecord {
    /// Requirement identifier.
    pub requirement_id: String,
    /// Closed-enum evidence class.
    pub evidence_class: String,
    /// Artifact reference.
    pub artifact_ref: ArtifactRef,
    /// Must equal `artifact_ref.sha256`.
    pub artifact_sha256: String,
    /// Raw outcome digest.
    pub raw_outcome_digest: String,
    /// Closed-enum availability.
    pub availability: String,
}

/// Digest result record (`result_record`).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DigestResult {
    /// Number of evidence records.
    pub record_count: usize,
    /// Manifest length in bytes.
    pub manifest_bytes: usize,
    /// SHA-256 hex of the manifest.
    pub evidence_digest: String,
}

impl DigestResult {
    /// Compact sorted-key JSON exactly like
    /// `json.dumps(result, sort_keys=True, separators=(",", ":"))`.
    #[must_use]
    pub fn to_compact_json(&self) -> String {
        format!(
            "{{\"evidence_digest\":\"{}\",\"manifest_bytes\":{},\"profile\":\"{PROFILE}\",\"record_count\":{},\"schema_version\":{SCHEMA_VERSION}}}",
            self.evidence_digest, self.manifest_bytes, self.record_count
        )
    }
}

/// `SHA256_RE`: exactly 64 lowercase hex digits.
#[must_use]
pub fn is_sha256_hex(value: &str) -> bool {
    let bytes = value.as_bytes();
    if bytes.len() != 64 {
        return false;
    }
    for &byte in bytes {
        if !byte.is_ascii_hexdigit() || byte.is_ascii_uppercase() {
            return false;
        }
    }
    true
}

/// `OPAQUE_ID_RE`: `^[A-Za-z0-9][A-Za-z0-9._-]{0,127}$`.
#[must_use]
pub fn is_opaque_id(value: &str) -> bool {
    let mut chars = value.chars();
    let Some(first) = chars.next() else {
        return false;
    };
    if !first.is_ascii_alphanumeric() {
        return false;
    }
    let mut len = 1_usize;
    for c in chars {
        if !(c.is_ascii_alphanumeric() || c == '.' || c == '_' || c == '-') {
            return false;
        }
        len += 1;
        if len > 128 {
            return false;
        }
    }
    true
}

/// `CLOSED_ENUM_RE`: `^[A-Z][A-Z0-9_]{0,127}$`.
#[must_use]
pub fn is_closed_enum(value: &str) -> bool {
    let mut chars = value.chars();
    let Some(first) = chars.next() else {
        return false;
    };
    if !first.is_ascii_uppercase() {
        return false;
    }
    let mut len = 1_usize;
    for c in chars {
        if !(c.is_ascii_uppercase() || c.is_ascii_digit() || c == '_') {
            return false;
        }
        len += 1;
        if len > 128 {
            return false;
        }
    }
    true
}

const LOWER_HEX: &[u8; 16] = b"0123456789abcdef";

fn push_hex_byte(out: &mut Vec<u8>, byte: u8) {
    out.push(LOWER_HEX[usize::from(byte >> 4)]);
    out.push(LOWER_HEX[usize::from(byte & 0x0F)]);
}

/// Append a JSON string with `ensure_ascii=False` semantics:
/// escape `"`, `\`, short C0 escapes, other C0 as lowercase `\u00xx`,
/// emit everything else as raw UTF-8.
fn append_json_string(out: &mut Vec<u8>, value: &str) {
    out.push(b'"');
    for c in value.chars() {
        match c {
            '"' => out.extend_from_slice(b"\\\""),
            '\\' => out.extend_from_slice(b"\\\\"),
            '\u{08}' => out.extend_from_slice(b"\\b"),
            '\u{09}' => out.extend_from_slice(b"\\t"),
            '\u{0A}' => out.extend_from_slice(b"\\n"),
            '\u{0C}' => out.extend_from_slice(b"\\f"),
            '\u{0D}' => out.extend_from_slice(b"\\r"),
            c if (c as u32) < 0x20 => {
                out.extend_from_slice(b"\\u00");
                push_hex_byte(out, c as u8);
            }
            c => {
                let mut buf = [0_u8; 4];
                out.extend_from_slice(c.encode_utf8(&mut buf).as_bytes());
            }
        }
    }
    out.push(b'"');
}

/// `canonical_json_bytes`: sorted keys, `(",", ":")` separators, trailing LF.
#[must_use]
pub fn canonical_json_bytes(record: &EvidenceRecord) -> Vec<u8> {
    let artifact = &record.artifact_ref;
    let mut out = Vec::with_capacity(256);
    out.extend_from_slice(b"{\"artifact_ref\":{\"artifact_id\":");
    append_json_string(&mut out, &artifact.artifact_id);
    out.extend_from_slice(b",\"bytes\":");
    out.extend_from_slice(artifact.bytes.to_string().as_bytes());
    out.extend_from_slice(b",\"sha256\":");
    append_json_string(&mut out, &artifact.sha256);
    out.extend_from_slice(b",\"store_profile_ref\":");
    append_json_string(&mut out, &artifact.store_profile_ref);
    out.extend_from_slice(b"},\"artifact_sha256\":");
    append_json_string(&mut out, &record.artifact_sha256);
    out.extend_from_slice(b",\"availability\":");
    append_json_string(&mut out, &record.availability);
    out.extend_from_slice(b",\"evidence_class\":");
    append_json_string(&mut out, &record.evidence_class);
    out.extend_from_slice(b",\"raw_outcome_digest\":");
    append_json_string(&mut out, &record.raw_outcome_digest);
    out.extend_from_slice(b",\"requirement_id\":");
    append_json_string(&mut out, &record.requirement_id);
    out.extend_from_slice(b"}\n");
    out
}

fn require_exact_keys(
    mut keys: Vec<&str>,
    expected: &[&str],
    repr: &str,
    label: &str,
) -> Result<(), EvidenceDigestError> {
    keys.sort_unstable();
    let mut expected_sorted = expected.to_vec();
    expected_sorted.sort_unstable();
    if keys != expected_sorted {
        return Err(EvidenceDigestError::new(format!(
            "{label} field set differs from {repr}"
        )));
    }
    Ok(())
}

fn json_string<'v>(
    value: &'v serde_json::Value,
    field: &str,
    label: &str,
) -> Result<&'v str, EvidenceDigestError> {
    value
        .get(field)
        .and_then(serde_json::Value::as_str)
        .ok_or_else(|| EvidenceDigestError::new(format!("{label}.{field} is invalid")))
}

fn json_u64(
    value: &serde_json::Value,
    field: &str,
    label: &str,
) -> Result<u64, EvidenceDigestError> {
    value
        .get(field)
        .and_then(serde_json::Value::as_u64)
        .ok_or_else(|| EvidenceDigestError::new(format!("{label}.{field} is not u64")))
}

fn toml_string<'v>(
    value: &'v toml::Value,
    field: &str,
    label: &str,
) -> Result<&'v str, EvidenceDigestError> {
    value
        .get(field)
        .and_then(toml::Value::as_str)
        .ok_or_else(|| EvidenceDigestError::new(format!("{label}.{field} is invalid")))
}

fn toml_u64(value: &toml::Value, field: &str, label: &str) -> Result<u64, EvidenceDigestError> {
    value
        .get(field)
        .and_then(toml::Value::as_integer)
        .and_then(|v| u64::try_from(v).ok())
        .ok_or_else(|| EvidenceDigestError::new(format!("{label}.{field} is not u64")))
}

fn validate_artifact_fields(
    store: &str,
    artifact_id: &str,
    digest: &str,
    label: &str,
) -> Result<(), EvidenceDigestError> {
    if !is_opaque_id(store) {
        return Err(EvidenceDigestError::new(format!(
            "{label}.store_profile_ref is invalid"
        )));
    }
    if !is_opaque_id(artifact_id) {
        return Err(EvidenceDigestError::new(format!(
            "{label}.artifact_id is invalid"
        )));
    }
    if !is_sha256_hex(digest) {
        return Err(EvidenceDigestError::new(format!(
            "{label}.sha256 is invalid"
        )));
    }
    Ok(())
}

fn finish_record(
    requirement: &str,
    evidence_class: &str,
    artifact: ArtifactRef,
    artifact_sha: &str,
    raw_digest: &str,
    availability: &str,
    label: &str,
) -> Result<EvidenceRecord, EvidenceDigestError> {
    if !is_opaque_id(requirement) {
        return Err(EvidenceDigestError::new(format!(
            "{label}.requirement_id is invalid"
        )));
    }
    if !is_closed_enum(evidence_class) {
        return Err(EvidenceDigestError::new(format!(
            "{label}.evidence_class is invalid"
        )));
    }
    if !is_sha256_hex(artifact_sha) {
        return Err(EvidenceDigestError::new(format!(
            "{label}.artifact_sha256 is invalid"
        )));
    }
    if artifact_sha != artifact.sha256 {
        return Err(EvidenceDigestError::new(format!(
            "{label}.artifact_sha256 differs from artifact_ref.sha256"
        )));
    }
    if !is_sha256_hex(raw_digest) {
        return Err(EvidenceDigestError::new(format!(
            "{label}.raw_outcome_digest is invalid"
        )));
    }
    if !is_closed_enum(availability) {
        return Err(EvidenceDigestError::new(format!(
            "{label}.availability is invalid"
        )));
    }
    Ok(EvidenceRecord {
        requirement_id: requirement.to_owned(),
        evidence_class: evidence_class.to_owned(),
        artifact_ref: artifact,
        artifact_sha256: artifact_sha.to_owned(),
        raw_outcome_digest: raw_digest.to_owned(),
        availability: availability.to_owned(),
    })
}

/// `normalize_evidence_record` for a JSON value.
fn normalize_json_record(
    value: &serde_json::Value,
    index: usize,
) -> Result<EvidenceRecord, EvidenceDigestError> {
    let label = format!("evidence[{index}]");
    let table = value
        .as_object()
        .ok_or_else(|| EvidenceDigestError::new(format!("{label} must be an object")))?;
    require_exact_keys(
        table.keys().map(String::as_str).collect(),
        &EXPECTED_FIELDS,
        EXPECTED_FIELDS_REPR,
        &label,
    )?;
    let artifact_value = table.get("artifact_ref").ok_or_else(|| {
        EvidenceDigestError::new(format!("{label}.artifact_ref must be an object"))
    })?;
    let artifact_table = artifact_value.as_object().ok_or_else(|| {
        EvidenceDigestError::new(format!("{label}.artifact_ref must be an object"))
    })?;
    let artifact_label = format!("{label}.artifact_ref");
    require_exact_keys(
        artifact_table.keys().map(String::as_str).collect(),
        &ARTIFACT_FIELDS,
        ARTIFACT_FIELDS_REPR,
        &artifact_label,
    )?;
    let store = json_string(artifact_value, "store_profile_ref", &artifact_label)?;
    let artifact_id = json_string(artifact_value, "artifact_id", &artifact_label)?;
    let size = json_u64(artifact_value, "bytes", &artifact_label)?;
    let digest = json_string(artifact_value, "sha256", &artifact_label)?;
    validate_artifact_fields(store, artifact_id, digest, &artifact_label)?;
    let artifact = ArtifactRef {
        store_profile_ref: store.to_owned(),
        artifact_id: artifact_id.to_owned(),
        bytes: size,
        sha256: digest.to_owned(),
    };
    finish_record(
        json_string(value, "requirement_id", &label)?,
        json_string(value, "evidence_class", &label)?,
        artifact,
        json_string(value, "artifact_sha256", &label)?,
        json_string(value, "raw_outcome_digest", &label)?,
        json_string(value, "availability", &label)?,
        &label,
    )
}

/// `normalize_evidence_record` for a TOML value.
fn normalize_toml_record(
    value: &toml::Value,
    index: usize,
) -> Result<EvidenceRecord, EvidenceDigestError> {
    let label = format!("evidence[{index}]");
    let table = value
        .as_table()
        .ok_or_else(|| EvidenceDigestError::new(format!("{label} must be an object")))?;
    require_exact_keys(
        table.keys().map(String::as_str).collect(),
        &EXPECTED_FIELDS,
        EXPECTED_FIELDS_REPR,
        &label,
    )?;
    let artifact_value = table.get("artifact_ref").ok_or_else(|| {
        EvidenceDigestError::new(format!("{label}.artifact_ref must be an object"))
    })?;
    let artifact_table = artifact_value.as_table().ok_or_else(|| {
        EvidenceDigestError::new(format!("{label}.artifact_ref must be an object"))
    })?;
    let artifact_label = format!("{label}.artifact_ref");
    require_exact_keys(
        artifact_table.keys().map(String::as_str).collect(),
        &ARTIFACT_FIELDS,
        ARTIFACT_FIELDS_REPR,
        &artifact_label,
    )?;
    let store = toml_string(artifact_value, "store_profile_ref", &artifact_label)?;
    let artifact_id = toml_string(artifact_value, "artifact_id", &artifact_label)?;
    let size = toml_u64(artifact_value, "bytes", &artifact_label)?;
    let digest = toml_string(artifact_value, "sha256", &artifact_label)?;
    validate_artifact_fields(store, artifact_id, digest, &artifact_label)?;
    let artifact = ArtifactRef {
        store_profile_ref: store.to_owned(),
        artifact_id: artifact_id.to_owned(),
        bytes: size,
        sha256: digest.to_owned(),
    };
    finish_record(
        toml_string(value, "requirement_id", &label)?,
        toml_string(value, "evidence_class", &label)?,
        artifact,
        toml_string(value, "artifact_sha256", &label)?,
        toml_string(value, "raw_outcome_digest", &label)?,
        toml_string(value, "availability", &label)?,
        &label,
    )
}

fn check_unique(records: &[EvidenceRecord]) -> Result<(), EvidenceDigestError> {
    for (i, record) in records.iter().enumerate() {
        if records[..i]
            .iter()
            .any(|prior| prior.requirement_id == record.requirement_id)
        {
            return Err(EvidenceDigestError::new(
                "evidence requirement_id values must be unique",
            ));
        }
    }
    Ok(())
}

fn emit_manifest(records: &[EvidenceRecord]) -> Result<Vec<u8>, EvidenceDigestError> {
    check_unique(records)?;
    let mut output = Vec::from(MAGIC);
    for record in records {
        output.extend_from_slice(&canonical_json_bytes(record));
    }
    if output.len() > MAX_MANIFEST_BYTES {
        return Err(EvidenceDigestError::new(format!(
            "evidence manifest exceeds {MAX_MANIFEST_BYTES} bytes"
        )));
    }
    Ok(output)
}

/// `render_evidence_manifest` for a JSON evidence array.
///
/// # Errors
///
/// Returns `EvidenceDigestError` when the value is not an array, exceeds
/// bounds, fails field validation, or carries duplicate requirement IDs.
pub fn render_evidence_manifest_json(
    value: &serde_json::Value,
) -> Result<Vec<u8>, EvidenceDigestError> {
    let items = value
        .as_array()
        .ok_or_else(|| EvidenceDigestError::new("evidence must be an array"))?;
    if items.len() > MAX_EVIDENCE {
        return Err(EvidenceDigestError::new(format!(
            "evidence count exceeds {MAX_EVIDENCE}"
        )));
    }
    let mut records = Vec::with_capacity(items.len());
    for (index, item) in items.iter().enumerate() {
        records.push(normalize_json_record(item, index)?);
    }
    emit_manifest(&records)
}

/// `render_evidence_manifest` for a TOML evidence array.
///
/// # Errors
///
/// Returns `EvidenceDigestError` when the value is not an array, exceeds
/// bounds, fails field validation, or carries duplicate requirement IDs.
pub fn render_evidence_manifest_toml(value: &toml::Value) -> Result<Vec<u8>, EvidenceDigestError> {
    let items = value
        .as_array()
        .ok_or_else(|| EvidenceDigestError::new("evidence must be an array"))?;
    if items.len() > MAX_EVIDENCE {
        return Err(EvidenceDigestError::new(format!(
            "evidence count exceeds {MAX_EVIDENCE}"
        )));
    }
    let mut records = Vec::with_capacity(items.len());
    for (index, item) in items.iter().enumerate() {
        records.push(normalize_toml_record(item, index)?);
    }
    emit_manifest(&records)
}

fn sha256_hex(bytes: &[u8]) -> String {
    use sha2::{Digest, Sha256};
    let mut hasher = Sha256::new();
    hasher.update(bytes);
    format!("{:x}", hasher.finalize())
}

/// `accepted_evidence_digest` for a JSON evidence array.
///
/// # Errors
///
/// Returns `EvidenceDigestError` when the manifest cannot be rendered.
pub fn accepted_evidence_digest_json(
    value: &serde_json::Value,
) -> Result<String, EvidenceDigestError> {
    Ok(sha256_hex(&render_evidence_manifest_json(value)?))
}

/// `accepted_evidence_digest` for a TOML evidence array.
///
/// # Errors
///
/// Returns `EvidenceDigestError` when the manifest cannot be rendered.
pub fn accepted_evidence_digest_toml(value: &toml::Value) -> Result<String, EvidenceDigestError> {
    Ok(sha256_hex(&render_evidence_manifest_toml(value)?))
}

/// `result_record` for a JSON evidence array.
///
/// # Errors
///
/// Returns `EvidenceDigestError` when the manifest cannot be rendered.
pub fn result_record_json(value: &serde_json::Value) -> Result<DigestResult, EvidenceDigestError> {
    let manifest = render_evidence_manifest_json(value)?;
    let count = value.as_array().map_or(0, Vec::len);
    Ok(DigestResult {
        record_count: count,
        manifest_bytes: manifest.len(),
        evidence_digest: sha256_hex(&manifest),
    })
}

/// `result_record` for a TOML evidence array.
///
/// # Errors
///
/// Returns `EvidenceDigestError` when the manifest cannot be rendered.
pub fn result_record_toml(value: &toml::Value) -> Result<DigestResult, EvidenceDigestError> {
    let manifest = render_evidence_manifest_toml(value)?;
    let count = value.as_array().map_or(0, Vec::len);
    Ok(DigestResult {
        record_count: count,
        manifest_bytes: manifest.len(),
        evidence_digest: sha256_hex(&manifest),
    })
}

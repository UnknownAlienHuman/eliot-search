//! Port of `tools/compute-accepted-evidence-digest.py` (T41, family E).
//!
//! Reads a `package_handoff_v1` TOML record or a JSON evidence array and
//! prints the compact digest result record. Failures use the exact
//! `ACCEPTED_EVIDENCE_DIGEST_INVALID` stderr prefix and exit code `2`.

use std::fs;
use std::path::Path;

use crate::accepted_evidence::{DigestResult, EvidenceDigestError};

/// Compute the compact digest JSON for a record file.
///
/// When `json_array` is set the file is a JSON evidence array, otherwise it
/// is a `package_handoff_v1` TOML record carrying `evidence`.
///
/// # Errors
///
/// Returns `EvidenceDigestError` when the file is unreadable, is not strict
/// UTF-8, fails to parse, is not a `package_handoff_v1` record, or carries
/// invalid evidence.
pub fn compute_from_record_file(
    path: &Path,
    json_array: bool,
) -> Result<String, EvidenceDigestError> {
    let raw = fs::read(path).map_err(|err| EvidenceDigestError::new(err.to_string()))?;
    let text = String::from_utf8(raw).map_err(|err| EvidenceDigestError::new(err.to_string()))?;
    let result: DigestResult = if json_array {
        let value: serde_json::Value =
            serde_json::from_str(&text).map_err(|err| EvidenceDigestError::new(err.to_string()))?;
        crate::accepted_evidence::result_record_json(&value)?
    } else {
        let record: toml::Value = text
            .parse()
            .map_err(|err: toml::de::Error| EvidenceDigestError::new(err.to_string()))?;
        if record.get("record_kind").and_then(toml::Value::as_str) != Some("package_handoff_v1") {
            return Err(EvidenceDigestError::new(
                "record_kind must be package_handoff_v1",
            ));
        }
        let evidence = record
            .get("evidence")
            .ok_or_else(|| EvidenceDigestError::new("evidence must be an array"))?;
        crate::accepted_evidence::result_record_toml(evidence)?
    };
    Ok(result.to_compact_json())
}

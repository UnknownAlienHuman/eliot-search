//! Canonical source-control cutover marker semantics.
//!
//! This module owns the deterministic marker schema, strict codec and replay
//! classification. It performs no filesystem I/O, process work, quarantine
//! mutation or serving-path switch. Those integration effects remain with the
//! data-root owner in `eliot-searchd`.

use std::fmt;

use search_contracts::{DataRootId, InstallationIncarnationId, SourceNamespaceId};
use sha2::{Digest, Sha256};

/// Exact marker name inside the data root's `control/` directory.
pub const CONTROL_CUTOVER_MARKER_FILE: &str = "control-cutover.v1";
/// Exact staged database schema that a cutover marker may bind.
pub const CONTROL_CUTOVER_STAGED_DATABASE_SCHEMA: &str = "source-map-content-v2";
/// Maximum accepted canonical marker size.
pub const MAX_CONTROL_CUTOVER_MARKER_BYTES: usize = 2048;

const MARKER_MAGIC: &str = "ELIOT-SEARCH-CONTROL-CUTOVER-V1";
const MARKER_VERSION_LINE: &str = "format_version=1";
const DATABASE_SUFFIX: &str = ".source-map.v2.redb";

/// Strict canonical-marker decoding failure.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ControlCutoverMarkerError {
    /// Bytes, identities, locator, schema or record digest are not canonical.
    Corrupt,
}

impl ControlCutoverMarkerError {
    /// Stable machine-readable reason code owned by the control adapter.
    #[must_use]
    pub const fn code(self) -> &'static str {
        match self {
            Self::Corrupt => "CONTROL_CUTOVER_MARKER_CORRUPT",
        }
    }
}

impl fmt::Display for ControlCutoverMarkerError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(self.code())
    }
}

impl std::error::Error for ControlCutoverMarkerError {}

/// Exact authority bound by one committed source-control cutover marker.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ControlCutoverMarker {
    /// Imported target namespace.
    pub target: SourceNamespaceId,
    /// Installation incarnation that owns the cutover.
    pub incarnation: InstallationIncarnationId,
    /// Data-root identity that owns the cutover.
    pub root: DataRootId,
    /// Non-zero owner epoch at publication.
    pub epoch: u64,
    /// Complete verified legacy catalog snapshot.
    pub catalog_snapshot: [u8; 32],
    /// Canonical source-mapping plan chain.
    pub plan_chain: [u8; 32],
    /// Canonical source-content manifest chain.
    pub content_chain: [u8; 32],
    /// Single staged database file name inside `control/`, never a path.
    pub database_file_name: String,
    /// Exact staged database schema.
    pub database_schema: String,
}

impl ControlCutoverMarker {
    /// Encodes canonical deterministic bytes.
    ///
    /// No timestamp or random value is added, so a lost acknowledgement can
    /// replay the exact operation and compare byte equality.
    #[must_use]
    pub fn encode(&self) -> Vec<u8> {
        let mut body = String::new();
        push_line(&mut body, MARKER_MAGIC);
        push_line(&mut body, MARKER_VERSION_LINE);
        push_field(&mut body, "target_namespace_id", &self.target.to_string());
        push_field(
            &mut body,
            "installation_incarnation_id",
            &self.incarnation.to_string(),
        );
        push_field(&mut body, "data_root_id", &self.root.to_string());
        push_field(&mut body, "owner_epoch", &self.epoch.to_string());
        push_field(
            &mut body,
            "catalog_snapshot_sha256",
            &hex(&self.catalog_snapshot),
        );
        push_field(&mut body, "plan_chain_sha256", &hex(&self.plan_chain));
        push_field(
            &mut body,
            "content_manifest_chain_sha256",
            &hex(&self.content_chain),
        );
        push_field(
            &mut body,
            "staged_database_locator",
            &["control/", self.database_file_name.as_str()].concat(),
        );
        push_field(&mut body, "staged_database_schema", &self.database_schema);
        let digest = digest(body.as_bytes());
        push_field(&mut body, "record_digest", &hex(&digest));
        body.into_bytes()
    }

    /// Strictly decodes one canonical marker.
    ///
    /// Any deviation in size, shape, order, spelling, identity formatting,
    /// database locator, schema or digest fails closed. A torn write is never
    /// interpreted as partial authority.
    ///
    /// # Errors
    ///
    /// Returns [`ControlCutoverMarkerError::Corrupt`] for every non-canonical
    /// input.
    pub fn decode(bytes: &[u8]) -> Result<Self, ControlCutoverMarkerError> {
        let invalid = || ControlCutoverMarkerError::Corrupt;
        if bytes.is_empty() || bytes.len() > MAX_CONTROL_CUTOVER_MARKER_BYTES {
            return Err(invalid());
        }
        let text = core::str::from_utf8(bytes).map_err(|_| invalid())?;
        if !text.ends_with('\n') {
            return Err(invalid());
        }
        let lines: Vec<&str> = text.lines().collect();
        if lines.len() != 12 || lines[0] != MARKER_MAGIC || lines[1] != MARKER_VERSION_LINE {
            return Err(invalid());
        }
        let field = |index: usize, key: &str| -> Result<&str, ControlCutoverMarkerError> {
            lines[index].strip_prefix(key).ok_or_else(invalid)
        };
        let target = SourceNamespaceId::parse(field(2, "target_namespace_id=")?)
            .map_err(|_| invalid())?;
        if target.as_bytes() == &[0; 16]
            || target.to_string() != field(2, "target_namespace_id=")?
        {
            return Err(invalid());
        }
        let incarnation = InstallationIncarnationId::parse(field(
            3,
            "installation_incarnation_id=",
        )?)
        .map_err(|_| invalid())?;
        if incarnation.to_string() != field(3, "installation_incarnation_id=")? {
            return Err(invalid());
        }
        let root = DataRootId::parse(field(4, "data_root_id=")?).map_err(|_| invalid())?;
        if root.to_string() != field(4, "data_root_id=")? {
            return Err(invalid());
        }
        let epoch_text = field(5, "owner_epoch=")?;
        let epoch: u64 = epoch_text.parse().map_err(|_| invalid())?;
        if epoch == 0 || epoch.to_string() != epoch_text {
            return Err(invalid());
        }
        let catalog_snapshot = canonical_digest(field(6, "catalog_snapshot_sha256=")?)?;
        let plan_chain = canonical_digest(field(7, "plan_chain_sha256=")?)?;
        let content_chain = canonical_digest(field(
            8,
            "content_manifest_chain_sha256=",
        )?)?;
        let locator = field(9, "staged_database_locator=")?;
        let database_file_name = locator.strip_prefix("control/").ok_or_else(invalid)?;
        if database_file_name.contains('/') || database_file_name.contains('\\') {
            return Err(invalid());
        }
        let stem = database_file_name
            .strip_suffix(DATABASE_SUFFIX)
            .ok_or_else(invalid)?;
        canonical_digest(stem)?;
        let database_schema = field(10, "staged_database_schema=")?;
        if database_schema != CONTROL_CUTOVER_STAGED_DATABASE_SCHEMA {
            return Err(invalid());
        }
        let digest_text = field(11, "record_digest=")?;
        let body_len = bytes
            .len()
            .checked_sub(lines[11].len() + 1)
            .ok_or_else(invalid)?;
        if hex(&digest(&bytes[..body_len])) != digest_text
            || canonical_digest(digest_text).is_err()
        {
            return Err(invalid());
        }
        Ok(Self {
            target,
            incarnation,
            root,
            epoch,
            catalog_snapshot,
            plan_chain,
            content_chain,
            database_file_name: database_file_name.to_owned(),
            database_schema: database_schema.to_owned(),
        })
    }

    /// Digest of the exact canonical body before its `record_digest` line.
    #[must_use]
    pub fn record_digest(&self) -> [u8; 32] {
        let bytes = self.encode();
        let body_len = bytes.len() - ("record_digest=".len() + 64 + 1);
        digest(&bytes[..body_len])
    }
}

/// Relationship between a proposed cutover and a committed marker.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ControlCutoverReplayDecision {
    /// Same owner, target, snapshot, chains, locator and schema.
    Identical,
    /// Same installation and root but different migrated history or artifact.
    Diverged,
    /// Different installation incarnation or data-root owner.
    ForeignOwner,
}

/// Classifies a cutover replay without filesystem access or state mutation.
#[must_use]
pub fn classify_control_cutover_replay(
    existing: &ControlCutoverMarker,
    proposed: &ControlCutoverMarker,
) -> ControlCutoverReplayDecision {
    if existing.incarnation != proposed.incarnation || existing.root != proposed.root {
        return ControlCutoverReplayDecision::ForeignOwner;
    }
    if existing.target == proposed.target
        && existing.catalog_snapshot == proposed.catalog_snapshot
        && existing.plan_chain == proposed.plan_chain
        && existing.content_chain == proposed.content_chain
        && existing.database_file_name == proposed.database_file_name
        && existing.database_schema == proposed.database_schema
    {
        ControlCutoverReplayDecision::Identical
    } else {
        ControlCutoverReplayDecision::Diverged
    }
}

fn digest(bytes: &[u8]) -> [u8; 32] {
    let value = Sha256::digest(bytes);
    let mut output = [0_u8; 32];
    output.copy_from_slice(&value);
    output
}

fn hex(bytes: &[u8]) -> String {
    const HEX: &[u8; 16] = b"0123456789abcdef";
    let mut output = String::with_capacity(bytes.len().saturating_mul(2));
    for &byte in bytes {
        output.push(char::from(HEX[usize::from(byte >> 4)]));
        output.push(char::from(HEX[usize::from(byte & 0x0f)]));
    }
    output
}

fn canonical_digest(text: &str) -> Result<[u8; 32], ControlCutoverMarkerError> {
    if text.len() != 64 {
        return Err(ControlCutoverMarkerError::Corrupt);
    }
    let mut output = [0_u8; 32];
    for (index, pair) in text.as_bytes().chunks_exact(2).enumerate() {
        let high = hex_nibble(pair[0]).ok_or(ControlCutoverMarkerError::Corrupt)?;
        let low = hex_nibble(pair[1]).ok_or(ControlCutoverMarkerError::Corrupt)?;
        output[index] = (high << 4) | low;
    }
    if hex(&output) != text {
        return Err(ControlCutoverMarkerError::Corrupt);
    }
    Ok(output)
}

const fn hex_nibble(value: u8) -> Option<u8> {
    match value {
        b'0'..=b'9' => Some(value - b'0'),
        b'a'..=b'f' => Some(value - b'a' + 10),
        _ => None,
    }
}

fn push_line(output: &mut String, line: &str) {
    output.push_str(line);
    output.push('\n');
}

fn push_field(output: &mut String, key: &str, value: &str) {
    output.push_str(key);
    output.push('=');
    output.push_str(value);
    output.push('\n');
}

#[cfg(test)]
mod tests {
    use super::*;

    fn sample_marker() -> ControlCutoverMarker {
        ControlCutoverMarker {
            target: SourceNamespaceId::parse("123e4567-e89b-12d3-a456-426614174000")
                .expect("valid target"),
            incarnation: InstallationIncarnationId::from_bytes([0x11; 16]),
            root: DataRootId::from_bytes([0x22; 16]),
            epoch: 3,
            catalog_snapshot: [0x33; 32],
            plan_chain: [0x44; 32],
            content_chain: [0x55; 32],
            database_file_name: format!("{}.source-map.v2.redb", "66".repeat(32)),
            database_schema: CONTROL_CUTOVER_STAGED_DATABASE_SCHEMA.to_owned(),
        }
    }

    #[test]
    fn marker_encode_is_deterministic_and_decodes_exactly() {
        let marker = sample_marker();
        let first = marker.encode();
        let second = sample_marker().encode();
        assert_eq!(first, second, "no timestamps or randomness in marker");
        let decoded = ControlCutoverMarker::decode(&first).expect("canonical marker");
        assert_eq!(decoded, marker);
        assert_eq!(decoded.record_digest(), marker.record_digest());
    }

    #[test]
    fn marker_decode_rejects_non_canonical_bytes() {
        let canonical = sample_marker().encode();
        let text = String::from_utf8(canonical.clone()).expect("UTF-8 marker");
        assert_eq!(text.lines().count(), 12, "exact marker shape");
        let mut cases: Vec<Vec<u8>> = Vec::new();
        cases.push(canonical[..canonical.len() - 1].to_vec());
        cases.push(text.replacen("CUTOVER", "CUT", 1).into_bytes());
        cases.push(format!("{text}owner_epoch=3\n").into_bytes());
        cases.push(
            text.replacen("owner_epoch=", "owner_epoch_x=", 1)
                .into_bytes(),
        );
        let digest_line = text
            .lines()
            .find(|line| line.starts_with("record_digest="))
            .expect("digest line");
        cases.push(
            text.replace(digest_line, &digest_line.to_uppercase())
                .into_bytes(),
        );
        cases.push(
            text.replacen("owner_epoch=3\n", "owner_epoch=03\n", 1)
                .into_bytes(),
        );
        cases.push(
            text.replacen("owner_epoch=3\n", "owner_epoch=0\n", 1)
                .into_bytes(),
        );
        cases.push(
            text.replacen("source-map-content-v2", "source-map-content-v9", 1)
                .into_bytes(),
        );
        let mut tampered = canonical;
        let position = tampered
            .iter()
            .position(|byte| *byte == b'=')
            .expect("field delimiter")
            + 1;
        tampered[position] = if tampered[position] == b'0' { b'1' } else { b'0' };
        cases.push(tampered);
        cases.push(text.replacen("control/", "control/../", 1).into_bytes());
        for (index, case) in cases.iter().enumerate() {
            assert!(
                ControlCutoverMarker::decode(case).is_err(),
                "case {index} must be rejected"
            );
        }
    }

    #[test]
    fn replay_decisions_separate_identity_history_and_owner() {
        let committed = sample_marker();
        assert_eq!(
            classify_control_cutover_replay(&committed, &sample_marker()),
            ControlCutoverReplayDecision::Identical
        );
        let mut diverged = sample_marker();
        diverged.catalog_snapshot = [0x77; 32];
        assert_eq!(
            classify_control_cutover_replay(&committed, &diverged),
            ControlCutoverReplayDecision::Diverged
        );
        let mut foreign = sample_marker();
        foreign.root = DataRootId::from_bytes([0x99; 16]);
        assert_eq!(
            classify_control_cutover_replay(&committed, &foreign),
            ControlCutoverReplayDecision::ForeignOwner
        );
        let mut successor_epoch = sample_marker();
        successor_epoch.epoch = 4;
        assert_eq!(
            classify_control_cutover_replay(&committed, &successor_epoch),
            ControlCutoverReplayDecision::Identical
        );
    }
}

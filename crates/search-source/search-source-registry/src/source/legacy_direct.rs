//! Legacy DIRECT source-event model owned by the source registry.
//!
//! This internal compatibility model owns the deterministic event schema,
//! digest-chain rules, idempotent append planning and replay collision checks.
//! It performs no filesystem I/O and stores no source bytes.

use std::collections::BTreeMap;
use std::fmt;

/// Exact first line of the legacy DIRECT event journal.
pub const LEGACY_DIRECT_LOG_HEADER: &str = "ELIOT_SEARCH_SOURCE_EVENTS_V1";
/// Zero predecessor used by the first journal event.
pub const LEGACY_DIRECT_ZERO_DIGEST: &str =
    "0000000000000000000000000000000000000000000000000000000000000000";

const RECORD_VERSION: &str = "V1";
const REVISION_ID_DOMAIN: &[u8] = b"eliot-search/direct-revision-id/v1";

/// Digest adapter supplied by the integration owner.
///
/// The registry owns framing and state semantics; the daemon supplies the
/// already-qualified SHA-256 primitive without introducing a second crypto
/// dependency or a second multipart profile.
pub trait LegacyDirectDigest {
    /// Ordinary digest of one byte string.
    fn digest(bytes: &[u8]) -> [u8; 32];
    /// Domain-separated length-prefixed digest of ordered byte strings.
    fn digest_parts(domain: &[u8], parts: &[&[u8]]) -> [u8; 32];
}

/// Closed legacy DIRECT journal failure.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum LegacyDirectJournalError {
    /// Event field count or record version is invalid.
    EventInvalid,
    /// A SHA-256 text field is malformed.
    DigestInvalid,
    /// Lifecycle tag is invalid.
    StateInvalid,
    /// Byte length is invalid or outside the admitted ceiling.
    LengthInvalid,
    /// Identity-strength tag is invalid.
    IdentityInvalid,
    /// Record digest does not match the exact encoded prefix.
    RecordDigestInvalid,
    /// Decimal sequence text cannot be represented.
    SequenceInvalid,
    /// Sequence or predecessor does not extend the current chain.
    ChainInvalid,
    /// Sequence space is exhausted.
    SequenceExhausted,
    /// Operation identity appears twice in replay.
    OperationDuplicate,
    /// Revision identity does not bind source, content and length.
    RevisionIdMismatch,
    /// Revision content digest is malformed.
    RevisionContentMismatch,
    /// Stable source identity changes its immutable file identity.
    SourceCollision,
    /// Stable revision identity changes its immutable content metadata.
    RevisionCollision,
    /// Planned append would exceed the event ceiling.
    EventLimitExceeded,
    /// Idempotent operation points at no current source record.
    OperationReadbackMissing,
    /// Idempotent operation payload differs from committed state.
    OperationConflict,
    /// One encoded event exceeds the line ceiling.
    EventTooLarge,
}

impl LegacyDirectJournalError {
    /// Stable daemon-compatible reason code.
    #[must_use]
    pub const fn code(self) -> &'static str {
        match self {
            Self::EventInvalid => "DIRECT_CONTROL_LOG_EVENT_INVALID",
            Self::DigestInvalid => "DIRECT_CONTROL_LOG_DIGEST_INVALID",
            Self::StateInvalid => "DIRECT_CONTROL_LOG_STATE_INVALID",
            Self::LengthInvalid => "DIRECT_CONTROL_LOG_LENGTH_INVALID",
            Self::IdentityInvalid => "DIRECT_CONTROL_LOG_IDENTITY_INVALID",
            Self::RecordDigestInvalid => "DIRECT_CONTROL_LOG_RECORD_DIGEST_INVALID",
            Self::SequenceInvalid => "DIRECT_CONTROL_LOG_SEQUENCE_INVALID",
            Self::ChainInvalid => "DIRECT_CONTROL_LOG_CHAIN_INVALID",
            Self::SequenceExhausted => "DIRECT_SOURCE_SEQUENCE_EXHAUSTED",
            Self::OperationDuplicate => "DIRECT_CONTROL_LOG_OPERATION_DUPLICATE",
            Self::RevisionIdMismatch => "DIRECT_REVISION_ID_MISMATCH",
            Self::RevisionContentMismatch => "DIRECT_REVISION_CONTENT_MISMATCH",
            Self::SourceCollision => "DIRECT_CONTROL_LOG_SOURCE_COLLISION",
            Self::RevisionCollision => "DIRECT_CONTROL_LOG_REVISION_COLLISION",
            Self::EventLimitExceeded => "DIRECT_SOURCE_EVENT_LIMIT_EXCEEDED",
            Self::OperationReadbackMissing => "DIRECT_OPERATION_READBACK_MISSING",
            Self::OperationConflict => "DIRECT_OPERATION_CONFLICT",
            Self::EventTooLarge => "DIRECT_SOURCE_EVENT_TOO_LARGE",
        }
    }
}

impl fmt::Display for LegacyDirectJournalError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(self.code())
    }
}

impl std::error::Error for LegacyDirectJournalError {}

/// Legacy source lifecycle encoded in the append-only journal.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum LegacyDirectSourceState {
    /// Source participates in the current corpus.
    Active,
    /// Source is retained as evidence but excluded from new search.
    Retired,
}

impl LegacyDirectSourceState {
    /// Canonical one-byte journal tag.
    #[must_use]
    pub const fn tag(self) -> &'static str {
        match self {
            Self::Active => "A",
            Self::Retired => "R",
        }
    }

    /// Parses the exact canonical journal tag.
    #[must_use]
    pub fn parse(value: &str) -> Option<Self> {
        match value {
            "A" => Some(Self::Active),
            "R" => Some(Self::Retired),
            _ => None,
        }
    }
}

/// Strength of the identity material used to derive a legacy source.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum LegacyDirectIdentityStrength {
    /// Native stable file identity was available.
    Native,
    /// Canonical path identity was the strongest admitted fallback.
    PathBound,
}

impl LegacyDirectIdentityStrength {
    /// Canonical journal tag.
    #[must_use]
    pub const fn tag(self) -> &'static str {
        match self {
            Self::Native => "native",
            Self::PathBound => "path-bound",
        }
    }

    /// Parses the exact canonical journal tag.
    #[must_use]
    pub fn parse(value: &str) -> Option<Self> {
        match value {
            "native" => Some(Self::Native),
            "path-bound" => Some(Self::PathBound),
            _ => None,
        }
    }
}

/// One decoded legacy DIRECT source event.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct LegacyDirectSourceRecord {
    /// Monotone one-based journal sequence.
    pub sequence: u64,
    /// Digest of the preceding event, or the zero digest for sequence one.
    pub previous_digest: String,
    /// Stable idempotency identity.
    pub operation_id: String,
    /// Resulting lifecycle.
    pub state: LegacyDirectSourceState,
    /// Stable source identity.
    pub source_id: String,
    /// Immutable revision identity.
    pub revision_id: String,
    /// Digest of exact retained bytes.
    pub content_digest: String,
    /// Exact retained byte length.
    pub byte_length: u64,
    /// Digest of native/path-bound file identity material.
    pub file_identity_digest: String,
    /// Digest of the canonical path identity.
    pub path_digest: String,
    /// Strength of the identity material.
    pub identity_strength: LegacyDirectIdentityStrength,
    /// Digest of all preceding fields in this record.
    pub record_digest: String,
}

impl LegacyDirectSourceRecord {
    /// Constructs one canonical record from an admitted draft.
    #[must_use]
    pub fn from_draft<D: LegacyDirectDigest>(
        sequence: u64,
        previous_digest: String,
        draft: LegacyDirectRecordDraft,
    ) -> Self {
        let mut record = Self {
            sequence,
            previous_digest,
            operation_id: draft.operation_id,
            state: draft.state,
            source_id: draft.source_id,
            revision_id: draft.revision_id,
            content_digest: draft.content_digest,
            byte_length: draft.byte_length,
            file_identity_digest: draft.file_identity_digest,
            path_digest: draft.path_digest,
            identity_strength: draft.identity_strength,
            record_digest: String::new(),
        };
        record.record_digest = hex(&D::digest(record.canonical_without_digest().as_bytes()));
        record
    }

    /// Canonical record prefix covered by `record_digest`.
    #[must_use]
    pub fn canonical_without_digest(&self) -> String {
        [
            RECORD_VERSION.to_owned(),
            self.sequence.to_string(),
            self.previous_digest.clone(),
            self.operation_id.clone(),
            self.state.tag().to_owned(),
            self.source_id.clone(),
            self.revision_id.clone(),
            self.content_digest.clone(),
            self.byte_length.to_string(),
            self.file_identity_digest.clone(),
            self.path_digest.clone(),
            self.identity_strength.tag().to_owned(),
        ]
        .join("\t")
    }

    /// Canonical newline-terminated journal line.
    #[must_use]
    pub fn line(&self) -> String {
        format!(
            "{}\t{}\n",
            self.canonical_without_digest(),
            self.record_digest
        )
    }
}

/// Input fields for one planned legacy journal event.
#[derive(Clone, Debug)]
pub struct LegacyDirectRecordDraft {
    /// Stable idempotency identity.
    pub operation_id: String,
    /// Resulting lifecycle.
    pub state: LegacyDirectSourceState,
    /// Stable source identity.
    pub source_id: String,
    /// Immutable revision identity.
    pub revision_id: String,
    /// Digest of exact retained bytes.
    pub content_digest: String,
    /// Exact retained byte length.
    pub byte_length: u64,
    /// Digest of native/path-bound file identity material.
    pub file_identity_digest: String,
    /// Digest of canonical path identity.
    pub path_digest: String,
    /// Strength of the identity material.
    pub identity_strength: LegacyDirectIdentityStrength,
}

/// Pure projected append result. The integration owner writes `encoded` once
/// and then verifies every returned record by exact readback.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct LegacyDirectAppendPlan {
    /// New and idempotently replayed records in request order.
    pub records: Vec<LegacyDirectSourceRecord>,
    /// Canonical bytes for new records only.
    pub encoded: String,
}

/// In-memory replay of the legacy DIRECT journal.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct LegacyDirectRegistryState {
    /// Last committed sequence.
    pub last_sequence: u64,
    /// Last committed record digest.
    pub last_digest: String,
    /// Current source record by stable source identity.
    pub latest: BTreeMap<String, LegacyDirectSourceRecord>,
    /// Committed operation identity to record digest.
    pub operations: BTreeMap<String, String>,
    /// First immutable record by revision identity.
    pub revisions: BTreeMap<String, LegacyDirectSourceRecord>,
    /// Number of committed events.
    pub event_count: usize,
}

impl Default for LegacyDirectRegistryState {
    fn default() -> Self {
        Self {
            last_sequence: 0,
            last_digest: LEGACY_DIRECT_ZERO_DIGEST.to_owned(),
            latest: BTreeMap::new(),
            operations: BTreeMap::new(),
            revisions: BTreeMap::new(),
            event_count: 0,
        }
    }
}

impl LegacyDirectRegistryState {
    /// Parses one line against the exact current chain head.
    pub fn parse_record<D: LegacyDirectDigest>(
        &self,
        trimmed: &str,
        max_source_bytes: u64,
    ) -> Result<LegacyDirectSourceRecord, LegacyDirectJournalError> {
        let fields: Vec<&str> = trimmed.splitn(14, '\t').collect();
        if fields.len() != 13 || fields[0] != RECORD_VERSION {
            return Err(LegacyDirectJournalError::EventInvalid);
        }
        let sequence = fields[1]
            .parse::<u64>()
            .map_err(|_| LegacyDirectJournalError::SequenceInvalid)?;
        let expected_sequence = self
            .last_sequence
            .checked_add(1)
            .ok_or(LegacyDirectJournalError::SequenceExhausted)?;
        if sequence != expected_sequence || fields[2] != self.last_digest {
            return Err(LegacyDirectJournalError::ChainInvalid);
        }
        for index in [2_usize, 3, 5, 6, 7, 9, 10, 12] {
            if decode_digest(fields[index]).is_none() {
                return Err(LegacyDirectJournalError::DigestInvalid);
            }
        }
        let state = LegacyDirectSourceState::parse(fields[4])
            .ok_or(LegacyDirectJournalError::StateInvalid)?;
        let byte_length = fields[8]
            .parse::<u64>()
            .map_err(|_| LegacyDirectJournalError::LengthInvalid)?;
        if byte_length > max_source_bytes {
            return Err(LegacyDirectJournalError::LengthInvalid);
        }
        let identity_strength = LegacyDirectIdentityStrength::parse(fields[11])
            .ok_or(LegacyDirectJournalError::IdentityInvalid)?;
        let canonical = fields[..12].join("\t");
        if hex(&D::digest(canonical.as_bytes())) != fields[12] {
            return Err(LegacyDirectJournalError::RecordDigestInvalid);
        }
        Ok(LegacyDirectSourceRecord {
            sequence,
            previous_digest: fields[2].to_owned(),
            operation_id: fields[3].to_owned(),
            state,
            source_id: fields[5].to_owned(),
            revision_id: fields[6].to_owned(),
            content_digest: fields[7].to_owned(),
            byte_length,
            file_identity_digest: fields[9].to_owned(),
            path_digest: fields[10].to_owned(),
            identity_strength,
            record_digest: fields[12].to_owned(),
        })
    }

    /// Validates one parsed record against idempotency and immutable identity
    /// state without mutating the replay.
    pub fn validate_record<D: LegacyDirectDigest>(
        &self,
        record: &LegacyDirectSourceRecord,
    ) -> Result<(), LegacyDirectJournalError> {
        let expected_sequence = self
            .last_sequence
            .checked_add(1)
            .ok_or(LegacyDirectJournalError::SequenceExhausted)?;
        if record.sequence != expected_sequence || record.previous_digest != self.last_digest {
            return Err(LegacyDirectJournalError::ChainInvalid);
        }
        for value in [
            record.previous_digest.as_str(),
            record.operation_id.as_str(),
            record.source_id.as_str(),
            record.revision_id.as_str(),
            record.content_digest.as_str(),
            record.file_identity_digest.as_str(),
            record.path_digest.as_str(),
            record.record_digest.as_str(),
        ] {
            if decode_digest(value).is_none() {
                return Err(LegacyDirectJournalError::DigestInvalid);
            }
        }
        if hex(&D::digest(record.canonical_without_digest().as_bytes()))
            != record.record_digest
        {
            return Err(LegacyDirectJournalError::RecordDigestInvalid);
        }
        if self.operations.contains_key(&record.operation_id) {
            return Err(LegacyDirectJournalError::OperationDuplicate);
        }
        verify_revision_identity::<D>(
            &record.source_id,
            &record.revision_id,
            &record.content_digest,
            record.byte_length,
        )?;
        if let Some(previous) = self.latest.get(&record.source_id)
            && previous.file_identity_digest != record.file_identity_digest
        {
            return Err(LegacyDirectJournalError::SourceCollision);
        }
        if let Some(previous) = self.revisions.get(&record.revision_id)
            && (previous.content_digest != record.content_digest
                || previous.byte_length != record.byte_length
                || previous.source_id != record.source_id)
        {
            return Err(LegacyDirectJournalError::RevisionCollision);
        }
        Ok(())
    }

    /// Commits one already-validated record to the projected replay.
    pub fn commit_record(&mut self, record: LegacyDirectSourceRecord) {
        self.last_sequence = record.sequence;
        self.last_digest.clone_from(&record.record_digest);
        self.operations
            .insert(record.operation_id.clone(), record.record_digest.clone());
        self.revisions
            .entry(record.revision_id.clone())
            .or_insert_with(|| record.clone());
        self.latest.insert(record.source_id.clone(), record);
        self.event_count = self.event_count.saturating_add(1);
    }

    /// Plans an idempotent append without touching the filesystem.
    ///
    /// New records are validated against a cloned projected state before any
    /// byte is returned, so collisions cannot be appended and discovered only
    /// after durable corruption.
    pub fn plan_append<D: LegacyDirectDigest>(
        &self,
        drafts: Vec<LegacyDirectRecordDraft>,
        max_events: usize,
        max_line_bytes: usize,
    ) -> Result<LegacyDirectAppendPlan, LegacyDirectJournalError> {
        if drafts.is_empty() {
            return Ok(LegacyDirectAppendPlan {
                records: Vec::new(),
                encoded: String::new(),
            });
        }
        if self.event_count.saturating_add(drafts.len()) > max_events {
            return Err(LegacyDirectJournalError::EventLimitExceeded);
        }
        let mut projected = self.clone();
        let mut records = Vec::with_capacity(drafts.len());
        let mut encoded = String::new();
        for draft in drafts {
            if let Some(existing_digest) = projected.operations.get(&draft.operation_id) {
                let existing = projected
                    .latest
                    .get(&draft.source_id)
                    .ok_or(LegacyDirectJournalError::OperationReadbackMissing)?;
                if existing_digest == &existing.record_digest
                    && existing.state == draft.state
                    && existing.revision_id == draft.revision_id
                    && existing.content_digest == draft.content_digest
                    && existing.byte_length == draft.byte_length
                    && existing.file_identity_digest == draft.file_identity_digest
                    && existing.path_digest == draft.path_digest
                    && existing.identity_strength == draft.identity_strength
                {
                    records.push(existing.clone());
                    continue;
                }
                return Err(LegacyDirectJournalError::OperationConflict);
            }
            let sequence = projected
                .last_sequence
                .checked_add(1)
                .ok_or(LegacyDirectJournalError::SequenceExhausted)?;
            let previous_digest = if projected.last_digest.is_empty() {
                LEGACY_DIRECT_ZERO_DIGEST.to_owned()
            } else {
                projected.last_digest.clone()
            };
            let record =
                LegacyDirectSourceRecord::from_draft::<D>(sequence, previous_digest, draft);
            let line = record.line();
            if line.len() > max_line_bytes {
                return Err(LegacyDirectJournalError::EventTooLarge);
            }
            projected.validate_record::<D>(&record)?;
            projected.commit_record(record.clone());
            encoded.push_str(&line);
            records.push(record);
        }
        Ok(LegacyDirectAppendPlan { records, encoded })
    }
}

/// Verifies the immutable revision identity formula.
pub fn verify_legacy_direct_revision_identity<D: LegacyDirectDigest>(
    source_id: &str,
    revision_id: &str,
    content_digest: &str,
    byte_length: u64,
) -> Result<(), LegacyDirectJournalError> {
    let content =
        decode_digest(content_digest).ok_or(LegacyDirectJournalError::RevisionContentMismatch)?;
    let expected = hex(&D::digest_parts(
        REVISION_ID_DOMAIN,
        &[
            source_id.as_bytes(),
            &content,
            &byte_length.to_be_bytes(),
        ],
    ));
    if expected == revision_id {
        Ok(())
    } else {
        Err(LegacyDirectJournalError::RevisionIdMismatch)
    }
}

fn hex(bytes: &[u8]) -> String {
    const TABLE: &[u8; 16] = b"0123456789abcdef";
    let mut output = String::with_capacity(bytes.len().saturating_mul(2));
    for &byte in bytes {
        output.push(char::from(TABLE[usize::from(byte >> 4)]));
        output.push(char::from(TABLE[usize::from(byte & 0x0f)]));
    }
    output
}

fn decode_digest(value: &str) -> Option<[u8; 32]> {
    if value.len() != 64 {
        return None;
    }
    let mut output = [0_u8; 32];
    for (index, pair) in value.as_bytes().chunks_exact(2).enumerate() {
        let high = hex_nibble(pair[0])?;
        let low = hex_nibble(pair[1])?;
        output[index] = (high << 4) | low;
    }
    Some(output)
}

const fn hex_nibble(value: u8) -> Option<u8> {
    match value {
        b'0'..=b'9' => Some(value - b'0'),
        b'a'..=b'f' => Some(value - b'a' + 10),
        b'A'..=b'F' => Some(value - b'A' + 10),
        _ => None,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    struct ToyDigest;

    impl LegacyDirectDigest for ToyDigest {
        fn digest(bytes: &[u8]) -> [u8; 32] {
            let mut output = [0_u8; 32];
            for (index, byte) in bytes.iter().enumerate() {
                let slot = index % output.len();
                output[slot] = output[slot]
                    .wrapping_add(*byte)
                    .rotate_left(u32::try_from(index % 8).expect("rotation is below eight"));
            }
            output
        }

        fn digest_parts(domain: &[u8], parts: &[&[u8]]) -> [u8; 32] {
            let mut framed = Vec::new();
            framed.extend_from_slice(
                &u64::try_from(domain.len())
                    .expect("test domain length")
                    .to_be_bytes(),
            );
            framed.extend_from_slice(domain);
            framed.extend_from_slice(
                &u64::try_from(parts.len())
                    .expect("test part count")
                    .to_be_bytes(),
            );
            for part in parts {
                framed.extend_from_slice(
                    &u64::try_from(part.len())
                        .expect("test part length")
                        .to_be_bytes(),
                );
                framed.extend_from_slice(part);
            }
            Self::digest(&framed)
        }
    }

    fn draft() -> LegacyDirectRecordDraft {
        let source_id = "11".repeat(32);
        let content_digest = "22".repeat(32);
        let byte_length = 3_u64;
        let revision_id = hex(&ToyDigest::digest_parts(
            REVISION_ID_DOMAIN,
            &[
                source_id.as_bytes(),
                &decode_digest(&content_digest).expect("test digest"),
                &byte_length.to_be_bytes(),
            ],
        ));
        LegacyDirectRecordDraft {
            operation_id: "55".repeat(32),
            state: LegacyDirectSourceState::Active,
            source_id,
            revision_id,
            content_digest,
            byte_length,
            file_identity_digest: "33".repeat(32),
            path_digest: "44".repeat(32),
            identity_strength: LegacyDirectIdentityStrength::Native,
        }
    }

    #[test]
    fn append_parse_and_replay_round_trip() {
        let state = LegacyDirectRegistryState::default();
        let plan = state
            .plan_append::<ToyDigest>(vec![draft()], 10, 4096)
            .expect("append plan");
        assert_eq!(plan.records.len(), 1);
        let line = plan.encoded.trim_end_matches('\n');
        let record = state
            .parse_record::<ToyDigest>(line, 1024)
            .expect("parse record");
        state
            .validate_record::<ToyDigest>(&record)
            .expect("validate record");
        let mut replay = state;
        replay.commit_record(record.clone());
        assert_eq!(replay.latest.get(&record.source_id), Some(&record));
        assert_eq!(replay.last_sequence, 1);
        assert_eq!(replay.event_count, 1);
    }

    #[test]
    fn append_replay_is_idempotent_and_conflicts_are_closed() {
        let state = LegacyDirectRegistryState::default();
        let first = state
            .plan_append::<ToyDigest>(vec![draft()], 10, 4096)
            .expect("first plan");
        let mut committed = state;
        committed.commit_record(first.records[0].clone());

        let replay = committed
            .plan_append::<ToyDigest>(vec![draft()], 10, 4096)
            .expect("idempotent replay");
        assert!(replay.encoded.is_empty());
        assert_eq!(replay.records, first.records);

        let mut conflict = draft();
        conflict.path_digest = "99".repeat(32);
        assert_eq!(
            committed.plan_append::<ToyDigest>(vec![conflict], 10, 4096),
            Err(LegacyDirectJournalError::OperationConflict)
        );
    }

    #[test]
    fn source_collision_fails_before_append() {
        let state = LegacyDirectRegistryState::default();
        let first = state
            .plan_append::<ToyDigest>(vec![draft()], 10, 4096)
            .expect("first plan");
        let mut committed = state;
        committed.commit_record(first.records[0].clone());

        let mut source_collision = draft();
        source_collision.operation_id = "66".repeat(32);
        source_collision.file_identity_digest = "77".repeat(32);
        assert_eq!(
            committed.plan_append::<ToyDigest>(vec![source_collision], 10, 4096),
            Err(LegacyDirectJournalError::SourceCollision)
        );
    }

    #[test]
    fn parser_rejects_tampering_and_chain_drift() {
        let state = LegacyDirectRegistryState::default();
        let plan = state
            .plan_append::<ToyDigest>(vec![draft()], 10, 4096)
            .expect("append plan");
        let line = plan.encoded.trim_end_matches('\n');
        let tampered = line.replacen("\tA\t", "\tR\t", 1);
        assert_eq!(
            state.parse_record::<ToyDigest>(&tampered, 1024),
            Err(LegacyDirectJournalError::RecordDigestInvalid)
        );
        let wrong_previous = line.replacen(LEGACY_DIRECT_ZERO_DIGEST, &"ff".repeat(32), 1);
        assert_eq!(
            state.parse_record::<ToyDigest>(&wrong_previous, 1024),
            Err(LegacyDirectJournalError::ChainInvalid)
        );
    }
}

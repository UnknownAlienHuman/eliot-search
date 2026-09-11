//! DIRECT store data model and source-registry compatibility aliases.

use std::path::PathBuf;

use search_source_registry::LegacyDirectDigest;

pub(super) use search_source_registry::{
    LEGACY_DIRECT_LOG_HEADER as SOURCE_LOG_HEADER,
    LEGACY_DIRECT_ZERO_DIGEST as ZERO_DIGEST,
    LegacyDirectIdentityStrength as IdentityStrength,
    LegacyDirectRecordDraft as RecordDraft,
    LegacyDirectRegistryState as RegistryState,
    LegacyDirectSourceRecord as SourceRecord,
    LegacyDirectSourceState as SourceState,
};

use crate::sha256;

pub(super) const CONTROL_DIRECTORY: &str = "control";
pub(super) const REVISION_DIRECTORY: &str = "revisions";
pub(super) const NAMESPACE_FILE: &str = "namespace.id";
pub(super) const SOURCE_LOG_FILE: &str = "source-events.log";
pub(super) const MAX_LOG_BYTES: u64 = 512 * 1024 * 1024;
pub(super) const MAX_LOG_LINE_BYTES: usize = 256 * 1024;
pub(super) const MAX_SOURCE_EVENTS: usize = 2_000_000;
pub(super) const MAX_DIRECTORY_FILES: usize = 100_000;
pub(super) const MAX_DIRECTORY_DEPTH: usize = 128;

pub(super) struct DirectDigest;

impl LegacyDirectDigest for DirectDigest {
    fn digest(bytes: &[u8]) -> [u8; 32] {
        sha256::digest(bytes)
    }

    fn digest_parts(domain: &[u8], parts: &[&[u8]]) -> [u8; 32] {
        sha256::digest_parts(domain, parts)
    }
}

/// Result of indexing one exact final-handle file snapshot.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct IndexedSource {
    pub(crate) source_id: String,
    pub(crate) revision_id: String,
    pub(crate) content_digest: String,
    pub(crate) path_digest: String,
    pub(crate) byte_length: u64,
    pub(crate) identity_strength: &'static str,
    pub(crate) changed: bool,
}

/// Exact source summary without persisted path text.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct SourceSummary {
    pub(crate) source_id: String,
    pub(crate) revision_id: String,
    pub(crate) content_digest: String,
    pub(crate) path_digest: String,
    pub(crate) byte_length: u64,
    pub(crate) identity_strength: &'static str,
    pub(crate) active: bool,
    pub(crate) sequence: u64,
}

/// One source-backed exact match over an immutable verified revision.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct StoredMatch {
    pub(crate) source_id: String,
    pub(crate) revision_id: String,
    pub(crate) content_digest: String,
    pub(crate) path_digest: String,
    pub(crate) evidence_id: String,
    pub(crate) byte_start: usize,
    pub(crate) byte_end: usize,
    pub(crate) line: usize,
    pub(crate) column_bytes: usize,
}

/// Explicit source-level gap during corpus search or verification.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct StoreGap {
    pub(crate) source_id: String,
    pub(crate) revision_id: String,
    pub(crate) reason: &'static str,
}

/// Truthful corpus-search result.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct StoreSearchResult {
    pub(crate) matches: Vec<StoredMatch>,
    pub(crate) gaps: Vec<StoreGap>,
    pub(crate) registered_sources: usize,
    pub(crate) active_sources: usize,
    pub(crate) searched_sources: usize,
    pub(crate) complete: bool,
    pub(crate) match_limit_reached: bool,
}

/// Exact readback-verification result over every referenced immutable revision.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct StoreVerification {
    pub(crate) source_events: usize,
    pub(crate) registered_sources: usize,
    pub(crate) active_sources: usize,
    pub(crate) referenced_revisions: usize,
    pub(crate) verified_revisions: usize,
    pub(crate) total_revision_bytes: u64,
}

/// Exact bounded revision slice.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct RevisionSlice {
    pub(crate) revision_id: String,
    pub(crate) content_digest: String,
    pub(crate) byte_start: u64,
    pub(crate) byte_end: u64,
    pub(crate) bytes: Vec<u8>,
}

#[derive(Clone, Debug)]
pub(super) struct FileSnapshot {
    pub(super) path_digest: String,
    pub(super) file_identity_digest: String,
    pub(super) identity_strength: IdentityStrength,
    pub(super) content_digest: String,
    pub(super) bytes: Vec<u8>,
}

/// Development retained-revision corpus under one already locked data root.
#[derive(Clone, Debug)]
pub struct DirectStore {
    pub(super) root: PathBuf,
    pub(super) namespace_id: [u8; 32],
    pub(super) registry: RegistryState,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn owner_codec_preserves_the_legacy_sha256_wire_vector() {
        let draft = RecordDraft {
            operation_id: "55".repeat(32),
            state: SourceState::Active,
            source_id: "11".repeat(32),
            revision_id:
                "466aa7a6a3f6ba26be1e7eb2b890e8049015b95c419d2877ebad6494d979b1ae"
                    .to_owned(),
            content_digest: "22".repeat(32),
            byte_length: 3,
            file_identity_digest: "33".repeat(32),
            path_digest: "44".repeat(32),
            identity_strength: IdentityStrength::Native,
        };
        let record = SourceRecord::from_draft::<DirectDigest>(
            1,
            ZERO_DIGEST.to_owned(),
            draft,
        );
        assert_eq!(
            record.record_digest,
            "80def9b4b29a19423784d4216e229fc7579528696fb200f8f9df0c6cd3265d41"
        );
        assert_eq!(
            record.line(),
            concat!(
                "V1\t1\t",
                "0000000000000000000000000000000000000000000000000000000000000000\t",
                "5555555555555555555555555555555555555555555555555555555555555555\t",
                "A\t",
                "1111111111111111111111111111111111111111111111111111111111111111\t",
                "466aa7a6a3f6ba26be1e7eb2b890e8049015b95c419d2877ebad6494d979b1ae\t",
                "2222222222222222222222222222222222222222222222222222222222222222\t",
                "3\t",
                "3333333333333333333333333333333333333333333333333333333333333333\t",
                "4444444444444444444444444444444444444444444444444444444444444444\t",
                "native\t",
                "80def9b4b29a19423784d4216e229fc7579528696fb200f8f9df0c6cd3265d41\n",
            )
        );
    }
}

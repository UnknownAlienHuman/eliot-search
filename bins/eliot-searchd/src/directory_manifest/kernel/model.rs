//! Directory inventory models and content-free summaries.

use std::collections::BTreeMap;

/// One exact source binding in a complete directory inventory.
#[derive(Clone, Debug, Eq, Ord, PartialEq, PartialOrd)]
pub struct DirectoryEntry {
    pub(crate) source_id: String,
    pub(crate) path_digest: String,
    pub(crate) revision_id: String,
}

/// One immutable verified directory inventory generation.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct DirectoryManifest {
    pub(crate) namespace_id: String,
    pub(crate) directory_digest: String,
    pub(crate) generation: u64,
    pub(crate) entries: BTreeMap<String, DirectoryEntry>,
    pub(crate) manifest_digest: String,
}

/// Result of explicit directory reconciliation.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct DirectorySyncResult {
    pub(crate) namespace_id: String,
    pub(crate) directory_digest: String,
    pub(crate) previous_generation: Option<u64>,
    pub(crate) generation: u64,
    pub(crate) previous_sources: usize,
    pub(crate) indexed_sources: usize,
    pub(crate) changed_sources: usize,
    pub(crate) missing_sources: usize,
    pub(crate) retired_sources: usize,
    pub(crate) moved_or_rebound_sources: usize,
    pub(crate) manifest_digest: String,
}

/// Verification summary for all immutable directory manifests.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct DirectoryManifestVerification {
    pub(crate) manifest_files: usize,
    pub(crate) directories: usize,
    pub(crate) current_entries: usize,
    pub(crate) highest_generation: u64,
}

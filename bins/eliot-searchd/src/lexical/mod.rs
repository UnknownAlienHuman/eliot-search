//! In-memory lexical index derived only from one immutable retained snapshot.

mod build;
mod manifest;
mod search;

use std::collections::BTreeMap;

use search_lexical::AnalyzerConfig;

use manifest::ManifestDocument;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) struct LexicalIndexLimits {
    pub(crate) max_terms: usize,
    pub(crate) max_postings: usize,
    pub(crate) max_query_terms: usize,
    pub(crate) max_results: usize,
    pub(crate) max_file_bytes: u64,
    pub(crate) max_excerpt_chars: usize,
}

impl LexicalIndexLimits {
    pub(crate) const fn baseline(
        max_results: usize,
        max_file_bytes: u64,
        max_excerpt_chars: usize,
    ) -> Self {
        Self {
            max_terms: 1_000_000,
            max_postings: 8_000_000,
            max_query_terms: 64,
            max_results,
            max_file_bytes,
            max_excerpt_chars,
        }
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
struct Posting {
    document_index: usize,
    frequency: u64,
    first_byte_offset: u64,
}

#[derive(Clone, Debug, Eq, PartialEq)]
struct IndexedDocument {
    source: ManifestDocument,
    token_count: u64,
}

#[derive(Clone, Debug)]
pub(crate) struct LexicalIndex {
    snapshot_id: String,
    snapshot_manifest_fingerprint: [u8; 32],
    index_fingerprint: [u8; 32],
    analyzer: AnalyzerConfig,
    documents: Vec<IndexedDocument>,
    postings: BTreeMap<String, Vec<Posting>>,
    posting_count: usize,
    average_document_length: f64,
    limits: LexicalIndexLimits,
}

impl LexicalIndex {
    pub(crate) fn snapshot_id(&self) -> &str {
        &self.snapshot_id
    }

    pub(crate) const fn snapshot_manifest_fingerprint(&self) -> [u8; 32] {
        self.snapshot_manifest_fingerprint
    }

    pub(crate) const fn index_fingerprint(&self) -> [u8; 32] {
        self.index_fingerprint
    }

    pub(crate) fn analyzer_id(&self) -> &str {
        self.analyzer.analyzer_id.as_str()
    }

    pub(crate) fn document_count(&self) -> usize {
        self.documents.len()
    }

    pub(crate) fn term_count(&self) -> usize {
        self.postings.len()
    }

    pub(crate) const fn posting_count(&self) -> usize {
        self.posting_count
    }
}

#[derive(Clone, Debug, PartialEq)]
pub(crate) struct LexicalMatch {
    pub(crate) root_index: usize,
    pub(crate) relative_path: String,
    pub(crate) revision_fingerprint: [u8; 32],
    pub(crate) score: f64,
    pub(crate) matched_terms: usize,
    pub(crate) line: usize,
    pub(crate) column_bytes: usize,
    pub(crate) byte_start: usize,
    pub(crate) excerpt: String,
}

#[derive(Clone, Debug, PartialEq)]
pub(crate) struct LexicalSearchResult {
    pub(crate) snapshot_id: String,
    pub(crate) snapshot_manifest_fingerprint: [u8; 32],
    pub(crate) index_fingerprint: [u8; 32],
    pub(crate) analyzer_id: String,
    pub(crate) denominator_documents: usize,
    pub(crate) indexed_terms: usize,
    pub(crate) indexed_postings: usize,
    pub(crate) query_term_count: usize,
    pub(crate) candidate_documents: usize,
    pub(crate) unavailable_revisions: usize,
    pub(crate) matches: Vec<LexicalMatch>,
    pub(crate) complete: bool,
    pub(crate) truncated: bool,
}

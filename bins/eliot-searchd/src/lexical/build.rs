//! Deterministic lexical-index construction from a frozen snapshot manifest.

use std::collections::BTreeMap;
use std::io::{self, ErrorKind};
use std::path::Path;

use search_contracts::{Blake3Digest32, NonZeroRevision, OpaqueId};
use search_lexical::{
    AnalyzerConfig, CaseNormalization, LexicalInput, TokenCharacterPolicy,
    DEFAULT_LEXICAL_LIMITS, analyze,
};

use crate::snapshot::fingerprint;

use super::manifest::{load_manifest_documents, read_retained_document};
use super::{IndexedDocument, LexicalIndex, LexicalIndexLimits, Posting};

impl LexicalIndex {
    pub(crate) fn build(
        data_root: &Path,
        snapshot_id: &str,
        manifest_path: &Path,
        snapshot_manifest_fingerprint: [u8; 32],
        limits: LexicalIndexLimits,
    ) -> io::Result<Self> {
        validate_limits(limits)?;
        let analyzer = AnalyzerConfig::new(
            OpaqueId::new("lexical:development-neutral-v1")
                .map_err(|_| io::Error::other("invalid lexical analyzer identity"))?,
            NonZeroRevision::new(1)
                .map_err(|_| io::Error::other("invalid lexical analyzer revision"))?,
            TokenCharacterPolicy::UnicodeAlphanumericAndUnderscore,
            CaseNormalization::UnicodeLowercase,
            1,
            true,
            Vec::<String>::new(),
            Blake3Digest32::from_bytes([0x6c; 32]),
            DEFAULT_LEXICAL_LIMITS,
        )
        .map_err(|error| io::Error::other(error.to_string()))?;
        let manifest_documents = load_manifest_documents(
            data_root,
            manifest_path,
            snapshot_id,
            snapshot_manifest_fingerprint,
            limits.max_file_bytes,
        )?;

        let mut documents = Vec::with_capacity(manifest_documents.len());
        let mut postings = BTreeMap::<String, Vec<Posting>>::new();
        let mut posting_count = 0_usize;
        let mut total_tokens = 0_u64;

        for (document_index, source) in manifest_documents.into_iter().enumerate() {
            let text = read_retained_document(&source, limits.max_file_bytes)?;
            let token_count = if text.is_empty() {
                0
            } else {
                let input_length = u64::try_from(text.len()).map_err(|_| {
                    io::Error::new(ErrorKind::InvalidData, "lexical input length overflow")
                })?;
                let analysis = analyze(
                    LexicalInput::new(
                        OpaqueId::new(format!("snapshot-document:{document_index}"))
                            .map_err(|_| io::Error::other("invalid lexical source identity"))?,
                        NonZeroRevision::new(1).map_err(|_| {
                            io::Error::other("invalid lexical source revision")
                        })?,
                        0,
                        0,
                        input_length,
                        text,
                    ),
                    &analyzer,
                    DEFAULT_LEXICAL_LIMITS,
                )
                .map_err(|error| io::Error::other(error.to_string()))?;
                let mut first_offsets = BTreeMap::<String, u64>::new();
                for token in &analysis.tokens {
                    first_offsets
                        .entry(token.term.clone())
                        .or_insert(token.unit_byte_start);
                }
                for term in &analysis.terms {
                    if !postings.contains_key(&term.term) && postings.len() >= limits.max_terms {
                        return Err(io::Error::new(
                            ErrorKind::InvalidData,
                            "lexical term ceiling exceeded",
                        ));
                    }
                    posting_count = posting_count.checked_add(1).ok_or_else(|| {
                        io::Error::new(ErrorKind::InvalidData, "lexical posting overflow")
                    })?;
                    if posting_count > limits.max_postings {
                        return Err(io::Error::new(
                            ErrorKind::InvalidData,
                            "lexical posting ceiling exceeded",
                        ));
                    }
                    let first_byte_offset = first_offsets
                        .get(&term.term)
                        .copied()
                        .ok_or_else(|| {
                            io::Error::new(
                                ErrorKind::InvalidData,
                                "lexical first-offset accounting mismatch",
                            )
                        })?;
                    postings.entry(term.term.clone()).or_default().push(Posting {
                        document_index,
                        frequency: term.frequency,
                        first_byte_offset,
                    });
                }
                analysis.receipt.emitted_token_count
            };
            total_tokens = total_tokens.checked_add(token_count).ok_or_else(|| {
                io::Error::new(ErrorKind::InvalidData, "lexical token accounting overflow")
            })?;
            documents.push(IndexedDocument {
                source,
                token_count,
            });
        }

        for posting_list in postings.values_mut() {
            posting_list.sort_by_key(|posting| posting.document_index);
            if posting_list
                .windows(2)
                .any(|pair| pair[0].document_index >= pair[1].document_index)
            {
                return Err(io::Error::new(
                    ErrorKind::InvalidData,
                    "lexical posting order is not canonical",
                ));
            }
        }
        let average_document_length = if documents.is_empty() {
            1.0
        } else {
            let average = total_tokens as f64 / documents.len() as f64;
            if average > 0.0 { average } else { 1.0 }
        };
        let index_fingerprint = index_fingerprint(
            snapshot_id,
            snapshot_manifest_fingerprint,
            &analyzer,
            &documents,
            &postings,
        )?;
        Ok(Self {
            snapshot_id: snapshot_id.to_owned(),
            snapshot_manifest_fingerprint,
            index_fingerprint,
            analyzer,
            documents,
            postings,
            posting_count,
            average_document_length,
            limits,
        })
    }
}

fn validate_limits(limits: LexicalIndexLimits) -> io::Result<()> {
    if limits.max_terms == 0
        || limits.max_postings == 0
        || limits.max_query_terms == 0
        || limits.max_results == 0
        || limits.max_results > 32
        || limits.max_file_bytes == 0
        || limits.max_excerpt_chars == 0
        || limits.max_excerpt_chars > 512
    {
        return Err(io::Error::new(
            ErrorKind::InvalidInput,
            "invalid lexical index limits",
        ));
    }
    Ok(())
}

fn index_fingerprint(
    snapshot_id: &str,
    snapshot_manifest_fingerprint: [u8; 32],
    analyzer: &AnalyzerConfig,
    documents: &[IndexedDocument],
    postings: &BTreeMap<String, Vec<Posting>>,
) -> io::Result<[u8; 32]> {
    let mut canonical = Vec::new();
    append(&mut canonical, snapshot_id.as_bytes())?;
    append(&mut canonical, &snapshot_manifest_fingerprint)?;
    append(&mut canonical, analyzer.analyzer_id.as_str().as_bytes())?;
    append(&mut canonical, analyzer.fingerprint().as_bytes())?;
    append_count(&mut canonical, documents.len())?;
    for document in documents {
        append_count(&mut canonical, document.source.root_index)?;
        append(&mut canonical, document.source.relative_path.as_bytes())?;
        append(&mut canonical, &document.source.revision_fingerprint)?;
        append(&mut canonical, &document.token_count.to_be_bytes())?;
    }
    append_count(&mut canonical, postings.len())?;
    for (term, posting_list) in postings {
        append(&mut canonical, term.as_bytes())?;
        append_count(&mut canonical, posting_list.len())?;
        for posting in posting_list {
            append_count(&mut canonical, posting.document_index)?;
            append(&mut canonical, &posting.frequency.to_be_bytes())?;
            append(&mut canonical, &posting.first_byte_offset.to_be_bytes())?;
        }
    }
    Ok(fingerprint(&canonical))
}

fn append_count(output: &mut Vec<u8>, value: usize) -> io::Result<()> {
    let value = u64::try_from(value).map_err(|_| {
        io::Error::new(ErrorKind::InvalidData, "lexical fingerprint count overflow")
    })?;
    append(output, &value.to_be_bytes())
}

fn append(output: &mut Vec<u8>, value: &[u8]) -> io::Result<()> {
    let length = u64::try_from(value.len()).map_err(|_| {
        io::Error::new(ErrorKind::InvalidData, "lexical fingerprint length overflow")
    })?;
    output
        .len()
        .checked_add(value.len().saturating_add(8))
        .ok_or_else(|| io::Error::new(ErrorKind::InvalidData, "lexical fingerprint overflow"))?;
    output.extend_from_slice(&length.to_be_bytes());
    output.extend_from_slice(value);
    Ok(())
}

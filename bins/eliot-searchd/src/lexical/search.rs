//! Deterministic BM25 retrieval over the frozen in-memory lexical index.

#![allow(clippy::cast_precision_loss, clippy::missing_errors_doc)]

use core::cmp::Ordering;
use std::collections::BTreeMap;
use std::io::{self, ErrorKind};

use search_contracts::{NonZeroRevision, OpaqueId};
use search_lexical::{LexicalInput, TermStatistics, DEFAULT_LEXICAL_LIMITS, analyze};

use super::manifest::read_retained_document;
use super::{LexicalIndex, LexicalMatch, LexicalSearchResult};

#[derive(Clone, Copy, Debug)]
struct Accumulator {
    score: f64,
    matched_terms: usize,
    first_byte_offset: u64,
}

#[derive(Clone, Copy, Debug)]
struct RankedCandidate {
    document_index: usize,
    score: f64,
    matched_terms: usize,
    first_byte_offset: u64,
}

impl LexicalIndex {
    /// Scores every indexed document against the analyzed query terms.
    fn accumulate_scores(
        &self,
        terms: &[TermStatistics],
        document_count: f64,
    ) -> io::Result<BTreeMap<usize, Accumulator>> {
        let mut accumulators = BTreeMap::<usize, Accumulator>::new();
        for term in terms {
            let Some(postings) = self.postings.get(&term.term) else {
                continue;
            };
            let document_frequency = postings.len() as f64;
            let inverse_document_frequency = if document_count == 0.0 {
                0.0
            } else {
                ((document_count - document_frequency + 0.5) / (document_frequency + 0.5)).ln_1p()
            };
            if !inverse_document_frequency.is_finite()
                || inverse_document_frequency < 0.0
            {
                return Err(io::Error::new(
                    ErrorKind::InvalidData,
                    "lexical inverse-document-frequency is invalid",
                ));
            }
            for posting in postings {
                let document = self
                    .documents
                    .get(posting.document_index)
                    .ok_or_else(|| {
                        io::Error::new(
                            ErrorKind::InvalidData,
                            "lexical posting references a missing document",
                        )
                    })?;
                let term_frequency = posting.frequency as f64;
                let document_length = document.token_count as f64;
                let k1 = 1.2_f64;
                let b = 0.75_f64;
                let denominator = k1.mul_add(b.mul_add(document_length / self.average_document_length, 1.0 - b), term_frequency);
                let contribution = inverse_document_frequency
                    * (term_frequency * (k1 + 1.0) / denominator);
                if !contribution.is_finite() || contribution < 0.0 {
                    return Err(io::Error::new(
                        ErrorKind::InvalidData,
                        "lexical score contribution is invalid",
                    ));
                }
                let accumulator = accumulators
                    .entry(posting.document_index)
                    .or_insert(Accumulator {
                        score: 0.0,
                        matched_terms: 0,
                        first_byte_offset: posting.first_byte_offset,
                    });
                accumulator.score += contribution;
                accumulator.matched_terms = accumulator.matched_terms.saturating_add(1);
                accumulator.first_byte_offset = accumulator
                    .first_byte_offset
                    .min(posting.first_byte_offset);
            }
        }
        Ok(accumulators)
    }

    /// Reopens retained documents for ranked candidates, skipping documents
    /// whose revisions are no longer available.
    fn materialize_matches(
        &self,
        ranked: Vec<RankedCandidate>,
    ) -> io::Result<(Vec<LexicalMatch>, usize)> {
        let mut unavailable_revisions = 0_usize;
        let mut matches = Vec::new();
        for candidate in ranked {
            if matches.len() >= self.limits.results {
                break;
            }
            let document = self
                .documents
                .get(candidate.document_index)
                .ok_or_else(|| {
                    io::Error::new(
                        ErrorKind::InvalidData,
                        "ranked candidate references a missing document",
                    )
                })?;
            let Ok(text) = read_retained_document(
                &document.source,
                self.limits.file_bytes,
            ) else {
                unavailable_revisions = unavailable_revisions.saturating_add(1);
                continue;
            };
            let offset = usize::try_from(candidate.first_byte_offset).map_err(|_| {
                io::Error::new(ErrorKind::InvalidData, "lexical byte offset overflow")
            })?;
            let location = locate_line(&text, offset)?;
            matches.push(LexicalMatch {
                root_index: document.source.root_index,
                relative_path: document.source.relative_path.clone(),
                revision_fingerprint: document.source.revision_fingerprint,
                score: candidate.score,
                matched_terms: candidate.matched_terms,
                line: location.line,
                column_bytes: location.column_bytes,
                byte_start: offset,
                excerpt: truncate_chars(location.text.trim(), self.limits.excerpt_chars),
            });
        }
        Ok((matches, unavailable_revisions))
    }

    pub(crate) fn search(&self, query: &str) -> io::Result<LexicalSearchResult> {
        if query.is_empty() || query.len() > 1_024 {
            return Err(io::Error::new(
                ErrorKind::InvalidInput,
                "lexical query must be non-empty and at most 1024 UTF-8 bytes",
            ));
        }
        let query_length = u64::try_from(query.len()).map_err(|_| {
            io::Error::new(ErrorKind::InvalidData, "lexical query length overflow")
        })?;
        let analysis = analyze(
            LexicalInput::new(
                OpaqueId::new("query:lexical")
                    .map_err(|_| io::Error::other("invalid lexical query identity"))?,
                NonZeroRevision::new(1)
                    .map_err(|_| io::Error::other("invalid lexical query revision"))?,
                0,
                0,
                query_length,
                query.to_owned(),
            ),
            &self.analyzer,
            DEFAULT_LEXICAL_LIMITS,
        )
        .map_err(|error| io::Error::other(error.to_string()))?;
        if analysis.terms.is_empty() {
            return Err(io::Error::new(
                ErrorKind::InvalidInput,
                "lexical query contains no searchable terms",
            ));
        }
        if analysis.terms.len() > self.limits.query_terms {
            return Err(io::Error::new(
                ErrorKind::InvalidInput,
                "lexical query exceeds its term ceiling",
            ));
        }

        let document_count = self.documents.len() as f64;
        let accumulators = self.accumulate_scores(&analysis.terms, document_count)?;

        let candidate_documents = accumulators.len();
        let mut ranked = accumulators
            .into_iter()
            .map(|(document_index, accumulator)| RankedCandidate {
                document_index,
                score: accumulator.score,
                matched_terms: accumulator.matched_terms,
                first_byte_offset: accumulator.first_byte_offset,
            })
            .collect::<Vec<_>>();
        ranked.sort_by(|left, right| {
            right
                .score
                .partial_cmp(&left.score)
                .unwrap_or(Ordering::Equal)
                .then_with(|| right.matched_terms.cmp(&left.matched_terms))
                .then_with(|| compare_documents(self, left.document_index, right.document_index))
        });

        let truncated = ranked.len() > self.limits.results;
        let (matches, unavailable_revisions) = self.materialize_matches(ranked)?;

        Ok(LexicalSearchResult {
            snapshot_id: self.snapshot_id.clone(),
            snapshot_manifest_fingerprint: self.snapshot_manifest_fingerprint,
            index_fingerprint: self.index_fingerprint,
            analyzer_id: self.analyzer.analyzer_id.as_str().to_owned(),
            denominator_documents: self.documents.len(),
            indexed_terms: self.postings.len(),
            indexed_postings: self.posting_count,
            query_term_count: analysis.terms.len(),
            candidate_documents,
            unavailable_revisions,
            complete: !truncated && unavailable_revisions == 0,
            truncated,
            matches,
        })
    }
}

/// Counts newline bytes with an explicit loop.
fn count_newlines(bytes: &[u8]) -> usize {
    let mut count = 0_usize;
    for byte in bytes {
        if *byte == b'\n' {
            count += 1;
        }
    }
    count
}

fn compare_documents(
    index: &LexicalIndex,
    left_index: usize,
    right_index: usize,
) -> Ordering {
    match (
        index.documents.get(left_index),
        index.documents.get(right_index),
    ) {
        (Some(left), Some(right)) => left
            .source
            .root_index
            .cmp(&right.source.root_index)
            .then_with(|| {
                left.source
                    .relative_path
                    .cmp(&right.source.relative_path)
            })
            .then_with(|| {
                left.source
                    .revision_fingerprint
                    .cmp(&right.source.revision_fingerprint)
            }),
        (Some(_), None) => Ordering::Less,
        (None, Some(_)) => Ordering::Greater,
        (None, None) => left_index.cmp(&right_index),
    }
}

struct LineLocation<'a> {
    line: usize,
    column_bytes: usize,
    text: &'a str,
}

fn locate_line(text: &str, offset: usize) -> io::Result<LineLocation<'_>> {
    if offset > text.len() || !text.is_char_boundary(offset) {
        return Err(io::Error::new(
            ErrorKind::InvalidData,
            "lexical byte offset is outside the retained revision",
        ));
    }
    let bytes = text.as_bytes();
    let line_start = bytes[..offset]
        .iter()
        .rposition(|byte| *byte == b'\n')
        .map_or(0, |position| position.saturating_add(1));
    let mut line_end = bytes[offset..]
        .iter()
        .position(|byte| *byte == b'\n')
        .map_or(bytes.len(), |relative| offset.saturating_add(relative));
    if line_end > line_start && bytes[line_end - 1] == b'\r' {
        line_end -= 1;
    }
    let line = count_newlines(&bytes[..line_start]).saturating_add(1);
    Ok(LineLocation {
        line,
        column_bytes: offset.saturating_sub(line_start),
        text: &text[line_start..line_end],
    })
}

fn truncate_chars(value: &str, limit: usize) -> String {
    let mut output = value.chars().take(limit).collect::<String>();
    if value.chars().count() > limit {
        output.push('…');
    }
    output
}

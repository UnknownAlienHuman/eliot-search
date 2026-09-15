//! Canonical read-only DIRECT search over verified retained revisions.

use crate::development::ScanResult;
use crate::direct_preparation::{
    CANONICAL_CORPUS_BUDGET, SPINE_GAP_BUDGET_EXHAUSTED,
    SPINE_GAP_MATCH_LIMIT, SPINE_GAP_VALIDATION_FAILED, scan_prepared,
    validate_query, validate_source_backed_match,
};
use crate::plaintext_direct_store::{
    RevisionMetadata, SourceSummary, StoreGap, StoreSearchResult, StoredMatch,
};
use crate::sha256;

use super::DirectStore;
use super::super::preparation_store;

const MAX_SEARCH_GAPS: usize = 100_000;

impl DirectStore {
    /// Searches verified retained revisions using saved profile-bound
    /// preparation. Missing/invalid preparation is an explicit gap; the query
    /// path performs no durable writes and never narrows the frozen denominator.
    pub(crate) fn search(
        &self,
        query: &str,
        ascii_insensitive: bool,
    ) -> Result<StoreSearchResult, String> {
        validate_query(query).map_err(str::to_owned)?;
        let namespace = self.inner.namespace_id();
        let active = self
            .inner
            .list_sources()
            .into_iter()
            .filter(|source| source.active)
            .collect::<Vec<_>>();
        let mut matches = Vec::new();
        let mut gaps = Vec::new();
        let mut searched_sources = 0_usize;
        let mut scanned_bytes = 0_u64;
        let mut complete = true;
        let mut match_limit_reached = false;

        for (index, source) in active.iter().enumerate() {
            if matches.len() >= CANONICAL_CORPUS_BUDGET.max_matches {
                complete = false;
                match_limit_reached = true;
                Self::push_remaining_gaps(
                    &active,
                    index,
                    SPINE_GAP_MATCH_LIMIT,
                    &mut gaps,
                    &mut complete,
                );
                break;
            }
            if index >= CANONICAL_CORPUS_BUDGET.max_sources {
                Self::push_remaining_gaps(
                    &active,
                    index,
                    SPINE_GAP_BUDGET_EXHAUSTED,
                    &mut gaps,
                    &mut complete,
                );
                break;
            }
            if scanned_bytes
                .checked_add(source.byte_length)
                .is_none_or(|total| {
                    total > CANONICAL_CORPUS_BUDGET.max_source_bytes
                })
            {
                Self::push_remaining_gaps(
                    &active,
                    index,
                    SPINE_GAP_BUDGET_EXHAUSTED,
                    &mut gaps,
                    &mut complete,
                );
                break;
            }
            scanned_bytes = scanned_bytes.saturating_add(source.byte_length);
            let metadata = RevisionMetadata {
                source_id: source.source_id.clone(),
                revision_id: source.revision_id.clone(),
                content_digest: source.content_digest.clone(),
                byte_length: source.byte_length,
            };
            let bytes = match self.read_verified_revision(&metadata) {
                Ok(bytes) => bytes,
                Err(reason) => {
                    if gaps.len() >= MAX_SEARCH_GAPS {
                        complete = false;
                        break;
                    }
                    gaps.push(StoreGap {
                        source_id: source.source_id.clone(),
                        revision_id: source.revision_id.clone(),
                        reason,
                    });
                    complete = false;
                    continue;
                }
            };
            let Ok(text) = String::from_utf8(bytes) else {
                if gaps.len() >= MAX_SEARCH_GAPS {
                    complete = false;
                    break;
                }
                gaps.push(StoreGap {
                    source_id: source.source_id.clone(),
                    revision_id: source.revision_id.clone(),
                    reason: "DIRECT_REVISION_NOT_UTF8",
                });
                complete = false;
                continue;
            };
            let ScanResult {
                matches: source_matches,
                coverage,
            } = match preparation_store::load(
                &self.root,
                &self.protector,
                &namespace,
                &metadata,
            )
            .and_then(|saved| {
                scan_prepared(&text, &saved, query, ascii_insensitive)
            }) {
                Ok(result) => result,
                Err(reason) => {
                    complete = false;
                    if gaps.len() >= MAX_SEARCH_GAPS {
                        break;
                    }
                    gaps.push(StoreGap {
                        source_id: source.source_id.clone(),
                        revision_id: source.revision_id.clone(),
                        reason,
                    });
                    continue;
                }
            };

            let validation_failed = source_matches.iter().any(|item| {
                validate_source_backed_match(
                    &text,
                    query,
                    ascii_insensitive,
                    item.byte_start,
                    item.byte_end,
                )
                .is_err()
            });
            if validation_failed {
                complete = false;
                if gaps.len() >= CANONICAL_CORPUS_BUDGET.max_gaps {
                    break;
                }
                gaps.push(StoreGap {
                    source_id: source.source_id.clone(),
                    revision_id: source.revision_id.clone(),
                    reason: SPINE_GAP_VALIDATION_FAILED,
                });
                continue;
            }

            searched_sources = searched_sources.saturating_add(1);
            if !coverage.complete {
                complete = false;
                match_limit_reached = coverage.match_limit_reached;
            }
            for item in source_matches {
                if matches.len() >= CANONICAL_CORPUS_BUDGET.max_matches {
                    complete = false;
                    match_limit_reached = true;
                    break;
                }
                let start = u64::try_from(item.byte_start)
                    .map_err(|_| "DIRECT_MATCH_OFFSET_OVERFLOW".to_owned())?;
                let end = u64::try_from(item.byte_end)
                    .map_err(|_| "DIRECT_MATCH_OFFSET_OVERFLOW".to_owned())?;
                let evidence_id = sha256::hex(&sha256::digest_parts(
                    b"eliot-search/direct-evidence/v1",
                    &[
                        source.source_id.as_bytes(),
                        source.revision_id.as_bytes(),
                        source.content_digest.as_bytes(),
                        &start.to_be_bytes(),
                        &end.to_be_bytes(),
                    ],
                ));
                matches.push(StoredMatch {
                    source_id: source.source_id.clone(),
                    revision_id: source.revision_id.clone(),
                    content_digest: source.content_digest.clone(),
                    path_digest: source.path_digest.clone(),
                    evidence_id,
                    byte_start: item.byte_start,
                    byte_end: item.byte_end,
                    line: item.line,
                    column_bytes: item.column_bytes,
                });
            }
            if match_limit_reached {
                Self::push_remaining_gaps(
                    &active,
                    index.saturating_add(1),
                    SPINE_GAP_MATCH_LIMIT,
                    &mut gaps,
                    &mut complete,
                );
                break;
            }
        }

        Ok(StoreSearchResult {
            matches,
            gaps,
            registered_sources: self.inner.list_sources().len(),
            active_sources: active.len(),
            searched_sources,
            complete,
            match_limit_reached,
        })
    }

    fn push_remaining_gaps(
        active: &[SourceSummary],
        from: usize,
        reason: &'static str,
        gaps: &mut Vec<StoreGap>,
        complete: &mut bool,
    ) {
        *complete = false;
        for source in active.iter().skip(from) {
            if gaps.len()
                >= CANONICAL_CORPUS_BUDGET.max_gaps.min(MAX_SEARCH_GAPS)
            {
                return;
            }
            gaps.push(StoreGap {
                source_id: source.source_id.clone(),
                revision_id: source.revision_id.clone(),
                reason,
            });
        }
    }
}

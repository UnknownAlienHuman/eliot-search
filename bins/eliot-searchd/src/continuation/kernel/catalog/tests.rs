//! In-memory paging mechanics only. These synthetic records do not qualify
//! entropy, authentication, source fences, native storage, or canonical T21 wiring.

use super::*;

fn record(count: usize, next_index: usize) -> ContinuationRecord {
    // Spare capacity makes replacing the retained allocation observable as well
    // as checking its address. No pointer is dereferenced by these tests.
    let mut matches = Vec::with_capacity(count + 37);
    for index in 0..count {
        matches.push(StoredMatch {
            source_id: "synthetic-source".to_owned(),
            revision_id: "synthetic-revision".to_owned(),
            content_digest: "synthetic-content".to_owned(),
            path_digest: "synthetic-path".to_owned(),
            evidence_id: format!("synthetic-evidence-{index}"),
            byte_start: index,
            byte_end: index + 1,
            line: index + 1,
            column_bytes: 1,
        });
    }
    ContinuationRecord {
        namespace_id: "synthetic-namespace".to_owned(),
        session_tag: 42,
        source_fence_digest: "synthetic-fence".to_owned(),
        matches,
        next_index,
        coverage: PageCoverage {
            registered_sources: 1,
            active_sources: 1,
            searched_sources: 1,
            completion: PageCompletion {
                corpus_complete: true,
                match_limit_reached: false,
            },
            total_matches: count,
            retained_matches: count,
            gap_count: 0,
            truncation: PageTruncation {
                candidate_window_truncated: false,
                gap_details_truncated: false,
            },
        },
        expires_at: Instant::now() + CONTINUATION_TTL,
    }
}

fn catalog(windows: &[(&str, usize, usize)]) -> ContinuationCatalog {
    let records = windows
        .iter()
        .map(|(token, count, next)| ((*token).to_owned(), record(*count, *next)))
        .collect::<BTreeMap<_, _>>();
    ContinuationCatalog {
        namespace_id: "synthetic-namespace".to_owned(),
        session_tag: 42,
        entropy_poisoned: false,
        retained_matches: records.values().map(|record| record.matches.len()).sum(),
        records,
    }
}

#[test]
fn intermediate_page_preserves_the_retained_allocation() {
    let mut catalog = catalog(&[("window", 8, 1)]);
    let before = &catalog.records["window"];
    let storage = before.matches.as_ptr();
    let capacity = before.matches.capacity();
    let evidence_storage = before.matches[0].evidence_id.as_ptr();
    let expected = before.matches[1..3].to_vec();

    let page = catalog.advance_window("window", 2).unwrap();

    assert_eq!(page.matches, expected);
    assert_eq!(page.page_start, 1);
    assert_eq!(page.page_end, 3);
    assert!(!page.exhausted);
    let after = &catalog.records["window"];
    assert_eq!(after.matches.as_ptr(), storage);
    assert_eq!(after.matches.capacity(), capacity);
    assert_eq!(after.matches[0].evidence_id.as_ptr(), evidence_storage);
    assert_eq!(after.next_index, 3);
    assert_eq!(catalog.retained_matches, 8);
}

#[test]
fn every_page_preserves_order_coverage_and_original_expiration() {
    let mut catalog = catalog(&[("window", 100, 1)]);
    let expected = catalog.records["window"].matches[1..].to_vec();
    let coverage = catalog.records["window"].coverage.clone();
    let deadline = catalog.records["window"].expires_at;
    let mut delivered = Vec::new();
    let mut cursor = 1;
    let mut previous_ttl = u64::MAX;
    let mut step = 0;

    loop {
        let page_size = [1, 3, 7, 11][step % 4];
        let page = catalog.advance_window("window", page_size).unwrap();
        assert_eq!(page.page_start, cursor);
        assert_eq!(page.page_end, (cursor + page_size).min(100));
        assert_eq!(page.coverage, coverage);
        assert!(page.gaps.is_empty());
        cursor = page.page_end;
        delivered.extend(page.matches);
        if page.exhausted {
            assert!(page.continuation_token.is_none());
            assert!(page.expires_in_ms.is_none());
            break;
        }
        assert_eq!(page.continuation_token.as_deref(), Some("window"));
        let ttl = page.expires_in_ms.unwrap();
        assert!(ttl <= previous_ttl);
        previous_ttl = ttl;
        let retained = &catalog.records["window"];
        assert_eq!(retained.expires_at, deadline);
        assert_eq!(retained.namespace_id, "synthetic-namespace");
        assert_eq!(retained.session_tag, 42);
        assert_eq!(retained.source_fence_digest, "synthetic-fence");
        assert_eq!(catalog.retained_matches, 100);
        step += 1;
        assert!(step <= 100, "paging did not advance");
    }

    assert_eq!(delivered, expected);
    assert!(catalog.records.is_empty());
    assert_eq!(catalog.retained_matches, 0);
    assert_eq!(
        catalog.advance_window("window", 1),
        Err(ContinuationError::NotFound),
    );
}

#[test]
fn final_page_releases_only_its_own_window() {
    let mut catalog = catalog(&[("first", 3, 1), ("second", 5, 1)]);
    let other_storage = catalog.records["second"].matches.as_ptr();
    let expected = catalog.records["first"].matches[1..].to_vec();

    let page = catalog.advance_window("first", 100).unwrap();

    assert_eq!(page.matches, expected);
    assert!(page.exhausted);
    assert!(page.continuation_token.is_none());
    assert!(page.expires_in_ms.is_none());
    assert!(!catalog.records.contains_key("first"));
    assert_eq!(catalog.records["second"].matches.as_ptr(), other_storage);
    assert_eq!(catalog.records["second"].next_index, 1);
    assert_eq!(catalog.records.len(), 1);
    assert_eq!(catalog.retained_matches, 5);
}

#[test]
fn unknown_token_does_not_mutate_an_existing_window() {
    let mut catalog = catalog(&[("window", 8, 1)]);
    let storage = catalog.records["window"].matches.as_ptr();

    assert_eq!(
        catalog.advance_window("unknown", 1),
        Err(ContinuationError::NotFound),
    );

    assert_eq!(catalog.records["window"].matches.as_ptr(), storage);
    assert_eq!(catalog.records["window"].next_index, 1);
    assert_eq!(catalog.records.len(), 1);
    assert_eq!(catalog.retained_matches, 8);
}

#[test]
fn advancing_a_live_window_still_sweeps_expired_other_windows() {
    let mut catalog = catalog(&[("live", 8, 1), ("expired", 5, 1)]);
    catalog.records.get_mut("expired").unwrap().expires_at = Instant::now();

    let page = catalog.advance_window("live", 2).unwrap();

    assert!(!page.exhausted);
    assert_eq!(catalog.records["live"].next_index, 3);
    assert!(!catalog.records.contains_key("expired"));
    assert_eq!(catalog.records.len(), 1);
    assert_eq!(catalog.retained_matches, 8);
}

#[test]
fn truncated_or_partial_coverage_is_not_promoted_to_complete() {
    let mut catalog = catalog(&[("window", 8, 1)]);
    let record = catalog.records.get_mut("window").unwrap();
    record.coverage.total_matches = 12;
    record.coverage.gap_count = 300;
    record.coverage.completion.corpus_complete = false;
    record.coverage.completion.match_limit_reached = true;
    record.coverage.truncation.candidate_window_truncated = true;
    record.coverage.truncation.gap_details_truncated = true;
    let coverage = record.coverage.clone();

    for page_size in [2, 100] {
        let page = catalog.advance_window("window", page_size).unwrap();
        assert_eq!(page.coverage, coverage);
        assert!(!page.coverage.complete());
        assert!(page.gaps.is_empty());
    }
    assert!(catalog.records.is_empty());
    assert_eq!(catalog.retained_matches, 0);
}

#[test]
fn invalidation_after_an_intermediate_page_releases_the_whole_window() {
    let mut catalog = catalog(&[("window", 8, 1)]);
    assert!(!catalog.advance_window("window", 2).unwrap().exhausted);
    assert_eq!(catalog.invalidate_all(), 1);
    assert_eq!(catalog.invalidate_all(), 0);
    catalog.drop_window("window");
    assert_eq!(catalog.retained_matches, 0);
    assert_eq!(
        catalog.advance_window("window", 1),
        Err(ContinuationError::NotFound),
    );
}

#[test]
fn empty_final_page_cannot_retain_or_reissue_a_window() {
    let mut catalog = catalog(&[("window", 3, 3)]);
    let page = catalog.advance_window("window", 1).unwrap();
    assert!(page.matches.is_empty());
    assert_eq!(page.page_start, 3);
    assert_eq!(page.page_end, 3);
    assert!(page.exhausted);
    assert!(page.continuation_token.is_none());
    assert!(page.expires_in_ms.is_none());
    assert_eq!(catalog.retained_matches, 0);
    assert!(catalog.records.is_empty());
}

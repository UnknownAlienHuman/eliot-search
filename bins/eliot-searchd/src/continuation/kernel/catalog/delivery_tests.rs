//! Synthetic page-delivery mechanics; not source/authentication qualification.

use super::*;

fn record() -> ContinuationRecord {
    let matches = (0..4).map(|index| StoredMatch {
        source_id: "fixture-source".into(),
        revision_id: "fixture-revision".into(),
        content_digest: "fixture-content".into(),
        path_digest: "fixture-path".into(),
        evidence_id: format!("fixture-{index}"),
        byte_start: index,
        byte_end: index + 1,
        line: 1,
        column_bytes: index + 1,
    }).collect();
    ContinuationRecord {
        namespace_id: "fixture-namespace".into(),
        session_tag: 1,
        source_fence_digest: "fixture-fence".into(),
        matches,
        next_index: 1,
        coverage: PageCoverage {
            registered_sources: 1,
            active_sources: 1,
            searched_sources: 1,
            completion: PageCompletion {
                corpus_complete: true,
                match_limit_reached: false,
            },
            total_matches: 4,
            retained_matches: 4,
            gap_count: 0,
            truncation: PageTruncation {
                candidate_window_truncated: false,
                gap_details_truncated: false,
            },
        },
        expires_at: Instant::now() + CONTINUATION_TTL,
    }
}

fn catalog() -> ContinuationCatalog {
    ContinuationCatalog {
        namespace_id: "fixture-namespace".into(),
        session_tag: 1,
        entropy_poisoned: false,
        records: BTreeMap::from([("existing".into(), record())]),
        retained_matches: 4,
    }
}

fn new_page(catalog: &mut ContinuationCatalog, expires_at: Instant) -> PreparedPage<'_> {
    let mut record = record();
    record.expires_at = expires_at;
    let page = SearchPage {
        matches: record.matches[..1].to_vec(),
        gaps: Vec::new(),
        coverage: record.coverage.clone(),
        page_start: 0,
        page_end: 1,
        exhausted: false,
        continuation_token: Some("new".into()),
        expires_in_ms: None,
    };
    PreparedPage::new(catalog, page, Some(record))
}

#[test]
fn dropped_preparation_preserves_cursor_deadline_and_retained_allocation() {
    for size in [1, 100] {
        let mut catalog = catalog();
        let pointer = catalog.records["existing"].matches.as_ptr();
        let deadline = catalog.records["existing"].expires_at;
        let prepared = catalog.prepare_window_with_clock("existing", size, Instant::now).unwrap();
        assert_eq!(prepared.page().page_start, 1);
        drop(prepared);
        assert_eq!(catalog.records["existing"].next_index, 1);
        assert_eq!(catalog.records["existing"].expires_at, deadline);
        assert_eq!(catalog.records["existing"].matches.as_ptr(), pointer);
        assert_eq!(catalog.retained_matches, 4);
    }
}

#[test]
fn handle_or_diagnostic_refusal_does_not_skip_an_undelivered_page() {
    for reason in ["DIRECT_RESULT_HANDLE_CAPACITY_EXCEEDED", "STORAGE_INSPECT_FAILED"] {
        let mut catalog = catalog();
        let expected = catalog.records["existing"].matches[1..3].to_vec();
        let prepared = catalog.prepare_window_with_clock("existing", 2, Instant::now).unwrap();
        assert_eq!(prepared.deliver(|_| Err(reason.into())), Err(reason.into()));
        assert_eq!(catalog.records["existing"].next_index, 1);
        let prepared = catalog.prepare_window_with_clock("existing", 2, Instant::now).unwrap();
        prepared.deliver(|page| {
            assert_eq!(page.matches, expected);
            assert_eq!((page.page_start, page.page_end), (1, 3));
            Ok(())
        }).unwrap();
        assert_eq!(catalog.records["existing"].next_index, 3);
    }
}

#[test]
fn failed_final_page_is_retained_until_a_complete_retry() {
    let mut catalog = catalog();
    let expected = catalog.records["existing"].matches[1..].to_vec();
    let prepared = catalog.prepare_window_with_clock("existing", 100, Instant::now).unwrap();
    assert!(prepared.page().exhausted);
    assert!(prepared.deliver(|_| Err("pre-output refusal".into())).is_err());
    assert_eq!(catalog.records["existing"].next_index, 1);
    assert_eq!(catalog.retained_matches, 4);
    let prepared = catalog.prepare_window_with_clock("existing", 100, Instant::now).unwrap();
    prepared.deliver(|page| {
        assert_eq!(page.matches, expected);
        assert!(page.exhausted);
        Ok(())
    }).unwrap();
    assert!(catalog.records.is_empty());
    assert_eq!(catalog.retained_matches, 0);
}

#[test]
fn aborted_first_page_leaves_no_new_token_or_window_charge() {
    for callback_error in [false, true] {
        let mut catalog = catalog();
        let prepared = new_page(&mut catalog, Instant::now() + CONTINUATION_TTL);
        if callback_error {
            assert!(prepared.deliver(|_| Err("before completion".into())).is_err());
        } else {
            drop(prepared);
        }
        assert_eq!(catalog.records.len(), 1);
        assert_eq!(catalog.retained_matches, 4);
        assert!(!catalog.records.contains_key("new"));
        assert_eq!(catalog.records["existing"].next_index, 1);
    }
}

#[test]
fn successful_first_page_publishes_exact_window_once() {
    let mut catalog = catalog();
    let deadline = Instant::now() + CONTINUATION_TTL;
    new_page(&mut catalog, deadline).deliver(|page| {
        assert_eq!(page.continuation_token.as_deref(), Some("new"));
        assert_eq!(page.page_end, 1);
        assert!(page.expires_in_ms.is_some());
        Ok(())
    }).unwrap();
    assert_eq!(catalog.records.len(), 2);
    assert_eq!(catalog.retained_matches, 8);
    assert_eq!(catalog.records["new"].expires_at, deadline);
    assert_eq!(catalog.records["new"].next_index, 1);
    assert_eq!(catalog.records["existing"].next_index, 1);
}

#[test]
fn late_expiry_of_existing_or_new_page_prevents_the_output_callback() {
    for first_page in [false, true] {
        let mut catalog = catalog();
        let deadline = catalog.records["existing"].expires_at;
        let prepared = if first_page {
            new_page(&mut catalog, deadline)
        } else {
            catalog.prepare_window_with_clock("existing", 100, Instant::now).unwrap()
        };
        let result = prepared.finish_with_clock(|| deadline, |_| {
            panic!("expired page must not start output")
        });
        assert!(matches!(
            result,
            Err(prepared::DeliveryError::Continuation(ContinuationError::Expired)),
        ));
        assert!(!catalog.records.contains_key("new"));
        assert_eq!(catalog.records.contains_key("existing"), first_page);
        assert_eq!(catalog.retained_matches, if first_page { 4 } else { 0 });
    }
}

#[test]
fn cleanup_sweep_does_not_change_the_target_before_success() {
    let mut catalog = catalog();
    let mut expired = record();
    expired.expires_at = Instant::now();
    catalog.records.insert("expired".into(), expired);
    catalog.retained_matches += 4;
    let prepared = catalog.prepare_window_with_clock("existing", 1, Instant::now).unwrap();
    assert!(prepared.deliver(|_| Err("declined".into())).is_err());
    assert!(!catalog.records.contains_key("expired"));
    assert_eq!(catalog.records["existing"].next_index, 1);
    assert_eq!(catalog.retained_matches, 4);
}

#[test]
fn output_failure_commits_nothing_even_after_a_partial_frame() {
    use std::io::Write;
    let mut catalog = catalog();
    let mut bytes = Vec::new();
    let prepared = catalog.prepare_window_with_clock("existing", 2, Instant::now).unwrap();
    let result = prepared.deliver(|_| {
        bytes.write_all(b"partial frame").unwrap();
        Err("SERVICE_OUTPUT_FAILED".into())
    });
    assert_eq!(result, Err("SERVICE_OUTPUT_FAILED".into()));
    assert!(!bytes.is_empty());
    assert_eq!(catalog.records["existing"].next_index, 1);
    // Actual SessionOutput/serve must now terminate and invalidate the session
    // on this error after output started. This state is not permission to replay bytes.
}

#[test]
fn output_success_is_not_reclassified_as_an_error_after_delivery() {
    let mut catalog = catalog();
    let deadline = catalog.records["existing"].expires_at;
    let prepared = catalog.prepare_window_with_clock("existing", 100, Instant::now).unwrap();
    let mut observations = 0;
    let result = prepared.finish_with_clock(|| {
        observations += 1;
        if observations == 1 { deadline - std::time::Duration::from_secs(1) } else { deadline }
    }, |_| Ok(()));
    assert!(result.is_ok());
    assert_eq!(observations, 1);
    assert!(catalog.records.is_empty());
}

#[test]
fn bounded_pages_keep_their_exact_order_across_preparation_refusals() {
    for size in 1..=4 {
        let mut catalog = catalog();
        let expected = catalog.records["existing"].matches[1..].to_vec();
        let mut emitted = Vec::new();
        while catalog.records.contains_key("existing") {
            let declined = catalog.prepare_window_with_clock("existing", size, Instant::now).unwrap();
            drop(declined);
            let prepared = catalog.prepare_window_with_clock("existing", size, Instant::now).unwrap();
            prepared.deliver(|page| {
                emitted.extend(page.matches.iter().cloned());
                Ok(())
            }).unwrap();
        }
        assert_eq!(emitted, expected);
        assert_eq!(catalog.retained_matches, 0);
    }
}

//! Synthetic unpublished-handle and token-reservation tests, not entropy proof.

use super::*;

fn record(deadline: Instant) -> ResultHandleRecord {
    ResultHandleRecord {
        namespace_id: "fixture-namespace".into(),
        session_tag: 1,
        source_fence_digest: "fixture-fence".into(),
        source_id: "fixture-source".into(),
        revision_id: "fixture-revision".into(),
        content_digest: "fixture-content".into(),
        byte_length: 16,
        expires_at: deadline,
    }
}

fn catalog() -> ResultHandleCatalog {
    ResultHandleCatalog {
        namespace_id: "fixture-namespace".into(),
        session_tag: 1,
        entropy_poisoned: false,
        records: BTreeMap::from([
            (sha256::hex(&[1; 32]), record(Instant::now() + RESULT_HANDLE_TTL)),
        ]),
    }
}

fn staged(catalog: &mut ResultHandleCatalog, deadline: Instant) -> PreparedHandles<'_> {
    let token = sha256::hex(&[2; 32]);
    let public = PublicHandledMatch {
        source_handle: token.clone(),
        evidence_id: "fixture-evidence".into(),
        byte_start: 1,
        byte_end: 2,
        line: 1,
        column_bytes: 2,
        source_byte_length: 16,
        expires_in_ms: 0,
    };
    PreparedHandles::new(catalog, vec![(token, record(deadline), public)], deadline)
}

#[test]
fn dropped_handle_batch_publishes_nothing() {
    let mut catalog = catalog();
    let prepared = staged(&mut catalog, Instant::now() + RESULT_HANDLE_TTL);
    assert_eq!(prepared.matches().len(), 1);
    drop(prepared);
    assert_eq!(catalog.records.len(), 1);
    assert!(!catalog.records.contains_key(&sha256::hex(&[2; 32])));
}

#[test]
fn late_handle_expiry_does_not_leave_any_unpublished_records() {
    let mut catalog = catalog();
    let deadline = Instant::now() + RESULT_HANDLE_TTL;
    let mut prepared = staged(&mut catalog, deadline);
    assert_eq!(prepared.revalidate_at(deadline), Err(ResultHandleError::Expired));
    drop(prepared);
    assert_eq!(catalog.records.len(), 1);
    assert!(catalog.records.contains_key(&sha256::hex(&[1; 32])));
}

#[test]
fn valid_batch_commits_exact_provenance_without_renewing_its_deadline() {
    let mut catalog = catalog();
    let deadline = Instant::now() + RESULT_HANDLE_TTL;
    let mut prepared = staged(&mut catalog, deadline);
    prepared.revalidate_at(deadline - std::time::Duration::from_secs(2)).unwrap();
    assert_eq!(prepared.matches()[0].expires_in_ms, 2_000);
    prepared.revalidate_at(deadline - std::time::Duration::from_secs(1)).unwrap();
    assert_eq!(prepared.matches()[0].expires_in_ms, 1_000);
    let public = prepared.commit();
    assert_eq!(public.len(), 1);
    assert_eq!(catalog.records.len(), 2);
    let retained = &catalog.records[&public[0].source_handle];
    assert_eq!(retained.expires_at, deadline);
    assert_eq!(retained.source_id, "fixture-source");
    assert_eq!(retained.revision_id, "fixture-revision");
}

#[test]
fn empty_batch_has_no_expiring_handles_or_insertions() {
    let mut catalog = catalog();
    let past = Instant::now();
    let mut prepared = PreparedHandles::new(&mut catalog, Vec::new(), past);
    assert_eq!(prepared.revalidate_at(past), Ok(()));
    assert!(prepared.commit().is_empty());
    assert_eq!(catalog.records.len(), 1);
}

#[test]
fn allocation_rejects_both_committed_and_same_batch_collisions() {
    let catalog = catalog();
    let reserved = BTreeSet::from([sha256::hex(&[2; 32])]);
    let mut values = [[1; 32], [2; 32], [3; 32]].into_iter();
    let allocated = catalog.allocate_token(&reserved, &mut || Ok(values.next().unwrap())).unwrap();
    assert_eq!(allocated, sha256::hex(&[3; 32]));
    assert!(values.next().is_none());
    assert_eq!(catalog.records.len(), 1);
}

#[test]
fn repeated_same_batch_collision_exhausts_a_finite_budget() {
    let catalog = catalog();
    let reserved = BTreeSet::from([sha256::hex(&[2; 32])]);
    let mut calls = 0;
    let result = catalog.allocate_token(&reserved, &mut || {
        calls += 1;
        Ok([2; 32])
    });
    assert_eq!(result, Err(ResultHandleError::TokenExhausted));
    assert_eq!(calls, 128);
    assert_eq!(catalog.records.len(), 1);
}

#[test]
fn entropy_failure_after_a_collision_does_not_publish_a_partial_batch() {
    let catalog = catalog();
    let reserved = BTreeSet::from([sha256::hex(&[2; 32])]);
    let mut calls = 0;
    let result = catalog.allocate_token(&reserved, &mut || {
        calls += 1;
        if calls == 1 { Ok([2; 32]) } else { Err(ResultHandleError::EntropyUnavailable) }
    });
    assert_eq!(result, Err(ResultHandleError::EntropyUnavailable));
    assert_eq!(calls, 2);
    assert_eq!(catalog.records.len(), 1);
}

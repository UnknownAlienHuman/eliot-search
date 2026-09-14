use super::*;
use std::fs;
use std::path::PathBuf;
use std::sync::atomic::{AtomicU64, Ordering};
use std::time::{SystemTime, UNIX_EPOCH};

use crate::development::DataRootGuard;
use crate::direct_store::{
    DirectStore, SourceSummary, StoreSearchResult, StoredMatch,
};
use crate::revision_protection::{
    TestCredentialGuard, lock_unit_vault_for_test,
};

static NEXT: AtomicU64 = AtomicU64::new(0);

struct Fixture {
    base: PathBuf,
    credentials: TestCredentialGuard,
    _owner: DataRootGuard,
    store: DirectStore,
}

impl Fixture {
    fn new(tag: &str, initial: &[u8]) -> Self {
        let stamp = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .map_or(0, |value| value.as_nanos());
        let base = std::env::temp_dir().join(format!(
            "eliot-t21-continuation-{tag}-{}-{stamp}-{}",
            std::process::id(),
            NEXT.fetch_add(1, Ordering::Relaxed),
        ));
        fs::create_dir_all(base.join("data")).unwrap();
        fs::create_dir_all(base.join("sources")).unwrap();
        let credentials = TestCredentialGuard::for_data_root(&base.join("data"));
        let owner = DataRootGuard::acquire(&base.join("data")).unwrap();
        let mut store = DirectStore::open(owner.canonical_root()).unwrap();
        let first = base.join("sources").join("first.txt");
        fs::write(&first, initial).unwrap();
        store.index_file(&first).unwrap();
        Self {
            base,
            credentials,
            _owner: owner,
            store,
        }
    }

    fn add_source(&mut self, name: &str, contents: &[u8]) {
        let path = self.base.join("sources").join(name);
        fs::write(&path, contents).unwrap();
        self.store.index_file(&path).unwrap();
    }

    fn namespace(&self) -> String {
        self.store.namespace_id()
    }
}

impl Drop for Fixture {
    fn drop(&mut self) {
        self.credentials.cleanup();
        let _ = fs::remove_dir_all(&self.base);
    }
}

fn test_match(
    summary: &SourceSummary,
    byte_start: usize,
    byte_end: usize,
    index: usize,
) -> StoredMatch {
    StoredMatch {
        source_id: summary.source_id.clone(),
        revision_id: summary.revision_id.clone(),
        content_digest: summary.content_digest.clone(),
        path_digest: summary.path_digest.clone(),
        evidence_id: format!("evidence-{index}"),
        byte_start,
        byte_end,
        line: 1,
        column_bytes: 1,
    }
}

fn search_result(matches: Vec<StoredMatch>) -> StoreSearchResult {
    StoreSearchResult {
        matches,
        gaps: Vec::new(),
        registered_sources: 1,
        active_sources: 1,
        searched_sources: 1,
        complete: true,
        match_limit_reached: false,
    }
}

fn three_matches(fixture: &Fixture) -> Vec<StoredMatch> {
    let summaries = fixture.store.list_sources();
    let summary = summaries.first().unwrap();
    vec![
        test_match(summary, 0, 6, 0),
        test_match(summary, 7, 13, 1),
        test_match(summary, 14, 20, 2),
    ]
}

fn is_opaque_token(token: &str) -> bool {
    token.len() == 64 && token.bytes().all(|byte| byte.is_ascii_hexdigit())
}

#[test]
fn tokens_are_unique_opaque_session_binders() {
    let _vault = lock_unit_vault_for_test();
    let fixture = Fixture::new("unique", b"needle one\nneedle two\nneedle three\n");
    let namespace = fixture.namespace();
    let mut first = ContinuationCatalog::new(&namespace);
    let mut second = ContinuationCatalog::new(&namespace);
    let mut tokens = Vec::new();
    for _ in 0..4 {
        for catalog in [&mut first, &mut second] {
            let page = catalog
                .create_page(
                    &fixture.store,
                    search_result(three_matches(&fixture)),
                    1,
                )
                .unwrap();
            let token = page.continuation_token.unwrap();
            assert!(is_opaque_token(&token), "{token}");
            tokens.push(token);
        }
    }
    tokens.sort();
    tokens.dedup();
    assert_eq!(tokens.len(), 8);
}

#[test]
fn possession_does_not_cross_session_or_unknown_token_boundaries() {
    let _vault = lock_unit_vault_for_test();
    let fixture = Fixture::new("session", b"needle one\nneedle two\nneedle three\n");
    let namespace = fixture.namespace();
    let mut first = ContinuationCatalog::new(&namespace);
    let mut second = ContinuationCatalog::new(&namespace);
    let page = first
        .create_page(
            &fixture.store,
            search_result(three_matches(&fixture)),
            1,
        )
        .unwrap();
    let raw = page.continuation_token.unwrap();
    assert_eq!(
        second.continue_page(&fixture.store, &raw, 1),
        Err(ContinuationError::NotFound),
    );
    let mut tampered = raw.clone();
    let first_byte = tampered.remove(0);
    tampered.insert(0, if first_byte == '0' { '1' } else { '0' });
    assert_eq!(
        first.continue_page(&fixture.store, &tampered, 1),
        Err(ContinuationError::NotFound),
    );
    assert_eq!(
        first.continue_page(&fixture.store, "", 1),
        Err(ContinuationError::NotFound),
    );
    assert_eq!(first.live_count(), 1);
}

#[test]
fn foreign_namespace_and_source_drift_drop_the_whole_window() {
    let _vault = lock_unit_vault_for_test();
    let mut fixture = Fixture::new("root-a", b"needle one\nneedle two\nneedle three\n");
    let foreign = Fixture::new("root-b", b"needle one\nneedle two\nneedle three\n");
    let mut catalog = ContinuationCatalog::new(&fixture.namespace());
    assert_eq!(
        catalog.create_page(
            &foreign.store,
            search_result(three_matches(&foreign)),
            1,
        ),
        Err(ContinuationError::SourceFenceChanged),
    );
    let page = catalog
        .create_page(
            &fixture.store,
            search_result(three_matches(&fixture)),
            1,
        )
        .unwrap();
    let token = page.continuation_token.unwrap();
    fixture.add_source("second.txt", b"needle four\n");
    assert_eq!(
        catalog.continue_page(&fixture.store, &token, 1),
        Err(ContinuationError::SourceFenceChanged),
    );
    assert_eq!(
        catalog.continue_page(&fixture.store, &token, 1),
        Err(ContinuationError::NotFound),
    );
    assert_eq!(catalog.retained_matches(), 0);
}

#[test]
fn expiry_reports_once_and_releases_pins() {
    let _vault = lock_unit_vault_for_test();
    let fixture = Fixture::new("expiry", b"needle one\nneedle two\nneedle three\n");
    let mut catalog = ContinuationCatalog::new(&fixture.namespace());
    let page = catalog
        .create_page(
            &fixture.store,
            search_result(three_matches(&fixture)),
            1,
        )
        .unwrap();
    let token = page.continuation_token.unwrap();
    assert!(catalog.force_expire_for_tests(&token));
    assert_eq!(
        catalog.continue_page(&fixture.store, &token, 1),
        Err(ContinuationError::Expired),
    );
    assert_eq!(catalog.live_count(), 0);
    assert_eq!(catalog.retained_matches(), 0);
}

#[test]
fn every_live_barrier_denial_drops_the_window() {
    let _vault = lock_unit_vault_for_test();
    let fixture = Fixture::new("barriers", b"needle one\nneedle two\nneedle three\n");
    for (barrier, expected) in [
        (
            LiveExpansionBarrier {
                generation_moved: false,
                access_revoked: true,
                purged: false,
            },
            ContinuationError::AccessRevoked,
        ),
        (
            LiveExpansionBarrier {
                generation_moved: false,
                access_revoked: false,
                purged: true,
            },
            ContinuationError::Purged,
        ),
        (
            LiveExpansionBarrier {
                generation_moved: true,
                access_revoked: false,
                purged: false,
            },
            ContinuationError::SourceFenceChanged,
        ),
    ] {
        let mut catalog = ContinuationCatalog::new(&fixture.namespace());
        let page = catalog
            .create_page(
                &fixture.store,
                search_result(three_matches(&fixture)),
                1,
            )
            .unwrap();
        let token = page.continuation_token.unwrap();
        assert_eq!(
            catalog.continue_page_with_live_barrier(
                &fixture.store,
                &token,
                1,
                barrier,
            ),
            Err(expected),
        );
        assert_eq!(catalog.live_count(), 0);
        assert_eq!(catalog.retained_matches(), 0);
    }
}

#[test]
fn page_bounds_and_exact_final_page_persist_no_history() {
    let _vault = lock_unit_vault_for_test();
    let fixture = Fixture::new("pages", b"needle one\nneedle two\nneedle three\n");
    let mut catalog = ContinuationCatalog::new(&fixture.namespace());
    assert_eq!(
        catalog.create_page(
            &fixture.store,
            search_result(three_matches(&fixture)),
            0,
        ),
        Err(ContinuationError::InvalidPageSize),
    );
    assert_eq!(
        catalog.create_page(
            &fixture.store,
            search_result(three_matches(&fixture)),
            MAX_PAGE_SIZE + 1,
        ),
        Err(ContinuationError::InvalidPageSize),
    );
    let summaries = fixture.store.list_sources();
    let summary = summaries.first().unwrap();
    let exact = vec![test_match(summary, 0, 6, 0), test_match(summary, 7, 13, 1)];
    let page = catalog
        .create_page(&fixture.store, search_result(exact), 2)
        .unwrap();
    assert!(page.exhausted);
    assert!(page.continuation_token.is_none());
    assert_eq!(catalog.live_count(), 0);
}

#[test]
fn capacity_exhaustion_and_final_pages_release_every_pin() {
    let _vault = lock_unit_vault_for_test();
    let fixture = Fixture::new("capacity", b"needle one\nneedle two\nneedle three\n");
    let mut catalog = ContinuationCatalog::new(&fixture.namespace());
    let mut tokens = Vec::new();
    for _ in 0..MAX_CONTINUATIONS {
        let page = catalog
            .create_page(
                &fixture.store,
                search_result(three_matches(&fixture)),
                1,
            )
            .unwrap();
        tokens.push(page.continuation_token.unwrap());
    }
    assert_eq!(
        catalog.create_page(
            &fixture.store,
            search_result(three_matches(&fixture)),
            1,
        ),
        Err(ContinuationError::CapacityExceeded),
    );
    for token in tokens {
        assert!(catalog.continue_page(&fixture.store, &token, 10).unwrap().exhausted);
    }
    assert_eq!(catalog.live_count(), 0);
    assert_eq!(catalog.retained_matches(), 0);
}

#[test]
fn restart_and_invalidate_forget_all_tokens() {
    let _vault = lock_unit_vault_for_test();
    let fixture = Fixture::new("restart", b"needle one\nneedle two\nneedle three\n");
    let namespace = fixture.namespace();
    let mut first = ContinuationCatalog::new(&namespace);
    let token = first
        .create_page(
            &fixture.store,
            search_result(three_matches(&fixture)),
            1,
        )
        .unwrap()
        .continuation_token
        .unwrap();
    drop(first);
    let mut restarted = ContinuationCatalog::new(&namespace);
    assert_eq!(
        restarted.continue_page(&fixture.store, &token, 1),
        Err(ContinuationError::NotFound),
    );

    let mut tokens = Vec::new();
    for _ in 0..3 {
        tokens.push(
            restarted
                .create_page(
                    &fixture.store,
                    search_result(three_matches(&fixture)),
                    1,
                )
                .unwrap()
                .continuation_token
                .unwrap(),
        );
    }
    assert_eq!(restarted.invalidate_all(), 3);
    assert_eq!(restarted.invalidate_all(), 0);
    for token in tokens {
        assert_eq!(
            restarted.continue_page(&fixture.store, &token, 1),
            Err(ContinuationError::NotFound),
        );
    }
}

#[test]
fn debug_redacts_tokens_and_entropy_is_256_bit() {
    let _vault = lock_unit_vault_for_test();
    let fixture = Fixture::new("redact", b"needle one\nneedle two\nneedle three\n");
    let mut catalog = ContinuationCatalog::new(&fixture.namespace());
    let token = catalog
        .create_page(
            &fixture.store,
            search_result(three_matches(&fixture)),
            1,
        )
        .unwrap()
        .continuation_token
        .unwrap();
    let rendered = format!("{catalog:?}");
    assert!(rendered.contains("live_windows"));
    assert!(!rendered.contains(&token));

    let first = qualified_entropy_32().unwrap();
    let second = qualified_entropy_32().unwrap();
    assert_eq!(first.len(), 32);
    assert_ne!(first, second);
}

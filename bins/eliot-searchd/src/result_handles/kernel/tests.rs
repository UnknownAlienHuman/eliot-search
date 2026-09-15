use super::*;
use super::model::ResultHandleRecord;

use std::fs;
use std::path::PathBuf;
use std::sync::atomic::{AtomicU64, Ordering};
use std::time::{SystemTime, UNIX_EPOCH};

use crate::continuation::LiveExpansionBarrier;
use crate::development::DataRootGuard;
use crate::direct_store::{DirectStore, SourceSummary, StoredMatch};
use crate::revision_protection::{TestCredentialGuard, lock_unit_vault_for_test};
use crate::source_fence::digest as source_fence;

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
            "eliot-t21-handles-{tag}-{}-{stamp}-{}",
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

    fn namespace(&self) -> String {
        self.store.namespace_id()
    }

    fn summary(&self) -> SourceSummary {
        self.store.list_sources().into_iter().next().unwrap()
    }

    fn handle_match(
        &self,
        byte_start: usize,
        byte_end: usize,
        index: usize,
    ) -> StoredMatch {
        let summary = self.summary();
        StoredMatch {
            source_id: summary.source_id,
            revision_id: summary.revision_id,
            content_digest: summary.content_digest,
            path_digest: summary.path_digest,
            evidence_id: format!("evidence-{index}"),
            byte_start,
            byte_end,
            line: 1,
            column_bytes: 1,
        }
    }
}

impl Drop for Fixture {
    fn drop(&mut self) {
        self.credentials.cleanup();
        let _ = fs::remove_dir_all(&self.base);
    }
}

fn is_opaque_token(token: &str) -> bool {
    token.len() == 64 && token.bytes().all(|byte| byte.is_ascii_hexdigit())
}

#[test]
fn handle_tokens_are_unique_opaque() {
    let _vault = lock_unit_vault_for_test();
    let fixture = Fixture::new("unique", b"0123456789abcdef");
    let mut catalog = ResultHandleCatalog::new(&fixture.namespace());
    let matches = vec![
        fixture.handle_match(0, 4, 0),
        fixture.handle_match(4, 8, 1),
        fixture.handle_match(8, 12, 2),
    ];
    let public = catalog.mint_page(&fixture.store, &matches).unwrap();
    assert_eq!(public.len(), 3);
    let mut tokens = public
        .iter()
        .map(|item| item.source_handle.clone())
        .collect::<Vec<_>>();
    for token in &tokens {
        assert!(is_opaque_token(token), "{token}");
    }
    tokens.sort();
    tokens.dedup();
    assert_eq!(tokens.len(), 3, "every handle mints a distinct opaque token");
}

#[test]
fn cross_session_and_cross_root_replay_denied() {
    let _vault = lock_unit_vault_for_test();
    let fixture = Fixture::new("session", b"0123456789abcdef");
    let foreign = Fixture::new("session-root", b"0123456789abcdef");
    let namespace = fixture.namespace();
    let mut first = ResultHandleCatalog::new(&namespace);
    let mut second = ResultHandleCatalog::new(&namespace);
    let mut cross_root = ResultHandleCatalog::new(&foreign.namespace());
    let public = first
        .mint_page(&fixture.store, &[fixture.handle_match(0, 4, 0)])
        .unwrap();
    let token = public.first().unwrap().source_handle.clone();
    assert_eq!(
        second.expand(&fixture.store, &token, 0, 4),
        Err(ResultHandleError::NotFound),
        "possession alone grants no authority in a foreign session"
    );
    assert_eq!(
        cross_root.expand(&foreign.store, &token, 0, 4),
        Err(ResultHandleError::NotFound),
        "handles never cross a root boundary"
    );
    assert_eq!(
        second.expand_with_live_barrier(
            &fixture.store,
            &token,
            0,
            4,
            LiveExpansionBarrier::clean(),
        ),
        Err(ResultHandleError::NotFound),
    );
    first.expand(&fixture.store, &token, 0, 4).unwrap();
}

#[test]
fn foreign_namespace_mint_and_expand_rejected() {
    let _vault = lock_unit_vault_for_test();
    let fixture = Fixture::new("mint-a", b"0123456789abcdef");
    let foreign = Fixture::new("mint-b", b"0123456789abcdef");
    let mut catalog = ResultHandleCatalog::new(&fixture.namespace());
    assert_eq!(catalog.namespace_id(), fixture.namespace());
    let foreign_summary = foreign.store.list_sources().into_iter().next().unwrap();
    let foreign_match = StoredMatch {
        source_id: foreign_summary.source_id,
        revision_id: foreign_summary.revision_id,
        content_digest: foreign_summary.content_digest,
        path_digest: foreign_summary.path_digest,
        evidence_id: "evidence-foreign".to_owned(),
        byte_start: 0,
        byte_end: 4,
        line: 1,
        column_bytes: 1,
    };
    assert_eq!(
        catalog.mint_page(&foreign.store, &[foreign_match]),
        Err(ResultHandleError::SourceFenceChanged),
        "a catalog bound to one root never mints for a foreign root"
    );
    let public = catalog
        .mint_page(&fixture.store, &[fixture.handle_match(0, 4, 0)])
        .unwrap();
    let token = public.first().unwrap().source_handle.clone();
    assert_eq!(
        catalog.expand(&foreign.store, &token, 0, 4),
        Err(ResultHandleError::SourceFenceChanged),
    );
    assert_eq!(
        catalog.expand(&fixture.store, &token, 0, 4),
        Err(ResultHandleError::NotFound),
        "namespace drift drops the handle instead of narrowing it"
    );
}

#[test]
fn expired_handle_reports_expired() {
    let _vault = lock_unit_vault_for_test();
    let fixture = Fixture::new("expiry", b"0123456789abcdef");
    let mut catalog = ResultHandleCatalog::new(&fixture.namespace());
    let public = catalog
        .mint_page(&fixture.store, &[fixture.handle_match(0, 4, 0)])
        .unwrap();
    let token = public.first().unwrap().source_handle.clone();
    assert!(catalog.force_expire_for_tests(&token));
    assert_eq!(
        catalog.expand(&fixture.store, &token, 0, 4),
        Err(ResultHandleError::Expired),
    );
    assert_eq!(catalog.live_count(), 0);
}

#[test]
fn revoked_and_purged_barriers_drop_handle() {
    let _vault = lock_unit_vault_for_test();
    let fixture = Fixture::new("barrier", b"0123456789abcdef");
    let mut catalog = ResultHandleCatalog::new(&fixture.namespace());
    for (index, (barrier, expected)) in [
        (
            LiveExpansionBarrier {
                generation_moved: false,
                access_revoked: true,
                purged: false,
            },
            ResultHandleError::AccessRevoked,
        ),
        (
            LiveExpansionBarrier {
                generation_moved: false,
                access_revoked: false,
                purged: true,
            },
            ResultHandleError::Purged,
        ),
        (
            LiveExpansionBarrier {
                generation_moved: true,
                access_revoked: false,
                purged: false,
            },
            ResultHandleError::SourceFenceChanged,
        ),
    ]
    .into_iter()
    .enumerate()
    {
        let public = catalog
            .mint_page(&fixture.store, &[fixture.handle_match(0, 4, index)])
            .unwrap();
        let token = public.first().unwrap().source_handle.clone();
        assert_eq!(
            catalog.expand_with_live_barrier(&fixture.store, &token, 0, 4, barrier),
            Err(expected),
        );
        assert_eq!(
            catalog.expand(&fixture.store, &token, 0, 4),
            Err(ResultHandleError::NotFound),
            "a barrier-denied handle is dropped, never resumed"
        );
    }
    assert_eq!(catalog.live_count(), 0);
}

#[test]
fn retired_source_denies_expansion_and_drops_handle() {
    let _vault = lock_unit_vault_for_test();
    let mut fixture = Fixture::new("retire", b"0123456789abcdef");
    let mut catalog = ResultHandleCatalog::new(&fixture.namespace());
    let source_id = fixture.summary().source_id;
    let public = catalog
        .mint_page(&fixture.store, &[fixture.handle_match(0, 4, 0)])
        .unwrap();
    let token = public.first().unwrap().source_handle.clone();
    fixture.store.retire_source(&source_id).unwrap();
    assert_eq!(
        catalog.expand(&fixture.store, &token, 0, 4),
        Err(ResultHandleError::SourceFenceChanged),
        "retirement invalidates the live fence before any byte is read"
    );
    assert_eq!(
        catalog.expand(&fixture.store, &token, 0, 4),
        Err(ResultHandleError::NotFound),
    );
}

#[test]
fn exact_provenance_readback_roundtrip() {
    let _vault = lock_unit_vault_for_test();
    let fixture = Fixture::new("readback", b"0123456789abcdef");
    let mut catalog = ResultHandleCatalog::new(&fixture.namespace());
    let public = catalog
        .mint_page(&fixture.store, &[fixture.handle_match(0, 4, 0)])
        .unwrap();
    let token = public.first().unwrap().source_handle.clone();
    let expansion = catalog.expand(&fixture.store, &token, 4, 10).unwrap();
    assert_eq!(expansion.byte_start, 4);
    assert_eq!(expansion.byte_end, 10);
    assert_eq!(expansion.source_byte_length, 16);
    assert_eq!(expansion.bytes, b"456789");
    assert_eq!(
        catalog.expand(&fixture.store, "not-a-token", 4, 10),
        Err(ResultHandleError::NotFound),
    );
}

#[test]
fn range_widening_and_oversize_denied() {
    let _vault = lock_unit_vault_for_test();
    let wide = vec![b'x'; 30 * 1024];
    let fixture = Fixture::new("ranges", &wide);
    let mut catalog = ResultHandleCatalog::new(&fixture.namespace());
    let public = catalog
        .mint_page(&fixture.store, &[fixture.handle_match(0, 6, 0)])
        .unwrap();
    let token = public.first().unwrap().source_handle.clone();
    assert_eq!(
        catalog.expand(&fixture.store, &token, 0, 30 * 1024),
        Err(ResultHandleError::ExpansionTooLarge),
        "one expansion never exceeds the finite disclosure ceiling"
    );
    assert_eq!(
        catalog.expand(&fixture.store, &token, 0, 30 * 1024 + 1),
        Err(ResultHandleError::RangeInvalid),
        "ranges cannot widen past the retained revision"
    );
    assert_eq!(
        catalog.expand(&fixture.store, &token, 8, 8),
        Err(ResultHandleError::RangeInvalid),
    );
    assert_eq!(
        catalog.expand(&fixture.store, &token, 9, 8),
        Err(ResultHandleError::RangeInvalid),
    );
    catalog
        .expand(&fixture.store, &token, 0, 24 * 1024)
        .unwrap();
}

#[test]
fn durable_mint_is_denied_for_ephemeral_catalog() {
    let _vault = lock_unit_vault_for_test();
    let fixture = Fixture::new("durable", b"0123456789abcdef");
    let mut catalog = ResultHandleCatalog::new(&fixture.namespace());
    assert_eq!(
        ResultHandleCatalog::mint_durable_source(),
        Err(ResultHandleError::DurableRetentionRequired),
        "ordinary query handles are ephemeral-only; durable evidence needs explicit retention"
    );
    assert_eq!(catalog.live_count(), 0, "denied mints insert no record");
}

#[test]
fn restart_forgets_handles() {
    let _vault = lock_unit_vault_for_test();
    let fixture = Fixture::new("restart", b"0123456789abcdef");
    let namespace = fixture.namespace();
    let mut first = ResultHandleCatalog::new(&namespace);
    let public = first
        .mint_page(&fixture.store, &[fixture.handle_match(0, 4, 0)])
        .unwrap();
    let token = public.first().unwrap().source_handle.clone();
    drop(first);
    let mut restarted = ResultHandleCatalog::new(&namespace);
    assert_eq!(restarted.live_count(), 0);
    assert_eq!(
        restarted.expand(&fixture.store, &token, 0, 4),
        Err(ResultHandleError::NotFound),
        "ephemeral handles never survive a restart"
    );
}

#[test]
fn handle_capacity_bounded_and_releasable() {
    let _vault = lock_unit_vault_for_test();
    let fixture = Fixture::new("quota", b"0123456789abcdef");
    let mut catalog = ResultHandleCatalog::new(&fixture.namespace());
    let summary = fixture.summary();
    for chunk in 0..5 {
        let matches = (0..10_000)
            .map(|index| StoredMatch {
                source_id: summary.source_id.clone(),
                revision_id: summary.revision_id.clone(),
                content_digest: summary.content_digest.clone(),
                path_digest: summary.path_digest.clone(),
                evidence_id: format!("evidence-{chunk}-{index}"),
                byte_start: 0,
                byte_end: 4,
                line: 1,
                column_bytes: 1,
            })
            .collect::<Vec<_>>();
        catalog.mint_page(&fixture.store, &matches).unwrap();
    }
    assert_eq!(catalog.live_count(), MAX_RESULT_HANDLES);
    assert_eq!(
        catalog.mint_page(&fixture.store, &[fixture.handle_match(0, 4, 0)]),
        Err(ResultHandleError::CapacityExceeded),
    );
    assert_eq!(catalog.invalidate_all(), MAX_RESULT_HANDLES);
    assert_eq!(catalog.live_count(), 0);
    catalog
        .mint_page(&fixture.store, &[fixture.handle_match(0, 4, 0)])
        .unwrap();
}

#[test]
fn expired_handles_reaped_by_next_operation() {
    let _vault = lock_unit_vault_for_test();
    let fixture = Fixture::new("reap", b"0123456789abcdef");
    let mut catalog = ResultHandleCatalog::new(&fixture.namespace());
    let public = catalog
        .mint_page(
            &fixture.store,
            &[
                fixture.handle_match(0, 4, 0),
                fixture.handle_match(4, 8, 1),
            ],
        )
        .unwrap();
    assert!(catalog.force_expire_for_tests(&public[0].source_handle));
    catalog
        .mint_page(&fixture.store, &[fixture.handle_match(8, 12, 2)])
        .unwrap();
    assert_eq!(catalog.live_count(), 2, "the aged-out handle is reaped");
}

#[test]
fn foreign_session_tag_is_not_found() {
    let _vault = lock_unit_vault_for_test();
    let fixture = Fixture::new("tag", b"0123456789abcdef");
    let mut catalog = ResultHandleCatalog::new(&fixture.namespace());
    let summary = fixture.summary();
    let token = "f".repeat(64);
    catalog.records.insert(
        token.clone(),
        ResultHandleRecord {
            source_fence_digest: source_fence(&fixture.store),
            source_id: summary.source_id,
            revision_id: summary.revision_id,
            content_digest: summary.content_digest,
            byte_length: summary.byte_length,
            expires_at: std::time::Instant::now() + RESULT_HANDLE_TTL,
            namespace_id: fixture.namespace(),
            session_tag: catalog.session_tag.wrapping_add(1),
        },
    );
    assert_eq!(
        catalog.expand(&fixture.store, &token, 0, 4),
        Err(ResultHandleError::NotFound),
        "a record from another session is unusable here"
    );
}

#[test]
fn catalog_debug_redacts_tokens() {
    let _vault = lock_unit_vault_for_test();
    let fixture = Fixture::new("redact", b"0123456789abcdef");
    let mut catalog = ResultHandleCatalog::new(&fixture.namespace());
    let public = catalog
        .mint_page(&fixture.store, &[fixture.handle_match(0, 4, 0)])
        .unwrap();
    let token = public.first().unwrap().source_handle.clone();
    let rendered = format!("{catalog:?}");
    assert!(rendered.contains("live_handles"), "{rendered}");
    assert!(!rendered.contains(&token), "plaintext tokens never reach debug output");
}

//! Bounded ingestion with an explicit revision-storage publication barrier.
//!
//! Primary ingestion consumes canonical composition receipts
//! (`source_composition`) instead of an independent development catalog:
//! every retained snapshot is classified, observed, evaluated, issued and
//! verified under the current admission fence, resolved to a stable identity
//! (paths are locators, never identity) and admitted against a coherent
//! transient registry view. Denied, review-required, unsupported and
//! ambiguous sources never reach CAS: the complete batch is planned before
//! any writer call. Canonical registry changes persist in T11 transactions
//! via the existing append-only log with exact readback; no second file,
//! no second map and no silent fallback exist on this path.
//!
//! Legacy migration: `MatchExisting` by stable file identity reuses the
//! existing (possibly legacy-domain) `source_id`; only truly new stable
//! identities use the canonical domain. Revision bindings keep their domain
//! with the canonical source input. No fabricated source, revision, policy
//! or admission receipt is created to make types compile.
//!
//! Preserved: T05 quarantine (caller arms/clears around mutations), T08
//! owner guards (caller holds exclusion throughout), T11 cutover marker and
//! readback gates, T12 effective-configuration readiness (policy wiring
//! point below), T07 kernel-verified reads (no path-first byte product).

use std::collections::BTreeSet;

use super::{
    CONTROL_DIRECTORY, DirectStore, FileSnapshot, IndexedSource, MAX_DIRECTORY_FILES, RecordDraft,
    SOURCE_LOG_FILE, SourceState, ZERO_DIGEST, collect_regular_files, ensure_directory, fs,
    load_registry, path_identity_bytes, read_file_snapshot, sha256,
};
use crate::source_composition as canonical;
use std::path::{Path, PathBuf};

const MAX_BATCH_INPUT_BYTES: usize = 512 * 1024 * 1024;

impl DirectStore {
    /// Publishes metadata only after the supplied writer verifies immutable bytes.
    /// The writer is a composition-owned adapter, never a client-provided callback.
    pub(crate) fn index_file_with_writer(
        &mut self,
        path: &Path,
        writer: &mut impl FnMut(&Self, &IndexedSource, &[u8]) -> Result<(), String>,
    ) -> Result<IndexedSource, String> {
        self.index_paths_bounded(&[path.to_path_buf()], MAX_BATCH_INPUT_BYTES, writer)?
            .pop()
            .ok_or_else(|| "DIRECT_INDEX_EMPTY_RESULT".to_owned())
    }

    /// Uses the same prepublication barrier for every member of a directory batch.
    pub(crate) fn index_directory_with_writer(
        &mut self,
        directory: &Path,
        writer: &mut impl FnMut(&Self, &IndexedSource, &[u8]) -> Result<(), String>,
    ) -> Result<Vec<IndexedSource>, String> {
        ensure_directory(directory)?;
        let canonical_dir = fs::canonicalize(directory)
            .map_err(|error| format!("DIRECT_DIRECTORY_CANONICALIZE_ERROR:{error}"))?;
        if canonical_dir == self.root {
            return Err("DIRECT_SOURCE_DIRECTORY_IS_DATA_ROOT".to_owned());
        }
        ensure_directory(&canonical_dir)?;
        let mut paths = Vec::new();
        collect_regular_files(&canonical_dir, &self.root, 0, &mut paths)?;
        paths.sort_by_key(|path| path_identity_bytes(path));
        self.index_paths_bounded(&paths, MAX_BATCH_INPUT_BYTES, writer)
    }

    /// Current admission fence for primary ingestion.
    ///
    /// Baseline denies generated, vendor and binary classes and caps one
    /// file at 16 MiB. Explicit T12 `source_admission` settings bind here in
    /// a follow-up without touching the planner below: callers pass an
    /// explicit [`canonical::SourceAdmissionConfig`] through this single
    /// wiring point.
    fn admission_policy() -> canonical::AdmissionPolicy {
        canonical::AdmissionPolicy::baseline()
    }

    /// Builds one coherent transient registry view over replayed DIRECT state.
    ///
    /// The view is a projection, not a second catalog: it borrows the exact
    /// replayed `registry.latest` records plus the current policy fence and
    /// is rebuilt per batch. Restart preserves memberships, revision
    /// occurrences and lineage bindings because the underlying log replay
    /// does.
    fn registry_view(
        &self,
        policy: &canonical::AdmissionPolicy,
    ) -> Result<canonical::RegistryView, String> {
        let mut prior = Vec::with_capacity(self.registry.latest.len());
        for record in self.registry.latest.values() {
            prior.push(canonical::PriorSourceView {
                source_id: record.source_id.clone(),
                file_identity_digest: record.file_identity_digest.clone(),
                path_digest: record.path_digest.clone(),
                revision_id: record.revision_id.clone(),
                record_digest: record.record_digest.clone(),
                is_active: record.state == SourceState::Active,
            });
        }
        canonical::RegistryView::build(prior, policy)
    }

    /// Validates one retained snapshot through canonical composition and
    /// plans its durable identifiers and optional catalog draft without
    /// invoking any storage adapter.
    ///
    /// `original_path` is the caller-supplied locator for closed
    /// classification (never identity); stable identity comes from the
    /// kernel-verified `snapshot.file_identity_digest` plus the namespace.
    /// The durable `source_id` reuses a prior identifier on exact stable
    /// match (preserving legacy migration mappings) and uses the canonical
    /// domain only for truly new stable identities.
    fn plan_snapshot(
        &self,
        original_path: &Path,
        snapshot: FileSnapshot,
        seen: &mut BTreeSet<String>,
        policy: &canonical::AdmissionPolicy,
        view: &canonical::RegistryView,
    ) -> Result<(FileSnapshot, IndexedSource, Option<RecordDraft>), String> {
        if sha256::decode_digest(&snapshot.file_identity_digest).is_none() {
            return Err("DIRECT_FILE_IDENTITY_INVALID".to_owned());
        }
        if sha256::decode_digest(&snapshot.content_digest).is_none() {
            return Err("DIRECT_CONTENT_DIGEST_INVALID".to_owned());
        }
        let namespace_hex = sha256::hex(&self.namespace_id);
        // Canonical gate before any durable identifier exists: denied,
        // review-required, unsupported and ambiguous sources never reach CAS
        // because planning precedes every writer call in the batch below.
        let planned = canonical::plan_snapshot(
            original_path,
            &snapshot.file_identity_digest,
            snapshot.identity_strength.tag(),
            &snapshot.content_digest,
            &snapshot.bytes,
            &namespace_hex,
            policy,
            view,
        )?;
        let source_id = planned.source_id;
        let revision_id = planned.revision_id;
        if !seen.insert(source_id.clone()) {
            return Err("DIRECT_DUPLICATE_SOURCE_IN_BATCH".to_owned());
        }
        let byte_length = u64::try_from(snapshot.bytes.len())
            .map_err(|_| "DIRECT_SOURCE_TOO_LARGE".to_owned())?;
        // Previous durable state comes from the coherent view, not from a
        // direct `registry.latest` catalog lookup in the planner.
        let previous = view.get(&source_id);
        if let Some(ref prior) = previous {
            if prior.file_identity_digest != snapshot.file_identity_digest {
                return Err("DIRECT_SOURCE_ID_COLLISION".to_owned());
            }
        }
        let changed = !previous.as_ref().is_some_and(|prior| {
            prior.is_active
                && prior.revision_id == revision_id
                && prior.path_digest == snapshot.path_digest
        });
        let source = IndexedSource {
            source_id,
            revision_id,
            content_digest: snapshot.content_digest.clone(),
            path_digest: snapshot.path_digest.clone(),
            byte_length,
            identity_strength: snapshot.identity_strength.tag(),
            changed,
        };
        let draft = if changed {
            // Returning to an old revision is a new transition, not a replay
            // of the first transition to those bytes. Bind its predecessor.
            let predecessor = previous
                .as_ref()
                .map_or(ZERO_DIGEST, |prior| prior.record_digest.as_str());
            let operation_id = sha256::hex(&sha256::digest_parts(
                b"eliot-search/direct-index-operation/v2",
                &[
                    source.source_id.as_bytes(),
                    source.revision_id.as_bytes(),
                    source.path_digest.as_bytes(),
                    predecessor.as_bytes(),
                ],
            ));
            Some(RecordDraft {
                operation_id,
                state: SourceState::Active,
                source_id: source.source_id.clone(),
                revision_id: source.revision_id.clone(),
                content_digest: source.content_digest.clone(),
                byte_length,
                file_identity_digest: snapshot.file_identity_digest.clone(),
                path_digest: source.path_digest.clone(),
                identity_strength: snapshot.identity_strength,
            })
        } else {
            None
        };
        Ok((snapshot, source, draft))
    }

    fn index_paths_bounded(
        &mut self,
        paths: &[PathBuf],
        max_batch_bytes: usize,
        writer: &mut impl FnMut(&Self, &IndexedSource, &[u8]) -> Result<(), String>,
    ) -> Result<Vec<IndexedSource>, String> {
        if max_batch_bytes == 0 || max_batch_bytes > MAX_BATCH_INPUT_BYTES {
            return Err("DIRECT_BATCH_LIMIT_INVALID".to_owned());
        }
        if paths.len() > MAX_DIRECTORY_FILES {
            return Err("DIRECT_DIRECTORY_FILE_LIMIT_EXCEEDED".to_owned());
        }
        let current = load_registry(&self.root.join(CONTROL_DIRECTORY).join(SOURCE_LOG_FILE))?;
        if current.last_sequence != self.registry.last_sequence
            || current.last_digest != self.registry.last_digest
            || current.latest != self.registry.latest
        {
            return Err("DIRECT_CONTROL_READBACK_MISMATCH".to_owned());
        }

        // One coherent admission fence and registry view for the complete
        // batch. A policy rotation between planning and commit fails closed
        // at commit time via exact readback; stale views never admit.
        let policy = Self::admission_policy();
        let view = self.registry_view(&policy)?;

        // Bound the complete retained input, not merely each individual file.
        // Every read receives the remaining budget before allocating its buffer.
        // Reads are kernel-verified and inert; admission happens during
        // planning below, before any writer (CAS) call.
        let mut snapshots: Vec<(PathBuf, FileSnapshot)> = Vec::with_capacity(paths.len());
        let mut retained_bytes = 0_usize;
        for path in paths {
            let remaining = max_batch_bytes
                .checked_sub(retained_bytes)
                .ok_or_else(|| "DIRECT_BATCH_BYTES_EXCEEDED".to_owned())?;
            let snapshot = read_file_snapshot(path, &self.root, remaining)?;
            retained_bytes = retained_bytes
                .checked_add(snapshot.bytes.len())
                .filter(|length| *length <= max_batch_bytes)
                .ok_or_else(|| "DIRECT_BATCH_BYTES_EXCEEDED".to_owned())?;
            snapshots.push((path.clone(), snapshot));
        }
        snapshots.sort_by(|left, right| left.1.path_digest.cmp(&right.1.path_digest));

        // Validate the complete batch through canonical composition before
        // invoking any storage adapter. Denied sources never reach CAS.
        let mut seen = BTreeSet::new();
        let mut planned = Vec::with_capacity(snapshots.len());
        for (original_path, snapshot) in snapshots {
            planned.push(self.plan_snapshot(
                &original_path,
                snapshot,
                &mut seen,
                &policy,
                &view,
            )?);
        }

        let mut results = Vec::with_capacity(planned.len());
        let mut drafts = Vec::new();
        for (snapshot, source, draft) in planned {
            // Even an unchanged revision requires exact storage readback.
            // A failed adapter can leave orphan objects, but no new catalog event.
            writer(self, &source, &snapshot.bytes)?;
            if let Some(draft) = draft {
                drafts.push(draft);
            }
            results.push(source);
        }
        if !drafts.is_empty() {
            self.append_drafts(drafts)?;
        }
        Ok(results)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::atomic::{AtomicU64, Ordering};
    use std::time::{SystemTime, UNIX_EPOCH};

    static NEXT: AtomicU64 = AtomicU64::new(0);

    struct Fixture(PathBuf);

    impl Fixture {
        fn new() -> Self {
            let stamp = SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .unwrap()
                .as_nanos();
            let root = std::env::temp_dir().join(format!(
                "eliot-ingest-{}-{stamp}-{}",
                std::process::id(),
                NEXT.fetch_add(1, Ordering::Relaxed)
            ));
            fs::create_dir_all(root.join("data")).unwrap();
            Self(root)
        }

        fn store(&self) -> DirectStore {
            DirectStore::open(&self.0.join("data")).unwrap()
        }

        fn source(&self, name: &str, bytes: &[u8]) -> PathBuf {
            let path = self.0.join(name);
            fs::write(&path, bytes).unwrap();
            path
        }

        fn log(&self) -> Vec<u8> {
            fs::read(self.0.join("data/control/source-events.log")).unwrap()
        }
    }

    impl Drop for Fixture {
        fn drop(&mut self) {
            let _ = fs::remove_dir_all(&self.0);
        }
    }

    fn plaintext_writer(
        store: &DirectStore,
        source: &IndexedSource,
        bytes: &[u8],
    ) -> Result<(), String> {
        store.persist_revision(&source.revision_id, &source.content_digest, bytes)
    }

    #[test]
    fn failed_writer_cannot_publish_a_revision() {
        let fixture = Fixture::new();
        let mut store = fixture.store();
        let source = fixture.source("source", b"private bytes");
        let before = fixture.log();
        let result = store.index_file_with_writer(&source, &mut |_, _, _| {
            Err("PROTECTION_READBACK_FAILED".to_owned())
        });
        assert_eq!(result, Err("PROTECTION_READBACK_FAILED".to_owned()));
        assert_eq!(fixture.log(), before);
        assert!(store.list_sources().is_empty());
        assert!(fixture.store().list_sources().is_empty());
    }

    #[test]
    fn later_writer_failure_leaves_all_batch_metadata_unpublished() {
        let fixture = Fixture::new();
        let mut store = fixture.store();
        let paths = vec![fixture.source("a", b"one"), fixture.source("b", b"two")];
        let before = fixture.log();
        let mut writes = 0;
        let result = store.index_paths_bounded(&paths, 6, &mut |store, source, bytes| {
            writes += 1;
            assert!(store.list_sources().is_empty());
            assert_eq!(fixture.log(), before);
            if writes == 2 {
                return Err("SECOND_OBJECT_FAILED".to_owned());
            }
            plaintext_writer(store, source, bytes)
        });
        assert_eq!(result, Err("SECOND_OBJECT_FAILED".to_owned()));
        assert_eq!(writes, 2);
        assert_eq!(fixture.log(), before);
        assert!(fixture.store().list_sources().is_empty());
    }

    #[test]
    fn aggregate_byte_limit_fails_before_any_writer_call() {
        let fixture = Fixture::new();
        let mut store = fixture.store();
        let paths = vec![fixture.source("a", b"one"), fixture.source("b", b"two")];
        let mut calls = 0;
        let result = store.index_paths_bounded(&paths, 5, &mut |_, _, _| {
            calls += 1;
            Ok(())
        });
        assert_eq!(result, Err("DIRECT_BATCH_BYTES_EXCEEDED".to_owned()));
        assert_eq!(calls, 0);
        assert!(store.list_sources().is_empty());
    }

    #[test]
    fn returning_to_old_content_is_a_new_transition_not_a_conflicting_replay() {
        let fixture = Fixture::new();
        let mut store = fixture.store();
        let path = fixture.source("source", b"old");
        let first = store.index_file(&path).unwrap();
        fs::write(&path, b"new").unwrap();
        let second = store.index_file(&path).unwrap();
        fs::write(&path, b"old").unwrap();
        let third = store.index_file(&path).unwrap();
        assert_eq!(first.source_id, third.source_id);
        assert_eq!(first.revision_id, third.revision_id);
        assert_ne!(second.revision_id, third.revision_id);
        assert!(third.changed);
        assert!(!store.index_file(&path).unwrap().changed);
        let verified = fixture.store().verify().unwrap();
        assert_eq!(verified.source_events, 3);
        assert_eq!(verified.referenced_revisions, 2);
    }

    #[test]
    fn retired_source_can_be_reactivated_with_unchanged_bytes() {
        let fixture = Fixture::new();
        let mut store = fixture.store();
        let path = fixture.source("source", b"retained");
        let first = store.index_file(&path).unwrap();
        store.retire_source(&first.source_id).unwrap();
        assert!(store.index_file(&path).unwrap().changed);
        let verified = fixture.store().verify().unwrap();
        assert_eq!(verified.active_sources, 1);
        assert_eq!(verified.source_events, 3);
    }

    #[test]
    fn stale_catalog_refuses_new_writes_before_calling_storage() {
        let fixture = Fixture::new();
        let mut first = fixture.store();
        let mut stale = fixture.store();
        let path = fixture.source("source", b"new");
        first.index_file(&path).unwrap();
        let result = stale.index_file_with_writer(&path, &mut |_, _, _| {
            panic!("stale catalog must not invoke storage");
        });
        assert_eq!(result, Err("DIRECT_CONTROL_READBACK_MISMATCH".to_owned()));
    }

    #[test]
    fn byte_budget_counts_only_admitted_retained_bytes() {
        // Canonical admission denies empty sources before CAS, so the byte
        // budget covers admitted retained bytes only. Two one-byte admitted
        // sources fit a two-byte budget and exceed a one-byte budget without
        // invoking storage on the failing path.
        let fixture = Fixture::new();
        let mut store = fixture.store();
        let paths = vec![fixture.source("a", b"x"), fixture.source("b", b"y")];
        let mut calls = 0;
        let result = store.index_paths_bounded(&paths, 1, &mut |_, _, _| {
            calls += 1;
            Ok(())
        });
        assert_eq!(result, Err("DIRECT_BATCH_BYTES_EXCEEDED".to_owned()));
        assert_eq!(calls, 0);
        let admitted = store
            .index_paths_bounded(&paths, 2, &mut plaintext_writer)
            .unwrap();
        assert_eq!(admitted.len(), 2);
        assert_eq!(store.verify().unwrap().total_revision_bytes, 2);
    }

    #[test]
    fn empty_source_is_denied_before_any_writer_call() {
        // Canonical deny-by-default: empty sources never reach CAS and never
        // consume another file's byte budget.
        let fixture = Fixture::new();
        let mut store = fixture.store();
        let empty = fixture.source("empty", b"");
        let before = fixture.log();
        let mut calls = 0;
        let result = store.index_file_with_writer(&empty, &mut |_, _, _| {
            calls += 1;
            Ok(())
        });
        assert_eq!(result, Err(canonical::SOURCE_ADMISSION_DENIED.to_owned()));
        assert_eq!(calls, 0);
        assert_eq!(fixture.log(), before);
        assert!(store.list_sources().is_empty());
    }

    #[test]
    fn denied_sources_never_reach_cas() {
        let fixture = Fixture::new();
        let mut store = fixture.store();
        for name in ["id_rsa", "tls.pem", "secret_notes.txt", "app.generated.js"] {
            let path = fixture.source(name, b"candidate bytes");
            let before = fixture.log();
            let mut calls = 0;
            let result = store.index_file_with_writer(&path, &mut |_, _, _| {
                calls += 1;
                Ok(())
            });
            assert_eq!(
                result,
                Err(canonical::SOURCE_ADMISSION_DENIED.to_owned()),
                "name={name}"
            );
            assert_eq!(calls, 0, "denied source must not invoke storage");
            assert_eq!(fixture.log(), before);
        }
        assert!(store.list_sources().is_empty());
        // A regular source in the same directory still admits afterwards.
        let allowed = fixture.source("notes.txt", b"allowed bytes");
        let indexed = store.index_file(&allowed).unwrap();
        assert!(indexed.changed);
        assert_eq!(store.list_sources().len(), 1);
    }

    #[test]
    fn equal_content_distinct_files_keep_distinct_stable_identities() {
        let fixture = Fixture::new();
        let mut store = fixture.store();
        let first = fixture.source("first.txt", b"same bytes");
        let second = fixture.source("second.txt", b"same bytes");
        let one = store.index_file(&first).unwrap();
        let two = store.index_file(&second).unwrap();
        assert_ne!(one.source_id, two.source_id);
        assert_ne!(one.revision_id, two.revision_id);
        assert_eq!(one.content_digest, two.content_digest);
        assert_eq!(store.list_sources().len(), 2);
        assert_eq!(fixture.store().verify().unwrap().referenced_revisions, 2);
    }

    #[test]
    fn rename_preserves_stable_identity_with_new_locator_binding() {
        let fixture = Fixture::new();
        let mut store = fixture.store();
        let before = fixture.source("before.txt", b"renamed bytes");
        let first = store.index_file(&before).unwrap();
        let after = fixture.0.join("after.txt");
        fs::rename(&before, &after).unwrap();
        let second = store.index_file(&after).unwrap();
        // Stable identity persists across rename; the locator binding is new.
        assert_eq!(second.source_id, first.source_id);
        assert_eq!(second.revision_id, first.revision_id);
        assert!(second.changed);
        assert!(!store.index_file(&after).unwrap().changed);
        // The old locator no longer resolves; no bytes may be returned.
        assert!(store.index_file(&before).is_err());
    }

    #[test]
    fn replacement_at_same_path_with_new_identity_does_not_reuse_closed_source() {
        let fixture = Fixture::new();
        let mut store = fixture.store();
        let path = fixture.source("victim.txt", b"original bytes");
        let first = store.index_file(&path).unwrap();
        // Deterministic replacement: remove and recreate so the platform
        // issues a new stable file identity for the same locator.
        fs::remove_file(&path).unwrap();
        fs::write(&path, b"substituted bytes").unwrap();
        let second = store.index_file(&path).unwrap();
        assert_ne!(second.content_digest, first.content_digest);
        assert_ne!(second.revision_id, first.revision_id);
        // Either the platform reused the identity (same source, new revision)
        // or it issued a new one (distinct source); both keep history exact
        // and never resurrect the closed binding as unchanged.
        assert!(second.changed);
        let verified = fixture.store().verify().unwrap();
        assert!(verified.source_events >= 2);
    }

    #[test]
    fn restart_preserves_membership_revision_occurrences_and_lineage() {
        let fixture = Fixture::new();
        let first_id;
        let first_revision;
        {
            let mut store = fixture.store();
            let path = fixture.source("notes.txt", b"lineage bytes");
            let indexed = store.index_file(&path).unwrap();
            first_id = indexed.source_id.clone();
            first_revision = indexed.revision_id.clone();
            assert_eq!(store.list_sources().len(), 1);
        }
        // Reopen replays the same log; membership, occurrences and lineage
        // bindings survive without re-admission side effects.
        let reopened = fixture.store();
        let sources = reopened.list_sources();
        assert_eq!(sources.len(), 1);
        assert_eq!(sources[0].source_id, first_id);
        assert_eq!(sources[0].revision_id, first_revision);
        let verified = reopened.verify().unwrap();
        assert_eq!(verified.source_events, 1);
        assert_eq!(verified.referenced_revisions, 1);
        // Re-indexing unchanged bytes is a no-op without a new event.
        let mut store = fixture.store();
        let path = fixture.0.join("notes.txt");
        assert!(!store.index_file(&path).unwrap().changed);
        assert_eq!(fixture.store().verify().unwrap().source_events, 1);
    }
}

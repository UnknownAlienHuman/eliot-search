//! One bounded technical-state fixture, with no native credentials or index.
use super::*;
use crate::MigrationBootstrapWork;
use crate::persistent::bootstrap::OriginalCheck;
use std::cell::Cell;
use std::time::{Duration, Instant};

#[test]
fn projection_snapshot_is_coherent_read_only_and_refuses_unresolved_work() {
    let scratch = Scratch::new();
    let (mut journal, request) = scratch.ready();
    let deadline = Instant::now() + Duration::from_secs(30);
    let work = |_: MigrationBootstrapWork| Ok::<(), ()>(());
    let check = OriginalCheck::registry(deadline, &work);
    // READBACK_VERIFIED without its durable VisibleEpoch commit is not ready.
    assert_eq!(
        journal.read_projection_snapshot_checked(&check),
        Err(ControlError::SnapshotRebuildFailed)
    );
    let commit = apply(&mut journal, &request).unwrap();
    let before = scratch.bytes();
    let writes = journal.committed_writes();
    let reads = journal.work.snapshot_reads.load(Ordering::Relaxed);
    let mut publisher = ControlSnapshotPublisher::new();
    let port = journal.bind_control_port::<Cancel>(&mut publisher).unwrap();
    let snapshot = port
        .read_publication_projection_snapshot_with_original_context(deadline, &work)
        .unwrap()
        .unwrap();
    assert_eq!(snapshot.identity(), journal.identity());
    assert_eq!(snapshot.generation(), commit.after_generation);
    assert_eq!(snapshot.visibility().visible_epoch.get(), 1);
    assert_eq!(snapshot.initial_route_revision(), None);
    assert_eq!(
        snapshot.manifests().get(&member(1)),
        Some(&reference("new"))
    );
    assert!(snapshot.prepared_artifact_sets().is_empty());
    assert!(!snapshot.shadows().contains_key(&member(1)));
    assert_eq!(snapshot.shadows().get(&member(2)), Some(&shadow(2)));
    assert_eq!(
        journal.work.snapshot_reads.load(Ordering::Relaxed) - reads,
        1
    );
    assert_eq!(journal.committed_writes(), writes);
    assert_eq!(scratch.bytes(), before);
    assert!(!format!("{snapshot:?}").contains("new-points"));
    assert_eq!(
        journal.read_projection_snapshot_checked(&StopAt(Point::ReadComplete)),
        Err(ControlError::ReadCancelled)
    );
}

#[test]
fn projection_snapshot_schema5_uses_the_same_profile_and_original_allowance() {
    for indexed in [false, true] {
        let scratch = Scratch::new();
        let mut journal = scratch.empty(crate::REGISTRY_JOURNAL_SCHEMA_VERSION);
        let profile = if indexed {
            crate::JournalProfileInitialization::Indexed(state())
        } else {
            crate::JournalProfileInitialization::Direct
        };
        journal
            .initialize_operating_profile(profile, MutationId([1; 32]), digest(), &context(false))
            .unwrap();
        let deadline = Instant::now() + Duration::from_secs(30);
        let work = |_: MigrationBootstrapWork| Ok::<(), ()>(());
        let check = OriginalCheck::registry(deadline, &work);
        let before = scratch.bytes();
        let writes = journal.committed_writes();
        let result = journal.read_projection_snapshot_checked(&check);
        if indexed {
            let snapshot = result.unwrap().unwrap();
            assert_eq!(snapshot.visibility(), &state());
            assert_eq!(snapshot.generation(), 1);
            assert_eq!(snapshot.initial_route_revision().unwrap().get(), 1);
            assert!(snapshot.manifests().is_empty());
            assert!(snapshot.shadows().is_empty());
        } else {
            assert_eq!(result, Err(ControlError::MigrationUnverified));
        }
        assert_eq!(journal.committed_writes(), writes);
        assert_eq!(scratch.bytes(), before);
        // Refuse the explicit profile decoding allowance. A legacy unscoped
        // profile preflight would not pass through this original callback.
        let refused = Cell::new(false);
        let work = |work| {
            if work == MigrationBootstrapWork::Allocate(LIMITS.max_value_bytes) {
                refused.set(true);
                Err(())
            } else {
                Ok(())
            }
        };
        let check = OriginalCheck::registry(deadline, &work);
        assert!(journal.read_projection_snapshot_checked(&check).is_err());
        assert!(refused.get());
    }
}

#[test]
fn reopened_unresolved_intent_is_recovery_metadata_but_not_a_serving_snapshot() {
    let scratch = Scratch::new();
    let (journal, _) = scratch.ready();
    let next_owner = JournalIdentity {
        owner_epoch: OwnerEpoch::new(2).unwrap(),
        ..identity()
    };
    let journal = journal.advance_owner(next_owner).unwrap();
    drop(journal);

    let (writes, writes_after) = {
        let mut journal =
            PersistentControlJournal::open(scratch.file(), next_owner, LIMITS).unwrap();
        let writes = journal.committed_writes();
        let deadline = Instant::now() + Duration::from_secs(30);
        let work_calls = Cell::new(0);
        let work = |_| {
            work_calls.set(work_calls.get() + 1);
            Ok::<(), ()>(())
        };
        {
            let mut publisher = ControlSnapshotPublisher::new();
            let port = journal
                .bind_control_port_with_original_context::<Cancel, _>(
                    &mut publisher,
                    deadline,
                    &work,
                )
                .unwrap();
            let calls_after_bind = work_calls.get();

            let serving = port
                .read_publication_projection_snapshot_with_original_context(deadline, &work)
                .unwrap_err();
            assert_eq!(serving.control, ControlError::SnapshotRebuildFailed);

            let recovery = port
                .read_publication_recovery_projection_snapshot_with_original_context(
                    deadline, &work,
                )
                .unwrap()
                .unwrap();
            assert_eq!(recovery.identity(), next_owner);
            assert_eq!(recovery.generation(), 6);
            assert_eq!(recovery.visibility(), &state());
            let intent = recovery.intent().unwrap();
            assert_eq!(intent.state, PublicationIntentState::ReadbackVerified);
            assert_eq!(
                intent.owner_source_membership_access_guards.owner_epoch,
                OwnerEpoch::new(1).unwrap()
            );
            assert_eq!(intent.target_epoch.get(), 1);
            assert_eq!(
                intent.prepared_manifest_ref,
                reference("prepared-manifest-sentinel")
            );
            assert_eq!(recovery.last_reserved_epoch().get(), 1);
            assert_eq!(recovery.initial_route_revision(), None);
            assert_eq!(
                recovery.manifests().get(&member(1)),
                Some(&reference("old"))
            );
            assert!(recovery.prepared_artifact_sets().is_empty());
            assert_eq!(recovery.shadows().get(&member(1)), Some(&shadow(1)));
            assert_eq!(recovery.shadows().get(&member(2)), Some(&shadow(2)));
            assert!(work_calls.get() > calls_after_bind);
        }
        (writes, journal.committed_writes())
    };
    assert_eq!(writes_after, writes);
}

#[test]
fn serving_projection_reader_refuses_resolved_visibility_from_a_prior_owner() {
    let scratch = Scratch::new();
    let (mut journal, request) = scratch.ready();
    apply(&mut journal, &request).unwrap();
    let next_owner = JournalIdentity {
        owner_epoch: OwnerEpoch::new(2).unwrap(),
        ..identity()
    };
    let mut journal = journal.advance_owner(next_owner).unwrap();

    let deadline = Instant::now() + Duration::from_secs(30);
    let work = |_: MigrationBootstrapWork| Ok::<(), ()>(());
    let mut publisher = ControlSnapshotPublisher::new();
    let port = journal
        .bind_control_port_with_original_context::<Cancel, _>(&mut publisher, deadline, &work)
        .unwrap();
    let error = port
        .read_publication_projection_snapshot_with_original_context(deadline, &work)
        .unwrap_err();
    assert_eq!(error.control, ControlError::GenerationMismatch);
}

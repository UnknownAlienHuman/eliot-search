//! One bounded technical-state fixture, with no native credentials or index.
use super::*;
use crate::MigrationBootstrapWork;
use crate::persistent::bootstrap::OriginalCheck;
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
    drop(port);
    assert_eq!(snapshot.identity(), journal.identity());
    assert_eq!(snapshot.generation(), commit.after_generation);
    assert_eq!(snapshot.visibility().visible_epoch.get(), 1);
    assert_eq!(
        snapshot.manifests().get(&member(1)),
        Some(&reference("new"))
    );
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

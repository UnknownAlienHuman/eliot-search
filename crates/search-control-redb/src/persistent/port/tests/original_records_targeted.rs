use super::*;
use crate::{
    ConditionalControlMutation, ControlMutation, ControlWrite, MigrationBootstrapWork as Work,
};
use std::sync::atomic::Ordering;
use std::time::{Duration, Instant};

fn deadline() -> Instant {
    Instant::now() + Duration::from_secs(60)
}

fn seed(bound: &mut crate::BoundControlJournal<'_, Cancel>) {
    let large = vec![b'x'; 37];
    let mutation = ControlMutation::new(
        MutationId([80; 32]),
        digest(),
        1,
        vec![
            ControlWrite {
                key: key(b"target/alpha"),
                value: value(b"A"),
            },
            ControlWrite {
                key: key(b"target/middle"),
                value: value(b"M"),
            },
            ControlWrite {
                key: key(b"target/zeta"),
                value: value(b"Z"),
            },
            ControlWrite {
                key: key(b"target/large"),
                value: value(&large),
            },
        ],
        vec![],
    );
    let command = ConditionalControlMutation::new(mutation, vec![]);
    let identity = control_mutation_identity(MutationId([80; 32])).unwrap();
    bound
        .transact_records_with_original_context(&command, &identity, deadline(), &|_| {
            Ok::<_, &str>(())
        })
        .unwrap();
}

fn bound_journal() -> (Scratch, PersistentControlJournal, ControlSnapshotPublisher) {
    let scratch = Scratch::new();
    let mut journal = scratch.create3();
    journal
        .initialize_publication_visibility(
            vis_state(),
            MutationId([79; 32]),
            digest(),
            &context(false),
        )
        .unwrap();
    let mut publisher = ControlSnapshotPublisher::new();
    {
        let mut bound = journal.bind_control_port::<Cancel>(&mut publisher).unwrap();
        seed(&mut bound);
    }
    (scratch, journal, publisher)
}

#[test]
fn targeted_original_read_returns_sorted_subset_and_explicit_missing_without_scan() {
    let (scratch, mut journal, mut publisher) = bound_journal();
    let before_generation = journal.read_snapshot().unwrap().generation;
    let before_bytes = scratch.bytes();
    let before_points = journal.work.point_reads.load(Ordering::Relaxed);
    let before_snapshots = journal.work.snapshot_reads.load(Ordering::Relaxed);
    {
        let bound = journal.bind_control_port::<Cancel>(&mut publisher).unwrap();
        let keys = [
            key(b"target/zeta"),
            key(b"target/missing"),
            key(b"target/alpha"),
        ];
        let readback = bound
            .read_control_keys_with_original_context(&keys, deadline(), &|_| Ok::<_, &str>(()))
            .unwrap();
        assert_eq!(readback.identity(), identity3());
        assert_eq!(readback.generation(), before_generation);
        assert_eq!(
            readback.records(),
            &[
                (key(b"target/alpha"), value(b"A")),
                (key(b"target/zeta"), value(b"Z"))
            ]
        );
        assert_eq!(readback.missing_keys(), &[key(b"target/missing")]);
        assert_eq!(
            readback.get(&key(b"target/alpha")).unwrap().as_bytes(),
            b"A"
        );
        assert!(readback.get(&key(b"target/middle")).is_none());
    }
    assert_eq!(
        journal.work.point_reads.load(Ordering::Relaxed) - before_points,
        3
    );
    assert_eq!(
        journal.work.snapshot_reads.load(Ordering::Relaxed),
        before_snapshots
    );
    assert_eq!(scratch.bytes(), before_bytes);
    assert_eq!(
        journal.read_snapshot().unwrap().generation,
        before_generation
    );
}

#[test]
fn targeted_original_read_rejects_duplicate_keys_without_reading_or_writing() {
    let (scratch, mut journal, mut publisher) = bound_journal();
    let before_generation = journal.read_snapshot().unwrap().generation;
    {
        let bound = journal.bind_control_port::<Cancel>(&mut publisher).unwrap();
        let before_bytes = scratch.bytes();
        let keys = [key(b"target/alpha"), key(b"target/alpha")];
        let error = bound
            .read_control_keys_with_original_context(&keys, deadline(), &|_| Ok::<_, &str>(()))
            .unwrap_err();
        assert_eq!(error.control, ControlError::DuplicateMutationKey);
        assert_eq!(error.first_context, None);
        assert_eq!(scratch.bytes(), before_bytes);
    }
    assert_eq!(
        journal.read_snapshot().unwrap().generation,
        before_generation
    );
}

#[test]
fn targeted_original_read_preserves_cancel_and_deadline_causes() {
    let (scratch, mut journal, mut publisher) = bound_journal();
    let before_generation = journal.read_snapshot().unwrap().generation;
    let before_bytes = scratch.bytes();
    {
        let bound = journal.bind_control_port::<Cancel>(&mut publisher).unwrap();
        let cancelled = bound
            .read_control_keys_with_original_context(&[key(b"target/alpha")], deadline(), &|_| {
                Err::<(), _>("original-cancel")
            })
            .unwrap_err();
        assert_eq!(cancelled.control, ControlError::ReadCancelled);
        assert_eq!(cancelled.first_context, Some("original-cancel"));

        let expired = bound
            .read_control_keys_with_original_context(
                &[key(b"target/alpha")],
                Instant::now(),
                &|_| Ok::<_, &str>(()),
            )
            .unwrap_err();
        assert_eq!(expired.control, ControlError::BudgetExceeded);
        assert_eq!(expired.first_context, None);
    }
    assert_eq!(scratch.bytes(), before_bytes);
    assert_eq!(
        journal.read_snapshot().unwrap().generation,
        before_generation
    );
}

#[test]
fn targeted_original_read_mid_read_refusal_keeps_first_cause_and_generation() {
    let (scratch, mut journal, mut publisher) = bound_journal();
    let before_generation = journal.read_snapshot().unwrap().generation;
    let before_bytes = scratch.bytes();
    {
        let bound = journal.bind_control_port::<Cancel>(&mut publisher).unwrap();
        let error = bound
            .read_control_keys_with_original_context(
                &[key(b"target/large"), key(b"target/zeta")],
                deadline(),
                &|unit| {
                    if matches!(unit, Work::Read(38)) {
                        Err("mid-read")
                    } else {
                        Ok(())
                    }
                },
            )
            .unwrap_err();
        assert_eq!(error.control, ControlError::ReadCancelled);
        assert_eq!(error.first_context, Some("mid-read"));
    }
    assert_eq!(scratch.bytes(), before_bytes);
    assert_eq!(
        journal.read_snapshot().unwrap().generation,
        before_generation
    );
}

//! T27 publication fault-process qualification.
//!
//! This harness exercises the daemon publication composition through its real
//! source file, included below, plus the durable [`FileJournal`] owned by
//! `search-control-redb`. Every test builds its state through [`fresh`] and
//! advances control generations only through [`bump`]; direct floor assignment
//! outside [`bump`] is forbidden and impossible because floor fields are
//! private to the owning crates.
//!
//! The Qdrant double below is a process-test double only: it models
//! transport loss as [`CompensateError::Unknown`] until an exact readback
//! resolves the outcome. It never manufactures a live receipt, spawns no
//! process and performs no broad-filter mutation: only explicit point IDs
//! travel through `upsert_exact` / `close_exact` / `readback_exact`, mirroring
//! the existing `RealDataPlane` surface. Physical reclaim does not exist here;
//! retirement is the logical [`RetiredManifest`] record only.

#![forbid(unsafe_code)]

#[path = "../src/publication_composition.rs"]
mod publication_composition;

use std::collections::BTreeMap;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicU64, Ordering};

use publication_composition::{
    cas, CommitKind, CompensateError, CompensateMutation, CompensatePointId, CompensateReadback,
    CompensateReceipt, FakeGuards, LiveGuardRead, MembershipFence, ProposeRequest,
    PublicationCodecError, PublicationFloor, PublicationGuards, Publisher, PublisherError,
    QdrantCompensate, RecoveryDecision, RecoveryHead, RetiredManifest,
};
use search_contracts::Epoch;
use search_control_redb::publication_codec::FileJournal;

static NEXT_HARNESS: AtomicU64 = AtomicU64::new(0);

/// Disposable harness state: a codec-level floor for [`cas`] discipline plus
/// a [`Publisher`] with one durable marker slot per reserved epoch.
struct TestState {
    floor: PublicationFloor,
    publisher: Publisher,
    base: PathBuf,
}

impl Drop for TestState {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.base);
    }
}

/// Builds explicit fresh state. There is no implicit construction path:
/// guards come from [`FakeGuards::fresh`] and the floor starts at generation
/// zero with both floors at epoch zero.
fn fresh() -> TestState {
    let stamp = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map_or(0, |elapsed| elapsed.as_nanos());
    let base = std::env::temp_dir().join(format!(
        "eliot-publication-fault-{}-{stamp}-{}",
        std::process::id(),
        NEXT_HARNESS.fetch_add(1, Ordering::Relaxed)
    ));
    std::fs::create_dir_all(&base).expect("harness directory is created");
    let guards = FakeGuards::fresh();
    let floor = PublicationFloor::new(
        0,
        Epoch::new(0).expect("epoch zero is valid"),
        Epoch::new(0).expect("epoch zero is valid"),
    )
    .expect("fresh floor is well-formed");
    let live = LiveGuardRead::new(guards.guards(), 1);
    let publisher = Publisher::new(floor, live);
    TestState {
        floor,
        publisher,
        base,
    }
}

/// The only floor mutation path in this file: applies `advance` to the two
/// floor epochs while stepping the generation by exactly one, then commits
/// through [`cas`]. Any direct floor write outside this helper is forbidden.
fn bump(state: &mut TestState, advance: impl FnOnce(Epoch, Epoch) -> (Epoch, Epoch)) {
    let current = state.floor.generation();
    let (visible, reserved) = advance(
        state.floor.floor_visible(),
        state.floor.floor_last_reserved(),
    );
    let next_generation = current
        .checked_add(1)
        .expect("harness generations never exhaust");
    let next = PublicationFloor::new(next_generation, visible, reserved)
        .expect("bump builds a well-formed floor");
    state.floor = cas(&state.floor, current, &next).expect("bump performs the exact step");
}

/// Marker slot bound to one reserved epoch. Retries reuse the same slot with
/// identical bytes; a forked epoch payload reports `JournalConflict`.
fn slot(base: &Path, epoch: Epoch) -> FileJournal {
    FileJournal::new(base, &format!("intent-{:020}", epoch.get())).expect("epoch slot is created")
}

/// Proposes the next reserved epoch with the currently live guards.
fn propose_next(
    state: &mut TestState,
    kind: CommitKind,
    tag: u8,
) -> publication_composition::ProposedCommit {
    let epoch = Epoch::new(state.publisher.last_reserved().get() + 1).expect("next epoch is valid");
    let journal = slot(&state.base, epoch);
    let request = ProposeRequest {
        epoch,
        guards: FakeGuards::fresh().guards(),
        observed_generation: state.publisher.live().observed_generation(),
        kind,
        intent_bytes: [tag; 32].to_vec(),
    };
    state
        .publisher
        .propose(&journal, request)
        .expect("harness proposal commits")
}

/// Process-test Qdrant double. Models loss as unknown-until-readback and
/// replays idempotent retries; never a live receipt.
struct LossyQdrant {
    lose_next_mutation: bool,
    points: BTreeMap<[u8; 16], (Epoch, Option<Epoch>)>,
    operations: BTreeMap<u64, ([u8; 32], Vec<[u8; 16]>)>,
}

impl LossyQdrant {
    const fn stable() -> Self {
        Self {
            lose_next_mutation: false,
            points: BTreeMap::new(),
            operations: BTreeMap::new(),
        }
    }

    fn record(
        &mut self,
        mutation: &CompensateMutation,
        ids: &[[u8; 16]],
    ) -> Result<CompensateReceipt, CompensateError> {
        if self.operations.len() >= 65_536 {
            return Err(CompensateError::Mismatch);
        }
        if let Some((digest, stored)) = self.operations.get(&mutation.operation_id) {
            if *digest != mutation.input_digest {
                return Err(CompensateError::Conflict);
            }
            let affected = stored
                .iter()
                .map(|bytes| CompensatePointId(*bytes))
                .collect();
            return Ok(CompensateReceipt {
                affected,
                replayed: true,
            });
        }
        let affected = ids.iter().map(|bytes| CompensatePointId(*bytes)).collect();
        self.operations
            .insert(mutation.operation_id, (mutation.input_digest, ids.to_vec()));
        Ok(CompensateReceipt {
            affected,
            replayed: false,
        })
    }
}

impl QdrantCompensate for LossyQdrant {
    fn upsert_exact(
        &mut self,
        ids: Vec<CompensatePointId>,
        mutation: CompensateMutation,
    ) -> Result<CompensateReceipt, CompensateError> {
        if ids.is_empty() || ids.len() > 1_024 {
            return Err(CompensateError::Mismatch);
        }
        if self.lose_next_mutation {
            self.lose_next_mutation = false;
            return Err(CompensateError::Unknown);
        }
        let raw: Vec<[u8; 16]> = ids.iter().map(|id| id.0).collect();
        let receipt = self.record(&mutation, &raw)?;
        if !receipt.replayed {
            for bytes in &raw {
                self.points
                    .insert(*bytes, (Epoch::new(1).expect("epoch one"), None));
            }
        }
        Ok(receipt)
    }

    fn close_exact(
        &mut self,
        ids: Vec<CompensatePointId>,
        valid_until: Epoch,
        mutation: CompensateMutation,
    ) -> Result<CompensateReceipt, CompensateError> {
        if ids.is_empty() || ids.len() > 1_024 {
            return Err(CompensateError::Mismatch);
        }
        if self.lose_next_mutation {
            self.lose_next_mutation = false;
            return Err(CompensateError::Unknown);
        }
        let raw: Vec<[u8; 16]> = ids.iter().map(|id| id.0).collect();
        for bytes in &raw {
            let (from, _) = self.points.get(bytes).ok_or(CompensateError::Mismatch)?;
            if valid_until <= *from {
                return Err(CompensateError::Mismatch);
            }
        }
        let receipt = self.record(&mutation, &raw)?;
        if !receipt.replayed {
            for bytes in &raw {
                if let Some(entry) = self.points.get_mut(bytes) {
                    entry.1 = Some(valid_until);
                }
            }
        }
        Ok(receipt)
    }

    fn readback_exact(
        &self,
        ids: Vec<CompensatePointId>,
    ) -> Result<CompensateReadback, CompensateError> {
        if ids.is_empty() || ids.len() > 1_024 {
            return Err(CompensateError::Mismatch);
        }
        let mut present = Vec::new();
        let mut missing = Vec::new();
        for id in ids {
            if self.points.contains_key(&id.0) {
                present.push(id);
            } else {
                missing.push(id);
            }
        }
        Ok(CompensateReadback { present, missing })
    }
}

#[test]
fn cas_requires_exact_generation_bump() {
    let mut state = fresh();
    bump(&mut state, |visible, reserved| (visible, reserved));
    assert_eq!(state.floor.generation(), 1);

    let exact = PublicationFloor::new(
        2,
        state.floor.floor_visible(),
        state.floor.floor_last_reserved(),
    )
    .expect("exact successor builds");
    let outdated = PublicationFloor::new(
        2,
        state.floor.floor_visible(),
        state.floor.floor_last_reserved(),
    )
    .expect("outdated candidate builds");
    assert_eq!(
        cas(&state.floor, 0, &outdated),
        Err(PublicationCodecError::ControlConflict)
    );
    let without_bump = PublicationFloor::new(
        1,
        state.floor.floor_visible(),
        state.floor.floor_last_reserved(),
    )
    .expect("same-generation candidate builds");
    assert_eq!(
        cas(&state.floor, 1, &without_bump),
        Err(PublicationCodecError::ControlConflict)
    );
    let skipped = PublicationFloor::new(
        3,
        state.floor.floor_visible(),
        state.floor.floor_last_reserved(),
    )
    .expect("skipped candidate builds");
    assert_eq!(
        cas(&state.floor, 1, &skipped),
        Err(PublicationCodecError::ControlConflict)
    );
    assert_eq!(
        cas(&state.floor, 1, &exact).expect("exact bump commits"),
        exact
    );
}

#[test]
fn floor_mutation_without_bump_is_conflict() {
    let mut state = fresh();
    bump(&mut state, |visible, reserved| {
        (
            visible,
            Epoch::new(reserved.get() + 1).expect("reservation advances"),
        )
    });
    let generation = state.floor.generation();

    let mutated = PublicationFloor::new(
        generation,
        Epoch::new(state.floor.floor_visible().get() + 1).expect("epoch"),
        Epoch::new(state.floor.floor_last_reserved().get() + 1).expect("epoch"),
    )
    .expect("same-generation mutation builds");
    assert_eq!(
        cas(&state.floor, generation, &mutated),
        Err(PublicationCodecError::ControlConflict)
    );

    let regressed = PublicationFloor::new(
        generation + 1,
        state.floor.floor_visible(),
        state.floor.floor_visible(),
    )
    .expect("regressed reservation builds");
    assert_eq!(
        cas(&state.floor, generation, &regressed),
        Err(PublicationCodecError::ControlConflict)
    );

    assert_eq!(
        PublicationFloor::new(
            generation + 1,
            Epoch::new(4).expect("epoch"),
            Epoch::new(3).expect("epoch"),
        ),
        Err(PublicationCodecError::ControlConflict)
    );
}

#[test]
fn concurrent_guard_change_blocks_commit() {
    let mut state = fresh();
    bump(&mut state, |visible, reserved| (visible, reserved));
    let first = propose_next(&mut state, CommitKind::full(), 0x11);
    state
        .publisher
        .observe_control_commit(first.control_generation())
        .expect("first commit is observed");

    let mut rotated = FakeGuards::fresh().guards();
    rotated.source_catalog_generation = rotated
        .source_catalog_generation
        .checked_add(1)
        .expect("guard space remains");
    state
        .publisher
        .rotate_live_guards(LiveGuardRead::new(rotated, 2));

    let pending_epoch =
        Epoch::new(state.publisher.last_reserved().get() + 1).expect("next epoch is valid");
    let outdated = ProposeRequest {
        epoch: pending_epoch,
        guards: FakeGuards::fresh().guards(),
        observed_generation: 1,
        kind: CommitKind::full(),
        intent_bytes: [0x22; 16].to_vec(),
    };
    assert_eq!(
        state
            .publisher
            .propose(&slot(&state.base, pending_epoch), outdated),
        Err(PublisherError::ControlConflict)
    );
    assert!(!state.publisher.has_active());

    let current = ProposeRequest {
        epoch: pending_epoch,
        guards: rotated,
        observed_generation: 2,
        kind: CommitKind::full(),
        intent_bytes: [0x23; 16].to_vec(),
    };
    state
        .publisher
        .propose(&slot(&state.base, pending_epoch), current)
        .expect("current observation commits");
}

#[test]
fn qdrant_loss_is_unknown_until_readback() {
    let mut state = fresh();
    bump(&mut state, |visible, reserved| (visible, reserved));
    let mut qdrant = LossyQdrant::stable();
    qdrant.lose_next_mutation = true;
    let id = CompensatePointId([0x31; 16]);
    let mutation = CompensateMutation {
        operation_id: 7,
        input_digest: [0x32; 32],
    };

    assert_eq!(
        qdrant.upsert_exact(vec![id], mutation),
        Err(CompensateError::Unknown)
    );
    let readback = qdrant
        .readback_exact(vec![id])
        .expect("readback is always definite");
    assert!(readback.present.is_empty());
    assert_eq!(readback.missing, [id]);

    let receipt = qdrant
        .upsert_exact(vec![id], mutation)
        .expect("retry with the same identity commits");
    assert!(!receipt.replayed);
    let readback = qdrant
        .readback_exact(vec![id])
        .expect("readback proves the retry");
    assert_eq!(readback.present, [id]);
    assert!(readback.missing.is_empty());

    let replay = qdrant
        .upsert_exact(vec![id], mutation)
        .expect("identical retry replays");
    assert!(replay.replayed);

    let close = CompensateMutation {
        operation_id: 8,
        input_digest: [0x33; 32],
    };
    qdrant
        .close_exact(vec![id], Epoch::new(9).expect("epoch"), close)
        .expect("explicit-ID close commits");
    let head = RecoveryHead {
        intent_durable: true,
        control_committed: false,
        snapshot_published: false,
        qdrant_verified: false,
        quarantined: false,
    };
    assert_eq!(
        RecoveryDecision::decide(head),
        RecoveryDecision::CompensateExact
    );
}

#[test]
fn uncommitted_epoch_never_visible() {
    let mut state = fresh();
    bump(&mut state, |visible, reserved| (visible, reserved));
    let before = state.publisher.visible_epoch();
    let generation_before = state.publisher.floor_generation();

    let proposed = propose_next(&mut state, CommitKind::full(), 0x44);
    assert_eq!(state.publisher.visible_epoch(), before);
    assert_eq!(state.publisher.floor_generation(), generation_before + 1);
    assert!(state.publisher.has_active());

    state
        .publisher
        .observe_control_commit(proposed.control_generation())
        .expect("control commit is observed");
    assert_eq!(state.publisher.visible_epoch(), proposed.epoch());
    assert_eq!(state.publisher.floor_generation(), generation_before + 2);
    assert!(!state.publisher.has_active());

    let invalidation = propose_next(&mut state, CommitKind::invalidation_only(), 0x45);
    let visible = state.publisher.visible_epoch();
    state
        .publisher
        .observe_control_commit(invalidation.control_generation())
        .expect("invalidation-only commit is observed");
    assert_eq!(state.publisher.visible_epoch(), visible);
}

#[test]
fn skipped_epoch_never_reused() {
    let mut state = fresh();
    bump(&mut state, |visible, reserved| (visible, reserved));
    let first = propose_next(&mut state, CommitKind::full(), 0x51);
    state
        .publisher
        .observe_control_commit(first.control_generation())
        .expect("first commit is observed");

    let skipped_epoch =
        Epoch::new(state.publisher.last_reserved().get() + 2).expect("skipped epoch builds");
    let skipped = ProposeRequest {
        epoch: skipped_epoch,
        guards: FakeGuards::fresh().guards(),
        observed_generation: 1,
        kind: CommitKind::full(),
        intent_bytes: [0x52; 16].to_vec(),
    };
    assert_eq!(
        state
            .publisher
            .propose(&slot(&state.base, skipped_epoch), skipped),
        Err(PublisherError::EpochMismatch)
    );

    let second = propose_next(&mut state, CommitKind::full(), 0x53);
    let fence = MembershipFence::full(2).expect("full fence builds");
    state
        .publisher
        .abandon(&fence)
        .expect("abandon with a full fence succeeds");
    assert!(!state.publisher.has_active());

    let reused = ProposeRequest {
        epoch: second.epoch(),
        guards: FakeGuards::fresh().guards(),
        observed_generation: 1,
        kind: CommitKind::full(),
        intent_bytes: [0x54; 16].to_vec(),
    };
    assert_eq!(
        state
            .publisher
            .propose(&slot(&state.base, second.epoch()), reused),
        Err(PublisherError::EpochMismatch)
    );
}

#[test]
fn abandon_requires_full_membership_fence() {
    let mut state = fresh();
    bump(&mut state, |visible, reserved| (visible, reserved));
    let proposed = propose_next(&mut state, CommitKind::full(), 0x61);

    let partial = MembershipFence::partial(1, 3).expect("partial fence builds");
    assert!(!partial.is_full());
    assert_eq!(
        state.publisher.abandon(&partial),
        Err(PublisherError::AbandonFenceMissing)
    );
    assert!(state.publisher.has_active());

    let retired = RetiredManifest::new([CompensatePointId([0x41; 16])].to_vec())
        .expect("logical retirement records");
    assert_eq!(retired.ids(), [CompensatePointId([0x41; 16])]);

    let full = MembershipFence::full(3).expect("full fence builds");
    assert!(full.is_full());
    assert_eq!(
        state.publisher.abandon(&full).expect("abandon succeeds"),
        proposed.epoch()
    );
    assert!(!state.publisher.has_active());
    assert_eq!(state.publisher.last_reserved(), proposed.epoch());
}

/// Crash-matrix rows: one observation head per recovery outcome. Kept beside
/// the test so the matrix stays reviewable without inflating the test body.
const fn crash_rows() -> [(RecoveryHead, RecoveryDecision); 8] {
    let clear = RecoveryHead {
        intent_durable: false,
        control_committed: false,
        snapshot_published: false,
        qdrant_verified: false,
        quarantined: false,
    };
    let mut rows = [(clear, RecoveryDecision::Continue); 8];
    rows[1].0.intent_durable = true;
    rows[1].1 = RecoveryDecision::CompensateExact;
    rows[2].0.intent_durable = true;
    rows[2].0.qdrant_verified = true;
    rows[3].0.intent_durable = true;
    rows[3].0.control_committed = true;
    rows[3].0.qdrant_verified = true;
    rows[3].1 = RecoveryDecision::PublishSnapshot;
    rows[4].0.intent_durable = true;
    rows[4].0.control_committed = true;
    rows[4].0.snapshot_published = true;
    rows[4].0.qdrant_verified = true;
    rows[5].0.intent_durable = true;
    rows[5].0.control_committed = true;
    rows[5].0.quarantined = true;
    rows[5].1 = RecoveryDecision::Blocked;
    rows[6].0.quarantined = true;
    rows[6].1 = RecoveryDecision::Blocked;
    rows[7].0.intent_durable = true;
    rows[7].0.qdrant_verified = true;
    rows[7].0.quarantined = true;
    rows[7].1 = RecoveryDecision::Blocked;
    rows
}

#[test]
fn crash_matrix_recovers_exactly_one_head() {
    let mut state = fresh();
    bump(&mut state, |visible, reserved| (visible, reserved));
    for (head, expected) in crash_rows() {
        assert_eq!(RecoveryDecision::decide(head), expected);
    }

    propose_next(&mut state, CommitKind::full(), 0x71);
    let contended_epoch = Epoch::new(state.publisher.last_reserved().get() + 1).expect("epoch");
    let contended = ProposeRequest {
        epoch: contended_epoch,
        guards: FakeGuards::fresh().guards(),
        observed_generation: 1,
        kind: CommitKind::full(),
        intent_bytes: [0x72; 16].to_vec(),
    };
    assert_eq!(
        state
            .publisher
            .propose(&slot(&state.base, contended_epoch), contended),
        Err(PublisherError::ControlConflict)
    );
}

#[test]
fn guards_have_no_default() {
    let mut state = fresh();
    bump(&mut state, |visible, reserved| (visible, reserved));

    let first = FakeGuards::fresh();
    let second = FakeGuards::fresh();
    assert_eq!(first.guards(), second.guards());

    let PublicationGuards {
        owner_epoch,
        source_catalog_generation,
        membership_generation,
        access_generation,
        shadow_generation,
        purge_generation,
        profile_digest,
    } = first.guards();
    assert_eq!(owner_epoch, FakeGuards::fresh().guards().owner_epoch);
    assert_eq!(source_catalog_generation, 7);
    assert_eq!(membership_generation, 5);
    assert_eq!(access_generation, 3);
    assert_eq!(shadow_generation, 2);
    assert_eq!(purge_generation, 2);
    assert_eq!(
        profile_digest,
        search_contracts::Blake3Digest32::from_bytes([0xA1; 32])
    );

    let live = LiveGuardRead::new(first.guards(), 1);
    assert_eq!(live.guards(), first.guards());
    assert_eq!(live.observed_generation(), 1);
    assert_eq!(state.publisher.live().guards(), first.guards());
}

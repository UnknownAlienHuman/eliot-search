//! Causal same-database fixtures with injected native observations, not installed qualification.
use super::*;
use super::super::tests::{Scratch, Platform, artifact, binding, deadline, fixture_digest};
use crate::migration::{SourceImportRecordArtifact, SourceMappingReadback, SourceLifecycleFlags};
use crate::persistent::PersistentControlJournal;
use search_contracts::{Sha256Digest32, SourceId, SourceRevisionId};
use std::sync::atomic::Ordering;

#[expect(clippy::unnecessary_wraps, reason = "Match the injected fallible checkpoint callback contract")]
fn okay(_: MigrationBootstrapWork) -> Result<(), ()> { Ok(()) }

pub(super) fn rows() -> Vec<SourceImportRow> {
    let hash = |byte| Sha256Digest32::from_bytes([byte; 32]);
    let first = SourceImportRow { sequence: 1, source: SourceId::from_bytes([1; 16]),
        revision: SourceRevisionId::from_bytes([2; 16]), previous_revision: None,
        occurrence: 1, source_event: 1, source_bytes: 7,
        lifecycle: SourceLifecycleFlags::new([true, true, false, true]),
        operation: hash(3), legacy_source: hash(4), legacy_revision: hash(5),
        content: hash(6), file_identity: hash(7), path: hash(8), event: hash(9),
        previous_event: hash(0), previous_source_event: hash(0) };
    let mut retained = first.clone();
    retained.sequence = 2; retained.source_event = 2;
    retained.previous_revision = Some(first.revision);
    retained.lifecycle = SourceLifecycleFlags::new([false, false, false, true]);
    retained.path = hash(10); retained.event = hash(11);
    retained.previous_event = first.event; retained.previous_source_event = first.event;
    let mut retired = retained.clone();
    retired.sequence = 3; retired.source_event = 3;
    retired.lifecycle = SourceLifecycleFlags::new([false, false, true, true]);
    retired.event = hash(12); retired.previous_event = retained.event;
    retired.previous_source_event = retained.event;
    vec![first, retained, retired]
}
pub(super) fn counts() -> SourceImportCounts {
    SourceImportCounts { events: 3, sources: 1, occurrences: 1, retained_events: 1, retirements: 1 }
}
pub(super) fn source_map(scratch: &Scratch, platform: Platform)
    -> SourceImportRecordArtifactPublication<Platform> {
    let mut writer = SourceImportRecordArtifact::create(&scratch.0, ".source-map.mapping-fixture.tmp", platform, deadline()).unwrap();
    writer.push(b"bounded source-map metadata\n", deadline()).unwrap();
    let frozen = writer.freeze(deadline()).unwrap();
    let name = format!("{}.source-map.v1", super::super::codec::hex(frozen.chain()));
    let mut replay = frozen.begin_readback(deadline()).unwrap();
    replay.compare(b"bounded source-map metadata\n", deadline()).unwrap();
    replay.finish(deadline()).unwrap().prepare_publication(&name, deadline()).unwrap()
}
fn completed(scratch: &Scratch) -> MigrationBootstrap {
    let mut owner = MigrationBootstrap::create(scratch.file(), binding(), deadline(), fixture_digest, okay).unwrap();
    let platform = Platform::default();
    let mut inventory = artifact(scratch, platform.clone());
    owner.publish_inventory(&mut inventory, &platform, deadline(), okay).unwrap();
    owner
}
pub(super) fn map_binding(publication: &SourceImportRecordArtifactPublication<Platform>) -> SourceImportBinding {
    SourceImportBinding { target_namespace: binding().target, legacy_namespace: binding().legacy_namespace,
        catalog_snapshot: binding().catalog_snapshot,
        mapping_profile: Sha256Digest32::from_bytes(crate::migration::source_mapping_profile_digest()),
        plan_chain: Sha256Digest32::from_bytes(*publication.chain()), events: 3, sources: 1 }
}
fn writing(scratch: &Scratch) -> OwnerBoundSourceStage {
    let bootstrap = completed(scratch);
    let platform = Platform::default();
    let mut publication = source_map(scratch, platform.clone());
    let mut stage = bootstrap.into_source_stage(map_binding(&publication), deadline(), okay).unwrap();
    stage.publish_source_map(&mut publication, &platform, deadline(), okay).unwrap();
    stage.initialize_mapping(deadline(), okay).unwrap();
    stage
}
fn readback(scratch: &Scratch) -> OwnerSourceMappingReadback {
    let mut stage = writing(scratch);
    for row in rows() { stage.push(row, deadline(), okay).unwrap(); }
    stage.begin_readback(counts(), deadline(), okay).unwrap()
}
fn open_file(scratch: &Scratch) -> std::fs::File {
    std::fs::OpenOptions::new().read(true).write(true).open(scratch.0.join("target.redb")).unwrap()
}

#[test]
fn exact_source_rows_share_original_database_and_legacy_readers_reject_profile() {
    let scratch = Scratch::new();
    let mut reader = readback(&scratch);
    let identity = reader.owner.bootstrap.journal.identity;
    let binding = reader.owner.binding;
    let read = reader.owner.bootstrap.journal.database.begin_read().unwrap();
    assert_eq!(read.list_tables().unwrap().count(), 7);
    assert_eq!(std::fs::read_dir(&scratch.0).unwrap().filter_map(Result::ok)
        .filter(|entry| entry.path().extension().is_some_and(|ext| ext == "redb")).count(), 1);
    assert!(crate::persistent::verify_tables(&read, &crate::persistent::TableProfile::ControlOnly).is_err());
    drop(read);
    for row in rows() { reader.compare(&row, deadline(), okay).unwrap(); }
    let receipt = reader.finish(deadline(), okay).unwrap();
    assert_eq!(receipt.expected, counts());
    assert!(receipt.render_json().contains("complete_inventory_candidate_not_performed"));
    drop(receipt);
    assert!(PersistentControlJournal::open(open_file(&scratch), identity, LIMITS).is_err());
    assert!(SourceMappingReadback::open(open_file(&scratch), binding, counts(), deadline()).is_err());
}

#[test]
fn first_mismatch_is_sticky_across_retry_and_finish() {
    let scratch = Scratch::new();
    let mut reader = readback(&scratch);
    let mut altered = rows()[0].clone(); altered.source_bytes += 1;
    let first = reader.compare(&altered, deadline(), okay).err().unwrap().control;
    assert_eq!(reader.compare(&rows()[0], deadline(), okay).err().unwrap().control, first);
    assert_eq!(reader.finish(deadline(), okay).err().unwrap().control, first);
}

#[test]
fn missing_live_inventory_or_foreign_mapping_profile_cannot_continue() {
    let scratch = Scratch::new();
    let owner = MigrationBootstrap::create(scratch.file(), binding(), deadline(), fixture_digest, okay).unwrap();
    let publication = source_map(&scratch, Platform::default());
    assert_eq!(owner.into_source_stage(map_binding(&publication), deadline(), okay).err().unwrap().control,
        ControlError::CommitOutcomeUnknown);
    let scratch = Scratch::new();
    let owner = completed(&scratch);
    let publication = source_map(&scratch, Platform::default());
    let mut foreign = map_binding(&publication); foreign.mapping_profile = Sha256Digest32::from_bytes([99; 32]);
    assert_eq!(owner.into_source_stage(foreign, deadline(), okay).err().unwrap().control, ControlError::IdentityMismatch);
}

#[test]
fn pre_dispatch_refusal_prevents_link_and_blocks_retry() {
    let scratch = Scratch::new();
    let owner = completed(&scratch);
    let platform = Platform::default();
    let mut publication = source_map(&scratch, platform.clone());
    let mut stage = owner.into_source_stage(map_binding(&publication), deadline(), okay).unwrap();
    let failure = stage.publish_source_map(&mut publication, &platform, deadline(), |_| Err("private refusal")).err().unwrap();
    assert_eq!(failure.first_context, Some("private refusal"));
    assert!(!scratch.0.join(publication.final_name()).exists());
    assert!(scratch.0.join(".source-map.mapping-fixture.tmp").exists());
    assert_eq!(stage.publish_source_map(&mut publication, &platform, deadline(), okay).err().unwrap().control,
        ControlError::CommitOutcomeUnknown);
}

#[test]
fn post_link_refusal_retains_pending_evidence_and_cannot_initialize() {
    let scratch = Scratch::new();
    let owner = completed(&scratch);
    let platform = Platform::default();
    let mut publication = source_map(&scratch, platform.clone());
    let mut stage = owner.into_source_stage(map_binding(&publication), deadline(), okay).unwrap();
    platform.cancel_after_sync.store(true, Ordering::Relaxed);
    let failure = stage.publish_source_map(&mut publication, &platform, deadline(), |_| {
        if platform.canceled.load(Ordering::Relaxed) { Err("private after-link refusal") } else { Ok(()) }
    }).err().unwrap();
    assert_eq!(failure.control, ControlError::CommitOutcomeUnknown);
    assert_eq!(failure.first_context, Some("private after-link refusal"));
    assert!(scratch.0.join(publication.final_name()).exists());
    assert!(scratch.0.join(".source-map.mapping-fixture.tmp").exists());
    let snapshot = stage.bootstrap.journal.read_snapshot().unwrap();
    let pending = snapshot.get(&ControlKey::new(MAP_ATTEMPT_KEY.to_vec(), LIMITS).unwrap()).unwrap();
    assert_eq!(pending.as_bytes(), codec::attempt(&stage.profile, stage.binding, &publication,
        super::super::tests::identity(1), 0).unwrap());
    assert_eq!(stage.initialize_mapping(deadline(), okay).err().unwrap().control, ControlError::CommitOutcomeUnknown);
}

#[test]
fn all_five_counts_and_original_context_are_required_for_receipt() {
    let scratch = Scratch::new();
    let mut reader = readback(&scratch);
    for row in rows() { reader.compare(&row, deadline(), okay).unwrap(); }
    reader.expected.retained_events = 0; reader.expected.retirements = 2;
    assert_eq!(reader.finish(deadline(), okay).err().unwrap().control, ControlError::MigrationUnverified);
    let scratch = Scratch::new();
    let mut reader = readback(&scratch);
    let failure = reader.compare(&rows()[0], deadline(), |_| Err("original context canceled")).err().unwrap();
    assert_eq!(failure.first_context, Some("original context canceled"));
    assert!(reader.finish(deadline(), okay).is_err());
}

#[test]
fn legacy_four_table_import_still_resumes_and_reads_exact_rows() {
    use crate::migration::SourceMappingImport;
    let scratch = Scratch::new();
    let publication = source_map(&scratch, Platform::default());
    let binding = map_binding(&publication);
    let mut writer = SourceMappingImport::create(scratch.file(), binding, deadline()).unwrap();
    for row in rows() { writer.push(row, deadline()).unwrap(); }
    writer.finish(counts(), deadline()).unwrap();
    let mut resumed = SourceMappingImport::resume(open_file(&scratch), binding, deadline()).unwrap();
    for row in rows() { resumed.push(row, deadline()).unwrap(); }
    resumed.finish(counts(), deadline()).unwrap();
    let mut readback = SourceMappingReadback::open(open_file(&scratch), binding, counts(), deadline()).unwrap();
    for row in rows() { readback.compare(&row, deadline()).unwrap(); }
    readback.finish(deadline()).unwrap();
}

#[test]
fn batch_crossing_reserves_bounded_work_and_full_denominator() {
    use std::cell::Cell;
    let scratch = Scratch::new();
    let bootstrap = completed(&scratch);
    let platform = Platform::default();
    let mut publication = source_map(&scratch, platform.clone());
    let mut binding = map_binding(&publication);
    binding.events = 513; binding.sources = 513;
    let mut stage = bootstrap.into_source_stage(binding, deadline(), okay).unwrap();
    stage.publish_source_map(&mut publication, &platform, deadline(), okay).unwrap();
    let allocated = Cell::new(0_usize);
    let largest = Cell::new(0_usize);
    let meter = |work| {
        if let MigrationBootstrapWork::Allocate(bytes) = work {
            allocated.set(allocated.get() + bytes); largest.set(largest.get().max(bytes));
        }
        Ok::<(), ()>(())
    };
    stage.initialize_mapping(deadline(), meter).unwrap();
    let mut emitted = Vec::new();
    let mut previous = Sha256Digest32::from_bytes([0; 32]);
    for number in 1_u16..=513 {
        let mut row = rows().remove(0);
        let mut id = [1; 16]; id[..2].copy_from_slice(&number.to_be_bytes());
        row.source = SourceId::from_bytes(id); row.revision = SourceRevisionId::from_bytes(id);
        let mut event = [2; 32]; event[..2].copy_from_slice(&number.to_be_bytes());
        row.event = Sha256Digest32::from_bytes(event); row.previous_event = previous;
        row.sequence = u64::from(number); previous = row.event;
        stage.push(row.clone(), deadline(), meter).unwrap(); emitted.push(row);
    }
    let expected = SourceImportCounts { events: 513, sources: 513, occurrences: 513,
        retained_events: 0, retirements: 0 };
    let mut reader = stage.begin_readback(expected, deadline(), meter).unwrap();
    for row in &emitted { reader.compare(row, deadline(), meter).unwrap(); }
    reader.finish(deadline(), meter).unwrap();
    // Capacity is one 256-row buffer plus bounded per-row encodings and metadata.
    // No table-population allocation is charged on every read/open.
    assert!(largest.get() <= 256 * std::mem::size_of::<SourceImportRow>() + 8192);
    assert!(allocated.get() < 1024 * 1024);
}

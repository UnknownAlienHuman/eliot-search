//! Real NTFS component observations; no NativeCtx, grant, cutover or serving proof.

use super::super::{LegacyInventoryEncoder, LegacyInventoryProfile, Stream, scan};
use super::*;
use search_contracts::{DataRootId, InstallationIncarnationId, Sha256Digest32, SourceNamespaceId};
use search_control_redb::migration::{
    CONTROL_CUTOVER_STAGED_DATABASE_SCHEMA, ControlCutoverMarker, LegacyInventoryBinding,
    LegacyInventoryOwner,
};
use std::fs::{self, OpenOptions};
use std::os::windows::fs::OpenOptionsExt;
use std::path::PathBuf;
use std::sync::Arc;
use std::sync::atomic::{AtomicU64, Ordering};
use std::time::{Duration, Instant, SystemTime, UNIX_EPOCH};

const OVERLAP: &str = "a-operation-publication-purge.pending";
const FLAT: &str = "b-operation-publication-purge.TMP";
const PUBLICATION: &str = "c-publication-only";
const PREFIX: &str = "search-control-view-";
const FAMILIES: [Family; 4] = [
    Family::Residue,
    Family::Operations,
    Family::Publications,
    Family::Security,
];

struct Scratch(PathBuf);
impl Scratch {
    fn new() -> Self {
        static NEXT: AtomicU64 = AtomicU64::new(0);
        let stamp = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_nanos();
        let root = fs::canonicalize(std::env::temp_dir())
            .unwrap()
            .join(format!(
                "{PREFIX}{}-{stamp}-{}",
                std::process::id(),
                NEXT.fetch_add(1, Ordering::Relaxed)
            ));
        fs::create_dir(&root).unwrap();
        Self(root)
    }
}
impl Drop for Scratch {
    fn drop(&mut self) {
        let root = fs::canonicalize(&self.0).unwrap();
        assert_eq!(root, self.0);
        assert_eq!(
            root.parent(),
            Some(fs::canonicalize(std::env::temp_dir()).unwrap().as_path())
        );
        assert!(
            root.file_name()
                .unwrap()
                .to_str()
                .unwrap()
                .starts_with(PREFIX)
        );
        fs::remove_dir_all(root).unwrap();
    }
}

fn encoder() -> LegacyInventoryEncoder {
    encoder_with_limits(LegacyInventoryLimits::BASELINE)
}

fn encoder_with_limits(limits: LegacyInventoryLimits) -> LegacyInventoryEncoder {
    // Transport metadata only, never an admitted NativeMigrationInput.
    let mut encoder =
        LegacyInventoryEncoder::with_profile(limits, LegacyInventoryProfile::LocationV2).unwrap();
    encoder
        .header(LegacyInventoryBinding {
            owner: LegacyInventoryOwner {
                installation: [1; 16],
                incarnation: [2; 16],
                root: [3; 16],
                invocation: [4; 16],
                request_blake3: [5; 32],
                record_blake3: [6; 32],
                physical_blake3: [7; 32],
                epoch: 1,
                generation: 1,
                native_limits: [1; 6],
            },
            target: SourceNamespaceId::from_bytes([8; 16]),
            namespace: Sha256Digest32::from_bytes([9; 32]),
            snapshot: Sha256Digest32::from_bytes([10; 32]),
            registration: None,
        })
        .unwrap();
    for family in Family::ALL {
        if family == Family::Residue {
            break;
        }
        encoder
            .end_family(family, Status::AbsentProven, true)
            .unwrap();
    }
    encoder
}

fn marker() -> ControlCutoverMarker {
    ControlCutoverMarker {
        target: SourceNamespaceId::from_bytes([11; 16]),
        incarnation: InstallationIncarnationId::from_bytes([12; 16]),
        root: DataRootId::from_bytes([13; 16]),
        epoch: 7,
        catalog_snapshot: [14; 32],
        plan_chain: [15; 32],
        content_chain: [16; 32],
        database_file_name: format!("{}.source-map.v2.redb", "a".repeat(64)),
        database_schema: CONTROL_CUTOVER_STAGED_DATABASE_SCHEMA.to_owned(),
    }
}

fn expected(
    source: &MigrationSource<'_, '_, '_>,
    family: Family,
    marker: &ControlCutoverMarker,
) -> Vec<Row> {
    let control = source.inventory_root().join("control");
    let subject = |relative: &str| io::locator(source, &control.join(relative)).unwrap();
    let orphan = u64::from(family == Family::Residue);
    let nested = format!("{OVERLAP}/nested");
    let mut rows = vec![
        row(
            Status::Gap,
            35,
            subject(OVERLAP),
            [fs::metadata(control.join(OVERLAP)).unwrap().len(), 0, 0],
        ),
        row(
            Status::Unsupported,
            14,
            subject(&format!("{OVERLAP}/control-cutover.v1")),
            [6, 0, orphan],
        ),
        row(
            Status::Gap,
            35,
            subject(&nested),
            [fs::metadata(control.join(&nested)).unwrap().len(), 1, 0],
        ),
        row(
            Status::Unsupported,
            14,
            subject(&format!("{nested}/leaf")),
            [4, 0, orphan],
        ),
        row(Status::Present, 36, subject(&nested), [0; 3]),
        row(Status::Present, 36, subject(OVERLAP), [0; 3]),
        row(Status::Unsupported, 14, subject(FLAT), [5, 0, 1]),
    ];
    if family == Family::Residue {
        rows.push(row(Status::Gap, 34, subject(PUBLICATION), [0; 3]));
    } else if family == Family::Publications {
        rows.push(row(
            Status::Unsupported,
            14,
            subject(PUBLICATION),
            [3, 0, 0],
        ));
        let bytes = marker.encode();
        rows.push(Row {
            status: Status::Present,
            kind: 15,
            refs: [
                subject("control-cutover.v1"),
                super::super::sha256::digest(&bytes),
                marker.catalog_snapshot,
            ],
            counts: [marker.epoch, bytes.len() as u64, 0],
        });
    }
    if family == Family::Residue {
        rows.push(row(Status::Gap, 34, subject("unowned"), [0; 3]));
    }
    rows.push(row(
        Status::Present,
        36,
        io::locator(source, &control).unwrap(),
        [0; 3],
    ));
    if family == Family::Operations {
        rows.push(Row {
            status: Status::Gap,
            kind: 348,
            refs: [ZERO; 3],
            counts: [348, 0, 0],
        });
    } else if matches!(family, Family::Publications | Family::Security) {
        rows.push(Row {
            status: Status::Gap,
            kind: 24,
            refs: [ZERO; 3],
            counts: [0; 3],
        });
    }
    rows
}

fn project(source: &MigrationSource<'_, '_, '_>, expected: impl Fn(Family) -> Vec<Row>) -> Vec<u8> {
    let mut bytes = Vec::new();
    let mut sink = |row: &[u8]| {
        bytes.extend_from_slice(row);
        Ok(())
    };
    let mut stream = Stream {
        source,
        sink: &mut sink,
        encoder: encoder(),
        statuses: [0; 7],
        complete: true,
        preparation_links: None,
        control_view: None,
    };
    let mut oracle = encoder();
    let mut oracle_bytes = Vec::new();
    let mut captured = None;
    for family in FAMILIES {
        stream.statuses = [0; 7];
        stream.complete = true;
        scan::control(&mut stream, source.inventory_root(), family).unwrap();
        let view = stream.control_view.as_ref().unwrap();
        if let Some(original) = captured.as_ref() {
            assert!(Arc::ptr_eq(original, view));
        } else {
            captured = Some(Arc::clone(view));
        }
        let mut observed = Vec::new();
        view.project(source, family, |row| {
            observed.push(row);
            Ok(())
        })
        .unwrap();
        // Tails belong to the original family producer, not physical observations.
        let mut rows = expected(family);
        let physical = if family == Family::Residue {
            rows.len()
        } else {
            rows.len() - 1
        };
        assert_eq!(observed, rows[..physical]);
        for row in rows.drain(..) {
            oracle_bytes.extend(
                oracle
                    .item(
                        family,
                        row.status,
                        row.kind,
                        row.refs.map(Sha256Digest32::from_bytes),
                        row.counts,
                    )
                    .unwrap(),
            );
        }
        let terminal = stream.terminal();
        let complete = stream.complete;
        let row = stream
            .encoder
            .end_family(family, terminal, complete)
            .unwrap();
        stream.send(&row).unwrap();
        let (expected_terminal, expected_complete) = if family == Family::Residue && physical == 1 {
            (Status::Present, true)
        } else {
            (Status::Gap, false)
        };
        assert_eq!((terminal, complete), (expected_terminal, expected_complete));
        oracle_bytes.extend(
            oracle
                .end_family(family, expected_terminal, expected_complete)
                .unwrap(),
        );
    }
    drop(stream);
    assert_eq!(
        bytes, oracle_bytes,
        "exact canonical row bytes/order and all family terminals"
    );
    bytes
}

#[test]
fn control_view_projects_overlap_without_body_reads_and_refuses_partial_install() {
    let scratch = Scratch::new();
    let store = crate::plaintext_direct_store::DirectStore::open(&scratch.0).unwrap();
    let source = MigrationSource::new(&store, Instant::now() + Duration::from_secs(120), None);
    assert!(source.native().is_none(), "NTFS component proof only");
    let control = source.inventory_root().join("control");
    fs::create_dir_all(control.join(format!("{OVERLAP}/nested"))).unwrap();
    for (relative, body) in [
        (
            format!("{OVERLAP}/control-cutover.v1"),
            b"nested".as_slice(),
        ),
        (format!("{OVERLAP}/nested/leaf"), b"leaf"),
        (FLAT.to_owned(), b"flat!"),
        (PUBLICATION.to_owned(), b"pub"),
        ("unowned".to_owned(), b"unknown"),
    ] {
        fs::write(control.join(relative), body).unwrap();
    }
    let marker = marker();
    let marker_path = control.join("control-cutover.v1");
    fs::write(&marker_path, marker.encode()).unwrap();
    assert_eq!(
        ControlCutoverMarker::decode(&marker.encode()).unwrap(),
        marker
    );
    // Real sharing denial prevents body opens, while metadata remains observable.
    let denied_paths = [
        control.join(FLAT),
        control.join(format!("{OVERLAP}/control-cutover.v1")),
    ];
    let held = denied_paths.each_ref().map(|path| {
        OpenOptions::new()
            .write(true)
            .share_mode(0)
            .open(path)
            .unwrap()
    });
    for path in &denied_paths {
        assert!(
            fs::read(path).is_err(),
            "body access must actually be denied"
        );
        assert!(matches!(
            io::metadata(&source, path).unwrap(),
            io::Observed::Object(_)
        ));
    }
    let first = project(&source, |family| expected(&source, family, &marker));
    assert!(!first.is_empty());
    // A fresh Stream must recapture. Failure after the earlier DFS entries cannot
    // install or emit their partial prefix, or fall back to the previous view.
    OpenOptions::new()
        .write(true)
        .open(&marker_path)
        .unwrap()
        .set_len(io::CONTROL_BYTES as u64 + 1)
        .unwrap();
    let mut prefix = Vec::new();
    let mut sink = |row: &[u8]| {
        prefix.extend_from_slice(row);
        Ok(())
    };
    let mut stream = Stream {
        source: &source,
        sink: &mut sink,
        encoder: encoder(),
        statuses: [0; 7],
        complete: true,
        preparation_links: None,
        control_view: None,
    };
    assert_eq!(
        scan::control(&mut stream, source.inventory_root(), Family::Residue),
        Err(limit())
    );
    assert!(stream.control_view.is_none());
    drop(stream);
    assert!(prefix.is_empty());
    drop(held);

    let empty = Scratch::new();
    let store = crate::plaintext_direct_store::DirectStore::open(&empty.0).unwrap();
    let source = MigrationSource::new(&store, Instant::now() + Duration::from_secs(120), None);
    project(&source, |family| {
        let mut rows = vec![row(
            Status::Present,
            36,
            io::locator(&source, &source.inventory_root().join("control")).unwrap(),
            [0; 3],
        )];
        if family == Family::Operations {
            rows.push(Row {
                status: Status::Gap,
                kind: 348,
                refs: [ZERO; 3],
                counts: [348, 0, 0],
            });
        } else if family != Family::Residue {
            rows.push(Row {
                status: Status::Gap,
                kind: 24,
                refs: [ZERO; 3],
                counts: [0; 3],
            });
        }
        rows
    });
}

#[test]
fn control_view_admits_slots_fanout_and_allocation_before_marker_io() {
    let scratch = Scratch::new();
    let store = crate::plaintext_direct_store::DirectStore::open(&scratch.0).unwrap();
    let source = MigrationSource::new(&store, Instant::now() + Duration::from_secs(120), None);
    assert!(
        source.native().is_none(),
        "component admission ordering only"
    );
    let path = source.inventory_root().join("control/control-cutover.v1");
    let encoded = marker().encode();
    fs::write(&path, &encoded).unwrap();
    assert_eq!(
        &*io::read(&source, &path, io::CONTROL_BYTES)
            .unwrap()
            .unwrap(),
        &encoded
    );
    let subject = io::locator(&source, &path).unwrap();
    let prefix = encoder();
    let empty = || Capture::new(&prefix, Family::Residue).unwrap();

    let mut exhausted = empty();
    exhausted.view.projected_rows = LegacyInventoryLimits::BASELINE.records;
    assert_eq!(
        exhausted.entry(&source, &path, subject, PUBLICATIONS, None, false),
        Err(limit())
    );
    assert_eq!(
        exhausted.view.entry_io_attempts, 0,
        "refused slot reaches neither metadata nor marker read"
    );
    assert_eq!(exhausted.view.events.capacity(), 0);
    assert_eq!(exhausted.view.reserved_slots, 0);

    let mut fanout = empty();
    fanout.view.projected_rows = LegacyInventoryLimits::BASELINE.records - 3;
    let overlap_path = source.inventory_root().join("control").join(FLAT);
    fs::write(&overlap_path, b"unsupported overlap body").unwrap();
    let overlap_subject = io::locator(&source, &overlap_path).unwrap();
    assert_eq!(
        fanout.entry(&source, &overlap_path, overlap_subject, ALL, None, true),
        Err(limit())
    );
    assert_eq!(
        fanout.view.entry_io_attempts, 0,
        "all projected families are admitted before I/O"
    );
    assert_eq!(
        fanout.view.projected_rows,
        LegacyInventoryLimits::BASELINE.records - 3
    );

    // Empty capacity must be charged to the same original context before the
    // actual metadata/marker seam, not after an observation has already escaped.
    let expired = MigrationSource::new(&store, Instant::now(), None);
    let mut allocation = empty();
    assert_eq!(
        allocation.entry(&expired, &path, subject, PUBLICATIONS, None, false),
        Err("DIRECT_MIGRATION_DEADLINE_EXCEEDED".to_owned())
    );
    assert_eq!(allocation.view.entry_io_attempts, 0);
    assert_eq!(allocation.view.events.capacity(), 0);
    assert_eq!(allocation.view.reserved_slots, 0);
    assert_eq!(allocation.view.projected_rows, 0);

    // Pending post-order directory outcomes consume capacity alongside filled
    // rows. Filling their reservations must never grow the backing allocation.
    let mut pending = empty();
    let slots = (0..9)
        .map(|_| pending.reserve(&source, ALL, Sizing::Outcome).unwrap())
        .collect::<Vec<_>>();
    assert_eq!(pending.view.reserved_slots, 9);
    assert_eq!(pending.view.projected_rows, 36);
    assert!(pending.view.events.capacity() >= 9);
    let capacity = pending.view.events.capacity();
    assert_eq!(
        pending
            .view
            .project(&source, Family::Residue, |_| panic!("unfinished view")),
        Err(changed())
    );
    for slot in slots {
        pending.fill(slot, row(Status::Present, 36, subject, [0; 3]), false);
        assert_eq!(pending.view.events.capacity(), capacity);
    }
    assert_eq!(pending.view.reserved_slots, 0);
    assert_eq!(pending.view.events.len(), 9);
    assert_eq!(fs::read(&path).unwrap(), encoded);
}

fn emit_prefix(encoder: &mut LegacyInventoryEncoder, family: Family, count: u64) {
    for _ in 0..count {
        encoder
            .item(
                family,
                Status::Present,
                27,
                [Sha256Digest32::from_bytes(ZERO); 3],
                [0; 3],
            )
            .unwrap();
    }
}

#[test]
fn owner_remaining_prefix_refuses_control_io_and_rechecks_cached_projections() {
    let scratch = Scratch::new();
    let store = crate::plaintext_direct_store::DirectStore::open(&scratch.0).unwrap();
    let source = MigrationSource::new(&store, Instant::now() + Duration::from_secs(120), None);
    assert!(
        source.native().is_none(),
        "owner accounting plus actual NTFS component only"
    );
    let path = source.inventory_root().join("control/control-cutover.v1");
    let encoded = marker().encode();
    fs::write(&path, &encoded).unwrap();
    let subject = io::locator(&source, &path).unwrap();
    let future = known_future(Family::Residue).unwrap();
    assert_eq!(
        future.0, 12,
        "three gaps, seven fixed items, two later directory outcomes"
    );

    // Genuine owner-encoded prefix, not a caller-assigned control-view count.
    let mut record_prefix = encoder_with_limits(LegacyInventoryLimits {
        records: 32,
        encoded_bytes: 65_536,
        ..LegacyInventoryLimits::BASELINE
    });
    let available = record_prefix.preflight_remaining(0, 0).unwrap();
    emit_prefix(
        &mut record_prefix,
        Family::Residue,
        available.records - future.0 - 4,
    );
    let before = record_prefix.preflight_remaining(0, 0).unwrap();
    let mut capture = Capture::new(&record_prefix, Family::Residue).unwrap();
    let _outcome = capture.reserve(&source, ALL, Sizing::Outcome).unwrap();
    assert_eq!(
        capture.entry(&source, &path, subject, PUBLICATIONS, None, false),
        Err(limit())
    );
    assert_eq!(capture.view.entry_io_attempts, 0);
    assert_eq!(
        capture.view.reserved_slots, 1,
        "rejected entry spends no slot"
    );
    assert_eq!(record_prefix.preflight_remaining(0, 0).unwrap(), before);

    let mut bytes = Vec::new();
    let mut sink = |row: &[u8]| {
        bytes.extend_from_slice(row);
        Ok(())
    };
    let mut stream = Stream {
        source: &source,
        sink: &mut sink,
        encoder: record_prefix,
        statuses: [0; 7],
        complete: true,
        preparation_links: None,
        control_view: None,
    };
    assert_eq!(
        scan::control(&mut stream, source.inventory_root(), Family::Residue),
        Err(limit())
    );
    assert!(stream.control_view.is_none());
    drop(stream);
    assert!(bytes.is_empty());

    // Derive a near-full byte ceiling from owner remaining capacity. Keeping
    // the same decimal width preserves the owner's header/footer byte lengths.
    let limits = LegacyInventoryLimits {
        records: 512,
        encoded_bytes: 65_536,
        ..LegacyInventoryLimits::BASELINE
    };
    let mut sizing = encoder_with_limits(limits);
    emit_prefix(&mut sizing, Family::Residue, 100);
    let remaining = sizing.preflight_remaining(0, 0).unwrap();
    let sizing_capture = Capture::new(&sizing, Family::Residue).unwrap();
    let outcome_bytes = sizing_capture.outcome_bytes.iter().sum::<u64>();
    let entry_bytes = sizing_capture.entry_bytes[2];
    let ceiling =
        limits.encoded_bytes - remaining.encoded_bytes + future.1 + outcome_bytes + entry_bytes - 1;
    assert!((10_000..65_536).contains(&ceiling));
    let mut byte_prefix = encoder_with_limits(LegacyInventoryLimits {
        encoded_bytes: ceiling,
        ..limits
    });
    emit_prefix(&mut byte_prefix, Family::Residue, 100);
    assert_eq!(
        byte_prefix
            .preflight_remaining(future.0 + 4, future.1 + outcome_bytes)
            .unwrap()
            .encoded_bytes,
        entry_bytes - 1
    );
    let mut capture = Capture::new(&byte_prefix, Family::Residue).unwrap();
    let _outcome = capture.reserve(&source, ALL, Sizing::Outcome).unwrap();
    assert_eq!(
        capture.entry(&source, &path, subject, PUBLICATIONS, None, false),
        Err(limit())
    );
    assert_eq!(capture.view.entry_io_attempts, 0);

    // After Residue has actually encoded, its cached rows and its two future
    // outcomes must not be charged again. New Operations prefix rows do count.
    let mut later = encoder_with_limits(LegacyInventoryLimits {
        records: 64,
        encoded_bytes: 65_536,
        ..LegacyInventoryLimits::BASELINE
    });
    let view =
        ControlView::capture(&source, source.inventory_root(), &later, Family::Residue).unwrap();
    view.project(&source, Family::Residue, |row| {
        later
            .item(
                Family::Residue,
                row.status,
                row.kind,
                row.refs.map(Sha256Digest32::from_bytes),
                row.counts,
            )
            .map(|_| ())
            .map_err(super::super::codec_reason)
    })
    .unwrap();
    later
        .end_family(Family::Residue, Status::Present, true)
        .unwrap();
    let required =
        known_future(Family::Operations).unwrap().0 + view.pending_records[1..].iter().sum::<u64>();
    let available = later.preflight_remaining(0, 0).unwrap().records;
    emit_prefix(&mut later, Family::Operations, available - required);
    view.preflight(&later, Family::Operations)
        .expect("already emitted Residue is excluded");
    emit_prefix(&mut later, Family::Operations, 1);
    let view = Arc::new(view);
    let attempts = view.entry_io_attempts;
    let mut bytes = Vec::new();
    let mut sink = |row: &[u8]| {
        bytes.extend_from_slice(row);
        Ok(())
    };
    let mut stream = Stream {
        source: &source,
        sink: &mut sink,
        encoder: later,
        statuses: [0; 7],
        complete: true,
        preparation_links: None,
        control_view: Some(Arc::clone(&view)),
    };
    assert_eq!(
        scan::control(&mut stream, source.inventory_root(), Family::Operations),
        Err(limit())
    );
    assert!(Arc::ptr_eq(stream.control_view.as_ref().unwrap(), &view));
    drop(stream);
    assert!(bytes.is_empty());
    assert_eq!(view.entry_io_attempts, attempts);
    assert_eq!(fs::read(path).unwrap(), encoded);
}

//! Real NTFS tuple parity over the original physical traversal kernel.
//! Component source context is absent; this is not NativeCtx or grant qualification.
use super::*;
use super::super::revision_residue::{Capture, Row, View};
use search_contracts::Sha256Digest32;
use search_control_redb::migration::{LegacyInventoryBinding, LegacyInventoryDecoder,
    LegacyInventoryDecodedRow, LegacyInventoryEncoder, LegacyInventoryLimits, LegacyInventoryOwner};
use std::fs;
use std::path::PathBuf;
use std::sync::atomic::{AtomicU64, Ordering};
use std::time::{Duration, Instant};

struct Scratch(PathBuf);
impl Scratch {
    fn new() -> Self {
        static NEXT: AtomicU64 = AtomicU64::new(0);
        let temp = fs::canonicalize(std::env::temp_dir()).unwrap();
        let path = temp.join(format!("search-revision-residue-{}-{}",
            std::process::id(), NEXT.fetch_add(1, Ordering::Relaxed)));
        fs::create_dir(&path).unwrap();
        fs::create_dir(path.join("data")).unwrap();
        Self(path)
    }
}
impl Drop for Scratch {
    fn drop(&mut self) {
        let current = fs::canonicalize(&self.0).unwrap();
        assert_eq!(current, self.0);
        assert_eq!(current.parent(), Some(fs::canonicalize(std::env::temp_dir()).unwrap().as_path()));
        assert!(current.file_name().unwrap().to_str().unwrap().starts_with("search-revision-residue-"));
        fs::remove_dir_all(&current).unwrap();
    }
}

fn binding() -> LegacyInventoryBinding {
    LegacyInventoryBinding {
        owner: LegacyInventoryOwner { installation: [1; 16], incarnation: [2; 16], root: [3; 16],
            invocation: [4; 16], request_blake3: [5; 32], record_blake3: [6; 32], physical_blake3: [7; 32],
            native_limits: [1; 6], epoch: 1, generation: 1 },
        target: SourceNamespaceId::from_bytes([8; 16]),
        namespace: Sha256Digest32::from_bytes([9; 32]), snapshot: Sha256Digest32::from_bytes([10; 32]),
        registration: None,
    }
}

fn state(family: Family, limits: LegacyInventoryLimits) -> (LegacyInventoryEncoder, LegacyInventoryDecoder) {
    let mut encoder = LegacyInventoryEncoder::with_profile(limits, LegacyInventoryProfile::LocationV2).unwrap();
    let mut decoder = LegacyInventoryDecoder::with_profile(binding(), limits, LegacyInventoryProfile::LocationV2).unwrap();
    decoder.push(&encoder.header(binding()).unwrap()).unwrap();
    for before in Family::ALL.into_iter().take(family as usize) {
        decoder.push(&encoder.end_family(before, Status::AbsentProven, true).unwrap()).unwrap();
    }
    (encoder, decoder)
}

fn physical(source: &super::super::MigrationSource<'_, '_, '_>, residue_only: bool,
    limits: LegacyInventoryLimits, installed: &mut Option<View>,
) -> (Result<(), String>, Vec<Row>) {
    let family = if residue_only { Family::Residue } else { Family::Revisions };
    let (encoder, mut decoder) = state(family, limits);
    let mut rows = Vec::new();
    let mut sink = |bytes: &[u8]| {
        match decoder.push(bytes).map_err(|_| changed())? {
            LegacyInventoryDecodedRow::Item { status, kind, refs, counts, .. } => {
                rows.push(Row { status, kind, refs: refs.map(|digest| *digest.as_bytes()), counts });
                Ok(())
            }
            _ => Err(changed()),
        }
    };
    let mut stream = Stream { source, sink: &mut sink, encoder, statuses: [0; 7], complete: true,
        preparation_links: None, control_view: None };
    let result = if residue_only {
        revision_tree(&mut stream, source.inventory_root(), family, true, None, None).map(|_| ())
    } else {
        Capture::new(source).and_then(|mut capture| {
            let scope = revision_tree(&mut stream, source.inventory_root(), family, false, None, Some(&mut capture))?;
            *installed = Some(capture.finish(scope)?);
            Ok(())
        })
    };
    (result, rows)
}

fn project(source: &super::super::MigrationSource<'_, '_, '_>, view: &View) -> Vec<Row> {
    let (encoder, _) = state(Family::Residue, LegacyInventoryLimits::BASELINE);
    view.preflight(&encoder).unwrap();
    let mut rows = Vec::new();
    view.project(source, |row| { rows.push(row); Ok(()) }).unwrap();
    rows
}

#[test]
fn revision_residue_matches_old_order_with_orphans_unknowns_hardlinks_and_depth() {
    let scratch = Scratch::new();
    let data = scratch.0.join("data");
    let input = scratch.0.join("source.rs");
    fs::write(&input, b"pub fn retained() {}\n").unwrap();
    let mut store = crate::plaintext_direct_store::DirectStore::open(&data).unwrap();
    store.index_file(&input).unwrap();
    let retained = store.inventory_revisions().next().unwrap().revision_id.clone();
    fs::create_dir_all(data.join("revisions/aa")).unwrap();
    let orphan = "a".repeat(64);
    fs::write(data.join(format!("revisions/aa/{orphan}.bin")), b"orphan plain").unwrap();
    fs::write(data.join(format!("revisions/aa/{orphan}.dpapi")), b"orphan protected wire").unwrap();
    fs::write(data.join(format!("revisions/aa/.{orphan}.1.tmp")), b"temporary").unwrap();
    fs::write(data.join("revisions/aa/unknown"), b"unsupported physical body").unwrap();
    fs::write(data.join(format!("revisions/aa/{}.bin", "b".repeat(64))), b"wrong shard").unwrap();
    let hard = format!("aa{}", "1".repeat(62));
    let hard_path = data.join(format!("revisions/aa/{hard}.bin"));
    fs::write(&hard_path, b"hardlinked").unwrap();
    fs::hard_link(&hard_path, scratch.0.join("second-link")).unwrap();
    assert!(eliot_searchd::native_file::observe(&fs::File::open(&hard_path).unwrap()).is_ok(), "NTFS required");
    fs::create_dir_all(data.join("revisions/aa/deep/a/b/c/d/e/f/g/h")).unwrap();
    fs::write(data.join("revisions/aa/deep/a/b/c/d/e/f/g/h/never-visited"), b"not another denominator").unwrap();
    fs::create_dir_all(data.join("revisions/not-a-shard/deep")).unwrap();
    let source = super::super::MigrationSource::new(&store, Instant::now() + Duration::from_secs(120), None);
    let (old, expected) = physical(&source, true, LegacyInventoryLimits::BASELINE, &mut None);
    old.unwrap();
    let mut cached = None;
    let (result, revisions) = physical(&source, false, LegacyInventoryLimits::BASELINE, &mut cached);
    result.unwrap();
    let observed = project(&source, cached.as_ref().unwrap());
    assert_eq!(observed, expected, "every ordered status/kind/reference/count tuple");
    let retained = digest(&retained).unwrap();
    assert!(revisions.iter().any(|row| row.kind == 10 && row.refs[2] == retained));
    assert!(!observed.iter().any(|row| row.kind == 10 && row.refs[2] == retained));
    assert!(observed.iter().any(|row| row.status == Status::Orphan && row.kind == 11));
    assert!(observed.iter().any(|row| row.status == Status::Gap && row.refs[2] == digest(&hard).unwrap()));
    assert!(observed.iter().any(|row| row.status == Status::Corrupt));
    assert!(observed.iter().any(|row| row.status == Status::Unsupported));
    assert!(observed.iter().any(|row| row.kind == 36));
}

#[test]
fn revision_residue_missing_tree_preserves_directory_outcome() {
    let scratch = Scratch::new();
    let store = crate::plaintext_direct_store::DirectStore::open(&scratch.0.join("data")).unwrap();
    fs::remove_dir(scratch.0.join("data/revisions")).unwrap();
    let source = super::super::MigrationSource::new(&store, Instant::now() + Duration::from_secs(30), None);
    let (old, expected) = physical(&source, true, LegacyInventoryLimits::BASELINE, &mut None);
    old.unwrap();
    let mut cached = None;
    physical(&source, false, LegacyInventoryLimits::BASELINE, &mut cached).0.unwrap();
    let observed = project(&source, cached.as_ref().unwrap());
    assert_eq!(observed, expected);
    assert_eq!(observed.len(), 1);
    assert_eq!((observed[0].status, observed[0].kind), (Status::AbsentProven, 16));
}

#[test]
fn revision_residue_error_prefix_never_installs_view() {
    let scratch = Scratch::new();
    let data = scratch.0.join("data");
    let store = crate::plaintext_direct_store::DirectStore::open(&data).unwrap();
    fs::create_dir_all(data.join("revisions/aa")).unwrap();
    fs::write(data.join("revisions/aa/first"), b"already observed").unwrap();
    fs::File::create(data.join("revisions/aa/zz-limit")).unwrap()
        .set_len(LEGACY_REVISION_MAX_OBJECT_BYTES as u64 + 1).unwrap();
    let source = super::super::MigrationSource::new(&store, Instant::now() + Duration::from_secs(30), None);
    let mut cached = None;
    let (result, prefix) = physical(&source, false, LegacyInventoryLimits::BASELINE, &mut cached);
    assert_eq!(result, Err("DIRECT_MIGRATION_INVENTORY_LIMIT".to_owned()));
    assert!(!prefix.is_empty());
    assert!(cached.is_none());
}

#[test]
fn revision_residue_capacity_refuses_before_first_physical_row() {
    let scratch = Scratch::new();
    let data = scratch.0.join("data");
    let store = crate::plaintext_direct_store::DirectStore::open(&data).unwrap();
    fs::remove_dir(data.join("revisions")).unwrap();
    fs::write(data.join("revisions"), b"not a directory").unwrap();
    let source = super::super::MigrationSource::new(&store, Instant::now() + Duration::from_secs(30), None);
    let mut cached = None;
    let limits = LegacyInventoryLimits { records: 14, ..LegacyInventoryLimits::BASELINE };
    let (result, prefix) = physical(&source, false, limits, &mut cached);
    assert_eq!(result, Err("DIRECT_MIGRATION_INVENTORY_LIMIT".to_owned()));
    assert!(prefix.is_empty());
    assert!(cached.is_none());
}

#[test]
fn revision_residue_projection_is_history_and_next_pass_is_fresh() {
    let scratch = Scratch::new();
    let data = scratch.0.join("data");
    let store = crate::plaintext_direct_store::DirectStore::open(&data).unwrap();
    fs::create_dir_all(data.join("revisions/aa")).unwrap();
    let path = data.join("revisions/aa/unknown");
    fs::write(&path, b"first pass").unwrap();
    let source = super::super::MigrationSource::new(&store, Instant::now() + Duration::from_secs(30), None);
    let mut first = None;
    physical(&source, false, LegacyInventoryLimits::BASELINE, &mut first).0.unwrap();
    let rows = project(&source, first.as_ref().unwrap());
    fs::write(&path, b"second independently observed pass").unwrap();
    assert_eq!(project(&source, first.as_ref().unwrap()), rows);
    let mut second = None;
    physical(&source, false, LegacyInventoryLimits::BASELINE, &mut second).0.unwrap();
    let fresh = project(&source, second.as_ref().unwrap());
    assert_ne!(fresh, rows);
    let (old, expected) = physical(&source, true, LegacyInventoryLimits::BASELINE, &mut None);
    old.unwrap();
    assert_eq!(fresh, expected);
}

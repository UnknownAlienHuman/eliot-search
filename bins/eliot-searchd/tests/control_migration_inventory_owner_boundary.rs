//! Component proof only: real Windows inventory I/O with a bounded synthetic
//! allowance. This does not qualify native-owner admission or installed startup.
#![cfg(windows)]

use std::cell::Cell;
use std::marker::PhantomData;
use std::path::{Path, PathBuf};
use search_control_redb::migration::LegacyInventoryStatus as Status;
use search_revision_store::LegacyRevisionObjectWork as Work;

struct MigrationSource<'a, 'b, 'c> {
    root: &'a Path,
    calls: Cell<usize>,
    refusal: Cell<bool>,
    _lifetimes: PhantomData<(&'b (), &'c ())>,
}
impl MigrationSource<'_, '_, '_> {
    fn inventory_root(&self) -> &Path { self.root }
    fn charge(&self, _: Work) -> Result<(), String> {
        self.checkpoint()?;
        self.calls.set(self.calls.get() + 1);
        Ok(())
    }
    fn checkpoint(&self) -> Result<(), String> {
        if self.refusal.get() { Err("ORIGINAL_CONTEXT_REFUSED".to_owned()) } else { Ok(()) }
    }
}
#[path = "../src/sha256.rs"]
mod sha256;
#[path = "../src/control_migration_inventory/io.rs"]
mod inventory_io;

struct Fixture(PathBuf);
impl Fixture {
    fn new() -> Self {
        static NEXT: std::sync::atomic::AtomicU64 = std::sync::atomic::AtomicU64::new(0);
        let now = std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).unwrap().as_nanos();
        let id = NEXT.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
        let path = std::env::temp_dir().join(format!("eliot-491-{}-{now}-{id}", std::process::id()));
        std::fs::create_dir(&path).unwrap();
        Self(path)
    }
    fn source(&self) -> MigrationSource<'_, '_, '_> {
        MigrationSource { root: &self.0, calls: Cell::new(0), refusal: Cell::new(false), _lifetimes: PhantomData }
    }
}
impl Drop for Fixture {
    fn drop(&mut self) {
        // Only this fixture's unique directory, never source/user state.
        std::fs::remove_dir_all(&self.0).unwrap();
    }
}

#[test]
fn ordered_windows_pages_cover_more_than_one_page_exactly_once() {
    let fixture = Fixture::new();
    for id in (0..97).rev() { std::fs::write(fixture.0.join(format!("{id:03}")), []).unwrap(); }
    let source = fixture.source();
    let first = inventory_io::page(&source, &fixture.0, None).unwrap().unwrap();
    assert_eq!(first.len(), 32);
    assert_eq!(first[0].value, "000");
    assert_eq!(first[31].value, "031");
    let mut names = Vec::new();
    assert_eq!(inventory_io::visit(&source, &fixture.0, |name| { names.push(name.value.clone()); Ok(()) }).unwrap(), None);
    assert_eq!(names.len(), 97);
    assert!(names.windows(2).all(|pair| pair[0] < pair[1]));
    assert_eq!(names.last().unwrap(), "096");
}

#[test]
fn same_handle_bytes_are_exact_and_over_limit_never_read_or_truncated() {
    let fixture = Fixture::new();
    let path = fixture.0.join("object.bin");
    let bytes = vec![0xA5; 131_073];
    std::fs::write(&path, &bytes).unwrap();
    let source = fixture.source();
    assert_eq!(inventory_io::read(&source, &path, bytes.len()).unwrap().unwrap().as_slice(), bytes);
    assert_eq!(inventory_io::read(&source, &path, bytes.len() - 1).unwrap_err(), "DIRECT_MIGRATION_INVENTORY_LIMIT");
}

#[test]
fn missing_file_and_non_file_are_distinct_dispositions() {
    let fixture = Fixture::new();
    let source = fixture.source();
    assert!(matches!(inventory_io::read(&source, &fixture.0.join("missing"), 8).unwrap(), Err(Status::AbsentProven)));
    let directory = fixture.0.join("directory");
    std::fs::create_dir(&directory).unwrap();
    assert!(matches!(inventory_io::read(&source, &directory, 8).unwrap(), Err(Status::Corrupt)));
}

#[test]
fn escaped_parent_is_refused_before_io() {
    let fixture = Fixture::new();
    let source = fixture.source();
    let escaped = fixture.0.join("..").join("unowned.bin");
    assert_eq!(inventory_io::read(&source, &escaped, 8).unwrap_err(), "DIRECT_MIGRATION_INVENTORY_CHANGED");
}

#[test]
fn original_context_refusal_stops_before_any_new_observation() {
    let fixture = Fixture::new();
    let source = fixture.source();
    source.refusal.set(true);
    assert_eq!(inventory_io::page(&source, &fixture.0, None).err().unwrap(), "ORIGINAL_CONTEXT_REFUSED");
    assert_eq!(source.calls.get(), 0);
}

#[test]
fn page_callback_drift_cannot_turn_disappearance_into_complete_absence() {
    let fixture = Fixture::new();
    let directory = fixture.0.join("objects");
    std::fs::create_dir(&directory).unwrap();
    std::fs::write(directory.join("000"), [1]).unwrap();
    let source = fixture.source();
    let outcome = inventory_io::visit(&source, &directory, |_| {
        std::fs::write(directory.join("new"), [2]).unwrap();
        Ok(())
    });
    assert_eq!(outcome.unwrap_err(), "DIRECT_MIGRATION_INVENTORY_CHANGED");
}

#[test]
fn visit_retains_leaf_directory_and_blocks_replacement_between_pages() {
    let fixture = Fixture::new();
    let directory = fixture.0.join("objects");
    std::fs::create_dir(&directory).unwrap();
    std::fs::write(directory.join("000"), [1]).unwrap();
    let source = fixture.source();
    let mut visited = 0;
    assert_eq!(inventory_io::visit(&source, &directory, |_| {
        assert!(std::fs::rename(&directory, fixture.0.join("replacement")).is_err());
        visited += 1;
        Ok(())
    }).unwrap(), None);
    assert_eq!(visited, 1);
}

#[test]
fn multi_link_object_is_a_gap_and_no_content_is_returned() {
    let fixture = Fixture::new();
    let object = fixture.0.join("original.bin");
    std::fs::write(&object, b"outside-domain").unwrap();
    std::fs::hard_link(&object, fixture.0.join("alias.bin")).unwrap();
    let source = fixture.source();
    assert!(matches!(inventory_io::read(&source, &object, 64).unwrap(), Err(Status::Gap)));
}

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
    revisions: Vec<FixtureRevision>,
    _lifetimes: PhantomData<(&'b (), &'c ())>,
}
impl MigrationSource<'_, '_, '_> {
    fn inventory_root(&self) -> &Path { self.root }
    fn inventory_namespace(&self) -> [u8; 32] { [0x12; 32] }
    fn inventory_revisions(&self) -> impl ExactSizeIterator<Item = &FixtureRevision> { self.revisions.iter() }
    fn native(&self) -> Option<FixtureInput> { Some(FixtureInput { protector: FixtureProtector }) }
    fn charge(&self, _: Work) -> Result<(), String> {
        self.checkpoint()?;
        self.calls.set(self.calls.get() + 1);
        Ok(())
    }
    fn checkpoint(&self) -> Result<(), String> {
        if self.refusal.get() { Err("ORIGINAL_CONTEXT_REFUSED".to_owned()) } else { Ok(()) }
    }
}
struct FixtureRevision { source_id: String, revision_id: String, content_digest: String, byte_length: u64 }
struct FixtureInput { protector: FixtureProtector }
struct FixtureProtector;
impl FixtureProtector { fn backend_name(&self) -> &'static str { "fixture-preparation" } }
mod direct_preparation {
    pub fn canonical_materializer_digest() -> Result<[u8; 32], &'static str> { Ok([0x13; 32]) }
    pub fn canonical_unitizer_digest() -> Result<[u8; 32], &'static str> { Ok([0x14; 32]) }
}
mod scan {
    use search_materializer::api::LegacyPreparationStoreDigest;
    pub struct PreparationRepresentationDigest;
    impl LegacyPreparationStoreDigest for PreparationRepresentationDigest {
        fn digest(bytes: &[u8]) -> [u8; 32] { super::sha256::digest(bytes) }
        fn digest_parts(domain: &[u8], parts: &[&[u8]]) -> [u8; 32] { super::sha256::digest_parts(domain, parts) }
    }
    pub fn digest(value: &str) -> Result<[u8; 32], String> { super::sha256::decode_digest(value).ok_or_else(changed) }
    pub fn changed() -> String { "DIRECT_MIGRATION_INVENTORY_CHANGED".to_owned() }
}
#[path = "../src/sha256.rs"]
mod sha256;
#[path = "../src/control_migration_inventory/io.rs"]
mod inventory_io;
use inventory_io as io;
#[path = "../src/control_migration_inventory/preparation_links.rs"]
mod preparation_links;

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
        MigrationSource { root: &self.0, calls: Cell::new(0), refusal: Cell::new(false), revisions: Vec::new(), _lifetimes: PhantomData }
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
        // NTFS directory-write timestamps can share a coarse clock tick. Make
        // this timestamp-boundary fixture cross it before changing membership.
        std::thread::sleep(std::time::Duration::from_millis(50));
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

#[test]
fn stale_unmapped_preparation_is_not_a_current_link_and_dangling_links_are_refused() {
    use search_materializer::api::{encode_legacy_preparation_binding, encode_legacy_preparation_reference,
        derive_legacy_preparation_lookup_key, derive_legacy_preparation_object_id,
        legacy_preparation_object_relative_locator, legacy_preparation_reference_relative_locator,
        LegacyDirectPreparationBinding, LegacyPreparationProtection, LegacyPreparationInventoryTree as Tree};
    let fixture = Fixture::new();
    let mut source = fixture.source();
    source.revisions.push(FixtureRevision { source_id: "21".repeat(32), revision_id: "22".repeat(32),
        content_digest: "23".repeat(32), byte_length: 17 });
    let binding = encode_legacy_preparation_binding(&LegacyDirectPreparationBinding {
        namespace: [0x12; 32], source_id: [0x21; 32], revision_id: [0x22; 32], content_digest: [0x23; 32],
        byte_length: 17, materializer_digest: [0x13; 32], unitizer_digest: [0x14; 32] });
    let key = derive_legacy_preparation_lookup_key::<scan::PreparationRepresentationDigest>(&binding, "fixture-preparation");
    let object = derive_legacy_preparation_object_id::<scan::PreparationRepresentationDigest>(&binding, "fixture-preparation", &[0x24; 32]);
    let reference_path = fixture.0.join("preparation").join(legacy_preparation_reference_relative_locator(&key));
    let object_path = fixture.0.join("preparation").join(legacy_preparation_object_relative_locator(&object, LegacyPreparationProtection::Plaintext));
    std::fs::create_dir_all(reference_path.parent().unwrap()).unwrap();
    std::fs::create_dir_all(object_path.parent().unwrap()).unwrap();
    let reference = encode_legacy_preparation_reference(&key, &[0x24; 32], 300, LegacyPreparationProtection::Plaintext).unwrap();
    std::fs::write(&reference_path, reference).unwrap();
    assert!(!preparation_links::is_linked(&source, &fixture.0, Tree::References, &key, reference_path.file_name().unwrap()).unwrap(), "missing target is a dangling link");
    std::fs::write(&object_path, b"bounded synthetic object").unwrap();
    assert!(preparation_links::is_linked(&source, &fixture.0, Tree::References, &key, reference_path.file_name().unwrap()).unwrap());
    assert!(preparation_links::is_linked(&source, &fixture.0, Tree::Objects, &object, object_path.file_name().unwrap()).unwrap());
    assert!(!preparation_links::is_linked(&source, &fixture.0, Tree::Objects, &object, std::ffi::OsStr::new("wrong.dpapi")).unwrap(), "protection locator cannot be relabelled");
    assert!(!preparation_links::is_linked(&source, &fixture.0, Tree::References, &[0x99; 32], std::ffi::OsStr::new("stale.ref")).unwrap(), "unmapped reference is not silently current");
    assert!(!preparation_links::is_linked(&source, &fixture.0, Tree::Objects, &[0x99; 32], std::ffi::OsStr::new("stale.bin")).unwrap(), "unmapped object is not silently current");
    std::fs::hard_link(&object_path, fixture.0.join("external-alias.bin")).unwrap();
    assert!(!preparation_links::is_linked(&source, &fixture.0, Tree::References, &key, reference_path.file_name().unwrap()).unwrap(), "target containment refusal cannot be downgraded to metadata presence");
}

#[test]
fn unknown_control_representation_is_incomplete_instead_of_silently_skipped() {
    let fixture = Fixture::new();
    let control = fixture.0.join("control");
    std::fs::create_dir_all(control.join("unrecognized" )).unwrap();
    std::fs::write(control.join("unrecognized/residue.tmp"), b"private bytes must not be read by unknown metadata handling").unwrap();
    let source = fixture.source();
    let mut names = Vec::new();
    assert_eq!(inventory_io::visit(&source, &control, |name| { names.push(name.value.clone()); Ok(()) }).unwrap(), None);
    assert_eq!(names, [std::ffi::OsString::from("unrecognized")]);
    // Static ownership guard: the real visitor's unmatched branch emits an
    // incomplete row before its skip, and does not dispatch a body reader.
    let scanner = include_str!("../src/control_migration_inventory/scan.rs");
    let unmatched = scanner.split("if !selected {").nth(1).unwrap().split("let subject = io::fingerprint").next().unwrap();
    assert!(unmatched.contains("family == Family::Residue"));
    let dispatch = scanner.split("if !selected {").nth(1).unwrap().split("let path = directory.join").next().unwrap();
    assert!(dispatch.contains("Status::Gap, 34"));
    assert!(!dispatch.contains("control_file("));
    assert!(!dispatch.contains("io::read("));
    let unknown_format = scanner.split("fn control_file(").nth(1).unwrap().split("match io::read(").next().unwrap();
    assert!(unknown_format.contains("strip_prefix(source.inventory_root())"));
    assert!(unknown_format.contains("parts.next().is_none()"));
    assert!(unknown_format.contains("Status::Unsupported"));
    assert!(unknown_format.contains("[subject, ZERO, ZERO]"));
}

#[test]
fn empty_temporary_control_directory_has_a_metadata_row_before_recursion() {
    let fixture = Fixture::new();
    let control = fixture.0.join("control");
    let temporary = control.join("empty.pending");
    std::fs::create_dir_all(&temporary).unwrap();
    let source = fixture.source();
    let mut directories = Vec::new();
    inventory_io::visit(&source, &control, |name| { directories.push(name.value.clone()); Ok(()) }).unwrap();
    assert_eq!(directories, [std::ffi::OsString::from("empty.pending")]);
    let mut children = 0;
    assert_eq!(inventory_io::visit(&source, &temporary, |_| { children += 1; Ok(()) }).unwrap(), None);
    assert_eq!(children, 0);
    // Source ownership guard: both the selected root container and nested
    // containers are accounted before recursion even when native I/O is empty.
    let scanner = include_str!("../src/control_migration_inventory/scan.rs");
    let initial = scanner.split("// Every known metadata container").nth(1).unwrap()
        .split("control_directory(stream, &path, family, 0)").next().unwrap();
    assert!(initial.contains("Status::Gap, 35"));
    let recursive = scanner.split("fn control_directory(").nth(1).unwrap()
        .split("control_directory(stream, &path, family, depth + 1)").next().unwrap();
    assert!(recursive.contains("Status::Gap, 35"));
}

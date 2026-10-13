//! Component native output checks with a synthetic original allowance.
//! No source-owner/complete-command/restart/installed qualification follows.
#![cfg(windows)]
use std::cell::Cell;
use std::marker::PhantomData;
use std::path::{Path, PathBuf};
use search_revision_store::LegacyRevisionObjectWork;

struct MigrationSource<'a, 'b, 'c> {
    refused: Cell<bool>,
    _borrow: PhantomData<(&'a (), &'b (), &'c ())>,
}
impl MigrationSource<'_, '_, '_> {
    fn checkpoint(&self) -> Result<(), String> {
        if self.refused.get() { Err("ORIGINAL_REFUSED".to_owned()) } else { Ok(()) }
    }
    fn charge(&self, _: LegacyRevisionObjectWork) -> Result<(), String> { self.checkpoint() }
}
#[path = "../src/control_migration_bootstrap/output.rs"]
mod output;

struct Fixture(PathBuf);
impl Fixture {
    fn new() -> Self {
        static NEXT: std::sync::atomic::AtomicU64 = std::sync::atomic::AtomicU64::new(0);
        let path = std::env::temp_dir().join(format!("eliot-bootstrap-output-{}-{}-{}",
            std::process::id(), std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH)
                .unwrap().as_nanos(), NEXT.fetch_add(1, std::sync::atomic::Ordering::Relaxed)));
        std::fs::create_dir(&path).unwrap(); Self(path)
    }
}
impl Drop for Fixture { fn drop(&mut self) { let _ = std::fs::remove_dir_all(&self.0); } }
fn source() -> MigrationSource<'static, 'static, 'static> {
    MigrationSource { refused: Cell::new(false), _borrow: PhantomData }
}

#[test]
fn actual_retained_output_prevents_ancestor_replacement_and_target_clobber() {
    let fixture = Fixture::new(); let source = source();
    let mut pins = output::Output::admit(&source, &fixture.0).unwrap();
    let (file, binding) = pins.create(&source, "fixture.owner-migration.v1.redb").unwrap();
    assert_ne!(binding.target.file, 0);
    assert!(std::fs::rename(&fixture.0, fixture.0.with_extension("moved")).is_err());
    assert!(pins.create(&source, "second.owner-migration.v1.redb").is_err());
    pins.verify(&source).unwrap();
    pins.sync_directory(&source).unwrap();
    drop(file);
}

#[test]
fn existing_target_is_preserved_and_never_reopened_as_original() {
    let fixture = Fixture::new(); let source = source();
    let target = fixture.0.join("fixture.owner-migration.v1.redb");
    std::fs::write(&target, b"foreign bytes").unwrap();
    let mut pins = output::Output::admit(&source, &fixture.0).unwrap();
    assert_eq!(pins.create(&source, "fixture.owner-migration.v1.redb").err().unwrap(),
        "DIRECT_MIGRATION_BOOTSTRAP_EXISTS");
    assert_eq!(std::fs::read(target).unwrap(), b"foreign bytes");
}

#[test]
fn original_refusal_and_relative_or_escaped_output_fail_closed() {
    let fixture = Fixture::new(); let source = source();
    assert!(output::Output::admit(&source, Path::new("relative-output")).is_err());
    let mut pins = output::Output::admit(&source, &fixture.0).unwrap();
    assert!(pins.create(&source, "../escaped.redb").is_err());
    source.refused.set(true);
    assert_eq!(pins.create(&source, "fixture.redb").err().unwrap(), "ORIGINAL_REFUSED");
    assert!(!fixture.0.join("fixture.redb").exists());
}

#[test]
fn bootstrap_uses_one_target_before_inventory_and_cannot_run_legacy_import() {
    // A source ownership guard, not a runtime command proof.
    let bootstrap = include_str!("../src/control_migration_bootstrap.rs");
    assert!(bootstrap.find("MigrationBootstrap::create(").unwrap()
        < bootstrap.find("inventory::stage_owned(").unwrap());
    assert!(!bootstrap.contains("redb_import::store("));
    assert!(!bootstrap.contains("owner.inspect("));
    let inventory = include_str!("../src/control_migration_inventory.rs");
    let owned = inventory.split("if let Some((journal, checkpoint)) = bootstrap").nth(1).unwrap()
        .split("let published = verified").next().unwrap();
    assert!(owned.contains("prepare_publication("));
    assert!(owned.contains("journal.publish_inventory("));
    let owner = include_str!("../../../crates/search-control-redb/src/persistent/bootstrap.rs");
    let dispatch = owner.split("pub fn publish_inventory").nth(1).unwrap();
    assert!(dispatch.find("self.write(ATTEMPT_KEY").unwrap() < dispatch.find("publication.publish_once(").unwrap());
    assert!(owner.contains("complete_candidate_not_performed"));
}

//! Real attribute drift through the actual revision-pin consumer, not NativeCtx qualification.
use super::*;
use std::sync::atomic::{AtomicU64, Ordering};
use std::time::{Duration, Instant};

struct Scratch(PathBuf);
impl Drop for Scratch {
    fn drop(&mut self) {
        if std::thread::panicking() { return; }
        let path = fs::canonicalize(&self.0).unwrap();
        assert_eq!(path, self.0);
        assert_eq!(path.parent(), Some(fs::canonicalize(std::env::temp_dir()).unwrap().as_path()));
        assert!(path.file_name().unwrap().to_str().unwrap().starts_with("search-revision-links-"));
        fs::remove_dir_all(path).unwrap();
    }
}

#[test]
fn revision_pin_composite_observation_refuses_real_attribute_drift() {
    static NEXT: AtomicU64 = AtomicU64::new(0);
    let path = fs::canonicalize(std::env::temp_dir()).unwrap().join(format!(
        "search-revision-links-{}-{}", std::process::id(), NEXT.fetch_add(1, Ordering::Relaxed)));
    fs::create_dir(&path).unwrap();
    let scratch = Scratch(path);
    let store = crate::plaintext_direct_store::DirectStore::open(&scratch.0).unwrap();
    let revision = "a".repeat(64);
    let path = scratch.0.join("revisions/aa");
    fs::create_dir(&path).unwrap();
    let path = path.join(format!("{revision}.bin"));
    fs::write(&path, b"same bytes").unwrap();
    let source = MigrationSource::new(&store, Instant::now() + Duration::from_secs(30), None);
    let held = PinnedRevisionRepresentations::open(&source, &revision).unwrap()
        .unwrap_or_else(|_| panic!("actual NTFS pin required"));
    let pinned = held.representation(LegacyRevisionProtection::Plaintext).unwrap();
    let original = fs::metadata(&path).unwrap().permissions();
    let mut changed_permissions = original.clone();
    changed_permissions.set_readonly(true);
    fs::set_permissions(&path, changed_permissions).unwrap();
    let metadata_stable = unchanged(pinned.metadata(), &fs::metadata(&path).unwrap());
    let observed = held.verify_same_original();
    fs::set_permissions(&path, original).unwrap();
    assert!(metadata_stable, "full native attributes, not removed Metadata guards, must detect drift");
    assert_eq!(observed, Err(changed()));
    held.verify_same_original().unwrap().unwrap();
}

//! Filesystem publication tests using synthetic encoded object bytes only.
//! These tests do not substitute for native DPAPI or encryption qualification.

use super::*;
use std::sync::{Arc, Barrier};
use std::sync::atomic::{AtomicU64, Ordering};

static NEXT: AtomicU64 = AtomicU64::new(0);

struct Scratch(PathBuf);
impl Scratch {
    fn new() -> Self {
        let stamp = SystemTime::now().duration_since(UNIX_EPOCH).unwrap().as_nanos();
        let root = std::env::temp_dir().join(format!(
            "eliot-immutable-{}-{stamp}-{}", std::process::id(), NEXT.fetch_add(1, Ordering::Relaxed),
        ));
        fs::create_dir_all(&root).unwrap();
        Self(root)
    }
    fn object(&self) -> PathBuf { self.0.join("object.dpapi") }
}
impl Drop for Scratch {
    fn drop(&mut self) { let _ = fs::remove_dir_all(&self.0); }
}

#[test]
fn immutable_object_replays_identical_bytes_without_replacement() {
    let scratch = Scratch::new();
    let path = scratch.object();
    persist_immutable_object(&path, b"encoded-object-a").unwrap();
    persist_immutable_object(&path, b"encoded-object-a").unwrap();
    assert_eq!(fs::read(&path).unwrap(), b"encoded-object-a");
    assert_eq!(fs::read_dir(&scratch.0).unwrap().count(), 1);
}

#[test]
fn immutable_object_conflict_preserves_the_existing_bytes() {
    let scratch = Scratch::new();
    let path = scratch.object();
    persist_immutable_object(&path, b"encoded-object-a").unwrap();
    assert_eq!(
        persist_immutable_object(&path, b"encoded-object-b"),
        Err("DIRECT_REVISION_IMMUTABLE_CONFLICT".to_owned()),
    );
    assert_eq!(fs::read(path).unwrap(), b"encoded-object-a");
}

#[test]
fn racing_publications_never_clobber_the_winning_object() {
    let scratch = Scratch::new();
    let barrier = Arc::new(Barrier::new(2));
    let mut jobs = Vec::new();
    for bytes in [b"encoded-object-a".as_slice(), b"encoded-object-b".as_slice()] {
        let path = scratch.object();
        let start = Arc::clone(&barrier);
        jobs.push(std::thread::spawn(move || {
            start.wait();
            (bytes, persist_immutable_object(&path, bytes))
        }));
    }
    let outcomes = jobs.into_iter().map(|job| job.join().unwrap()).collect::<Vec<_>>();
    let winners = outcomes.iter().filter(|(_, result)| result.is_ok()).collect::<Vec<_>>();
    assert_eq!(winners.len(), 1, "{outcomes:?}");
    assert_eq!(fs::read(scratch.object()).unwrap().as_slice(), winners[0].0);
    assert_eq!(fs::read_dir(&scratch.0).unwrap().count(), 1);
}

#[test]
fn empty_encoded_object_is_rejected_before_creating_a_file() {
    let scratch = Scratch::new();
    assert_eq!(
        persist_immutable_object(&scratch.object(), &[]),
        Err("DIRECT_REVISION_PROTECTED_SIZE_INVALID".to_owned()),
    );
    assert!(!scratch.object().exists());
}

#[cfg(unix)]
#[test]
fn existing_symlink_cannot_redirect_immutable_publication() {
    let scratch = Scratch::new();
    let outside = scratch.0.join("outside");
    fs::write(&outside, b"do-not-change").unwrap();
    std::os::unix::fs::symlink(&outside, scratch.object()).unwrap();
    assert!(persist_immutable_object(&scratch.object(), b"encoded-object").is_err());
    assert_eq!(fs::read(outside).unwrap(), b"do-not-change");
}

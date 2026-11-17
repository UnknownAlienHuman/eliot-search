//! Root lifecycle, exclusion and layout-classification cases.
//!
//! These exercise the real filesystem through a unique private directory under
//! the OS temporary directory. They never touch an operator data root.

use std::fs;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicBool, Ordering};
use std::time::{Duration, Instant};

use crate::plain_root::{
    CONTROL_DB_FILE, CONTROL_DIR, INCOMPLETE_IMPORT_MARKER, INIT_MARKER, LOCK_FILE, MANIFEST_FILE,
    OBJECT_DIR, PlainRootError, PlainRootStatus, discard_incomplete_plain, initialize_plain,
    inspect_plain, open_plain,
};

/// A unique directory that removes itself, including a locked root's files.
struct Scratch(PathBuf);

impl Scratch {
    fn new(label: &str) -> Self {
        // The counter keeps parallel test threads in separate trees without a
        // temporary-file dependency.
        static NEXT: std::sync::atomic::AtomicU64 = std::sync::atomic::AtomicU64::new(0);
        let unique = NEXT.fetch_add(1, Ordering::Relaxed);
        let path = std::env::temp_dir().join(format!(
            "eliot-root-{label}-{}-{unique}",
            std::process::id()
        ));
        let _ = fs::remove_dir_all(&path);
        Self(path)
    }

    fn root(&self) -> &Path {
        &self.0
    }
}

impl Drop for Scratch {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.0);
    }
}

/// An already expired absolute deadline.
fn expired() -> Instant {
    Instant::now()
        .checked_sub(Duration::from_secs(1))
        .expect("a monotonic clock past its first second")
}

fn budget() -> (Instant, AtomicBool) {
    (
        Instant::now() + Duration::from_secs(30),
        AtomicBool::new(false),
    )
}

fn status(path: &Path) -> PlainRootStatus {
    let (deadline, cancel) = budget();
    inspect_plain(path, deadline, &cancel)
        .expect("inspection reads the layout")
        .status
}

#[test]
fn an_absent_path_becomes_a_committed_root_and_then_opens() {
    let scratch = Scratch::new("lifecycle");
    assert_eq!(status(scratch.root()), PlainRootStatus::Absent);

    let (deadline, cancel) = budget();
    let guard = initialize_plain(scratch.root(), deadline, &cancel).expect("initialize");
    assert_eq!(guard.path(), scratch.root());
    assert_eq!(
        guard.control_path(),
        scratch.root().join(CONTROL_DIR).join(CONTROL_DB_FILE)
    );
    assert_eq!(guard.object_dir(), scratch.root().join(OBJECT_DIR));
    // The manifest is published last, so its presence is what makes the layout
    // committed. The lock, objects and control directories precede it.
    for name in [LOCK_FILE, MANIFEST_FILE, OBJECT_DIR, CONTROL_DIR] {
        assert!(
            scratch.root().join(name).exists(),
            "committed root is missing {name}"
        );
    }
    drop(guard);

    assert_eq!(status(scratch.root()), PlainRootStatus::Current);
    let (deadline, cancel) = budget();
    let reopened = open_plain(scratch.root(), deadline, &cancel).expect("reopen committed root");
    assert_eq!(reopened.path(), scratch.root());
}

#[test]
fn the_open_handle_owns_exclusion_for_its_whole_lifetime() {
    let scratch = Scratch::new("exclusion");
    let (deadline, cancel) = budget();
    let held = initialize_plain(scratch.root(), deadline, &cancel).expect("initialize");

    // Exclusion comes from the open handle, not from the lock file existing.
    let (deadline, cancel) = budget();
    assert_eq!(
        open_plain(scratch.root(), deadline, &cancel).err(),
        Some(PlainRootError::Busy)
    );
    // Initialize refuses a committed layout by state before it reaches the
    // lock, so it reports AlreadyExists rather than the holder's exclusion.
    let (deadline, cancel) = budget();
    assert_eq!(
        initialize_plain(scratch.root(), deadline, &cancel).err(),
        Some(PlainRootError::AlreadyExists)
    );

    drop(held);
    let (deadline, cancel) = budget();
    open_plain(scratch.root(), deadline, &cancel).expect("released root opens again");
}

#[test]
fn a_relative_path_is_refused_before_any_filesystem_work() {
    let (deadline, cancel) = budget();
    assert_eq!(
        inspect_plain(Path::new("relative-root"), deadline, &cancel).err(),
        Some(PlainRootError::InvalidPath)
    );
    let (deadline, cancel) = budget();
    assert_eq!(
        initialize_plain(Path::new("relative-root"), deadline, &cancel).err(),
        Some(PlainRootError::InvalidPath)
    );
}

#[test]
fn an_unknown_entry_makes_the_layout_unsupported_rather_than_current() {
    let scratch = Scratch::new("foreign");
    let (deadline, cancel) = budget();
    drop(initialize_plain(scratch.root(), deadline, &cancel).expect("initialize"));
    assert_eq!(status(scratch.root()), PlainRootStatus::Current);

    // A foreign file is never silently tolerated, adopted or removed.
    fs::write(scratch.root().join("foreign.txt"), b"not ours").expect("write foreign entry");
    assert_eq!(status(scratch.root()), PlainRootStatus::Unsupported);
    let (deadline, cancel) = budget();
    assert_eq!(
        open_plain(scratch.root(), deadline, &cancel).err(),
        Some(PlainRootError::Unsupported)
    );
    fs::remove_file(scratch.root().join("foreign.txt")).expect("remove foreign entry");
    assert_eq!(status(scratch.root()), PlainRootStatus::Current);
}

#[test]
fn a_committed_root_is_never_discarded_as_incomplete() {
    let scratch = Scratch::new("discard");
    let (deadline, cancel) = budget();
    drop(initialize_plain(scratch.root(), deadline, &cancel).expect("initialize"));
    fs::write(
        scratch.root().join(OBJECT_DIR).join("object-fixture"),
        b"keep",
    )
    .expect("write object");

    // A committed root is an AlreadyExists refusal for discard, never a
    // destination it may clear.
    let (deadline, cancel) = budget();
    assert_eq!(
        discard_incomplete_plain(scratch.root(), deadline, &cancel).err(),
        Some(PlainRootError::AlreadyExists)
    );
    // The refusal removed nothing.
    assert_eq!(status(scratch.root()), PlainRootStatus::Current);
    assert!(
        scratch
            .root()
            .join(OBJECT_DIR)
            .join("object-fixture")
            .exists()
    );
}

#[test]
fn an_interrupted_initialization_is_incomplete_and_discardable() {
    let scratch = Scratch::new("incomplete");
    // Lock plus published initialization marker, with no manifest: exactly the
    // state a crash between marker publication and manifest publication leaves.
    fs::create_dir_all(scratch.root().join(OBJECT_DIR)).expect("objects");
    fs::create_dir_all(scratch.root().join(CONTROL_DIR)).expect("control");
    fs::write(scratch.root().join(LOCK_FILE), b"").expect("lock");
    fs::write(
        scratch.root().join(INIT_MARKER),
        b"eliot-plain-root-initializing-v1\n",
    )
    .expect("marker");
    assert_eq!(
        status(scratch.root()),
        PlainRootStatus::IncompleteInitialization
    );

    let (deadline, cancel) = budget();
    discard_incomplete_plain(scratch.root(), deadline, &cancel).expect("discard incomplete");
    // Discard clears the marker, the empty object/control layout and the
    // metadata stages. The stable lock is deliberately left in place, because
    // the operation holds it while running, so the swept directory still
    // classifies as an unfinished initialization rather than Absent.
    let remaining: Vec<_> = fs::read_dir(scratch.root())
        .expect("read swept root")
        .map(|entry| entry.expect("entry").file_name())
        .collect();
    assert_eq!(remaining, vec![std::ffi::OsString::from(LOCK_FILE)]);
    assert_eq!(
        status(scratch.root()),
        PlainRootStatus::IncompleteInitialization
    );
}

#[test]
fn retained_bytes_make_an_interrupted_initialization_corrupt_not_discardable() {
    let scratch = Scratch::new("corrupt");
    fs::create_dir_all(scratch.root().join(OBJECT_DIR)).expect("objects");
    fs::write(scratch.root().join(LOCK_FILE), b"").expect("lock");
    fs::write(
        scratch.root().join(INIT_MARKER),
        b"eliot-plain-root-initializing-v1\n",
    )
    .expect("marker");
    // An uncommitted root that already holds objects is not a safe discard
    // target: the bytes are evidence, so the layout is reported corrupt.
    fs::write(
        scratch.root().join(OBJECT_DIR).join("object-fixture"),
        b"keep",
    )
    .expect("write object");

    assert_eq!(status(scratch.root()), PlainRootStatus::Corrupt);
    let (deadline, cancel) = budget();
    assert_eq!(
        discard_incomplete_plain(scratch.root(), deadline, &cancel).err(),
        Some(PlainRootError::Corrupt)
    );
    assert!(
        scratch
            .root()
            .join(OBJECT_DIR)
            .join("object-fixture")
            .exists(),
        "a corrupt root keeps its retained bytes"
    );
}

#[test]
fn an_import_marker_withholds_a_committed_root_from_ordinary_opens() {
    let scratch = Scratch::new("import");
    let (deadline, cancel) = budget();
    drop(initialize_plain(scratch.root(), deadline, &cancel).expect("initialize"));

    fs::write(scratch.root().join(INCOMPLETE_IMPORT_MARKER), b"marked").expect("import marker");
    assert_eq!(status(scratch.root()), PlainRootStatus::IncompleteImport);
    // The marker is reported as its own cause, not a generic wrong state.
    let (deadline, cancel) = budget();
    assert_eq!(
        open_plain(scratch.root(), deadline, &cancel).err(),
        Some(PlainRootError::IncompleteImport)
    );

    fs::remove_file(scratch.root().join(INCOMPLETE_IMPORT_MARKER)).expect("clear marker");
    let (deadline, cancel) = budget();
    open_plain(scratch.root(), deadline, &cancel).expect("cleared marker reopens");
}

#[test]
fn an_expired_deadline_and_observed_cancellation_both_stop_root_work() {
    let scratch = Scratch::new("stop");
    let deadline_passed = expired();
    let cancel = AtomicBool::new(false);
    assert_eq!(
        inspect_plain(scratch.root(), deadline_passed, &cancel).err(),
        Some(PlainRootError::Deadline)
    );

    let (deadline, _) = budget();
    let cancelled = AtomicBool::new(true);
    assert_eq!(
        inspect_plain(scratch.root(), deadline, &cancelled).err(),
        Some(PlainRootError::Cancelled)
    );
    assert_eq!(
        initialize_plain(scratch.root(), deadline, &cancelled).err(),
        Some(PlainRootError::Cancelled)
    );
    // Neither stop created the root.
    assert!(!scratch.root().exists());
}

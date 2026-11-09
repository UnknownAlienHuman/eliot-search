//! Bounded single-handle acquisition and its closed refusals.

use std::fs;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};
use std::time::{Duration, Instant};

use crate::{MAX_SOURCE_FILE_BYTES, SourceReadError, acquire_file};

struct Scratch(PathBuf);

impl Scratch {
    fn new(label: &str) -> Self {
        static NEXT: AtomicU64 = AtomicU64::new(0);
        let unique = NEXT.fetch_add(1, Ordering::Relaxed);
        let path = std::env::temp_dir().join(format!(
            "eliot-reader-{label}-{}-{unique}",
            std::process::id()
        ));
        let _ = fs::remove_dir_all(&path);
        fs::create_dir_all(&path).expect("scratch directory");
        Self(path)
    }

    fn file(&self, name: &str, bytes: &[u8]) -> PathBuf {
        let path = self.0.join(name);
        fs::write(&path, bytes).expect("fixture file");
        path
    }

    fn dir(&self) -> &Path {
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

fn read(path: &Path, maximum: u64) -> Result<(Vec<u8>, u64), SourceReadError> {
    let (deadline, cancel) = budget();
    acquire_file(path, maximum, deadline, &cancel).map(|acquired| {
        let (bytes, _key, length) = acquired.into_parts();
        (bytes, length)
    })
}

#[test]
fn exact_bytes_and_length_come_back_unchanged() {
    let scratch = Scratch::new("exact");
    // Content that must not be decoded, normalized or line-ending translated.
    let body = b"alpha\r\nbeta\n\0\xF0\x9F\xA6\x80 tail";
    let path = scratch.file("mixed.bin", body);
    let (bytes, length) = read(&path, MAX_SOURCE_FILE_BYTES).expect("acquire");
    assert_eq!(bytes, body);
    assert_eq!(length, body.len() as u64);
}

#[test]
fn an_empty_file_is_a_valid_zero_byte_source() {
    let scratch = Scratch::new("empty");
    let path = scratch.file("empty.txt", b"");
    let (bytes, length) = read(&path, MAX_SOURCE_FILE_BYTES).expect("acquire empty");
    assert!(bytes.is_empty());
    assert_eq!(length, 0);
}

#[test]
fn the_same_handle_reports_one_stable_native_identity() {
    let scratch = Scratch::new("identity");
    let path = scratch.file("stable.txt", b"stable bytes");
    let (deadline, cancel) = budget();
    let first = acquire_file(&path, MAX_SOURCE_FILE_BYTES, deadline, &cancel).expect("first");
    let (deadline, cancel) = budget();
    let second = acquire_file(&path, MAX_SOURCE_FILE_BYTES, deadline, &cancel).expect("second");
    let (_, first_key, first_len) = first.into_parts();
    let (_, second_key, second_len) = second.into_parts();
    assert_eq!(first_key, second_key);
    assert_eq!(first_len, second_len);
}

#[test]
fn a_directory_is_not_a_regular_file() {
    let scratch = Scratch::new("directory");
    // Windows refuses to open a directory for reading; Unix opens it and the
    // metadata check rejects it. Both are closed refusals, never empty success.
    // A directory is always a closed refusal and never empty success. The exact
    // code is platform-dependent: Unix opens the directory and the metadata
    // check rejects it as NotRegularFile, while Windows fails the open itself
    // and reports Io, because ACCESS_DENIED is not specialized into a
    // directory verdict. Both outcomes are refusals carrying no bytes.
    assert!(matches!(
        read(scratch.dir(), MAX_SOURCE_FILE_BYTES),
        Err(SourceReadError::NotRegularFile | SourceReadError::OpenFailed | SourceReadError::Io)
    ));
}

#[test]
fn a_missing_path_fails_to_open_instead_of_reading_nothing() {
    let scratch = Scratch::new("missing");
    assert_eq!(
        read(&scratch.dir().join("absent.txt"), MAX_SOURCE_FILE_BYTES),
        Err(SourceReadError::OpenFailed)
    );
}

#[test]
fn the_caller_bound_is_checked_against_the_observed_length() {
    let scratch = Scratch::new("bound");
    let path = scratch.file("ten.txt", b"0123456789");
    assert!(read(&path, 10).is_ok());
    assert_eq!(read(&path, 9), Err(SourceReadError::TooLarge));
    // A zero or over-format bound is refused before the file is opened.
    assert_eq!(read(&path, 0), Err(SourceReadError::TooLarge));
    assert_eq!(
        read(&path, MAX_SOURCE_FILE_BYTES + 1),
        Err(SourceReadError::TooLarge)
    );
    // An invalid bound is refused even when the path does not exist, so the
    // check cannot be confused with a filesystem observation.
    assert_eq!(
        read(&scratch.dir().join("absent.txt"), 0),
        Err(SourceReadError::TooLarge)
    );
}

#[test]
fn an_expired_deadline_and_observed_cancellation_both_stop_acquisition() {
    let scratch = Scratch::new("stop");
    let path = scratch.file("body.txt", b"body");
    let cancel = AtomicBool::new(false);
    assert_eq!(
        acquire_file(&path, MAX_SOURCE_FILE_BYTES, expired(), &cancel),
        Err(SourceReadError::Deadline)
    );
    let (deadline, _) = budget();
    let cancelled = AtomicBool::new(true);
    assert_eq!(
        acquire_file(&path, MAX_SOURCE_FILE_BYTES, deadline, &cancelled),
        Err(SourceReadError::Cancelled)
    );
}

#[test]
fn every_failure_code_is_a_stable_content_free_token() {
    for (error, code) in [
        (SourceReadError::NotRegularFile, "SOURCE_NOT_REGULAR_FILE"),
        (SourceReadError::OpenFailed, "SOURCE_OPEN_FAILED"),
        (SourceReadError::TooLarge, "SOURCE_TOO_LARGE"),
        (
            SourceReadError::IdentityUnavailable,
            "SOURCE_IDENTITY_UNAVAILABLE",
        ),
        (
            SourceReadError::ChangedDuringRead,
            "SOURCE_CHANGED_DURING_READ",
        ),
        (
            SourceReadError::AllocationFailed,
            "SOURCE_ALLOCATION_FAILED",
        ),
        (SourceReadError::Deadline, "SOURCE_DEADLINE"),
        (SourceReadError::Cancelled, "SOURCE_CANCELLED"),
        (SourceReadError::Io, "SOURCE_IO"),
    ] {
        assert_eq!(error.code(), code);
    }
}

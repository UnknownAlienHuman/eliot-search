use std::io::ErrorKind;
use std::sync::atomic::{AtomicU64, Ordering};

use super::*;

struct Fixture {
    root: PathBuf,
}

impl Fixture {
    fn new() -> Self {
        static NEXT: AtomicU64 = AtomicU64::new(0);
        for _ in 0..128 {
            let root = std::env::temp_dir().join(format!(
                "eliot-qdrant-boundary-fs-{}-{}",
                std::process::id(),
                NEXT.fetch_add(1, Ordering::Relaxed),
            ));
            match fs::create_dir(&root) {
                Ok(()) => return Self { root },
                Err(error) if error.kind() == ErrorKind::AlreadyExists => {}
                Err(error) => panic!("cannot create fixture: {error}"),
            }
        }
        panic!("fixture directory collision budget exhausted");
    }

    fn write(&self, relative: &str, text: &str) -> PathBuf {
        let path = self.root.join(relative);
        fs::create_dir_all(path.parent().expect("fixture parent"))
            .expect("create fixture parent");
        fs::write(&path, text).expect("write fixture");
        path
    }
}

impl Drop for Fixture {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.root);
    }
}

#[test]
fn entry_budget_stops_enumeration() {
    let fixture = Fixture::new();
    fixture.write("a.rs", "");
    fixture.write("b.rs", "");
    let mut budget = ScanBudget::with_limits(ScanLimits {
        entries: 1,
        ..ScanLimits::default()
    });
    let mut files = Vec::new();
    let mut errors = Vec::new();
    collect_files(
        &fixture.root,
        &fixture.root,
        0,
        &mut budget,
        &mut files,
        &mut errors,
    );
    assert!(errors.iter().any(|error| error.contains("entry limit")));
}

#[test]
fn depth_budget_stops_recursion() {
    let fixture = Fixture::new();
    fixture.write("a/b/c.rs", "");
    let mut budget = ScanBudget::with_limits(ScanLimits {
        depth: 1,
        ..ScanLimits::default()
    });
    let mut files = Vec::new();
    let mut errors = Vec::new();
    collect_files(
        &fixture.root,
        &fixture.root,
        0,
        &mut budget,
        &mut files,
        &mut errors,
    );
    assert!(errors.iter().any(|error| error.contains("depth limit")));
}

#[test]
fn per_file_and_aggregate_byte_limits_fail_closed() {
    let fixture = Fixture::new();
    let path = fixture.write("large.rs", "1234");

    let mut per_file_budget = ScanBudget::with_limits(ScanLimits {
        file_bytes: 3,
        ..ScanLimits::default()
    });
    let mut per_file_errors = Vec::new();
    assert!(
        read_text(
            &path,
            "large.rs",
            &mut per_file_budget,
            &mut per_file_errors,
        )
        .is_none()
    );
    assert!(
        per_file_errors
            .iter()
            .any(|error| error.contains("per-file limit"))
    );

    let mut aggregate_budget = ScanBudget::with_limits(ScanLimits {
        total_bytes: 3,
        ..ScanLimits::default()
    });
    let mut aggregate_errors = Vec::new();
    assert!(
        read_text(
            &path,
            "large.rs",
            &mut aggregate_budget,
            &mut aggregate_errors,
        )
        .is_none()
    );
    assert!(
        aggregate_errors
            .iter()
            .any(|error| error.contains("aggregate limit"))
    );
}

#[cfg(unix)]
#[test]
fn symbolic_link_is_reported_instead_of_skipped() {
    use std::os::unix::fs::symlink;

    let fixture = Fixture::new();
    let target = fixture.write("target.rs", "");
    symlink(target, fixture.root.join("escape.rs")).expect("create symlink");

    let mut budget = ScanBudget::default();
    let mut files = Vec::new();
    let mut errors = Vec::new();
    collect_files(
        &fixture.root,
        &fixture.root,
        0,
        &mut budget,
        &mut files,
        &mut errors,
    );
    assert!(
        errors
            .iter()
            .any(|error| error.contains("symbolic links are not allowed"))
    );
}

#[test]
fn entry_budget_bounds_buffering_before_sorting() {
    let observed = std::cell::Cell::new(0_usize);
    let entries = std::iter::from_fn(|| {
        let next = observed.get();
        observed.set(next + 1);
        Some(Ok(next))
    });
    let mut budget = ScanBudget::with_limits(ScanLimits {
        entries: 2,
        ..ScanLimits::default()
    });
    let mut errors = Vec::new();
    assert!(collect_bounded_entries(entries, ".", &mut budget, &mut errors).is_none());
    assert_eq!(observed.get(), 3);
    assert_eq!(budget.entries_seen, 3);
    assert!(budget.stopped);
    assert_eq!(errors.len(), 1);
}

#[test]
fn exact_entry_budget_counts_nested_entries_once() {
    let fixture = Fixture::new();
    fixture.write("a.rs", "");
    fixture.write("nested/b.rs", "");
    let mut budget = ScanBudget::with_limits(ScanLimits {
        entries: 3,
        ..ScanLimits::default()
    });
    let mut files = Vec::new();
    let mut errors = Vec::new();
    collect_files(
        &fixture.root,
        &fixture.root,
        0,
        &mut budget,
        &mut files,
        &mut errors,
    );
    assert!(errors.is_empty(), "{errors:?}");
    assert_eq!(budget.entries_seen, 3);
    assert_eq!(files.len(), 2);
    assert!(!budget.stopped);
}

#[test]
fn directory_error_does_not_return_a_partial_listing() {
    let mut budget = ScanBudget::default();
    let mut errors = Vec::new();
    let entries = [Ok(1), Err(std::io::Error::other("fixture enumeration failure"))];
    assert!(
        collect_bounded_entries(entries, "nested", &mut budget, &mut errors).is_none()
    );
    assert_eq!(budget.entries_seen, 2);
    assert_eq!(errors.len(), 1);
    assert!(errors[0].contains("nested: unable to enumerate directory"));
}

#[test]
fn invalid_utf8_consumes_the_aggregate_byte_budget() {
    let fixture = Fixture::new();
    let invalid = fixture.write("invalid.rs", "");
    fs::write(&invalid, [0xff, 0xfe, 0xfd]).expect("write invalid UTF-8 fixture");
    let valid = fixture.write("valid.rs", "123");
    let mut budget = ScanBudget::with_limits(ScanLimits {
        total_bytes: 5,
        ..ScanLimits::default()
    });
    let mut errors = Vec::new();
    assert!(read_text(&invalid, "invalid.rs", &mut budget, &mut errors).is_none());
    assert_eq!(budget.bytes_read, 3);
    assert!(errors[0].contains("unable to read UTF-8 text"));
    assert!(read_text(&valid, "valid.rs", &mut budget, &mut errors).is_none());
    assert!(budget.stopped);
    assert_eq!(budget.bytes_read, 3);
    assert!(errors.iter().any(|error| error.contains("aggregate limit")));
}

struct FailedReader;

impl Read for FailedReader {
    fn read(&mut self, _buffer: &mut [u8]) -> std::io::Result<usize> {
        Err(std::io::Error::other("fixture read failure"))
    }
}

#[test]
fn failed_io_charges_bytes_read_before_the_error() {
    let mut budget = ScanBudget::with_limits(ScanLimits {
        total_bytes: 3,
        ..ScanLimits::default()
    });
    let mut errors = Vec::new();
    let reader = std::io::Cursor::new(b"12").chain(FailedReader);
    assert!(read_bounded_utf8(reader, "partial.rs", &mut budget, &mut errors).is_none());
    assert_eq!(budget.bytes_read, 2);
    assert_eq!(budget.remaining_bytes(), 1);
    assert_eq!(errors.len(), 1);
    assert!(errors[0].contains("fixture read failure"));
}

#[test]
fn per_file_growth_does_not_refund_consumed_bytes() {
    let mut budget = ScanBudget::with_limits(ScanLimits {
        file_bytes: 3,
        total_bytes: 8,
        ..ScanLimits::default()
    });
    let mut errors = Vec::new();
    assert!(
        read_bounded_utf8(b"12345".as_slice(), "grew.rs", &mut budget, &mut errors)
            .is_none()
    );
    assert_eq!(budget.bytes_read, 4);
    assert_eq!(budget.remaining_bytes(), 4);
    assert!(!budget.stopped);
    assert!(errors[0].contains("grew beyond the bounded read allowance"));
}

#[test]
fn aggregate_growth_stops_the_scan_after_one_probe_byte() {
    let mut budget = ScanBudget::with_limits(ScanLimits {
        total_bytes: 3,
        ..ScanLimits::default()
    });
    let mut errors = Vec::new();
    assert!(
        read_bounded_utf8(b"12345".as_slice(), "grew.rs", &mut budget, &mut errors)
            .is_none()
    );
    assert_eq!(budget.bytes_read, 4);
    assert!(budget.stopped);
    assert!(errors[0].contains("aggregate limit"));
    assert!(read_bounded_utf8(FailedReader, "later.rs", &mut budget, &mut errors).is_none());
    assert_eq!(budget.bytes_read, 4);
    assert_eq!(errors.len(), 1);
}

#[test]
fn valid_utf8_is_counted_in_bytes_at_the_exact_limit() {
    let mut budget = ScanBudget::with_limits(ScanLimits {
        total_bytes: 4,
        ..ScanLimits::default()
    });
    let mut errors = Vec::new();
    for text in ["é", "β", ""] {
        assert_eq!(
            read_bounded_utf8(text.as_bytes(), "valid.rs", &mut budget, &mut errors),
            Some(text.to_owned())
        );
    }
    assert_eq!(budget.bytes_read, 4);
    assert!(!budget.stopped);
    assert!(errors.is_empty(), "{errors:?}");
}

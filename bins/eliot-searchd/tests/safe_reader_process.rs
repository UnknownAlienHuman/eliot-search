//! T07 final-handle process regressions: primary ingestion reads through the
//! shared safe-reader kernel, proves final-object/ancestor containment on
//! the opened handle, and fails closed on link/escape/hardlink/revocation
//! faults without publishing anything or executing source content.

use std::fs;
use std::io::Read;
use std::path::{Path, PathBuf};
use std::process::{Command, ExitStatus, Stdio};
use std::sync::atomic::{AtomicU64, Ordering};
use std::thread;
use std::time::{Duration, Instant, SystemTime, UNIX_EPOCH};

// Shared Credential Manager cleanup; each harness uses a subset of it.
#[allow(dead_code)]
mod common;

static NEXT: AtomicU64 = AtomicU64::new(0);

struct Fixture {
    base: PathBuf,
    data: PathBuf,
    files: PathBuf,
    guard: common::RevisionKeyTreeGuard,
}

impl Fixture {
    fn new() -> Self {
        let stamp = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_nanos();
        let base = std::env::temp_dir().join(format!(
            "eliot-safe-reader-{}-{stamp}-{}",
            std::process::id(),
            NEXT.fetch_add(1, Ordering::Relaxed)
        ));
        let data = base.join("data");
        let files = base.join("files");
        fs::create_dir_all(&data).unwrap();
        fs::create_dir_all(&files).unwrap();
        let guard = common::RevisionKeyTreeGuard::for_tree(&base);
        Self {
            base,
            data,
            files,
            guard,
        }
    }

    fn run(args: &[&str]) -> (ExitStatus, String, String) {
        // Windows Credential Manager races under parallel load: a write
        // without readback (`WRITE_OUTCOME_UNKNOWN`) or a transient
        // `NOT_FOUND` against existing objects (`MISSING`) are environmental
        // vault noise, not product verdicts. Retry those exact codes with a
        // bounded backoff; every other failure returns immediately so real
        // denials (links, escape, hardlinks) can never be retried away.
        // Quarantine refusals are never retried: they are T05 correctness
        // signals, not vault noise.
        let mut attempt = 0_u32;
        loop {
            let outcome = Self::run_once(args);
            if outcome.0.success() || attempt >= 7 {
                return outcome;
            }
            let noisy = outcome.2.contains("DIRECT_REVISION_KEY_");
            if !noisy {
                return outcome;
            }
            std::thread::sleep(Duration::from_millis(25_u64 << attempt.min(5)));
            attempt += 1;
        }
    }

    fn run_once(args: &[&str]) -> (ExitStatus, String, String) {
        let mut child = Command::new(env!("CARGO_BIN_EXE_eliot-searchd"))
            .args(args)
            .stdin(Stdio::null())
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .spawn()
            .expect("primary daemon");
        let stdout = child.stdout.take().unwrap();
        let stderr = child.stderr.take().unwrap();
        // Drain both pipes concurrently, with a finite byte ceiling, to avoid
        // deadlock if a failing binary floods diagnostics or never exits.
        let out = thread::spawn(move || read_output(stdout));
        let err = thread::spawn(move || read_output(stderr));
        let deadline = Instant::now() + Duration::from_secs(30);
        let status = loop {
            if let Some(status) = child.try_wait().unwrap() {
                break status;
            }
            if Instant::now() >= deadline {
                let _ = child.kill();
                let _ = child.wait();
                panic!("primary daemon exceeded the test deadline");
            }
            thread::sleep(Duration::from_millis(10));
        };
        (status, out.join().unwrap(), err.join().unwrap())
    }

    fn ok(args: &[&str]) -> String {
        let (status, stdout, stderr) = Self::run(args);
        assert!(
            status.success(),
            "status={status} stdout={stdout} stderr={stderr}"
        );
        stdout
    }

    fn err(args: &[&str]) -> String {
        let (status, stdout, stderr) = Self::run(args);
        assert!(
            !status.success(),
            "expected failure stdout={stdout} stderr={stderr}"
        );
        assert!(
            stdout.is_empty(),
            "failures must not emit products: {stdout}"
        );
        stderr
    }

    fn source_count(&self) -> usize {
        let output = Self::ok(&["--list-sources", self.data.to_str().unwrap()]);
        assert!(
            output.contains("\"event\":\"source_list_complete\""),
            "{output}"
        );
        output
            .lines()
            .filter(|line| line.contains("\"event\":\"source\""))
            .count()
    }

    fn write(&self, name: &str, bytes: &[u8]) -> PathBuf {
        let path = self.files.join(name);
        fs::write(&path, bytes).unwrap();
        path
    }
}

impl Drop for Fixture {
    fn drop(&mut self) {
        self.guard.cleanup();
        let _ = fs::remove_dir_all(&self.base);
    }
}

fn read_output(reader: impl Read) -> String {
    const MAX_OUTPUT: u64 = 1024 * 1024;
    let mut bytes = Vec::new();
    reader.take(MAX_OUTPUT + 1).read_to_end(&mut bytes).unwrap();
    assert!(
        bytes.len() <= usize::try_from(MAX_OUTPUT).expect("test output ceiling fits"),
        "test output ceiling exceeded"
    );
    String::from_utf8(bytes).expect("UTF-8 output")
}

fn symlink_file(target: &Path, link: &Path) {
    #[cfg(unix)]
    std::os::unix::fs::symlink(target, link).unwrap();
    #[cfg(windows)]
    std::os::windows::fs::symlink_file(target, link).unwrap();
}

fn symlink_dir(target: &Path, link: &Path) {
    #[cfg(unix)]
    std::os::unix::fs::symlink(target, link).unwrap();
    #[cfg(windows)]
    std::os::windows::fs::symlink_dir(target, link).unwrap();
}

#[test]
fn kernel_verified_index_roundtrip_is_searchable() {
    let fixture = Fixture::new();
    let source = fixture.write("hello.txt", b"kernel final-handle evidence alpha");
    let output = Fixture::ok(&[
        "--index-file",
        fixture.data.to_str().unwrap(),
        source.to_str().unwrap(),
    ]);
    assert!(output.contains("\"event\":\"source_indexed\""), "{output}");
    assert!(
        output.contains("\"identity_strength\":\"native\""),
        "{output}"
    );
    let searched = Fixture::ok(&[
        "--search-root",
        fixture.data.to_str().unwrap(),
        "final-handle",
    ]);
    assert!(
        searched
            .lines()
            .any(|line| line.contains("\"event\":\"match\"")),
        "{searched}"
    );
    assert_eq!(fixture.source_count(), 1);
}

#[test]
fn symlink_escape_is_denied_without_publication() {
    let fixture = Fixture::new();
    let outside = fixture.base.join("outside.txt");
    fs::write(&outside, b"foreign bytes").unwrap();
    let link = fixture.files.join("link.txt");
    symlink_file(&outside, &link);
    let stderr = Fixture::err(&[
        "--index-file",
        fixture.data.to_str().unwrap(),
        link.to_str().unwrap(),
    ]);
    assert!(stderr.contains("DIRECT_SOURCE_LINK_DENIED"), "{stderr}");
    assert!(!stderr.contains("foreign"), "{stderr}");
    assert_eq!(fixture.source_count(), 0);
}

#[test]
fn ancestor_junction_is_denied_without_publication() {
    let fixture = Fixture::new();
    let sub = fixture.files.join("sub");
    fs::create_dir(&sub).unwrap();
    fs::write(sub.join("file.txt"), b"inside").unwrap();
    let outside = fixture.base.join("foreign");
    fs::create_dir(&outside).unwrap();
    fs::write(outside.join("file.txt"), b"foreign bytes").unwrap();
    fs::remove_file(sub.join("file.txt")).unwrap();
    fs::remove_dir(&sub).unwrap();
    symlink_dir(&outside, &sub);
    let target = sub.join("file.txt");
    let stderr = Fixture::err(&[
        "--index-file",
        fixture.data.to_str().unwrap(),
        target.to_str().unwrap(),
    ]);
    // The admitted parent hint itself is a foreign link here, so the base
    // is reported relocated; a resolved-outside final object would report
    // escape instead. All three fail closed without bytes or publication.
    assert!(
        stderr.contains("DIRECT_SOURCE_ESCAPE_DENIED")
            || stderr.contains("DIRECT_SOURCE_LINK_DENIED")
            || stderr.contains("DIRECT_SOURCE_ROOT_RELOCATED"),
        "{stderr}"
    );
    assert!(!stderr.contains("foreign"), "{stderr}");
    assert_eq!(fixture.source_count(), 0);
    let _ = fs::remove_file(&sub);
}

#[test]
fn hardlink_is_denied_without_publication() {
    let fixture = Fixture::new();
    let outside = fixture.base.join("shared.txt");
    fs::write(&outside, b"shared bytes").unwrap();
    let alias = fixture.files.join("alias.txt");
    fs::hard_link(&outside, &alias).unwrap();
    let stderr = Fixture::err(&[
        "--index-file",
        fixture.data.to_str().unwrap(),
        alias.to_str().unwrap(),
    ]);
    assert!(stderr.contains("DIRECT_SOURCE_HARDLINK_DENIED"), "{stderr}");
    assert_eq!(fixture.source_count(), 0);
}

#[test]
fn scan_file_denies_symlink_and_reads_regular() {
    let fixture = Fixture::new();
    let regular = fixture.write("regular.txt", b"scan me please target");
    let output = Fixture::ok(&["--scan-file", "target", regular.to_str().unwrap()]);
    assert!(output.contains("\"event\":\"scan_started\""), "{output}");
    assert!(output.contains("\"same_handle_verified\":true"), "{output}");
    let outside = fixture.base.join("scan-outside.txt");
    fs::write(&outside, b"target").unwrap();
    let link = fixture.files.join("scan-link.txt");
    symlink_file(&outside, &link);
    let stderr = Fixture::err(&["--scan-file", "target", link.to_str().unwrap()]);
    assert!(stderr.contains("SCAN_FILE_LINK_DENIED"), "{stderr}");
}

#[test]
fn script_source_is_inert_data() {
    let fixture = Fixture::new();
    let marker = fixture.base.join("T07_PWNED_MARKER");
    let script = format!("@echo off\r\necho pwned > {}\r\n", marker.display());
    let source = fixture.write("run.bat", script.as_bytes());
    let output = Fixture::ok(&[
        "--index-file",
        fixture.data.to_str().unwrap(),
        source.to_str().unwrap(),
    ]);
    assert!(output.contains("\"event\":\"source_indexed\""), "{output}");
    // The payload was read as inert bytes; nothing executed it.
    assert!(!marker.try_exists().unwrap(), "source must never execute");
    let searched = Fixture::ok(&["--search-root", fixture.data.to_str().unwrap(), "pwned"]);
    assert!(
        searched
            .lines()
            .any(|line| line.contains("\"event\":\"match\"")),
        "{searched}"
    );
}

#[test]
fn missing_source_is_denied_without_panic() {
    let fixture = Fixture::new();
    let missing = fixture.files.join("absent.txt");
    let stderr = Fixture::err(&[
        "--index-file",
        fixture.data.to_str().unwrap(),
        missing.to_str().unwrap(),
    ]);
    assert!(stderr.contains("DIRECT_SOURCE_ACCESS_DENIED"), "{stderr}");
    assert_eq!(fixture.source_count(), 0);
}

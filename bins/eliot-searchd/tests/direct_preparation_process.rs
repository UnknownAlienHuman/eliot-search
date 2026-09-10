//! Exercises shared preparation through the actual primary daemon executable.

use std::fs;
use std::fmt::Write as _;
use std::io::Read;
use std::path::PathBuf;
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
    source: PathBuf,
    guard: common::RevisionKeyGuard,
}
impl Fixture {
    fn new() -> Self {
        let stamp = SystemTime::now().duration_since(UNIX_EPOCH).unwrap().as_nanos();
        let base = std::env::temp_dir().join(format!("eliot-preparation-{}-{stamp}-{}",
            std::process::id(), NEXT.fetch_add(1, Ordering::Relaxed)));
        let data = base.join("data");
        let source = base.join("source.txt");
        fs::create_dir_all(&data).unwrap();
        let guard = common::RevisionKeyGuard::for_data_root(&data);
        Self { base, data, source, guard }
    }

    fn run(args: &[&str]) -> (ExitStatus, String, String) {
        Self::run_with_stdin(args, Stdio::null())
    }

    fn run_with_stdin(args: &[&str], input: Stdio) -> (ExitStatus, String, String) {
        let mut child = Command::new(env!("CARGO_BIN_EXE_eliot-searchd"))
            .args(args).stdin(input).stdout(Stdio::piped()).stderr(Stdio::piped())
            .spawn().expect("primary daemon");
        let stdout = child.stdout.take().unwrap();
        let stderr = child.stderr.take().unwrap();
        // Drain both pipes concurrently, with a finite byte ceiling, to avoid
        // deadlock if a failing binary floods diagnostics or never exits.
        let out = thread::spawn(move || read_output(stdout));
        let err = thread::spawn(move || read_output(stderr));
        let deadline = Instant::now() + Duration::from_secs(30);
        let status = loop {
            if let Some(status) = child.try_wait().unwrap() { break status; }
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
        assert!(status.success(), "status={status} stdout={stdout} stderr={stderr}");
        stdout
    }

    fn index(&self, bytes: &[u8]) -> String {
        fs::write(&self.source, bytes).unwrap();
        let output = Self::ok(&["--index-file", self.data.to_str().unwrap(), self.source.to_str().unwrap()]);
        // Capture the namespace created by the first open so Drop can delete
        // this test's revision-key credential.
        self.guard.refresh();
        output
    }

    fn search(&self, query: &str) -> String {
        Self::ok(&["--search-root", self.data.to_str().unwrap(), query])
    }
}
impl Drop for Fixture {
    fn drop(&mut self) {
        // Delete this test's revision-key credential before removing the
        // directory that holds control/namespace.id. Best-effort, never
        // panics.
        self.guard.cleanup();
        let _ = fs::remove_dir_all(&self.base);
    }
}

fn read_output(reader: impl Read) -> String {
    const MAX_OUTPUT: u64 = 1024 * 1024;
    let mut bytes = Vec::new();
    reader.take(MAX_OUTPUT + 1).read_to_end(&mut bytes).unwrap();
    assert!(bytes.len() <= usize::try_from(MAX_OUTPUT).expect("test output ceiling fits"), "test output ceiling exceeded");
    String::from_utf8(bytes).expect("UTF-8 output")
}

fn field<'a>(output: &'a str, key: &str) -> &'a str {
    // This helper is only for daemon-generated hex identifiers, not arbitrary JSON.
    let needle = format!("\"{key}\":\"");
    output.split_once(&needle).expect("field").1.split('"').next().unwrap()
}

fn matches(output: &str) -> Vec<&str> {
    output.lines().filter(|line| line.contains("\"event\":\"match\"")).collect()
}

#[test]
fn primary_search_keeps_match_crossing_the_real_unit_boundary() {
    let fixture = Fixture::new();
    let start = 16 * 1024 - 2;
    let text = format!("{}ABCDEFGH{}", "x".repeat(start), "z".repeat(64 * 1024));
    fixture.index(text.as_bytes());
    let output = fixture.search("ABCDEFGH");
    let rows = matches(&output);
    assert_eq!(rows.len(), 1, "{output}");
    assert!(rows[0].contains(&format!("\"byte_start\":{start},")));
    assert!(rows[0].contains(&format!("\"byte_end\":{},", start + 8)));
    assert!(output.contains("\"complete\":true"), "{output}");
}

#[test]
fn primary_search_uses_materializer_line_coordinates_without_normalization() {
    let fixture = Fixture::new();
    fixture.index("a\r\nβ\rc\n𐀀 target".as_bytes());
    let output = fixture.search("target");
    let rows = matches(&output);
    assert_eq!(rows.len(), 1, "{output}");
    assert!(rows[0].contains("\"line\":3,"), "{output}");
    assert!(rows[0].contains("\"column_bytes\":5,"), "{output}");
}

#[test]
fn binary_preparation_gap_cannot_become_an_exact_negative() {
    let fixture = Fixture::new();
    fixture.index(b"a\0b");
    let output = fixture.search("absent");
    assert!(matches(&output).is_empty());
    assert!(output.contains("MATERIALIZATION_BINARY_CONTENT"), "{output}");
    assert!(output.contains("\"complete\":false"), "{output}");
    assert!(output.contains("\"searched_sources\":0"), "{output}");
}

#[test]
fn reindex_restart_and_source_deletion_preserve_exact_historical_readback() {
    let fixture = Fixture::new();
    let old = b"retained-old needle";
    let first = fixture.index(old);
    let old_revision = field(&first, "revision_id").to_owned();
    fixture.index(b"retained-new value");
    fs::remove_file(&fixture.source).unwrap();
    assert_eq!(matches(&fixture.search("retained-new")).len(), 1);
    assert!(matches(&fixture.search("retained-old")).is_empty());
    let end = old.len().to_string();
    let output = Fixture::ok(&["--read-revision", fixture.data.to_str().unwrap(), &old_revision, "0", &end]);
    let hex = old.iter().fold(String::new(), |mut output, byte| {
        let _ = write!(output, "{byte:02x}");
        output
    });
    assert!(output.contains(&hex), "{output}");
}

#[test]
fn primary_search_returns_each_overlapping_match_once() {
    let fixture = Fixture::new();
    fixture.index(b"aaaaa");
    let output = fixture.search("aaa");
    let rows = matches(&output);
    assert_eq!(rows.len(), 3, "{output}");
    for (index, row) in rows.iter().enumerate() {
        assert!(row.contains(&format!("\"byte_start\":{index},")), "{output}");
    }
}

#[test]
fn one_shot_file_and_stdin_preserve_exact_matches_without_creating_a_catalog() {
    let fixture = Fixture::new();
    let text = "α aAa\r\nb\0aaaa\n";
    fs::write(&fixture.source, text).unwrap();
    let file = Fixture::ok(&["--scan-file-ascii-insensitive", "aa", fixture.source.to_str().unwrap()]);
    let (status, stdin, stderr) = Fixture::run_with_stdin(&["--scan-stdin-ascii-insensitive", "aa"],
        Stdio::from(fs::File::open(&fixture.source).unwrap()));
    assert!(status.success(), "{stderr}");
    let file_rows = matches(&file);
    let stdin_rows = matches(&stdin);
    assert_eq!(file_rows.len(), 5, "{file}");
    assert_eq!(stdin_rows.len(), file_rows.len(), "{stdin}");
    for ((file_row, stdin_row), start) in file_rows.iter().zip(&stdin_rows).zip([3, 4, 10, 11, 12]) {
        assert!(file_row.contains(&format!("\"byte_start\":{start},")), "{file_row}");
        assert_eq!(file_row.replace("\"source_backed\":true", "\"source_backed\":false"), *stdin_row);
    }
    assert!(file.contains("\"complete\":true") && stdin.contains("\"complete\":true"));
    assert_eq!(fs::read(&fixture.source).unwrap(), text.as_bytes());
    assert_eq!(fs::read_dir(&fixture.data).unwrap().count(), 0);
}

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
    // T17 limit-edge cases stream up to 100k bounded matches (~40 MB).
    // The ceiling stays finite and explicit; larger output still fails closed.
    const MAX_OUTPUT: u64 = 64 * 1024 * 1024;
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
fn aba_reindex_preserves_exact_old_revision_readback_and_current_search() {
    let fixture = Fixture::new();
    let first = fixture.index(b"needle-alpha v1");
    let revision_a1 = field(&first, "revision_id").to_owned();
    let second = fixture.index(b"unrelated-beta v2");
    let revision_b = field(&second, "revision_id").to_owned();
    let third = fixture.index(b"needle-alpha v1");
    let revision_a2 = field(&third, "revision_id").to_owned();
    // Same bytes reuse the same immutable revision; changed bytes get a new one.
    assert_eq!(revision_a1, revision_a2);
    assert_ne!(revision_a1, revision_b);
    // Current corpus reflects the latest admission: A matches, B does not.
    assert_eq!(matches(&fixture.search("needle-alpha")).len(), 1);
    assert!(matches(&fixture.search("unrelated-beta")).is_empty());
    // Both retained revisions remain exactly readable after reindex.
    for (revision, expected) in [
        (revision_a1.as_str(), b"needle-alpha v1".as_slice()),
        (revision_b.as_str(), b"unrelated-beta v2".as_slice()),
    ] {
        let output = Fixture::ok(&[
            "--read-revision",
            fixture.data.to_str().unwrap(),
            revision,
            "0",
            &expected.len().to_string(),
        ]);
        let hex = expected.iter().fold(String::new(), |mut out, byte| {
            let _ = write!(out, "{byte:02x}");
            out
        });
        assert!(output.contains(&hex), "{output}");
    }
}

#[test]
fn empty_corpus_and_empty_file_are_typed_complete_without_matches() {
    let fixture = Fixture::new();
    // Empty corpus: authoritative empty denominator, no gaps, still typed.
    let output = fixture.search("absent");
    assert!(matches(&output).is_empty(), "{output}");
    assert!(output.contains("\"complete\":true"), "{output}");
    assert!(output.contains("\"searched_sources\":0"), "{output}");
    assert!(output.contains("\"active_sources\":0"), "{output}");
    // Empty files are denied at canonical admission before CAS: a typed
    // refusal, never an admitted empty revision relabelled as success.
    fs::write(&fixture.source, b"").unwrap();
    let (status, _, stderr) = Fixture::run(&[
        "--index-file",
        fixture.data.to_str().unwrap(),
        fixture.source.to_str().unwrap(),
    ]);
    assert!(!status.success(), "empty index must not succeed");
    assert!(stderr.contains("SOURCE_ADMISSION_DENIED"), "{stderr}");
    // The refused admission leaves the corpus empty and still typed complete.
    let output = fixture.search("absent");
    assert!(matches(&output).is_empty(), "{output}");
    assert!(output.contains("\"complete\":true"), "{output}");
    assert!(output.contains("\"active_sources\":0"), "{output}");
}

fn tail(output: &str) -> String {
    const KEEP: usize = 2048;
    if output.len() <= KEEP {
        output.to_owned()
    } else {
        format!(
            "<{} bytes>...{}",
            output.len(),
            &output[output.len() - KEEP..]
        )
    }
}

#[test]
fn limit_exactly_reached_is_complete_but_one_extra_match_is_degraded() {
    let fixture = Fixture::new();
    // Exactly at the shared output ceiling: complete, no truncation flag.
    let exact = "a".repeat(100_000);
    fs::write(&fixture.source, exact.as_bytes()).unwrap();
    fixture.index(exact.as_bytes());
    let output = fixture.search("a");
    assert_eq!(
        matches(&output).len(),
        100_000,
        "exact-limit tail={}",
        tail(&output)
    );
    assert!(output.contains("\"complete\":true"), "{}", tail(&output));
    assert!(
        output.contains("\"match_limit_reached\":false"),
        "{}",
        tail(&output)
    );
    // One additional match beyond the ceiling: degraded typed data, never success.
    let fixture = Fixture::new();
    let over = "a".repeat(100_001);
    fs::write(&fixture.source, over.as_bytes()).unwrap();
    fixture.index(over.as_bytes());
    let output = fixture.search("a");
    assert_eq!(
        matches(&output).len(),
        100_000,
        "truncated tail={}",
        tail(&output)
    );
    assert!(output.contains("\"complete\":false"), "{}", tail(&output));
    assert!(
        output.contains("\"match_limit_reached\":true"),
        "{}",
        tail(&output)
    );
}

#[test]
fn truncated_denominator_reports_every_unattempted_source_as_gap() {
    let fixture = Fixture::new();
    // Three sources of 50k matches each: the first two fill the 100k ceiling,
    // so the third source is never attempted and must appear as an explicit
    // gap instead of silently narrowing the denominator (invariant 6).
    let dir = fixture.base.join("many");
    fs::create_dir_all(&dir).unwrap();
    for name in ["s1.txt", "s2.txt", "s3.txt"] {
        fs::write(dir.join(name), "a".repeat(50_000).as_bytes()).unwrap();
    }
    let output = Fixture::ok(&[
        "--index-directory",
        fixture.data.to_str().unwrap(),
        dir.to_str().unwrap(),
    ]);
    fixture.guard.refresh();
    assert!(output.contains("\"changed\":3"), "{}", tail(&output));
    let output = fixture.search("a");
    assert_eq!(matches(&output).len(), 100_000, "tail={}", tail(&output));
    assert!(output.contains("\"complete\":false"), "{}", tail(&output));
    assert!(
        output.contains("\"match_limit_reached\":true"),
        "{}",
        tail(&output)
    );
    // The unattempted remainder is an explicit typed gap, not an omission.
    assert!(
        output.contains("\"event\":\"source_gap\""),
        "{}",
        tail(&output)
    );
    assert!(
        output.contains("DIRECT_MATCH_LIMIT_REACHED")
            || output.contains("DIRECT_DENOMINATOR_TRUNCATED"),
        "{}",
        tail(&output)
    );
}

#[test]
fn missing_preparation_is_explicit_gap_without_query_time_repair() {
    let fixture = Fixture::new();
    fixture.index(b"repairable needle here");
    let before = fixture.search("needle");
    assert_eq!(matches(&before).len(), 1, "{before}");
    assert!(before.contains("\"complete\":true"), "{before}");
    // Delete every stored preparation object: the next query must report an
    // explicit gap and must not rebuild durable state on the query path.
    let preparation = fixture.data.join("preparation");
    let snapshot = list_files(&preparation);
    assert!(!snapshot.is_empty(), "expected stored preparation");
    fs::remove_dir_all(&preparation).unwrap();
    let output = fixture.search("needle");
    assert!(matches(&output).is_empty(), "{output}");
    assert!(output.contains("\"complete\":false"), "{output}");
    assert!(
        output.contains("DIRECT_PREPARATION_UNAVAILABLE")
            || output.contains("DIRECT_PREPARATION_OBJECT_INVALID")
            || output.contains("DIRECT_PREPARATION_REFERENCE_READ_FAILED"),
        "{output}"
    );
    assert!(
        !preparation.exists(),
        "query path must not repair missing preparation"
    );
}

#[test]
fn repeated_bounded_queries_cause_no_durable_writes_without_qdrant() {
    let fixture = Fixture::new();
    fixture.index(b"stable needle for no-write check");
    // Explicit DIRECT never consults indexed search: bogus Qdrant endpoints
    // must not block the source-backed path (invariant: one real primary path
    // without a second index).
    let output = std::process::Command::new(env!("CARGO_BIN_EXE_eliot-searchd"))
        .args(["--search-root", fixture.data.to_str().unwrap(), "needle"])
        .env("QDRANT_URL", "http://127.0.0.1:9")
        .env("ELIOT_QDRANT_ENDPOINT", "http://127.0.0.1:9")
        .output()
        .expect("primary daemon");
    assert!(output.status.success(), "{output:?}");
    let stdout = String::from_utf8(output.stdout).expect("UTF-8");
    assert_eq!(matches(&stdout).len(), 1, "{stdout}");
    // Owner succession rewrites its own epoch files on every process; the
    // read-only claim covers the admitted corpus (control log, revisions,
    // preparation), never the live-owner exclusion records.
    let before = list_corpus_files(&fixture.data);
    for _ in 0..50 {
        let output = fixture.search("needle");
        assert_eq!(matches(&output).len(), 1, "{output}");
        assert!(output.contains("\"complete\":true"), "{output}");
    }
    assert_eq!(
        list_corpus_files(&fixture.data),
        before,
        "queries must be read-only over the admitted corpus"
    );
}

#[test]
fn cross_unit_unicode_crlf_reports_source_byte_coordinates() {
    let fixture = Fixture::new();
    let prefix = "x".repeat(16 * 1024 - 2);
    let text = format!("{prefix}β\r\n𐀀 needle\r\ntail");
    let start = text.find("needle").unwrap();
    fixture.index(text.as_bytes());
    let output = fixture.search("needle");
    let rows = matches(&output);
    assert_eq!(rows.len(), 1, "{output}");
    assert!(
        rows[0].contains(&format!("\"byte_start\":{start},")),
        "{output}"
    );
    assert!(
        rows[0].contains(&format!("\"byte_end\":{},", start + 6)),
        "{output}"
    );
    assert!(output.contains("\"complete\":true"), "{output}");
}

fn list_files(root: &std::path::Path) -> Vec<(PathBuf, u64)> {
    let mut out = Vec::new();
    let mut stack = vec![root.to_path_buf()];
    while let Some(dir) = stack.pop() {
        let entries = fs::read_dir(&dir).unwrap_or_else(|_| panic!("read {}", dir.display()));
        for entry in entries {
            let entry = entry.unwrap();
            let path = entry.path();
            let kind = entry.file_type().unwrap();
            if kind.is_dir() {
                stack.push(path);
            } else if kind.is_file() {
                out.push((path, entry.metadata().unwrap().len()));
            }
        }
    }
    out.sort();
    out
}

fn list_corpus_files(data_root: &std::path::Path) -> Vec<(PathBuf, u64)> {
    list_files(data_root)
        .into_iter()
        .filter(|(path, _)| {
            path.strip_prefix(data_root).is_ok_and(|relative| {
                relative.starts_with("control")
                    || relative.starts_with("revisions")
                    || relative.starts_with("preparation")
            })
        })
        .collect()
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

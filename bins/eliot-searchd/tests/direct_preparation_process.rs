//! Exercises shared preparation through the actual primary daemon executable.

use std::fs;
use std::io::Read;
use std::path::PathBuf;
use std::process::{Command, ExitStatus, Stdio};
use std::sync::atomic::{AtomicU64, Ordering};
use std::thread;
use std::time::{Duration, Instant, SystemTime, UNIX_EPOCH};

static NEXT: AtomicU64 = AtomicU64::new(0);

struct Fixture {
    base: PathBuf,
    data: PathBuf,
    source: PathBuf,
}
impl Fixture {
    fn new() -> Self {
        let stamp = SystemTime::now().duration_since(UNIX_EPOCH).unwrap().as_nanos();
        let base = std::env::temp_dir().join(format!("eliot-preparation-{}-{stamp}-{}",
            std::process::id(), NEXT.fetch_add(1, Ordering::Relaxed)));
        let data = base.join("data");
        let source = base.join("source.txt");
        fs::create_dir_all(&data).unwrap();
        Self { base, data, source }
    }

    fn run(&self, args: &[&str]) -> (ExitStatus, String, String) {
        let mut child = Command::new(env!("CARGO_BIN_EXE_eliot-searchd"))
            .args(args).stdin(Stdio::null()).stdout(Stdio::piped()).stderr(Stdio::piped())
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

    fn ok(&self, args: &[&str]) -> String {
        let (status, stdout, stderr) = self.run(args);
        assert!(status.success(), "status={status} stdout={stdout} stderr={stderr}");
        stdout
    }

    fn index(&self, bytes: &[u8]) -> String {
        fs::write(&self.source, bytes).unwrap();
        self.ok(&["--index-file", self.data.to_str().unwrap(), self.source.to_str().unwrap()])
    }

    fn search(&self, query: &str) -> String {
        self.ok(&["--search-root", self.data.to_str().unwrap(), query])
    }
}
impl Drop for Fixture {
    fn drop(&mut self) { let _ = fs::remove_dir_all(&self.base); }
}

fn read_output(reader: impl Read) -> String {
    const MAX_OUTPUT: u64 = 1024 * 1024;
    let mut bytes = Vec::new();
    reader.take(MAX_OUTPUT + 1).read_to_end(&mut bytes).unwrap();
    assert!(bytes.len() <= MAX_OUTPUT as usize, "test output ceiling exceeded");
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
    let output = fixture.ok(&["--read-revision", fixture.data.to_str().unwrap(), &old_revision, "0", &end]);
    let hex = old.iter().map(|byte| format!("{byte:02x}")).collect::<String>();
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

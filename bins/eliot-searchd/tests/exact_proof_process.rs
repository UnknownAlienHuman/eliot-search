//! T34 exact-proof process evidence through the primary daemon.
//!
//! The frozen-denominator exact proof (plan with live barrier revalidation,
//! partial/timeout/cancel never a complete negative) is exercised through the
//! stable primary-daemon CLI surface only: no provider/query composition
//! changes, no indexed/Qdrant path (out of scope for T34).
//!
//! - positive: an admitted two-source denominator proves a source-backed
//!   witness with `complete=true`, and a complete negative only where every
//!   denominator item completed;
//! - denied-partial: refused admission never enters the denominator and an
//!   unsupported unit becomes an explicit gap, so every affected search stays
//!   `complete=false` (invariant 15);
//! - gaps: a missing source root is an explicit observation gap that blocks
//!   currentness and sync without retiring retained proofs (T31);
//! - truncation: an unattempted admitted remainder is an explicit gap, never
//!   a narrowed denominator relabelled complete (invariant 6).
//!
//! Unsafe code stays confined to `common` (the Win32 credential-cleanup call
//! shared by every process test); this file adds none.

#[allow(dead_code)]
mod common;

use std::fs;
use std::io::Read;
use std::path::{Path, PathBuf};
use std::process::{Command, ExitStatus, Stdio};
use std::sync::atomic::{AtomicU64, Ordering};
use std::thread;
use std::time::{Duration, Instant, SystemTime, UNIX_EPOCH};

static NEXT: AtomicU64 = AtomicU64::new(0);

struct Fixture {
    base: PathBuf,
    data: PathBuf,
    files: PathBuf,
    guard: common::RevisionKeyGuard,
}

impl Fixture {
    fn new() -> Self {
        let stamp = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .expect("system time")
            .as_nanos();
        let base = std::env::temp_dir().join(format!(
            "eliot-exact-proof-{}-{stamp}-{}",
            std::process::id(),
            NEXT.fetch_add(1, Ordering::Relaxed),
        ));
        let data = base.join("data");
        let files = base.join("files");
        fs::create_dir_all(&data).expect("fixture data dir");
        fs::create_dir_all(&files).expect("fixture files dir");
        let guard = common::RevisionKeyGuard::for_data_root(&data);
        Self {
            base,
            data,
            files,
            guard,
        }
    }

    fn run(&self, args: &[&str]) -> (ExitStatus, String, String) {
        let mut child = Command::new(env!("CARGO_BIN_EXE_eliot-searchd"))
            .args(args)
            .stdin(Stdio::null())
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .spawn()
            .expect("primary daemon");
        let stdout = child.stdout.take().expect("stdout pipe");
        let stderr = child.stderr.take().expect("stderr pipe");
        let out = thread::spawn(move || read_output(stdout));
        let err = thread::spawn(move || read_output(stderr));
        let deadline = Instant::now() + Duration::from_secs(30);
        let status = loop {
            if let Some(status) = child.try_wait().expect("poll child") {
                break status;
            }
            if Instant::now() >= deadline {
                let _ = child.kill();
                let _ = child.wait();
                panic!("primary daemon exceeded the test deadline");
            }
            thread::sleep(Duration::from_millis(10));
        };
        let stdout = out.join().expect("drain stdout");
        let stderr = err.join().expect("drain stderr");
        self.guard.refresh();
        (status, stdout, stderr)
    }

    fn ok(&self, args: &[&str]) -> String {
        let (status, stdout, stderr) = self.run(args);
        assert!(
            status.success(),
            "status={status} stdout={stdout} stderr={stderr}"
        );
        stdout
    }

    fn write_file(&self, name: &str, bytes: &[u8]) -> PathBuf {
        let path = self.files.join(name);
        fs::write(&path, bytes).expect("fixture file");
        path
    }

    fn index(&self, file: &Path) -> String {
        self.ok(&[
            "--index-file",
            self.data.to_str().expect("data path"),
            file.to_str().expect("file path"),
        ])
    }

    fn search(&self, query: &str) -> String {
        self.ok(&[
            "--search-root",
            self.data.to_str().expect("data path"),
            query,
        ])
    }
}

impl Drop for Fixture {
    fn drop(&mut self) {
        self.guard.cleanup();
        let _ = fs::remove_dir_all(&self.base);
    }
}

fn read_output(reader: impl Read) -> String {
    const MAX_OUTPUT: u64 = 64 * 1024 * 1024;
    let mut bytes = Vec::new();
    reader
        .take(MAX_OUTPUT + 1)
        .read_to_end(&mut bytes)
        .expect("drain pipe");
    assert!(
        u64::try_from(bytes.len()).expect("output fits") <= MAX_OUTPUT,
        "test output ceiling exceeded"
    );
    String::from_utf8(bytes).expect("UTF-8 output")
}

fn field<'a>(output: &'a str, key: &str) -> &'a str {
    let needle = format!("\"{key}\":\"");
    output
        .split_once(&needle)
        .expect("field present")
        .1
        .split('"')
        .next()
        .expect("field value")
}

fn matches(output: &str) -> Vec<&str> {
    output
        .lines()
        .filter(|line| line.contains("\"event\":\"match\""))
        .collect()
}

fn gaps(output: &str) -> Vec<&str> {
    output
        .lines()
        .filter(|line| line.contains("\"event\":\"source_gap\""))
        .collect()
}

fn hex_bytes(bytes: &[u8]) -> String {
    bytes.iter().fold(String::new(), |mut out, byte| {
        use std::fmt::Write as _;
        write!(out, "{byte:02x}").expect("hex push");
        out
    })
}

#[test]
fn positive_witness_reconstructs_from_retained_bytes() {
    let fixture = Fixture::new();
    let alpha = fixture.write_file("alpha.txt", b"alpha needle beta");
    let gamma = fixture.write_file("gamma.txt", b"gamma delta");
    let indexed = fixture.index(&alpha);
    let revision = field(&indexed, "revision_id").to_owned();
    fixture.index(&gamma);

    // Positive witness over the frozen admitted denominator: source-backed,
    // complete, no gaps, both sources searched.
    let output = fixture.search("needle");
    let rows = matches(&output);
    assert_eq!(rows.len(), 1, "{output}");
    assert!(rows[0].contains("\"byte_start\":6,"), "{output}");
    assert!(rows[0].contains("\"byte_end\":12,"), "{output}");
    assert!(rows[0].contains("\"source_backed\":true"), "{output}");
    assert!(output.contains("\"complete\":true"), "{output}");
    assert!(output.contains("\"gaps\":0"), "{output}");
    assert!(output.contains("\"searched_sources\":2"), "{output}");
    assert!(output.contains("\"active_sources\":2"), "{output}");

    // A complete negative is legitimate only where every denominator item
    // completed with zero matches.
    let output = fixture.search("absent-needle-xyz");
    assert!(matches(&output).is_empty(), "{output}");
    assert!(output.contains("\"complete\":true"), "{output}");
    assert!(output.contains("\"gaps\":0"), "{output}");

    // The witness reconstructs from exact retained bytes after the fact.
    let text = b"alpha needle beta";
    let slice = fixture.ok(&[
        "--read-revision",
        fixture.data.to_str().expect("data path"),
        &revision,
        "0",
        &text.len().to_string(),
    ]);
    assert!(slice.contains(&hex_bytes(text)), "{slice}");

    // The store verifies every referenced revision it claims to prove over.
    let verified = fixture.ok(&["--verify-root", fixture.data.to_str().expect("data path")]);
    assert!(
        verified.contains("\"referenced_revisions\":2"),
        "{verified}"
    );
    assert!(verified.contains("\"verified_revisions\":2"), "{verified}");
    let listed = fixture.ok(&["--list-sources", fixture.data.to_str().expect("data path")]);
    assert!(listed.contains("\"sources\":2"), "{listed}");
}

#[test]
fn denied_admission_and_unsupported_unit_stay_partial_never_complete_negative() {
    let fixture = Fixture::new();
    let good = fixture.write_file("good.txt", b"alpha needle beta");
    fixture.index(&good);

    // Refused admission is a typed denial: it never enters the denominator.
    let empty = fixture.write_file("empty.txt", b"");
    let (status, _, stderr) = fixture.run(&[
        "--index-file",
        fixture.data.to_str().expect("data path"),
        empty.to_str().expect("file path"),
    ]);
    assert!(!status.success(), "empty index must not succeed");
    assert!(stderr.contains("SOURCE_ADMISSION_DENIED"), "{stderr}");

    // An admitted but unsupported unit becomes an explicit gap, never silent
    // narrowing: the healthy source still proves its witness as partial data.
    let binary = fixture.write_file("binary.dat", b"a\0b");
    fixture.index(&binary);
    let output = fixture.search("needle");
    assert_eq!(matches(&output).len(), 1, "{output}");
    assert_eq!(gaps(&output).len(), 1, "{output}");
    assert!(
        output.contains("MATERIALIZATION_BINARY_CONTENT"),
        "{output}"
    );
    assert!(output.contains("\"complete\":false"), "{output}");
    assert!(output.contains("\"active_sources\":2"), "{output}");

    // Zero matches over a gapped denominator is incomplete data, never a
    // complete negative proof (invariant 15).
    let output = fixture.search("absent-needle-xyz");
    assert!(matches(&output).is_empty(), "{output}");
    assert_eq!(gaps(&output).len(), 1, "{output}");
    assert!(output.contains("\"complete\":false"), "{output}");
    assert!(
        !output.contains("\"complete\":true"),
        "partial outcome must never read as success: {output}"
    );
}

#[test]
fn observation_gap_blocks_currentness_without_retiring_retained_proofs() {
    let fixture = Fixture::new();
    let root = fixture.base.join("watched");
    fs::create_dir(&root).expect("watched dir");
    fs::write(root.join("note.txt"), b"current workspace needle").expect("note");

    let registered = fixture.ok(&[
        "--register-source-root",
        fixture.data.to_str().expect("data path"),
        root.to_str().expect("root path"),
    ]);
    assert!(registered.contains("\"persisted\":true"), "{registered}");
    let synced = fixture.ok(&[
        "--sync-source-roots",
        fixture.data.to_str().expect("data path"),
    ]);
    assert!(synced.contains("\"complete\":true"), "{synced}");
    assert!(
        synced.contains("\"current_workspace_proven\":false"),
        "{synced}"
    );
    let output = fixture.search("current");
    assert_eq!(matches(&output).len(), 1, "{output}");
    assert!(output.contains("\"complete\":true"), "{output}");
    assert!(gaps(&output).is_empty(), "{output}");

    // The root disappears: an explicit observation gap, never an empty
    // inventory, blocks currentness and sync (T31 live barrier).
    fs::remove_dir_all(&root).expect("remove watched root");
    let listed = fixture.ok(&["--source-roots", fixture.data.to_str().expect("data path")]);
    assert!(listed.contains("\"unavailable\":1"), "{listed}");
    assert!(listed.contains("\"event\":\"source_gap\""), "{listed}");
    assert!(
        listed.contains("\"current_workspace_proven\":false"),
        "{listed}"
    );
    let (status, _, stderr) = fixture.run(&[
        "--sync-source-roots",
        fixture.data.to_str().expect("data path"),
    ]);
    assert!(!status.success(), "gapped sync must fail closed");
    assert!(stderr.contains("SOURCE_ROOTS_UNAVAILABLE"), "{stderr}");

    // No mass retire: the retained revision still proves its witness.
    let output = fixture.search("current");
    assert_eq!(matches(&output).len(), 1, "{output}");
}

#[test]
fn truncated_denominator_reports_every_unattempted_source_as_gap() {
    let fixture = Fixture::new();
    let many = fixture.files.join("many");
    fs::create_dir(&many).expect("many dir");
    for name in ["s1.txt", "s2.txt", "s3.txt"] {
        fs::write(many.join(name), "a".repeat(50_000).as_bytes()).expect("bulk file");
    }
    let indexed = fixture.ok(&[
        "--index-directory",
        fixture.data.to_str().expect("data path"),
        many.to_str().expect("many path"),
    ]);
    assert!(indexed.contains("\"changed\":3"), "{indexed}");

    // The first two sources fill the 100k corpus ceiling, so the third is
    // never attempted: it must appear as an explicit typed gap instead of
    // silently narrowing the denominator (invariant 6), and the outcome stays
    // degraded data, never success (invariant 15).
    let output = fixture.search("a");
    assert_eq!(matches(&output).len(), 100_000, "match count");
    assert!(output.contains("\"complete\":false"), "{output}");
    assert!(output.contains("\"match_limit_reached\":true"), "{output}");
    assert!(output.contains("\"active_sources\":3"), "{output}");
    assert!(output.contains("\"searched_sources\":2"), "{output}");
    assert!(!gaps(&output).is_empty(), "unattempted source needs a gap");
    assert!(
        output.contains("DIRECT_MATCH_LIMIT_REACHED")
            || output.contains("DIRECT_DENOMINATOR_TRUNCATED"),
        "{output}"
    );
}

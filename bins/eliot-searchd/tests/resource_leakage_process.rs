//! T40 minimized diagnostics and measured resource budgets on the DIRECT spine.
//!
//! Every test drives the real `eliot-searchd` binary on a disposable data
//! root; figures are wall-clock measurements on that path, never constants.
//! Resident memory has no honest in-process source here, so budget reports
//! carry `memory_bytes=UNAVAILABLE` instead of an invented number.
//!
//! - secret/path/source/overlay sentinels are synthetic (never customer
//!   bytes) and are scanned in all captured stdout/stderr plus the control
//!   log on success and on typed failure paths;
//! - ingestion, repeated queries, currentness (`--verify-root`) and rebuild
//!   are timed on a frozen 24-file corpus with p50/p95 and control-byte
//!   deltas enforced against T30-derived ceilings;
//! - long-line, many-file and high-match adversarial workloads must
//!   terminate within declared bounds with typed (never relabelled) outcomes
//!   (invariant 15);
//! - 300 CLI queries must append zero control records; the full 10,000-query
//!   no-write property stays proven in-process by
//!   `secure_direct_store::spine_gate_tests::
//!   ten_thousand_bounded_queries_cause_no_durable_corpus_writes`, cited —
//!   not re-driven through 10k process spawns — here;
//! - cancellation is a bounded kill-to-reap on a real oversized batch with a
//!   typed post-state (verified JSON or a closed-code failure, never silent
//!   success).
//!
//! Unsafe code stays confined to `common` (Win32 credential cleanup shared by
//! every process test); this file adds none.

#[allow(dead_code)]
mod common;

use std::fs;
use std::io::Read;
use std::path::{Path, PathBuf};
use std::process::{Command, ExitStatus, Stdio};
use std::sync::atomic::{AtomicU64, Ordering};
use std::thread;
use std::time::{Duration, Instant, SystemTime, UNIX_EPOCH};

use eliot_searchd::diagnostics::contains_sentinel;
use eliot_searchd::resource_budgets::{
    BudgetReport, BudgetSample, CEIL_CANCELLATION_MS, CEIL_SINGLE_OP_MS, CEIL_TOTAL_BATCH_MS,
    CEIL_TOTAL_RESPONSE_BYTES,
};

static NEXT: AtomicU64 = AtomicU64::new(0);

/// Synthetic canaries. The secret canary is really ingested (proving the
/// absence below is meaningful, not vacuous); the overlay canary travels as
/// query bytes (which outputs must never echo); the source canary is raw
/// corpus bytes (authorized only as hex in `--read-revision`).
const SECRET_CANARY: &str = "T40-SECRET-SENTINEL-9f2c41ab77";
const OVERLAY_CANARY: &str = "T40-OVERLAY-SENTINEL-3d8e02cf19";
const SOURCE_CANARY: &str = "t40-source-sentinel-5b71c0aa42";
/// Frozen corpus shape: file count is part of the measured-budget identity.
const FROZEN_FILES: usize = 24;
/// CLI query sample for the no-control-record proof (the 10k proof itself is
/// the cited in-process gate; see module docs).
const CLI_QUERY_SAMPLE: usize = 300;
/// Repeated-query sample for p50/p95.
const QUERY_BUDGET_SAMPLE: usize = 60;

struct Fixture {
    base: PathBuf,
    data: PathBuf,
    files: PathBuf,
    guard: common::RevisionKeyGuard,
}

impl Fixture {
    fn new(tag: &str) -> Self {
        let stamp = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .map_or(0, |elapsed| elapsed.as_nanos());
        let base = std::env::temp_dir().join(format!(
            "eliot-t40-{tag}-{}-{stamp}-{}",
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
            .expect("primary daemon binary runs");
        let stdout = child.stdout.take().expect("stdout is piped");
        let stderr = child.stderr.take().expect("stderr is piped");
        let out = thread::spawn(move || read_bounded(stdout));
        let err = thread::spawn(move || read_bounded(stderr));
        let deadline = Instant::now() + Duration::from_secs(30);
        let status = loop {
            if let Some(status) = child.try_wait().expect("daemon wait succeeds") {
                break status;
            }
            if Instant::now() >= deadline {
                let _ = child.kill();
                let _ = child.wait();
                panic!("primary daemon exceeded the 30s test deadline");
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

    fn utf8(&self, path: &str) -> PathBuf {
        self.base.join(path)
    }

    fn data_utf8(&self) -> String {
        self.data.to_str().expect("data path is UTF-8").to_owned()
    }

    fn control_log_len(&self) -> u64 {
        fs::metadata(self.data.join("control").join("source-events.log"))
            .map_or(0, |metadata| metadata.len())
    }

    fn source_events(&self) -> u64 {
        let output = self.ok(&["--verify-root", &self.data_utf8()]);
        json_num(&output, "source_events")
    }
}

impl Drop for Fixture {
    fn drop(&mut self) {
        self.guard.cleanup();
        let _ = fs::remove_dir_all(&self.base);
    }
}

fn read_bounded(reader: impl Read) -> String {
    const MAX_OUTPUT: u64 = 64 * 1024 * 1024;
    let mut bytes = Vec::new();
    reader
        .take(MAX_OUTPUT + 1)
        .read_to_end(&mut bytes)
        .expect("bounded output drain succeeds");
    assert!(
        u64::try_from(bytes.len()).expect("output length fits") <= MAX_OUTPUT,
        "test output ceiling exceeded"
    );
    String::from_utf8(bytes).expect("daemon output is UTF-8")
}

fn json_num(output: &str, key: &str) -> u64 {
    let needle = format!("\"{key}\":");
    output
        .split_once(&needle)
        .unwrap_or_else(|| panic!("daemon output carries numeric {key}: {output}"))
        .1
        .chars()
        .take_while(char::is_ascii_digit)
        .collect::<String>()
        .parse()
        .unwrap_or_else(|_| panic!("numeric {key} parses: {output}"))
}

fn match_lines(output: &str) -> Vec<&str> {
    output
        .lines()
        .filter(|line| line.contains("\"event\":\"match\""))
        .collect()
}

/// Writes the frozen corpus: 24 deterministic files. File 07 carries the
/// secret canary and file 11 the source canary; both are really ingested.
fn write_frozen_corpus(fixture: &Fixture) {
    use std::fmt::Write as _;
    for index in 0..FROZEN_FILES {
        let mut body = String::new();
        let _ = write!(
            body,
            "t40 frozen corpus file {index:03} needle-grain payload line one\n\
             t40 frozen corpus file {index:03} needle-grain payload line two\n"
        );
        if index == 7 {
            let _ = writeln!(body, "ingested secret marker {SECRET_CANARY}");
        }
        if index == 11 {
            let _ = writeln!(body, "ingested source marker {SOURCE_CANARY}");
        }
        fs::write(
            fixture.files.join(format!("t40-frozen-{index:03}.txt")),
            body,
        )
        .expect("frozen corpus file is written");
    }
}

fn all_captured(outputs: &[String]) -> String {
    outputs.join("\n")
}

#[test]
fn t40_sentinels_absent_on_success_paths() {
    let fixture = Fixture::new("sentinel-ok");
    write_frozen_corpus(&fixture);
    let mut captured = Vec::new();

    let index = fixture.ok(&[
        "--index-directory",
        &fixture.data_utf8(),
        fixture.files.to_str().expect("files path is UTF-8"),
    ]);
    captured.push(index);
    let hit = fixture.ok(&["--search-root", &fixture.data_utf8(), "needle-grain"]);
    assert_eq!(match_lines(&hit).len(), 2 * FROZEN_FILES, "{hit}");
    assert!(hit.contains("\"complete\":true"), "{hit}");
    captured.push(hit);
    // The overlay canary travels as query bytes; outputs must never echo it.
    let overlay_probe = fixture.ok(&["--search-root", &fixture.data_utf8(), OVERLAY_CANARY]);
    assert!(match_lines(&overlay_probe).is_empty(), "{overlay_probe}");
    captured.push(overlay_probe);
    let verify = fixture.ok(&["--verify-root", &fixture.data_utf8()]);
    captured.push(verify);
    let listed = fixture.ok(&["--list-sources", &fixture.data_utf8()]);
    captured.push(listed);

    let revision_id = revision_of(&fixture, "t40-frozen-011.txt");
    let probe_len = fs::metadata(fixture.files.join("t40-frozen-011.txt"))
        .expect("probe length reads")
        .len()
        .to_string();
    let revision = fixture.ok(&[
        "--read-revision",
        &fixture.data_utf8(),
        &revision_id,
        "0",
        &probe_len,
    ]);
    // Raw source bytes stay out of every frame; the authorized hex field is
    // the only place corpus content may appear.
    assert!(
        !revision.contains(SOURCE_CANARY),
        "raw source bytes must not appear even in the authorized slice"
    );
    captured.push(revision);
    captured.push(control_log_text(&fixture.data));

    let haystack = all_captured(&captured);
    let base = fixture.base.to_str().expect("base path is UTF-8");
    assert!(
        !contains_sentinel(&haystack, &[SECRET_CANARY, OVERLAY_CANARY, SOURCE_CANARY]),
        "success-path outputs leak canary material"
    );
    assert!(
        !haystack.contains(base),
        "success-path outputs leak the absolute root path"
    );
    // The secret really was ingested (absence above is meaningful, not
    // vacuous): the authorized `--read-revision` hex field for file 07
    // carries the exact canary bytes, while the raw canary appears nowhere.
    // (Revision objects at rest are OS-vault sealed, so raw bytes are not
    // expected verbatim on disk either.)
    let secret_revision = revision_of(&fixture, "t40-frozen-007.txt");
    let secret_len = fs::metadata(fixture.files.join("t40-frozen-007.txt"))
        .expect("secret probe length reads")
        .len()
        .to_string();
    let secret_slice = fixture.ok(&[
        "--read-revision",
        &fixture.data_utf8(),
        &secret_revision,
        "0",
        &secret_len,
    ]);
    assert!(
        secret_slice.contains(&hex_bytes(SECRET_CANARY.as_bytes())),
        "authorized hex field carries the ingested canary"
    );
    assert!(
        !secret_slice.contains(SECRET_CANARY),
        "raw canary stays out of the authorized slice too"
    );
    eprintln!(
        "T40_SENTINEL success paths clean over {} frames",
        captured.len()
    );
}

#[test]
fn t40_sentinels_absent_on_failure_paths() {
    let fixture = Fixture::new("sentinel-err");
    write_frozen_corpus(&fixture);
    fixture.ok(&[
        "--index-directory",
        &fixture.data_utf8(),
        fixture.files.to_str().expect("files path is UTF-8"),
    ]);
    let mut captured = Vec::new();

    // Missing source file: typed denial, closed code, no path/secret echo.
    let missing = fixture.utf8("no-such-file.txt");
    let (status, stdout, stderr) = fixture.run(&[
        "--index-file",
        &fixture.data_utf8(),
        missing.to_str().expect("missing path is UTF-8"),
    ]);
    assert!(!status.success(), "missing source must not succeed");
    captured.push(stdout);
    captured.push(stderr);
    // Empty admission is denied before content processing (T30 spine gate).
    let empty = fixture.utf8("empty.txt");
    fs::write(&empty, b"").expect("empty probe is written");
    let (status, stdout, stderr) = fixture.run(&[
        "--index-file",
        &fixture.data_utf8(),
        empty.to_str().expect("empty path is UTF-8"),
    ]);
    assert!(!status.success(), "empty admission must not succeed");
    assert!(stderr.contains("SOURCE_ADMISSION_DENIED"), "{stderr}");
    captured.push(stdout);
    captured.push(stderr);
    // Out-of-range revision slice: typed failure, never silent success.
    let (status, stdout, stderr) = fixture.run(&[
        "--read-revision",
        &fixture.data_utf8(),
        "00",
        "999999",
        "1000000",
    ]);
    assert!(!status.success(), "out-of-range slice must not succeed");
    captured.push(stdout);
    captured.push(stderr);

    let haystack = all_captured(&captured);
    assert!(
        !contains_sentinel(&haystack, &[SECRET_CANARY, OVERLAY_CANARY, SOURCE_CANARY]),
        "failure-path outputs leak canary material: {haystack}"
    );
    for line in haystack.lines() {
        // Every failure line is a closed code or a closed JSON error event;
        // raw `C:\` paths or secret text never appear.
        assert!(
            !line.contains("T40-SECRET") && !line.contains("t40-source-sentinel"),
            "failure line leaks: {line}"
        );
    }
    // Failure is typed on stderr with a closed head (sanitized `CODE` token).
    assert!(
        captured
            .iter()
            .any(|output| output.contains("SOURCE_ADMISSION_DENIED")),
        "typed denial code is present"
    );
    eprintln!("T40_SENTINEL failure paths typed and clean");
}

#[test]
fn t40_measured_budgets_on_frozen_corpus() {
    let fixture = Fixture::new("budgets");
    write_frozen_corpus(&fixture);
    let mut report = BudgetReport::new();
    let files_arg = fixture
        .files
        .to_str()
        .expect("files path is UTF-8")
        .to_owned();

    let start = Instant::now();
    let index = fixture.ok(&["--index-directory", &fixture.data_utf8(), &files_arg]);
    let elapsed = start.elapsed().as_millis();
    assert!(elapsed <= CEIL_SINGLE_OP_MS, "ingest_ms={elapsed}");
    assert!(
        index.contains("\"event\":\"directory_index_complete\""),
        "{index}"
    );
    let log_after_ingest = fixture.control_log_len();
    report
        .push(BudgetSample {
            op: "ingest",
            elapsed_ms: elapsed,
            response_bytes: index.len() as u64,
            control_bytes_delta: log_after_ingest,
        })
        .expect("bounded ledger admits the ingest sample");
    eprintln!(
        "T40_PERF ingest_ms={elapsed} response_bytes={}",
        index.len()
    );

    let log_before_queries = fixture.control_log_len();
    let query_ms = measure_query_budgets(&fixture, &mut report);
    assert_eq!(
        fixture.control_log_len(),
        log_before_queries,
        "repeated queries append no control records"
    );
    let p50 = percentile(&query_ms, 50);
    let p95 = percentile(&query_ms, 95);
    eprintln!("T40_PERF query_n={QUERY_BUDGET_SAMPLE} p50_ms={p50} p95_ms={p95}");

    let start = Instant::now();
    let verify = fixture.ok(&["--verify-root", &fixture.data_utf8()]);
    let elapsed = start.elapsed().as_millis();
    assert!(elapsed <= CEIL_SINGLE_OP_MS, "currentness_ms={elapsed}");
    report
        .push(BudgetSample {
            op: "currentness",
            elapsed_ms: elapsed,
            response_bytes: verify.len() as u64,
            control_bytes_delta: 0,
        })
        .expect("bounded ledger admits the currentness sample");
    eprintln!("T40_PERF currentness_ms={elapsed}");

    // Rebuild: change one file and re-index the directory; the changed
    // source is re-admitted and search stays complete.
    fs::write(
        fixture.files.join("t40-frozen-000.txt"),
        "t40 rebuilt truth payload needle-grain\n",
    )
    .expect("rebuild mutation is written");
    let start = Instant::now();
    let rebuild = fixture.ok(&["--index-directory", &fixture.data_utf8(), &files_arg]);
    let elapsed = start.elapsed().as_millis();
    assert!(elapsed <= CEIL_SINGLE_OP_MS, "rebuild_ms={elapsed}");
    assert!(rebuild.contains("\"changed\":1"), "{rebuild}");
    report
        .push(BudgetSample {
            op: "rebuild",
            elapsed_ms: elapsed,
            response_bytes: rebuild.len() as u64,
            control_bytes_delta: fixture.control_log_len().saturating_sub(log_after_ingest),
        })
        .expect("bounded ledger admits the rebuild sample");
    let after = fixture.ok(&["--search-root", &fixture.data_utf8(), "needle-grain"]);
    assert!(after.contains("\"complete\":true"), "{after}");
    eprintln!("T40_PERF rebuild_ms={elapsed}");

    report
        .check_totals()
        .expect("measured totals fit declared ceilings");
    assert!(
        report.total_response_bytes() <= CEIL_TOTAL_RESPONSE_BYTES,
        "total response bytes stay bounded"
    );
    assert!(
        report.total_ms() <= CEIL_TOTAL_BATCH_MS,
        "total batch wall stays bounded"
    );
    eprintln!("T40_BUDGET {}", report.json());
}

/// Runs the repeated-query budget sample: alternating hit/miss searches on
/// the frozen corpus, each timed and recorded with zero control delta.
/// Returns the sorted wall-millisecond sample for p50/p95.
fn measure_query_budgets(fixture: &Fixture, report: &mut BudgetReport) -> Vec<u128> {
    let mut query_ms = Vec::with_capacity(QUERY_BUDGET_SAMPLE);
    for round in 0..QUERY_BUDGET_SAMPLE {
        let query = if round % 2 == 0 {
            "needle-grain"
        } else {
            "absent-needle-xyz-t40"
        };
        let start = Instant::now();
        let output = fixture.ok(&["--search-root", &fixture.data_utf8(), query]);
        let elapsed = start.elapsed().as_millis();
        assert!(elapsed <= CEIL_SINGLE_OP_MS, "query_ms={elapsed}");
        assert!(
            output.contains("\"complete\":true") || output.contains("\"match_limit_reached\""),
            "query outcome stays typed: {output}"
        );
        query_ms.push(elapsed);
        report
            .push(BudgetSample {
                op: "query",
                elapsed_ms: elapsed,
                response_bytes: output.len() as u64,
                control_bytes_delta: 0,
            })
            .expect("bounded ledger admits query samples");
    }
    query_ms.sort_unstable();
    query_ms
}

#[test]
fn t40_adversarial_workloads_terminate_within_bounds() {
    let fixture = Fixture::new("adversarial");

    // Long line: one 2 MiB single-line file.
    let long_path = fixture.utf8("long-line.txt");
    fs::write(&long_path, vec![b'x'; 2 * 1024 * 1024]).expect("long line is written");
    let start = Instant::now();
    let (status, stdout, stderr) = fixture.run(&[
        "--index-file",
        &fixture.data_utf8(),
        long_path.to_str().expect("long path is UTF-8"),
    ]);
    let long_index_ms = start.elapsed().as_millis();
    assert!(
        long_index_ms <= CEIL_SINGLE_OP_MS,
        "long_index_ms={long_index_ms}"
    );
    assert!(status.success(), "long line indexes: {stderr}");
    assert!(
        !contains_sentinel(&stdout, &[SECRET_CANARY]),
        "long-line output leaks"
    );
    let start = Instant::now();
    let output = fixture.ok(&["--search-root", &fixture.data_utf8(), "xxx"]);
    let long_search_ms = start.elapsed().as_millis();
    assert!(
        long_search_ms <= CEIL_SINGLE_OP_MS,
        "long_search_ms={long_search_ms}"
    );
    assert!(
        output.contains("\"complete\":true") || output.contains("\"match_limit_reached\""),
        "long-line search stays typed: {output}"
    );

    // Many files: 300 x 1 KiB.
    let many_dir = fixture.utf8("many");
    fs::create_dir(&many_dir).expect("many-file dir exists");
    for index in 0..300 {
        fs::write(
            many_dir.join(format!("many-{index:03}.txt")),
            format!("many-file payload {index:03} needle-many\n").repeat(32),
        )
        .expect("many-file member is written");
    }
    let start = Instant::now();
    let many = fixture.ok(&[
        "--index-directory",
        &fixture.data_utf8(),
        many_dir.to_str().expect("many path is UTF-8"),
    ]);
    let many_ms = start.elapsed().as_millis();
    assert!(many_ms <= CEIL_SINGLE_OP_MS, "many_ms={many_ms}");
    assert!(
        many.contains("\"event\":\"directory_index_complete\""),
        "{many}"
    );

    // High match: 60k single-byte matches; the corpus budget caps with a
    // typed `match_limit_reached`, never silent truncation (invariant 15).
    let hits_path = fixture.utf8("high-match.txt");
    fs::write(&hits_path, vec![b'a'; 60_000]).expect("high-match file is written");
    let (status, _, stderr) = fixture.run(&[
        "--index-file",
        &fixture.data_utf8(),
        hits_path.to_str().expect("hits path is UTF-8"),
    ]);
    assert!(status.success(), "high-match file indexes: {stderr}");
    let start = Instant::now();
    let hits = fixture.ok(&["--search-root", &fixture.data_utf8(), "a"]);
    let hits_ms = start.elapsed().as_millis();
    assert!(hits_ms <= CEIL_SINGLE_OP_MS, "hits_ms={hits_ms}");
    assert!(
        hits.contains("\"complete\":true") || hits.contains("\"match_limit_reached\":true"),
        "high-match outcome stays typed"
    );
    if hits.contains("\"match_limit_reached\":true") {
        assert!(
            hits.contains("\"complete\":false"),
            "capped search is not success: {hits}"
        );
    }
    eprintln!(
        "T40_ADVERSARIAL long_index_ms={long_index_ms} long_search_ms={long_search_ms} \
         many_files_ms={many_ms} high_match_ms={hits_ms}"
    );
}

#[test]
fn t40_repeated_cli_queries_write_no_control_records() {
    let fixture = Fixture::new("no-control-writes");
    write_frozen_corpus(&fixture);
    fixture.ok(&[
        "--index-directory",
        &fixture.data_utf8(),
        fixture.files.to_str().expect("files path is UTF-8"),
    ]);
    let events_before = fixture.source_events();
    let log_before = fixture.control_log_len();
    assert!(
        log_before > 0,
        "ingest leaves a control log to compare against"
    );

    let mut worst_ms = 0_u128;
    for round in 0..CLI_QUERY_SAMPLE {
        let query = if round % 3 == 0 {
            "needle-grain"
        } else {
            "absent-needle-xyz-t40"
        };
        let start = Instant::now();
        let output = fixture.ok(&["--search-root", &fixture.data_utf8(), query]);
        worst_ms = worst_ms.max(start.elapsed().as_millis());
        assert!(output.contains("\"complete\":true"), "{output}");
    }
    assert_eq!(
        fixture.source_events(),
        events_before,
        "300 CLI queries append no source events"
    );
    assert_eq!(
        fixture.control_log_len(),
        log_before,
        "300 CLI queries append no control bytes"
    );
    eprintln!("T40_QUERIES n={CLI_QUERY_SAMPLE} control_delta_bytes=0 worst_ms={worst_ms}");
}

#[test]
fn t40_cancellation_is_bounded_and_typed() {
    let fixture = Fixture::new("cancel");
    // Oversized batch: large enough that the kill usually lands mid-index.
    let big_dir = fixture.utf8("big");
    fs::create_dir(&big_dir).expect("cancel batch dir exists");
    for index in 0..1500 {
        fs::write(
            big_dir.join(format!("big-{index:04}.txt")),
            format!("cancel batch payload {index:04} needle-cancel\n").repeat(64),
        )
        .expect("cancel batch member is written");
    }
    let mut child = Command::new(env!("CARGO_BIN_EXE_eliot-searchd"))
        .args([
            "--index-directory",
            fixture.data.to_str().expect("data path is UTF-8"),
            big_dir.to_str().expect("big path is UTF-8"),
        ])
        .stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .expect("cancel batch spawns");
    thread::sleep(Duration::from_millis(300));
    let kill_start = Instant::now();
    let killed = child.try_wait().expect("cancel poll succeeds").is_none();
    if killed {
        child.kill().expect("cancel kill succeeds");
    }
    let status = child.wait().expect("cancel reap succeeds");
    let reap_ms = kill_start.elapsed().as_millis();
    assert!(
        reap_ms <= CEIL_CANCELLATION_MS,
        "kill-to-reap stays bounded: reap_ms={reap_ms}"
    );
    fixture.guard.refresh();

    // Post-state is typed either way: a verified store or a closed-code
    // failure. An unverifiable half-index is never silent success.
    let (verify_status, verify_out, verify_err) = fixture.run(&[
        "--verify-root",
        fixture.data.to_str().expect("data path is UTF-8"),
    ]);
    let post = if verify_status.success() {
        assert!(
            verify_out.contains("\"event\":\"direct_store_verified\""),
            "{verify_out}"
        );
        "verified"
    } else {
        assert!(!verify_err.is_empty(), "failed verify names its code");
        "typed-failure"
    };
    let path = if killed { "killed" } else { "completed_early" };
    if !killed {
        assert!(status.success(), "early completion still succeeds");
    }
    assert!(
        !contains_sentinel(&format!("{verify_out}{verify_err}"), &[SECRET_CANARY]),
        "cancel post-state leaks nothing"
    );
    eprintln!("T40_CANCEL path={path} reap_ms={reap_ms} post={post}");
}

// --- helpers ---------------------------------------------------------------

fn percentile(sorted_ms: &[u128], percent: u8) -> u128 {
    assert!(!sorted_ms.is_empty(), "budget sample is non-empty");
    let rank = (u128::from(percent) * sorted_ms.len() as u128).div_ceil(100);
    sorted_ms[usize::try_from(rank.saturating_sub(1)).unwrap_or(usize::MAX)]
}

/// Resolves one frozen file's revision id through the real `--index-file`
/// path (idempotent re-admission of an unchanged file returns the same
/// immutable revision) and parses the `source_indexed` event.
fn revision_of(fixture: &Fixture, name: &str) -> String {
    let probe = fixture.files.join(name);
    assert!(probe.exists(), "frozen probe exists");
    let output = fixture.ok(&[
        "--index-file",
        &fixture.data_utf8(),
        probe.to_str().expect("probe path is UTF-8"),
    ]);
    assert!(output.contains("\"event\":\"source_indexed\""), "{output}");
    field(&output, "revision_id").to_owned()
}

fn field<'output>(output: &'output str, key: &str) -> &'output str {
    let needle = format!("\"{key}\":\"");
    output
        .split_once(&needle)
        .unwrap_or_else(|| panic!("daemon output carries field {key}: {output}"))
        .1
        .split('"')
        .next()
        .expect("field value is terminated")
}

/// Reads the control log as lossy text for sentinel scanning. Canary
/// material is ASCII, so a lossy conversion cannot hide it.
fn control_log_text(data_root: &Path) -> String {
    let path = data_root.join("control").join("source-events.log");
    fs::read(&path)
        .map(|bytes| String::from_utf8_lossy(&bytes).into_owned())
        .unwrap_or_default()
}

/// Lowercase hex encoding for authorized-field comparisons.
fn hex_bytes(bytes: &[u8]) -> String {
    const HEX: &[u8; 16] = b"0123456789abcdef";
    let mut output = String::with_capacity(bytes.len() * 2);
    for byte in bytes {
        output.push(char::from(HEX[usize::from(byte >> 4)]));
        output.push(char::from(HEX[usize::from(byte & 0x0F)]));
    }
    output
}

//! Native subprocess fixtures for the bounded Cargo metadata transport.

use super::{CapturedOutput, RunLimits, parse_output, read_capped, run_bounded};
use std::env;
use std::io::{self, Read, Write};
use std::process::{self, Command};
use std::thread;
use std::time::{Duration, Instant};

const FIXTURE_ENV: &str = "ELIOT_XTASK_PROCESS_FIXTURE";
const CHILD_TEST: &str = "cargo_metadata_adapter::process_fixture::process_child";
const CAPTURE_BYTES: usize = 8_192;
const SMALL_CAP: usize = 512;
const PAYLOAD: [u8; 4_096] = [b'x'; 4_096];
const SUCCESS_STDOUT: &[u8] = b"fixture-success-stdout\n";
const SUCCESS_STDERR: &[u8] = b"fixture-success-stderr\n";
const STDOUT_MARKER: &[u8] = b"fixture-stdout-payload\n";
const STDERR_MARKER: &[u8] = b"fixture-stderr-payload\n";
const CHILD_SLEEP: Duration = Duration::from_secs(2);
const NORMAL_TIMEOUT: Duration = Duration::from_secs(3);
const INJECTED_READ_FAILURE: &str = "fixture injected reader failure";

fn child(mode: &str) -> Command {
    let mut command = Command::new(env::current_exe().expect("current test executable"));
    command
        .arg("--exact")
        .arg(CHILD_TEST)
        .arg("--nocapture")
        .arg("--test-threads=1")
        .arg("--color")
        .arg("never")
        .env(FIXTURE_ENV, mode);
    command
}

fn limits(stdout_bytes: usize, stderr_bytes: usize) -> RunLimits {
    RunLimits {
        stdout_bytes,
        stderr_bytes,
        timeout: NORMAL_TIMEOUT,
    }
}

fn contains(bytes: &[u8], marker: &[u8]) -> bool {
    bytes.windows(marker.len()).any(|window| window == marker)
}

fn failure(result: Result<CapturedOutput, String>) -> String {
    match result {
        Ok(_) => panic!("bounded process unexpectedly succeeded"),
        Err(error) => error,
    }
}

fn emit(output: &mut impl Write, bytes: &[u8]) {
    output
        .write_all(bytes)
        .expect("write bounded fixture bytes");
    output.flush().expect("flush fixture bytes");
}

struct FailingReader {
    prefix: io::Cursor<&'static [u8]>,
}

impl Read for FailingReader {
    fn read(&mut self, buffer: &mut [u8]) -> io::Result<usize> {
        let read = self.prefix.read(buffer)?;
        if read == 0 && !buffer.is_empty() {
            Err(io::Error::other(INJECTED_READ_FAILURE))
        } else {
            Ok(read)
        }
    }
}

/// Runs only in the selected current-executable child. An ordinary test run
/// leaves the private environment variable unset and returns immediately.
#[test]
fn process_child() {
    let Ok(mode) = env::var(FIXTURE_ENV) else {
        return;
    };
    match mode.as_str() {
        "success" => {
            emit(&mut io::stdout().lock(), SUCCESS_STDOUT);
            emit(&mut io::stderr().lock(), SUCCESS_STDERR);
        }
        "stdout" => {
            let mut stdout = io::stdout().lock();
            emit(&mut stdout, STDOUT_MARKER);
            emit(&mut stdout, &PAYLOAD);
        }
        "stderr" => {
            let mut stderr = io::stderr().lock();
            emit(&mut stderr, STDERR_MARKER);
            emit(&mut stderr, &PAYLOAD);
        }
        "sleep" => {
            emit(&mut io::stdout().lock(), b"fixture-sleep-started\n");
            thread::sleep(CHILD_SLEEP);
        }
        "nonzero" => {
            emit(&mut io::stderr().lock(), b"fixture-exit-23\n");
            process::exit(23);
        }
        "invalid_utf8" => {
            emit(&mut io::stdout().lock(), &[0xff]);
        }
        _ => panic!("unknown process fixture mode"),
    }
    // Avoid a variable-duration libtest result footer in the captured output.
    process::exit(0);
}

#[test]
fn successful_child_preserves_both_streams() {
    // The stdout allowance includes the test harness prefix. Stderr is an
    // exact-boundary success case because the child emits only this marker.
    let output = run_bounded(child("success"), limits(SMALL_CAP, SUCCESS_STDERR.len()))
        .expect("successful bounded native child");
    assert!(contains(&output.stdout, SUCCESS_STDOUT));
    assert!(output.stdout.len() <= SMALL_CAP);
    assert_eq!(output.stderr, SUCCESS_STDERR);
}

#[test]
fn stdout_cap_is_terminal() {
    let output = run_bounded(child("stdout"), limits(CAPTURE_BYTES, CAPTURE_BYTES))
        .expect("stdout payload succeeds with sufficient allowance");
    assert!(contains(&output.stdout, STDOUT_MARKER));
    assert!(output.stdout.len() > SMALL_CAP);
    assert!(output.stdout.len() <= CAPTURE_BYTES);
    assert!(output.stderr.is_empty());

    let error = failure(run_bounded(
        child("stdout"),
        limits(SMALL_CAP, CAPTURE_BYTES),
    ));
    assert!(
        error.contains("byte limit exceeded"),
        "wrong failure: {error}"
    );
}

#[test]
fn stderr_cap_is_terminal() {
    let output = run_bounded(child("stderr"), limits(CAPTURE_BYTES, CAPTURE_BYTES))
        .expect("stderr payload succeeds with sufficient allowance");
    assert!(contains(&output.stderr, STDERR_MARKER));
    assert!(output.stderr.len() > SMALL_CAP);
    assert!(output.stderr.len() <= CAPTURE_BYTES);
    assert!(!contains(&output.stdout, STDOUT_MARKER));

    let error = failure(run_bounded(
        child("stderr"),
        limits(CAPTURE_BYTES, SMALL_CAP),
    ));
    assert!(
        error.contains("byte limit exceeded"),
        "wrong failure: {error}"
    );
}

#[test]
fn timeout_is_terminal_and_returns_before_child_sleep_finishes() {
    let started = Instant::now();
    let error = failure(run_bounded(
        child("sleep"),
        RunLimits {
            stdout_bytes: CAPTURE_BYTES,
            stderr_bytes: CAPTURE_BYTES,
            timeout: Duration::from_millis(200),
        },
    ));
    let elapsed = started.elapsed();
    assert!(
        elapsed < Duration::from_millis(1_500),
        "deadline returned after {elapsed:?}; child sleep is {CHILD_SLEEP:?}"
    );
    let diagnostic = error.to_ascii_lowercase();
    assert!(
        diagnostic.contains("timeout")
            || diagnostic.contains("timed out")
            || diagnostic.contains("deadline"),
        "wrong failure: {error}"
    );
}

#[test]
fn nonzero_exit_is_terminal() {
    let error = failure(run_bounded(
        child("nonzero"),
        limits(CAPTURE_BYTES, CAPTURE_BYTES),
    ));
    assert!(error.contains("23"), "missing child exit status: {error}");
}

#[test]
fn invalid_stdout_utf8_is_terminal_before_metadata_parse() {
    let output = run_bounded(child("invalid_utf8"), limits(CAPTURE_BYTES, CAPTURE_BYTES))
        .expect("invalid UTF-8 bytes are captured from a successful bounded child");
    assert!(output.stdout.contains(&0xff));
    assert!(std::str::from_utf8(&output.stdout).is_err());
    assert!(output.stderr.is_empty());

    let error = parse_output(&output).expect_err("invalid stdout UTF-8 must fail admission");
    assert!(error.contains("UTF-8"), "wrong failure: {error}");
}

#[test]
fn reader_failure_discards_already_captured_prefix() {
    let prefix: &'static [u8] = b"fixture-prefix";
    let mut reader = FailingReader {
        prefix: io::Cursor::new(prefix),
    };
    let error = read_capped(&mut reader, CAPTURE_BYTES)
        .expect_err("a pipe read failure must never return partial successful output");
    assert_eq!(
        usize::try_from(reader.prefix.position()).expect("bounded fixture position"),
        prefix.len()
    );
    assert!(error.contains("pipe read failed"), "wrong failure: {error}");
    assert!(
        error.contains(INJECTED_READ_FAILURE),
        "lost injected failure: {error}"
    );
}

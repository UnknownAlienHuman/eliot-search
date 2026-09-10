//! T08 durable-owner process regressions: one live owner per data root.
//!
//! Black-box coverage through the product binary: a second live process is
//! denied while the owner lock is held (no PID/timeout stealing), clean
//! restart preserves installation/root identities and advances the monotone
//! epoch by exactly one, a killed holder is succeeded (never impersonated),
//! and a copied/relocated root is denied while the original keeps serving.
//! Owner-state assertions read only the documented `key=value` lines of
//! `.eliot-search-owner-state.v1`; the file format itself is owned and
//! unit-tested beside the composition code.

use std::fs;
use std::io::{BufRead, BufReader, Read, Write};
use std::path::{Path, PathBuf};
use std::process::{Child, Command, ExitStatus, Stdio};
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::mpsc;
use std::thread::{self, JoinHandle};
use std::time::{Duration, Instant, SystemTime, UNIX_EPOCH};

// Shared Credential Manager cleanup; each harness uses a subset of it.
#[allow(dead_code)]
mod common;

const TIMEOUT: Duration = Duration::from_secs(30);
const ALREADY_OWNED: &str = "DATA_ROOT_ALREADY_OWNED";
const GUARD_MISMATCH: &str = "OWNER_GUARD_MISMATCH";
const OWNER_STATE_A: &str = ".eliot-search-owner-state-a.v1";
const OWNER_STATE_B: &str = ".eliot-search-owner-state-b.v1";
const MAX_COPY_FILES: usize = 10_000;

static NEXT: AtomicU64 = AtomicU64::new(0);

struct Scratch {
    base: PathBuf,
    data: PathBuf,
    tree: common::RevisionKeyTreeGuard,
}

impl Scratch {
    fn new() -> Self {
        let stamp = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_nanos();
        let base = std::env::temp_dir().join(format!(
            "eliot-owner-{}-{stamp}-{}",
            std::process::id(),
            NEXT.fetch_add(1, Ordering::Relaxed),
        ));
        fs::create_dir(&base).unwrap();
        let data = base.join("data");
        fs::create_dir(&data).unwrap();
        let tree = common::RevisionKeyTreeGuard::for_tree(&base);
        Self { base, data, tree }
    }
}

impl Drop for Scratch {
    fn drop(&mut self) {
        self.tree.cleanup();
        let _ = fs::remove_dir_all(&self.base);
    }
}

/// Long-lived `--serve-data-root` holder used to prove live-lock exclusion.
struct ServeSession {
    child: Child,
    input: Option<std::process::ChildStdin>,
    output: Option<JoinHandle<String>>,
    errors: Option<JoinHandle<String>>,
    first_line: String,
}

impl ServeSession {
    fn start(root: &Path) -> Self {
        let mut child = Command::new(env!("CARGO_BIN_EXE_eliot-searchd"))
            .args(["--serve-data-root"])
            .arg(root)
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .spawn()
            .unwrap();
        let input = child.stdin.take();
        let stdout = child.stdout.take().unwrap();
        let stderr = child.stderr.take().unwrap();
        let (ready_tx, ready_rx) = mpsc::channel();
        let output = thread::spawn(move || {
            let mut reader = BufReader::new(stdout);
            let mut first = Vec::new();
            Read::take(&mut reader, 65_537)
                .read_until(b'\n', &mut first)
                .unwrap();
            let first = String::from_utf8(first).unwrap();
            let _ = ready_tx.send(first.clone());
            let mut rest = read_output(reader);
            let mut combined = first;
            combined.push_str(&rest);
            rest.clear();
            combined
        });
        let errors = thread::spawn(move || read_output(stderr));
        let mut session = Self {
            child,
            input,
            output: Some(output),
            errors: Some(errors),
            first_line: String::new(),
        };
        let first = ready_rx
            .recv_timeout(TIMEOUT)
            .expect("bounded service startup");
        session.first_line = first;
        session
    }

    fn shutdown(mut self) -> (ExitStatus, String, String) {
        let mut input = self.input.take().unwrap();
        let (close_tx, close_rx) = mpsc::channel();
        let send = thread::spawn(move || {
            let sent = input.write_all(b"shutdown\n").and_then(|()| input.flush());
            let _ = close_rx.recv_timeout(TIMEOUT);
            sent
        });
        let deadline = Instant::now() + TIMEOUT;
        let status = loop {
            if let Some(status) = self.child.try_wait().unwrap() {
                break status;
            }
            if Instant::now() >= deadline {
                let _ = self.child.kill();
                let _ = self.child.wait();
                let _ = close_tx.send(());
                let _ = send.join();
                panic!("service did not terminate within the fixture deadline");
            }
            thread::sleep(Duration::from_millis(10));
        };
        let _ = close_tx.send(());
        let _ = send.join().unwrap();
        let output = self.output.take().unwrap().join().unwrap();
        let errors = self.errors.take().unwrap().join().unwrap();
        (status, output, errors)
    }

    /// Crash without destructors: the OS releases exclusion, durable owner
    /// state stays exactly as last published.
    fn crash(mut self) {
        let _ = self.child.kill();
        let _ = self.child.wait();
        self.input.take();
        // Reap reader threads without blocking on a dead peer.
        if let Some(output) = self.output.take() {
            let _ = output.join();
        }
        if let Some(errors) = self.errors.take() {
            let _ = errors.join();
        }
    }
}

impl Drop for ServeSession {
    fn drop(&mut self) {
        let _ = self.child.kill();
        let _ = self.child.wait();
    }
}

fn read_output(reader: impl Read) -> String {
    let mut bytes = Vec::new();
    reader.take(1_048_577).read_to_end(&mut bytes).unwrap();
    assert!(bytes.len() <= 1_048_576, "fixture output limit");
    String::from_utf8(bytes).unwrap()
}

/// Runs one short-lived daemon command to completion.
fn run_once(args: &[&str]) -> (ExitStatus, String) {
    let mut command = Command::new(env!("CARGO_BIN_EXE_eliot-searchd"));
    for arg in args {
        command.arg(arg);
    }
    let output = command
        .stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .output()
        .unwrap();
    let mut combined = String::from_utf8(output.stdout).unwrap();
    combined.push_str(&String::from_utf8(output.stderr).unwrap());
    (output.status, combined)
}

/// Reads the newest durable owner-state slot: alternating slots keep the
/// previous valid generation beside the latest publication.
fn owner_state_bytes(root: &Path) -> Vec<u8> {
    let mut candidates = Vec::new();
    for name in [OWNER_STATE_A, OWNER_STATE_B] {
        if let Ok(bytes) = fs::read(root.join(name)) {
            candidates.push(bytes);
        }
    }
    assert!(!candidates.is_empty(), "owner state present");
    candidates
        .into_iter()
        .max_by_key(|bytes| generation_of(bytes))
        .unwrap()
}

fn generation_of(bytes: &[u8]) -> u64 {
    let text = String::from_utf8_lossy(bytes);
    for line in text.lines() {
        if let Some(value) = line.strip_prefix("generation=") {
            return value.parse().unwrap();
        }
    }
    panic!("owner state missing generation");
}

/// Reads one documented `key=value` line from the durable owner-state file.
fn owner_field(root: &Path, key: &str) -> String {
    let bytes = owner_state_bytes(root);
    let text = String::from_utf8(bytes).expect("owner state UTF-8");
    let prefix = format!("{key}=");
    for line in text.lines() {
        if let Some(value) = line.strip_prefix(&prefix) {
            return value.to_owned();
        }
    }
    panic!("owner state missing field {key}: {text}");
}

fn copy_tree(source: &Path, target: &Path) {
    let mut pending = vec![source.to_path_buf()];
    let mut files = 0_usize;
    fs::create_dir_all(target).unwrap();
    while let Some(current) = pending.pop() {
        for entry in fs::read_dir(&current).unwrap() {
            let entry = entry.unwrap();
            let relative = entry.path().strip_prefix(source).unwrap().to_owned();
            let destination = target.join(relative);
            let file_type = entry.file_type().unwrap();
            if file_type.is_dir() {
                fs::create_dir_all(&destination).unwrap();
                pending.push(entry.path());
            } else if file_type.is_file() {
                files += 1;
                assert!(files <= MAX_COPY_FILES, "fixture copy budget exceeded");
                fs::copy(entry.path(), &destination).unwrap();
            }
        }
    }
}

#[test]
fn second_process_is_denied_while_the_live_lock_is_held() {
    let scratch = Scratch::new();
    let holder = ServeSession::start(&scratch.data);
    assert!(
        holder.first_line.contains("data_root_ready"),
        "{}",
        holder.first_line
    );

    let (status, output) = run_once(&["--serve-data-root", scratch.data.to_str().unwrap()]);
    assert!(!status.success(), "{output}");
    assert!(output.contains(ALREADY_OWNED), "{output}");
    assert!(!output.contains("data_root_ready"), "{output}");

    // The live holder is unaffected and still shuts down cleanly.
    let (status, output, errors) = holder.shutdown();
    assert!(status.success(), "{output} {errors}");
    assert!(output.contains("\"clean\":true"), "{output}");
}

#[test]
fn elapsed_time_does_not_steal_a_live_lock() {
    let scratch = Scratch::new();
    let holder = ServeSession::start(&scratch.data);
    assert!(
        holder.first_line.contains("data_root_ready"),
        "{}",
        holder.first_line
    );

    // Sleep past any plausible lease window: liveness is the OS lock alone,
    // never an elapsed timestamp, so the second process must still be denied.
    thread::sleep(Duration::from_secs(2));
    let (status, output) = run_once(&["--serve-data-root", scratch.data.to_str().unwrap()]);
    assert!(!status.success(), "{output}");
    assert!(output.contains(ALREADY_OWNED), "{output}");

    let (status, output, errors) = holder.shutdown();
    assert!(status.success(), "{output} {errors}");
    assert!(output.contains("\"clean\":true"), "{output}");
}

#[test]
fn clean_restart_preserves_installation_and_advances_epoch_by_one() {
    let scratch = Scratch::new();
    let holder = ServeSession::start(&scratch.data);
    assert!(
        holder.first_line.contains("data_root_ready"),
        "{}",
        holder.first_line
    );
    let (status, output, errors) = holder.shutdown();
    assert!(status.success(), "{output} {errors}");

    // Clean shutdown persists a RELEASED tombstone at epoch one.
    assert_eq!(owner_field(&scratch.data, "epoch"), "1");
    assert_eq!(owner_field(&scratch.data, "lifecycle"), "RELEASED");
    let installation = owner_field(&scratch.data, "installation_id");
    let incarnation = owner_field(&scratch.data, "installation_incarnation_id");
    let root_id = owner_field(&scratch.data, "data_root_id");
    assert_eq!(installation.len(), 32);
    assert_eq!(incarnation.len(), 32);
    assert_eq!(root_id.len(), 32);

    let holder = ServeSession::start(&scratch.data);
    assert!(
        holder.first_line.contains("data_root_ready"),
        "{}",
        holder.first_line
    );
    let (status, output, errors) = holder.shutdown();
    assert!(status.success(), "{output} {errors}");

    // Guarded succession: same installation and physical root, epoch plus one.
    assert_eq!(owner_field(&scratch.data, "epoch"), "2");
    assert_eq!(owner_field(&scratch.data, "lifecycle"), "RELEASED");
    assert_eq!(owner_field(&scratch.data, "installation_id"), installation);
    assert_eq!(
        owner_field(&scratch.data, "installation_incarnation_id"),
        incarnation
    );
    assert_eq!(owner_field(&scratch.data, "data_root_id"), root_id);
}

#[test]
fn crashed_owner_is_succeeded_never_impersonated() {
    let scratch = Scratch::new();
    let holder = ServeSession::start(&scratch.data);
    assert!(
        holder.first_line.contains("data_root_ready"),
        "{}",
        holder.first_line
    );
    let first_token = owner_field(&scratch.data, "owner_token");
    assert_eq!(owner_field(&scratch.data, "epoch"), "1");
    holder.crash();

    // Durable state is exactly the last publication; no release was written.
    assert_eq!(owner_field(&scratch.data, "epoch"), "1");
    assert_eq!(owner_field(&scratch.data, "lifecycle"), "ACTIVE");
    assert_eq!(owner_field(&scratch.data, "owner_token"), first_token);

    let holder = ServeSession::start(&scratch.data);
    assert!(
        holder.first_line.contains("data_root_ready"),
        "{}",
        holder.first_line
    );
    let (status, output, errors) = holder.shutdown();
    assert!(status.success(), "{output} {errors}");

    // Successor advances the monotone epoch and mints a fresh creation
    // token instead of reusing the dead owner's identity.
    assert_eq!(owner_field(&scratch.data, "epoch"), "2");
    assert_ne!(owner_field(&scratch.data, "owner_token"), first_token);
}

#[test]
fn copied_root_is_denied_and_original_keeps_serving() {
    let scratch = Scratch::new();
    let holder = ServeSession::start(&scratch.data);
    assert!(
        holder.first_line.contains("data_root_ready"),
        "{}",
        holder.first_line
    );
    let (status, output, errors) = holder.shutdown();
    assert!(status.success(), "{output} {errors}");
    assert_eq!(owner_field(&scratch.data, "epoch"), "1");

    let copied = scratch.base.join("copy");
    copy_tree(&scratch.data, &copied);
    let copied_a = fs::read(copied.join(OWNER_STATE_A)).unwrap();
    let copied_b = fs::read(copied.join(OWNER_STATE_B)).unwrap();

    let (status, output) = run_once(&["--serve-data-root", copied.to_str().unwrap()]);
    assert!(!status.success(), "{output}");
    assert!(output.contains(GUARD_MISMATCH), "{output}");
    assert!(!output.contains("data_root_ready"), "{output}");
    // Denial precedes any durable mutation of the copied tree.
    assert_eq!(fs::read(copied.join(OWNER_STATE_A)).unwrap(), copied_a);
    assert_eq!(fs::read(copied.join(OWNER_STATE_B)).unwrap(), copied_b);

    // The original root is unaffected and serves again at the next epoch.
    let holder = ServeSession::start(&scratch.data);
    assert!(
        holder.first_line.contains("data_root_ready"),
        "{}",
        holder.first_line
    );
    let (status, output, errors) = holder.shutdown();
    assert!(status.success(), "{output} {errors}");
    assert_eq!(owner_field(&scratch.data, "epoch"), "2");
}

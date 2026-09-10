//! T05 quarantine process regressions: persistent marker blocks READY and stale reads.

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
const MARKER_REL: &str = "control/catalog-quarantine.marker";
const MARKER_PAYLOAD: &[u8] =
    b"ELIOT_SEARCH_CATALOG_QUARANTINE_V1\nreason=SERVICE_MUTATION_OUTCOME_UNKNOWN\n";
const QUARANTINE_CODE: &str = "SERVICE_CATALOG_QUARANTINED";
const MUTATION_UNKNOWN: &str = "SERVICE_MUTATION_OUTCOME_UNKNOWN";

static NEXT: AtomicU64 = AtomicU64::new(0);

struct Scratch(PathBuf, common::RevisionKeyTreeGuard);
impl Scratch {
    fn new() -> Self {
        let stamp = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_nanos();
        let path = std::env::temp_dir().join(format!(
            "eliot-quarantine-{}-{stamp}-{}",
            std::process::id(),
            NEXT.fetch_add(1, Ordering::Relaxed),
        ));
        fs::create_dir(&path).unwrap();
        let guard = common::RevisionKeyTreeGuard::for_tree(&path);
        Self(path, guard)
    }
}
impl Drop for Scratch {
    fn drop(&mut self) {
        self.1.cleanup();
        let _ = fs::remove_dir_all(&self.0);
    }
}

struct Spawned {
    child: Child,
    input: Option<std::process::ChildStdin>,
    output: Option<JoinHandle<String>>,
    errors: Option<JoinHandle<String>>,
    first_line: String,
}

impl Spawned {
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
            // Avoid unused-mut warning while keeping bounded concatenation explicit.
            rest.clear();
            combined
        });
        let errors = thread::spawn(move || read_output(stderr));
        // Guard must exist before waiting so a startup timeout kills/reaps the child.
        let mut spawned = Self {
            child,
            input,
            output: Some(output),
            errors: Some(errors),
            first_line: String::new(),
        };
        let first = ready_rx
            .recv_timeout(TIMEOUT)
            .expect("bounded service startup");
        spawned.first_line = first;
        spawned
    }

    fn exchange(mut self, bytes: Vec<u8>) -> (ExitStatus, String, String) {
        let mut input = self.input.take().unwrap();
        let (close_tx, close_rx) = mpsc::channel();
        let send = thread::spawn(move || {
            let sent = input.write_all(&bytes).and_then(|()| input.flush());
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
}

impl Drop for Spawned {
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

#[cfg(unix)]
fn path_hex(path: &Path) -> String {
    use std::os::unix::ffi::OsStrExt;
    hex(path.as_os_str().as_bytes())
}
#[cfg(windows)]
fn path_hex(path: &Path) -> String {
    use std::os::windows::ffi::OsStrExt;
    hex(&path
        .as_os_str()
        .encode_wide()
        .flat_map(u16::to_le_bytes)
        .collect::<Vec<_>>())
}
#[cfg(not(any(unix, windows)))]
fn path_hex(path: &Path) -> String {
    hex(path.to_str().unwrap().as_bytes())
}

fn hex(bytes: &[u8]) -> String {
    use std::fmt::Write as _;
    bytes.iter().fold(String::new(), |mut output, byte| {
        let _ = write!(output, "{byte:02x}");
        output
    })
}

fn query_hex(query: &str) -> String {
    hex(query.as_bytes())
}

#[test]
fn quarantine_marker_blocks_ready_and_survives_reopen() {
    let scratch = Scratch::new();
    let root = scratch.0.join("data");
    fs::create_dir_all(root.join("control")).unwrap();
    fs::write(root.join(MARKER_REL), MARKER_PAYLOAD).unwrap();
    let before = fs::read(root.join(MARKER_REL)).unwrap();

    let spawned = Spawned::start(&root);
    assert!(
        !spawned.first_line.contains("data_root_ready"),
        "quarantined root must not claim READY: {}",
        spawned.first_line
    );
    let first = spawned.first_line.clone();
    let (status, output, errors) = spawned.exchange(b"health\nshutdown\n".to_vec());
    assert!(!status.success(), "{output} {errors}");
    let combined = format!("{output}{errors}{first}");
    assert!(combined.contains(QUARANTINE_CODE), "{combined}");
    for forbidden in ["\"clean\":true", "data_root_stopped"] {
        assert!(!combined.contains(forbidden), "{combined}");
    }
    // No implicit repair: marker bytes survive the refused startup unchanged.
    assert_eq!(fs::read(root.join(MARKER_REL)).unwrap(), before);
    assert_eq!(fs::read(root.join(MARKER_REL)).unwrap(), MARKER_PAYLOAD);

    // Reopen observes the same persistent marker instead of fresh state.
    let spawned = Spawned::start(&root);
    assert!(
        !spawned.first_line.contains("data_root_ready"),
        "reopen must still refuse READY: {}",
        spawned.first_line
    );
    let (status, output, errors) = spawned.exchange(b"version\n".to_vec());
    assert!(!status.success(), "{output} {errors}");
    assert_eq!(fs::read(root.join(MARKER_REL)).unwrap(), MARKER_PAYLOAD);
}

#[test]
fn uncertain_mutation_leaves_persistent_quarantine_and_refuses_stale_reads() {
    let scratch = Scratch::new();
    let root = scratch.0.join("data");
    fs::create_dir(&root).unwrap();
    let source = scratch.0.join("source.txt");
    fs::write(&source, b"quarantine sentinel").unwrap();

    // Normal startup is READY before any uncertain effect. Keep the service
    // running while the fault is injected, mirroring the existing fail-stop
    // regression: replacing the log with a directory forces the next append
    // to fail after dispatch without proving no durable effect.
    let spawned = Spawned::start(&root);
    assert!(
        spawned.first_line.contains("data_root_ready"),
        "{}",
        spawned.first_line
    );

    // Force an uncertain catalog effect: the log path becomes a directory so
    // the next append cannot prove no durable effect.
    let log = root.join("control/source-events.log");
    let saved = fs::read(&log).unwrap();
    let stash = scratch.0.join("saved-source-events.log");
    fs::rename(&log, &stash).unwrap();
    fs::create_dir(&log).unwrap();

    let commands = format!(
        "index-file\t{}\nsearch\tsensitive\t{}\ngc\tdry-run\nhealth\nshutdown\n",
        path_hex(&source),
        query_hex("sentinel"),
    );
    let (status, output, errors) = spawned.exchange(commands.into_bytes());
    assert!(!status.success(), "{output} {errors}");
    assert!(output.contains(MUTATION_UNKNOWN), "{output} {errors}");
    for forbidden in [
        "source_list_complete",
        "\"event\":\"health\"",
        "data_root_stopped",
        "\"clean\":true",
    ] {
        assert!(!output.contains(forbidden), "{output}");
    }
    // Persistent marker is armed before the uncertain mutation and survives it.
    let marker = root.join(MARKER_REL);
    assert!(
        marker.is_file(),
        "quarantine marker must survive uncertain effects"
    );
    let marker_bytes = fs::read(&marker).unwrap();
    assert!(
        marker_bytes.len() <= 256 && marker_bytes == MARKER_PAYLOAD,
        "marker must stay bounded and exact: len={}",
        marker_bytes.len()
    );

    // Explicit log repair alone must not clear the quarantine.
    fs::remove_dir(&log).unwrap();
    fs::write(&log, &saved).unwrap();
    assert_eq!(fs::read(&marker).unwrap(), MARKER_PAYLOAD);
    let spawned = Spawned::start(&root);
    assert!(
        !spawned.first_line.contains("data_root_ready"),
        "reopen after uncertain effects must not claim READY: {}",
        spawned.first_line
    );
    let first = spawned.first_line.clone();
    let (status, output, errors) = spawned.exchange(
        format!(
            "search\tsensitive\t{}\nretire\t{}\ngc\tapply\nhealth\n",
            query_hex("sentinel"),
            "0".repeat(64),
        )
        .into_bytes(),
    );
    assert!(!status.success(), "{output} {errors}");
    let combined = format!("{output}{errors}{first}");
    assert!(combined.contains(QUARANTINE_CODE), "{combined}");
    for forbidden in [
        "\"clean\":true",
        "data_root_stopped",
        "\"event\":\"health\"",
    ] {
        assert!(!combined.contains(forbidden), "{combined}");
    }
    assert_eq!(fs::read(&marker).unwrap(), MARKER_PAYLOAD);

    // Only explicit marker removal after exact log readback restores serving.
    fs::remove_file(&marker).unwrap();
    let spawned = Spawned::start(&root);
    assert!(
        spawned.first_line.contains("data_root_ready"),
        "{}",
        spawned.first_line
    );
    let (status, output, errors) = spawned.exchange(b"health\nshutdown\n".to_vec());
    assert!(status.success(), "{output} {errors}");
    assert!(output.contains("\"clean\":true"), "{output}");
}

#[test]
fn corrupt_quarantine_marker_is_fail_closed() {
    let scratch = Scratch::new();
    let root = scratch.0.join("data");
    fs::create_dir_all(root.join("control")).unwrap();
    // Oversized/corrupt marker must not be reinterpreted as absent or repaired.
    let corrupt = vec![b'X'; 300];
    fs::write(root.join(MARKER_REL), &corrupt).unwrap();

    let spawned = Spawned::start(&root);
    assert!(
        !spawned.first_line.contains("data_root_ready"),
        "corrupt marker must not yield READY: {}",
        spawned.first_line
    );
    let first = spawned.first_line.clone();
    let (status, output, errors) = spawned.exchange(b"health\n".to_vec());
    assert!(!status.success(), "{output} {errors}");
    let combined = format!("{output}{errors}{first}");
    assert!(combined.contains(QUARANTINE_CODE), "{combined}");
    assert_eq!(fs::read(root.join(MARKER_REL)).unwrap(), corrupt);
}

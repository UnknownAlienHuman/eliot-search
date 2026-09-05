//! Regression tests through the actual primary service, on disposable roots.

use std::fs;
use std::io::{BufRead, BufReader, Read, Write};
use std::path::{Path, PathBuf};
use std::process::{Child, ChildStdin, Command, ExitStatus, Stdio};
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::mpsc;
use std::thread::{self, JoinHandle};
use std::time::{Duration, Instant, SystemTime, UNIX_EPOCH};

const TIMEOUT: Duration = Duration::from_secs(30);
static NEXT: AtomicU64 = AtomicU64::new(0);

struct Scratch(PathBuf);
impl Scratch {
    fn new() -> Self {
        let stamp = SystemTime::now().duration_since(UNIX_EPOCH).unwrap().as_nanos();
        let path = std::env::temp_dir().join(format!(
            "eliot-session-{}-{stamp}-{}",
            std::process::id(),
            NEXT.fetch_add(1, Ordering::Relaxed),
        ));
        fs::create_dir(&path).unwrap();
        Self(path)
    }
}
impl Drop for Scratch {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.0);
    }
}

struct Service {
    child: Child,
    input: Option<ChildStdin>,
    output: Option<JoinHandle<String>>,
    errors: Option<JoinHandle<String>>,
}
impl Service {
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
            Read::take(&mut reader, 65_537).read_until(b'\n', &mut first).unwrap();
            let first = String::from_utf8(first).unwrap();
            let _ = ready_tx.send(first);
            read_output(reader)
        });
        let errors = thread::spawn(move || read_output(stderr));
        let service = Self { child, input, output: Some(output), errors: Some(errors) };
        // Construct the guard before waiting: a startup timeout kills/reaps it.
        let ready = ready_rx.recv_timeout(TIMEOUT).expect("bounded service startup");
        assert!(ready.contains("\"event\":\"data_root_ready\""), "{ready}");
        service
    }

    fn exchange(mut self, bytes: Vec<u8>, keep_input_open: bool) -> (ExitStatus, String, String) {
        let mut input = self.input.take().unwrap();
        let (close_tx, close_rx) = mpsc::channel();
        let send = thread::spawn(move || {
            let sent = input.write_all(&bytes).and_then(|()| input.flush());
            if keep_input_open && sent.is_ok() {
                let _ = close_rx.recv_timeout(TIMEOUT);
            }
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
impl Drop for Service {
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
    hex(&path.as_os_str().encode_wide().flat_map(u16::to_le_bytes).collect::<Vec<_>>())
}
#[cfg(not(any(unix, windows)))]
fn path_hex(path: &Path) -> String {
    hex(path.to_str().unwrap().as_bytes())
}
fn hex(bytes: &[u8]) -> String {
    bytes.iter().map(|byte| format!("{byte:02x}")).collect()
}

#[test]
fn invalid_parameters_before_dispatch_keep_service_usable() {
    let scratch = Scratch::new();
    let (status, output, errors) = Service::start(&scratch.0).exchange(
        b"index-file\tzz\nversion\nshutdown\n".to_vec(), false,
    );
    assert!(status.success(), "{output} {errors}");
    assert!(output.contains("SERVICE_HEX_INVALID"));
    assert!(output.contains("\"event\":\"version\""));
    assert!(output.contains("\"clean\":true"));
}

#[test]
fn failed_catalog_mutation_stops_without_serving_queued_reads() {
    let scratch = Scratch::new();
    let root = scratch.0.join("data");
    fs::create_dir(&root).unwrap();
    let source = scratch.0.join("source.txt");
    fs::write(&source, b"source sentinel").unwrap();
    let service = Service::start(&root);
    let log = root.join("control/source-events.log");
    let saved_log = scratch.0.join("saved-source-events.log");
    let original = fs::read(&log).unwrap();
    fs::rename(&log, &saved_log).unwrap();
    fs::create_dir(&log).unwrap();
    let commands = format!("index-file\t{}\nlist-sources\nhealth\nshutdown\n", path_hex(&source));
    let (status, output, errors) = service.exchange(commands.into_bytes(), false);
    assert!(!status.success(), "{output}");
    assert!(output.contains("SERVICE_MUTATION_OUTCOME_UNKNOWN"), "{output} {errors}");
    for forbidden in ["source_list_complete", "\"event\":\"health\"", "data_root_stopped", "\"clean\":true"] {
        assert!(!output.contains(forbidden), "{output}");
    }
    assert!(log.is_dir());
    assert_eq!(fs::read(&saved_log).unwrap(), original);
    // Explicit fixture repair, not a repair path in the runtime.
    fs::remove_dir(&log).unwrap();
    fs::rename(saved_log, &log).unwrap();
    let (status, output, errors) = Service::start(&root).exchange(b"health\nshutdown\n".to_vec(), false);
    assert!(status.success(), "{output} {errors}");
    assert!(output.contains("\"registered_sources\":0"));
}

#[test]
fn oversized_unterminated_frame_exits_while_client_still_holds_stdin() {
    let scratch = Scratch::new();
    let (status, output, errors) = Service::start(&scratch.0)
        .exchange(vec![b'x'; 256 * 1024 + 2], true);
    assert!(!status.success(), "{output}");
    assert!(output.contains("SERVICE_COMMAND_TOO_LARGE"), "{output} {errors}");
    assert!(!output.contains("data_root_stopped"));
}

#[test]
fn invalid_utf8_cannot_be_followed_by_a_valid_queued_command() {
    let scratch = Scratch::new();
    let (status, output, errors) = Service::start(&scratch.0)
        .exchange(b"\xff\nversion\nshutdown\n".to_vec(), false);
    assert!(!status.success(), "{output}");
    assert!(output.contains("SERVICE_COMMAND_NOT_UTF8"), "{output} {errors}");
    assert!(!output.contains("\"event\":\"version\""));
    assert!(!output.contains("\"clean\":true"));
}

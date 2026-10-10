//! Native ordinary-owner DRAINING crash proof over actual primary daemon modules.

#![cfg(all(test, windows))]

use std::collections::BTreeMap;
use std::ffi::OsString;
use std::fs;
use std::io::{self, BufRead, BufReader, Read, Write};
use std::path::{Path, PathBuf};
use std::process::{Child, Command, Stdio};
use std::sync::mpsc;
use std::thread::{self, JoinHandle};
use std::time::{Duration, SystemTime, UNIX_EPOCH};

use crate::development::DataRootGuard;
use crate::owner_composition::{DataRootRequest, InitializationRequest, initialize_new_request};

// This variable carries only a test-child locator. Only this test reads it;
// production initialization and lifecycle code have no fault-injection branch.
const CHILD_ROOT: &str = "ELIOT_SEARCH_TEST_266_DRAINING_ROOT";
const INITIALIZATION_ID: &str = "26600000000000000000000000000004";
const READY: &[u8] = b"266_DRAINING_DURABLE\n";
const WAIT: Duration = Duration::from_secs(30);

pub struct Scratch(pub(crate) PathBuf);

impl Scratch {
    pub(crate) fn new() -> Self {
        let stamp = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_nanos();
        let root = std::env::temp_dir()
            .join(format!("eliot-draining-266-{}-{stamp}", std::process::id()));
        fs::create_dir(&root).unwrap();
        Self(fs::canonicalize(root).unwrap())
    }
}

impl Drop for Scratch {
    fn drop(&mut self) {
        let temporary = fs::canonicalize(std::env::temp_dir()).unwrap();
        assert_eq!(self.0.parent(), Some(temporary.as_path()));
        // Cleanup concerns only this disposable native fixture, after its child
        // is reaped. It never invokes recovery or publishes an owner release.
        if let Ok(text) = fs::read_to_string(self.0.join("control/namespace.id")) {
            let namespace = crate::sha256::decode_digest(text.trim_end()).unwrap();
            search_os_secrets_windows::delete_legacy_revision_root_secret_for_test(&namespace)
                .unwrap();
        }
        fs::remove_dir_all(&self.0).unwrap();
    }
}

fn request(root: &Path, flag: &str, extra: Option<&str>) -> DataRootRequest {
    let mut arguments = vec![OsString::from(flag), root.as_os_str().to_owned()];
    if let Some(extra) = extra {
        arguments.push(OsString::from(extra));
    }
    DataRootRequest::from_cli(&arguments).unwrap()
}

fn drain_in_child(root: &Path) -> ! {
    let initialization = InitializationRequest::parse(INITIALIZATION_ID).unwrap();
    let initialize = request(root, "--initialize-data-root", Some(INITIALIZATION_ID));
    let receipt = initialize_new_request(root, &initialization, &initialize).unwrap();
    assert!(!receipt.replayed);
    drop(initialize);

    let admission = request(root, "--serve-data-root", None);
    let mut guard = DataRootGuard::open_existing_request(root, &admission).unwrap();
    assert_eq!(guard.epoch(), 2, "fixture must exercise ordinary ownership");
    guard
        .begin_drain(search_runtime_owner::DrainReason::Shutdown)
        .unwrap();

    // begin_drain returns only after production slot publication and exact
    // readback. A new line separates the acknowledgement from libtest output.
    let mut output = io::stdout().lock();
    output.write_all(b"\n").unwrap();
    output.write_all(READY).unwrap();
    output.flush().unwrap();
    drop(output);

    // The parent retains this pipe and kills the child after acknowledgement.
    // Keep the real guard alive throughout the wait; never release_cleanly.
    let mut unexpected = [0_u8; 1];
    let result = io::stdin().read(&mut unexpected);
    let retained_epoch = guard.epoch();
    panic!("DRAINING child resumed without termination: {result:?}, epoch={retained_epoch}");
}

fn exact_test_name() -> &'static str {
    concat!(
        module_path!(),
        "::ordinary_draining_crash_refuses_existing_admission"
    )
    .split_once("::")
    .unwrap()
    .1
}

fn await_acknowledgement(
    output: impl Read,
    sender: &mpsc::Sender<Result<(), String>>,
    ready: &[u8],
) {
    let mut reader = BufReader::new(output);
    let mut total = 0_usize;
    loop {
        let mut line = Vec::new();
        let read = reader.by_ref().take(4097).read_until(b'\n', &mut line);
        match read {
            Ok(0) => {
                let _ = sender.send(Err("child exited before durable drain".to_owned()));
                return;
            }
            Err(error) => {
                let _ = sender.send(Err(error.to_string()));
                return;
            }
            Ok(_) => {}
        }
        total += line.len();
        if line.len() > 4096 || total > 32 * 1024 {
            let _ = sender.send(Err(
                "child acknowledgement exceeded fixture bound".to_owned()
            ));
            return;
        }
        if line == ready {
            let _ = sender.send(Ok(()));
            return;
        }
    }
}

pub struct NativeCrashChild {
    process: Child,
    reader: Option<JoinHandle<()>>,
}

impl NativeCrashChild {
    pub(crate) fn start(root: &Path, locator: &str, test_name: &str, ready: &'static [u8]) -> Self {
        let mut process = Command::new(std::env::current_exe().unwrap())
            .args(["--exact", test_name, "--nocapture", "--test-threads=1"])
            .env(locator, root.as_os_str())
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::inherit())
            .spawn()
            .unwrap();
        let stdout = process.stdout.take().unwrap();
        let (sender, receiver) = mpsc::channel();
        let reader = thread::spawn(move || await_acknowledgement(stdout, &sender, ready));
        let child = Self {
            process,
            reader: Some(reader),
        };
        receiver
            .recv_timeout(WAIT)
            .expect("bounded durable-drain acknowledgement")
            .unwrap();
        child
    }

    pub(crate) fn crash(&mut self) {
        assert!(self.process.try_wait().unwrap().is_none());
        self.process.kill().unwrap();
        assert!(!self.process.wait().unwrap().success());
        self.reader.take().unwrap().join().unwrap();
    }
}

impl Drop for NativeCrashChild {
    fn drop(&mut self) {
        // Reap before scratch cleanup even if acknowledgement or an assertion
        // fails. Killing drops no Rust guard and cannot certify RELEASED.
        let _ = self.process.kill();
        let _ = self.process.wait();
        if let Some(reader) = self.reader.take() {
            let _ = reader.join();
        }
    }
}

pub type TreeSnapshot = BTreeMap<PathBuf, (bool, Vec<u8>, SystemTime)>;

pub fn snapshot(root: &Path) -> TreeSnapshot {
    fn collect(root: &Path, path: &Path, result: &mut TreeSnapshot) {
        let metadata = fs::symlink_metadata(path).unwrap();
        assert!(!metadata.file_type().is_symlink());
        assert!(metadata.is_dir() || metadata.is_file());
        assert!(result.len() < 256, "unexpected fixture tree size");
        let bytes = if metadata.is_file() {
            assert!(metadata.len() < 16 * 1024);
            fs::read(path).unwrap()
        } else {
            Vec::new()
        };
        result.insert(
            path.strip_prefix(root).unwrap().to_owned(),
            (metadata.is_dir(), bytes, metadata.modified().unwrap()),
        );
        if metadata.is_dir() {
            for entry in fs::read_dir(path).unwrap() {
                collect(root, &entry.unwrap().path(), result);
            }
        }
    }
    let mut result = BTreeMap::new();
    collect(root, root, &mut result);
    result
}

#[test]
fn ordinary_draining_crash_refuses_existing_admission() {
    if let Some(root) = std::env::var_os(CHILD_ROOT) {
        drain_in_child(Path::new(&root));
    }

    let scratch = Scratch::new();
    let root = &scratch.0;
    let mut child = NativeCrashChild::start(root, CHILD_ROOT, exact_test_name(), READY);
    child.crash();
    drop(child);

    // Read-only assertions over production-written field lines, not a second
    // record codec, digest implementation, or manufactured recovery input.
    assert!(
        [
            ".eliot-search-owner-state-a.v1",
            ".eliot-search-owner-state-b.v1",
        ]
        .into_iter()
        .any(|name| {
            let text = fs::read_to_string(root.join(name)).unwrap();
            text.lines().any(|line| line == "lifecycle=DRAINING")
                && text.lines().any(|line| line == "epoch=2")
        }),
        "actual ordinary DRAINING record absent after native termination"
    );
    let retained = snapshot(root);

    let mutation = request(root, "--serve-data-root", None);
    match DataRootGuard::open_existing_request(root, &mutation) {
        Err(error) => assert_eq!(error, "OWNER_RECOVERY_QUARANTINED"),
        Ok(_) => panic!("abandoned DRAINING owner admitted ordinary mutation"),
    }
    assert_eq!(
        snapshot(root),
        retained,
        "mutation admission changed crash state"
    );

    let inspection = request(root, "--health-data-root", None);
    let result: Result<(), String> =
        DataRootGuard::with_inspection_request(root, &inspection, |_| {
            panic!("abandoned DRAINING owner admitted inspection")
        });
    assert_eq!(result.unwrap_err(), "OWNER_RECOVERY_QUARANTINED");
    assert_eq!(snapshot(root), retained, "inspection changed crash state");
}

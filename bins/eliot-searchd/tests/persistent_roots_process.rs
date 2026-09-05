//! Process-level coverage for the actual primary daemon, not a scanner model.
//!
//! Each invocation starts a fresh process and must restore the same data root.

use std::ffi::{OsStr, OsString};
use std::fs;
use std::path::{Path, PathBuf};
use std::process::{Command, Output, Stdio};
use std::sync::atomic::{AtomicU64, Ordering};
use std::time::{SystemTime, UNIX_EPOCH};

struct Sandbox {
    base: PathBuf,
    data: PathBuf,
    source: PathBuf,
}

impl Sandbox {
    fn new() -> Self {
        static NEXT: AtomicU64 = AtomicU64::new(0);
        let stamp = SystemTime::now().duration_since(UNIX_EPOCH).unwrap().as_nanos();
        let base = std::env::temp_dir().join(format!(
            "eliot-roots-process-{}-{stamp}-{}",
            std::process::id(),
            NEXT.fetch_add(1, Ordering::Relaxed),
        ));
        fs::create_dir(&base).unwrap();
        let base = fs::canonicalize(base).unwrap();
        let data = base.join("data");
        let source = base.join("source");
        fs::create_dir(&data).unwrap();
        fs::create_dir(&source).unwrap();
        Self { base, data, source }
    }

    fn invoke(&self, command: &str, tail: &[&OsStr]) -> Output {
        Command::new(env!("CARGO_BIN_EXE_eliot-searchd"))
            .arg(command)
            .arg(&self.data)
            .args(tail)
            .stdin(Stdio::null())
            .output()
            .expect("spawn primary daemon")
    }

    fn success(&self, command: &str, tail: &[&OsStr]) -> String {
        let output = self.invoke(command, tail);
        assert!(
            output.status.success(),
            "{command} failed: stdout={} stderr={}",
            String::from_utf8_lossy(&output.stdout),
            String::from_utf8_lossy(&output.stderr),
        );
        String::from_utf8(output.stdout).expect("UTF-8 protocol output")
    }

    fn register(&self) {
        let output = self.success("--register-source-root", &[self.source.as_os_str()]);
        assert!(output.contains("\"persisted\":true"));
        assert!(output.contains("\"access_granted\":false"));
    }

    fn sync(&self) -> String {
        self.success("--sync-source-roots", &[])
    }
}

impl Drop for Sandbox {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.base);
    }
}

fn digest_field(output: &str, key: &str) -> String {
    let prefix = format!("\"{key}\":\"");
    let value = output.split_once(&prefix).expect("digest field present").1
        .split_once('"').expect("closed digest field").0;
    assert_eq!(value.len(), 64);
    assert!(value.bytes().all(|byte| byte.is_ascii_hexdigit()));
    value.to_owned()
}

fn bytes_hex(bytes: &[u8]) -> String {
    const HEX: &[u8; 16] = b"0123456789abcdef";
    let mut encoded = String::with_capacity(bytes.len() * 2);
    for byte in bytes {
        encoded.push(char::from(HEX[usize::from(byte >> 4)]));
        encoded.push(char::from(HEX[usize::from(byte & 15)]));
    }
    encoded
}

#[test]
fn registration_survives_process_restart_without_implicit_indexing() {
    let sandbox = Sandbox::new();
    sandbox.register();
    assert!(sandbox.data.join("control/source-roots.v1").is_file());
    assert!(!sandbox.data.join("revisions").exists());
    let reopened = sandbox.success("--source-roots", &[]);
    assert!(reopened.contains("\"configured\":1"));
    assert!(reopened.contains("\"available\":1"));
    assert!(reopened.contains("\"current_workspace_proven\":false"));
    sandbox.success("--unregister-source-root", &[sandbox.source.as_os_str()]);
    let reopened = sandbox.success("--source-roots", &[]);
    assert!(reopened.contains("\"configured\":0"));
}

#[test]
fn explicit_sync_uses_persisted_roots_and_retains_exact_historical_bytes() {
    let sandbox = Sandbox::new();
    let original = "needle\r\nёж\n".as_bytes();
    fs::write(sandbox.source.join("note.txt"), original).unwrap();
    sandbox.register();
    let synced = sandbox.sync();
    assert!(synced.contains("\"indexed_sources\":1"));
    assert!(synced.contains("\"current_workspace_proven\":false"));
    assert!(synced.contains("\"qdrant_available\":false"));
    let search = sandbox.success("--search-root", &[OsStr::new("needle")]);
    assert!(search.contains("\"matches\":1"));
    let sources = sandbox.success("--list-sources", &[]);
    let revision = OsString::from(digest_field(&sources, "revision_id"));
    let length = OsString::from(original.len().to_string());
    fs::write(sandbox.source.join("note.txt"), b"replacement\n").unwrap();
    sandbox.sync();
    let search = sandbox.success("--search-root", &[OsStr::new("needle")]);
    assert!(search.contains("\"matches\":0"));
    let old = sandbox.success(
        "--read-revision",
        &[revision.as_os_str(), OsStr::new("0"), length.as_os_str()],
    );
    assert!(old.contains(&bytes_hex(original)));
}

#[test]
fn unavailable_root_is_not_an_empty_inventory_and_does_not_retire_sources() {
    let sandbox = Sandbox::new();
    fs::write(sandbox.source.join("note.txt"), b"retained needle\n").unwrap();
    sandbox.register();
    sandbox.sync();
    let log_path = sandbox.data.join("control/source-events.log");
    let log_before = fs::read(&log_path).unwrap();
    fs::remove_dir_all(&sandbox.source).unwrap();
    let failed = sandbox.invoke("--sync-source-roots", &[]);
    assert!(!failed.status.success());
    assert!(String::from_utf8_lossy(&failed.stderr).contains("SOURCE_ROOTS_UNAVAILABLE"));
    assert_eq!(fs::read(&log_path).unwrap(), log_before);
    let roots = sandbox.success("--source-roots", &[]);
    assert!(roots.contains("\"configured\":1"));
    assert!(roots.contains("\"unavailable\":1"));
    let retained = sandbox.success("--search-root", &[OsStr::new("needle")]);
    assert!(retained.contains("\"matches\":1"));
}

#[test]
fn corrupt_root_registration_blocks_open_instead_of_resetting_to_empty() {
    let sandbox = Sandbox::new();
    sandbox.register();
    let config = sandbox.data.join("control/source-roots.v1");
    fs::write(&config, b"broken-registration").unwrap();
    let failed = sandbox.invoke("--source-roots", &[]);
    assert!(!failed.status.success());
    assert!(String::from_utf8_lossy(&failed.stderr).contains("SOURCE_ROOT_CATALOG_CORRUPT"));
    assert!(!String::from_utf8_lossy(&failed.stdout).contains("source_roots_complete"));
    assert_eq!(fs::read(config).unwrap(), b"broken-registration");
}

#[test]
fn public_help_includes_persistent_registration_commands() {
    let output = Command::new(env!("CARGO_BIN_EXE_eliot-searchd"))
        .arg("--help")
        .stdin(Stdio::null())
        .output()
        .unwrap();
    assert!(output.status.success());
    let help = String::from_utf8(output.stdout).unwrap();
    for command in ["--source-roots", "--register-source-root", "--unregister-source-root", "--sync-source-roots"] {
        assert!(help.contains(command), "missing help: {command}");
    }
    // Keep this test coupled to the primary binary named by Cargo, not a guessed
    // path under target/ or an alternate snapshot/standalone sealed executable.
    assert!(Path::new(env!("CARGO_BIN_EXE_eliot-searchd")).is_file());
}

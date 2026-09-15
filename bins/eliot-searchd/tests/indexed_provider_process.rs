//! Real daemon + real CLI regression for the indexed-query qualification gate.

#[allow(dead_code)]
mod common;

use std::fs;
use std::io::{BufRead, BufReader};
use std::net::{Ipv4Addr, TcpListener};
use std::path::{Path, PathBuf};
use std::process::{Child, Command, Stdio};
use std::sync::atomic::{AtomicU64, Ordering};
use std::time::{SystemTime, UNIX_EPOCH};

static NEXT: AtomicU64 = AtomicU64::new(0);

struct Fixture {
    root: PathBuf,
    token_file: PathBuf,
    credential_guard: common::RevisionKeyTreeGuard,
}

impl Fixture {
    fn new() -> Self {
        let stamp = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .expect("clock after epoch")
            .as_nanos();
        let root = std::env::temp_dir().join(format!(
            "eliot-indexed-cli-{}-{stamp}-{}",
            std::process::id(),
            NEXT.fetch_add(1, Ordering::Relaxed)
        ));
        fs::create_dir_all(root.join("data")).expect("create data root");
        let token_file = root.join("auth.token");
        fs::write(&token_file, [0x41_u8; 48]).expect("write token");
        let credential_guard = common::RevisionKeyTreeGuard::for_tree(&root);
        Self {
            root,
            token_file,
            credential_guard,
        }
    }
}

impl Drop for Fixture {
    fn drop(&mut self) {
        self.credential_guard.cleanup();
        let _ = fs::remove_dir_all(&self.root);
    }
}

struct Daemon {
    child: Child,
    _stdout: BufReader<std::process::ChildStdout>,
    port: u16,
}

impl Daemon {
    fn start(fixture: &Fixture) -> Self {
        let port = TcpListener::bind((Ipv4Addr::LOCALHOST, 0))
            .expect("reserve port")
            .local_addr()
            .expect("local address")
            .port();
        let mut child = Command::new(env!("CARGO_BIN_EXE_eliot-searchd"))
            .arg("--serve-loopback-data-root")
            .arg(fixture.root.join("data"))
            .arg(port.to_string())
            .arg(&fixture.token_file)
            .stdin(Stdio::null())
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .spawn()
            .expect("spawn daemon");
        let stdout = child.stdout.take().expect("daemon stdout");
        let mut reader = BufReader::new(stdout);
        let mut direct = String::new();
        reader.read_line(&mut direct).expect("direct ready line");
        assert!(direct.contains("direct_child_ready"), "{direct}");
        let mut loopback = String::new();
        reader.read_line(&mut loopback).expect("loopback ready line");
        assert!(loopback.contains("loopback_ready"), "{loopback}");
        Self {
            child,
            _stdout: reader,
            port,
        }
    }
}

impl Drop for Daemon {
    fn drop(&mut self) {
        let _ = self.child.kill();
        let _ = self.child.wait();
    }
}

fn cli_binary() -> PathBuf {
    Path::new(env!("CARGO_BIN_EXE_eliot-searchd")).with_file_name(if cfg!(windows) {
        "eliot-search.exe"
    } else {
        "eliot-search"
    })
}

#[test]
fn indexed_query_is_typed_unavailable_without_qualification() {
    let fixture = Fixture::new();
    let daemon = Daemon::start(&fixture);
    let cli_root = fixture.root.join("cli");
    fs::create_dir_all(cli_root.join("runtime")).expect("create runtime dir");
    fs::write(
        cli_root.join("runtime").join("endpoint.v1"),
        format!(
            "ELIOT_SEARCH_ENDPOINT_V1\naddress=127.0.0.1:{}\n",
            daemon.port
        ),
    )
    .expect("write endpoint");

    let output = Command::new(cli_binary())
        .args(["indexed", "needle", "--data-root"])
        .arg(&cli_root)
        .arg("--token-file")
        .arg(&fixture.token_file)
        .stdin(Stdio::null())
        .output()
        .expect("run CLI");
    let stdout = String::from_utf8_lossy(&output.stdout);
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert_eq!(output.status.code(), Some(2), "stdout={stdout}\nstderr={stderr}");
    assert!(
        stdout.contains("PROVIDER_INDEXED_QUERY_UNAVAILABLE"),
        "stdout={stdout}\nstderr={stderr}"
    );
    assert!(
        stdout.contains("INDEXED_NOT_ACCEPTED"),
        "stdout={stdout}\nstderr={stderr}"
    );
    assert!(
        !stdout.contains("\"status\":\"ok\""),
        "unavailable indexed query must not be success: {stdout}"
    );
    assert!(
        stderr.contains("PROVIDER_INDEXED_QUERY_UNAVAILABLE"),
        "stdout={stdout}\nstderr={stderr}"
    );
}

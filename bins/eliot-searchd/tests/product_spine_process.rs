//! T30 product-spine process test: supported CLI -> authenticated daemon loopback
//! (DIRECT/CAS/redb half) joined with the exact qualified native Qdrant
//! 1.19.0 server (indexed half) into one executed proof.
//!
//! Every DIRECT assertion drives the real `eliot-searchd` binary on a fresh
//! disposable data root. Every indexed assertion drives the real
//! `C:\Tools\Qdrant\1.19.0\qdrant.exe` on disposable storage with
//! OS-assigned loopback ports over blocking loopback HTTP/1.1 built on
//! `std::net::TcpStream` only: no new dependencies, no async runtime, no
//! mock. A missing Qdrant executable fails the test closed (hard error),
//! never a silent skip. Each live fixture kills its server and removes its
//! storage on drop, on both the pass and the panic paths.
//!
//! The join between the halves is explicit: daemon-issued `revision_id` /
//! `source_id` hex travels inside Qdrant point payloads, and the readback
//! assertions require the exact same hex back.
//!
//! Explicit gaps (not claimed here, not invented):
//! - the daemon currently exposes no single CLI command that serves an
//!   indexed Qdrant query; the provider `query` op stays gated
//!   (`PROVIDER_QUERY_UNAVAILABLE` / `SEARCH_NOT_ACCEPTED`). The spine is
//!   therefore proven as two joined halves through one process, not as one
//!   CLI verb end to end.
//! - the qualified gRPC transport (`qdrant-client` 1.19.0, `RealDataPlane`)
//!   is owned by `search-qdrant-bridge` and cannot be linked from this
//!   target without a manifest change (integration-owner owned). Parity of
//!   that transport stays covered by
//!   `search-qdrant-bridge --test real_dataplane`. This file proves the same
//!   exact server binary over its loopback REST surface with identical
//!   strictness (explicit IDs, `wait=true`, strong ordering, exact
//!   readback, exact filtered counts).

#[allow(dead_code)]
mod common;

use std::fmt::Write as _;
use std::fs;
use std::io::{Read, Write};
use std::net::{Ipv4Addr, SocketAddr, TcpListener, TcpStream};
use std::path::{Path, PathBuf};
use std::process::{Child, Command, ExitStatus, Stdio};
use std::sync::atomic::{AtomicU64, Ordering};
use std::thread;
use std::time::{Duration, Instant, SystemTime, UNIX_EPOCH};

// ---------------------------------------------------------------------------
// Acceptance ceilings. Generous loopback bounds fixed before measurement;
// any breach fails closed and is reported with the measured value.
// ---------------------------------------------------------------------------

/// Single daemon invocation must finish well inside the harness deadline.
const CEIL_DAEMON_OP_MS: u128 = 15_000;
/// Qdrant collection setup plus one bounded upsert batch.
const CEIL_QDRANT_WRITE_MS: u128 = 20_000;
/// One filtered query plus exact readback round trip.
const CEIL_QDRANT_READ_MS: u128 = 10_000;
/// Whole joint perf pass, including one disposable server spawn.
const CEIL_JOINT_PASS_MS: u128 = 120_000;

const QDRANT_EXE: &str = r"C:\Tools\Qdrant\1.19.0\qdrant.exe";
const QDRANT_VERSION_EVIDENCE: &str = "1.19.0";
const SPINE_COLLECTION: &str = "t30_spine";
const SPINE_VECTOR: &str = "lex_spine";
/// Offline migration-plan target namespace (test-only, same family as the
/// T10 suite; disposable output dirs make it collision-free).
const MIGRATION_TARGET: &str = "123e4567-e89b-12d3-a456-426614174000";

static NEXT: AtomicU64 = AtomicU64::new(0);

fn unique_tag(prefix: &str) -> PathBuf {
    let nanos = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map_or(0, |elapsed| elapsed.as_nanos());
    std::env::temp_dir().join(format!(
        "{prefix}-{}-{nanos}-{}",
        std::process::id(),
        NEXT.fetch_add(1, Ordering::Relaxed)
    ))
}

// ---------------------------------------------------------------------------
// Daemon half: real primary binary on a disposable data root.
// ---------------------------------------------------------------------------

struct DaemonFixture {
    base: PathBuf,
    data: PathBuf,
    guard: common::RevisionKeyGuard,
}

impl DaemonFixture {
    fn new() -> Self {
        let base = unique_tag("eliot-t30-spine");
        let data = base.join("data");
        fs::create_dir_all(&data).expect("spine data root is created");
        let guard = common::RevisionKeyGuard::for_data_root(&data);
        Self { base, data, guard }
    }

    fn run(args: &[&str]) -> (ExitStatus, String, String) {
        let mut child = Command::new(env!("CARGO_BIN_EXE_eliot-searchd"))
            .args(args)
            .stdin(Stdio::null())
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .spawn()
            .expect("primary daemon binary runs");
        let stdout = child.stdout.take().expect("daemon stdout is piped");
        let stderr = child.stderr.take().expect("daemon stderr is piped");
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
        (
            status,
            out.join().expect("stdout drain"),
            err.join().expect("stderr drain"),
        )
    }

    fn ok(args: &[&str]) -> String {
        let (status, stdout, stderr) = Self::run(args);
        assert!(
            status.success(),
            "status={status} stdout={stdout} stderr={stderr}"
        );
        stdout
    }

    fn index_named(&self, name: &str, bytes: &[u8]) -> String {
        let path = self.base.join(name);
        fs::write(&path, bytes).expect("spine source is written");
        let output = Self::ok(&[
            "--index-file",
            self.data.to_str().expect("data path is UTF-8"),
            path.to_str().expect("source path is UTF-8"),
        ]);
        self.guard.refresh();
        output
    }

    fn search(&self, query: &str) -> String {
        Self::ok(&[
            "--search-root",
            self.data.to_str().expect("data path is UTF-8"),
            query,
        ])
    }

    fn read_revision(&self, revision: &str, start: usize, end: usize) -> String {
        Self::ok(&[
            "--read-revision",
            self.data.to_str().expect("data path is UTF-8"),
            revision,
            &start.to_string(),
            &end.to_string(),
        ])
    }
}

impl Drop for DaemonFixture {
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

fn match_lines(output: &str) -> Vec<&str> {
    output
        .lines()
        .filter(|line| line.contains("\"event\":\"match\""))
        .collect()
}

fn json_num(output: &str, key: &str) -> u64 {
    let needle = format!("\"{key}\":");
    let rest = output
        .split_once(&needle)
        .unwrap_or_else(|| panic!("numeric field {key} present: {output}"))
        .1;
    let digits: String = rest.chars().take_while(char::is_ascii_digit).collect();
    assert!(
        !digits.is_empty(),
        "numeric field {key} has digits: {output}"
    );
    digits.parse().expect("numeric field parses")
}

fn hex_of(bytes: &[u8]) -> String {
    let mut out = String::with_capacity(bytes.len() * 2);
    for byte in bytes {
        out.push(char::from(b"0123456789abcdef"[usize::from(byte >> 4)]));
        out.push(char::from(b"0123456789abcdef"[usize::from(byte & 0x0F)]));
    }
    out
}

fn copy_dir_recursive(source: &Path, target: &Path) {
    fs::create_dir_all(target).expect("migration target is created");
    for entry in fs::read_dir(source).expect("migration source lists") {
        let entry = entry.expect("migration entry reads");
        let from = entry.path();
        let to = target.join(entry.file_name());
        let kind = entry.file_type().expect("migration entry type reads");
        if kind.is_dir() {
            copy_dir_recursive(&from, &to);
        } else if kind.is_file() {
            fs::copy(&from, &to).expect("migration file copies");
        }
    }
}

/// Temp dir removed on drop, including panic unwinds: every auxiliary tree
/// outside the owning fixture goes through this guard.
struct ScopedTempDir {
    path: PathBuf,
}

impl ScopedTempDir {
    fn path(&self) -> &Path {
        &self.path
    }
}

impl Drop for ScopedTempDir {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.path);
    }
}

// ---------------------------------------------------------------------------
// Indexed half: exact native server on disposable storage, std-only HTTP.
// ---------------------------------------------------------------------------

struct LiveQdrant {
    child: Option<Child>,
    dir: PathBuf,
    http_port: u16,
}

impl LiveQdrant {
    fn spawn() -> Self {
        assert!(
            Path::new(QDRANT_EXE).is_file(),
            "qualified native server is absent at {QDRANT_EXE}: refusing mock-as-live"
        );
        let http_port = free_loopback_port();
        let grpc_port = free_loopback_port();
        let dir = unique_tag("eliot-t30-qdrant");
        fs::create_dir_all(dir.join("storage")).expect("qdrant storage is created");
        let config = format!(
            "storage:\n  storage_path: ./storage\nservice:\n  host: 127.0.0.1\n  http_port: \
             {http_port}\n  grpc_port: {grpc_port}\n"
        );
        fs::write(dir.join("config.yaml"), config).expect("qdrant config is written");
        let out_log = fs::File::create(dir.join("qdrant-out.log")).expect("out log is created");
        let err_log = fs::File::create(dir.join("qdrant-err.log")).expect("err log is created");
        let child = Command::new(QDRANT_EXE)
            .arg("--config-path")
            .arg(dir.join("config.yaml"))
            .current_dir(&dir)
            .stdout(Stdio::from(out_log))
            .stderr(Stdio::from(err_log))
            .spawn()
            .expect("native qdrant spawns");
        let mut server = Self {
            child: Some(child),
            dir,
            http_port,
        };
        server.wait_ready();
        server
    }

    fn address(&self) -> SocketAddr {
        SocketAddr::from((Ipv4Addr::LOCALHOST, self.http_port))
    }

    fn wait_ready(&mut self) {
        let deadline = Instant::now() + Duration::from_secs(60);
        loop {
            if let Ok((200, body)) = self.request("GET", "/readyz", None)
                && body.contains("ready")
            {
                return;
            }
            assert!(
                !self.is_exited(),
                "qdrant exited during startup: {}",
                self.log_tail()
            );
            assert!(
                Instant::now() < deadline,
                "qdrant not ready in 60s: {}",
                self.log_tail()
            );
            thread::sleep(Duration::from_millis(250));
        }
    }

    fn is_exited(&mut self) -> bool {
        self.child
            .as_mut()
            .is_some_and(|child| child.try_wait().expect("qdrant wait succeeds").is_some())
    }

    fn log_tail(&self) -> String {
        let mut combined = String::new();
        for name in ["qdrant-out.log", "qdrant-err.log"] {
            if let Ok(content) = fs::read_to_string(self.dir.join(name)) {
                let start = content.len().saturating_sub(1024);
                let _ = write!(combined, "--- {name} ---\n{}\n", &content[start..]);
            }
        }
        combined
    }

    /// One blocking HTTP/1.1 round trip. Reads exactly `Content-Length`
    /// bytes, so a kept-alive connection can never stall the test.
    fn request(
        &self,
        method: &str,
        path: &str,
        body: Option<&str>,
    ) -> Result<(u16, String), String> {
        let payload = body.unwrap_or("");
        let http_request = format!(
            "{method} {path} HTTP/1.1\r\nHost: 127.0.0.1\r\nContent-Type: \
             application/json\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{payload}",
            payload.len()
        );
        let mut stream = TcpStream::connect_timeout(&self.address(), Duration::from_secs(10))
            .map_err(|error| format!("qdrant dial failed: {error}"))?;
        stream
            .set_read_timeout(Some(Duration::from_secs(15)))
            .map_err(|error| format!("qdrant timeout setup failed: {error}"))?;
        stream
            .write_all(http_request.as_bytes())
            .map_err(|error| format!("qdrant send failed: {error}"))?;
        let mut head = Vec::new();
        let mut byte = [0_u8; 1];
        loop {
            stream
                .read_exact(&mut byte)
                .map_err(|error| format!("qdrant head read failed: {error}"))?;
            head.push(byte[0]);
            if head.len() >= 4 && head[head.len() - 4..] == *b"\r\n\r\n" {
                break;
            }
            assert!(head.len() <= 64 * 1024, "qdrant response head is bounded");
        }
        let head_text =
            String::from_utf8(head).map_err(|error| format!("qdrant head is UTF-8: {error}"))?;
        let status: u16 = head_text
            .split_whitespace()
            .nth(1)
            .and_then(|code| code.parse().ok())
            .ok_or_else(|| format!("qdrant status parses: {head_text}"))?;
        let length: usize = head_text
            .lines()
            .find_map(|line| {
                let (name, value) = line.split_once(':')?;
                (name.trim().eq_ignore_ascii_case("content-length"))
                    .then(|| value.trim().parse().ok())?
            })
            .ok_or_else(|| format!("qdrant answers with Content-Length: {head_text}"))?;
        assert!(length <= 16 * 1024 * 1024, "qdrant body is bounded");
        let mut content = vec![0_u8; length];
        stream
            .read_exact(&mut content)
            .map_err(|error| format!("qdrant body read failed: {error}"))?;
        let text =
            String::from_utf8(content).map_err(|error| format!("qdrant body is UTF-8: {error}"))?;
        Ok((status, text))
    }

    fn get(&self, path: &str) -> Result<(u16, String), String> {
        self.request("GET", path, None)
    }

    fn put(&self, path: &str, body: &str) -> (u16, String) {
        self.request("PUT", path, Some(body))
            .expect("qdrant PUT round trip succeeds")
    }

    fn post(&self, path: &str, body: &str) -> (u16, String) {
        self.request("POST", path, Some(body))
            .expect("qdrant POST round trip succeeds")
    }

    fn delete(&self, path: &str) -> (u16, String) {
        self.request("DELETE", path, None)
            .expect("qdrant DELETE round trip succeeds")
    }

    /// Kill the server but keep the storage directory: models a crash or a
    /// restart window. The outcome of any in-flight write is unknown until
    /// an exact readback after [`Self::restart`] resolves it.
    fn kill(&mut self) {
        if let Some(mut child) = self.child.take() {
            let _ = child.kill();
            let _ = child.wait();
        }
    }

    /// Respawn on the same storage directory and wait for readiness.
    /// Committed points must read back intact.
    fn restart(&mut self) {
        assert!(self.child.is_none(), "restart requires a killed server");
        let out_log =
            fs::File::create(self.dir.join("qdrant-out.log")).expect("restart out log is created");
        let err_log =
            fs::File::create(self.dir.join("qdrant-err.log")).expect("restart err log is created");
        let child = Command::new(QDRANT_EXE)
            .arg("--config-path")
            .arg(self.dir.join("config.yaml"))
            .current_dir(&self.dir)
            .stdout(Stdio::from(out_log))
            .stderr(Stdio::from(err_log))
            .spawn()
            .expect("native qdrant restarts");
        self.child = Some(child);
        self.wait_ready();
    }
}

impl Drop for LiveQdrant {
    fn drop(&mut self) {
        self.kill();
        let _ = fs::remove_dir_all(&self.dir);
    }
}

fn free_loopback_port() -> u16 {
    TcpListener::bind((Ipv4Addr::LOCALHOST, 0))
        .expect("loopback port probe binds")
        .local_addr()
        .expect("probe address reads")
        .port()
}

fn create_spine_collection(qdrant: &LiveQdrant) {
    let (status, body) = qdrant.put(
        &format!("/collections/{SPINE_COLLECTION}"),
        &format!(
            "{{\"vectors\":{{\"size\":4,\"distance\":\"Dot\"}},\"sparse_vectors\":{{\"{SPINE_VECTOR}\":{{\"modifier\":\"idf\"}}}}}}"
        ),
    );
    assert_eq!(status, 200, "collection creates: {body}");
    assert!(body.contains("\"status\":\"ok\""), "{body}");
    for (name, schema) in [
        ("tenant", "keyword"),
        ("access_partition", "keyword"),
        ("valid_from_epoch", "integer"),
    ] {
        let (status, body) = qdrant.put(
            &format!("/collections/{SPINE_COLLECTION}/index"),
            &format!("{{\"field_name\":\"{name}\",\"field_schema\":\"{schema}\"}}"),
        );
        assert_eq!(status, 200, "payload index {name} creates: {body}");
    }
}

/// One spine point. `terms` are the sparse lexical legs; the payload carries
/// the exact daemon-issued revision/source hex that joins the two halves.
fn spine_point(
    id: u64,
    tenant: &str,
    revision_hex: &str,
    source_hex: &str,
    terms: &[(u32, f32)],
) -> String {
    let indices = terms
        .iter()
        .map(|(index, _)| index.to_string())
        .collect::<Vec<_>>()
        .join(",");
    let values = terms
        .iter()
        .map(|(_, value)| value.to_string())
        .collect::<Vec<_>>()
        .join(",");
    format!(
        "{{\"id\":{id},\"vector\":{{\"{SPINE_VECTOR}\":{{\"indices\":[{indices}],\"values\":[{values}]}}}},\
         \"payload\":{{\"tenant\":\"{tenant}\",\"access_partition\":\"partition-a\",\
         \"valid_from_epoch\":42,\"revision\":\"{revision_hex}\",\"source\":\"{source_hex}\"}}}}"
    )
}

fn upsert_points(qdrant: &LiveQdrant, points: &[String]) {
    let (status, body) = qdrant.put(
        &format!("/collections/{SPINE_COLLECTION}/points?wait=true&ordering=strong"),
        &format!("{{\"points\":[{}]}}", points.join(",")),
    );
    assert_eq!(status, 200, "bounded upsert commits: {body}");
    assert!(body.contains("\"status\":\"completed\""), "{body}");
}

fn exact_count(qdrant: &LiveQdrant, tenant: &str) -> u64 {
    let (status, body) = qdrant.post(
        &format!("/collections/{SPINE_COLLECTION}/points/count"),
        &format!(
            "{{\"filter\":{{\"must\":[{{\"key\":\"tenant\",\"match\":{{\"value\":\"{tenant}\"}}}}]}},\"exact\":true}}"
        ),
    );
    assert_eq!(status, 200, "exact count serves: {body}");
    body.split_once("\"count\":")
        .and_then(|(_, tail)| tail.split(|char: char| !char.is_ascii_digit()).next())
        .and_then(|digits| digits.parse().ok())
        .unwrap_or_else(|| panic!("exact count parses: {body}"))
}

fn filtered_query(qdrant: &LiveQdrant, tenant: &str, indices: &[u32]) -> (u16, String) {
    let list = indices
        .iter()
        .map(u32::to_string)
        .collect::<Vec<_>>()
        .join(",");
    let ones = indices.iter().map(|_| "1.0").collect::<Vec<_>>().join(",");
    qdrant
        .request(
            "POST",
            &format!("/collections/{SPINE_COLLECTION}/points/query"),
            Some(&format!(
                "{{\"vector\":{{\"name\":\"{SPINE_VECTOR}\",\"vector\":{{\"indices\":[{list}],\"values\":[{ones}]}}}},\
                 \"filter\":{{\"must\":[{{\"key\":\"tenant\",\"match\":{{\"value\":\"{tenant}\"}}}}]}},\
                 \"limit\":8,\"with_payload\":true}}"
            )),
        )
        .expect("filtered query round trip succeeds")
}

fn query_ids(body: &str) -> Vec<u64> {
    let mut ids = Vec::new();
    for chunk in body.split("\"id\":").skip(1) {
        let digits: String = chunk
            .chars()
            .take_while(|digit: &char| digit.is_ascii_digit())
            .collect();
        if let Ok(id) = digits.parse() {
            ids.push(id);
        }
    }
    ids.sort_unstable();
    ids
}

// ---------------------------------------------------------------------------
// DIRECT half: fresh root, A-B-A, restart, migrated copy, deny.
// ---------------------------------------------------------------------------

#[test]
fn spine_fresh_root_ingest_search_validated() {
    let fixture = DaemonFixture::new();
    let start = Instant::now();
    fixture.index_named("doc-a.txt", b"alpha spine needle one");
    fixture.index_named("doc-b.txt", b"beta spine needle two");
    fixture.index_named("doc-c.txt", b"gamma unrelated content");
    let index_ms = start.elapsed().as_millis();
    assert!(index_ms <= CEIL_DAEMON_OP_MS, "index_ms={index_ms}");

    let start = Instant::now();
    let needle = fixture.search("needle");
    let search_ms = start.elapsed().as_millis();
    assert!(search_ms <= CEIL_DAEMON_OP_MS, "search_ms={search_ms}");
    assert_eq!(match_lines(&needle).len(), 2, "{needle}");
    assert!(needle.contains("\"complete\":true"), "{needle}");

    let single = fixture.search("unrelated");
    assert_eq!(match_lines(&single).len(), 1, "{single}");
    let absent = fixture.search("absent-needle-xyz");
    assert!(match_lines(&absent).is_empty(), "{absent}");
    assert!(absent.contains("\"complete\":true"), "{absent}");

    let verify = DaemonFixture::ok(&[
        "--verify-root",
        fixture.data.to_str().expect("data path is UTF-8"),
    ]);
    assert!(!verify.is_empty(), "verify-root answers");
    let listed = DaemonFixture::ok(&[
        "--list-sources",
        fixture.data.to_str().expect("data path is UTF-8"),
    ]);
    assert!(!listed.is_empty(), "list-sources answers");
    eprintln!("T30_PERF fresh index_ms={index_ms} search_ms={search_ms}");
}

#[test]
fn spine_aba_change_delete_history() {
    let fixture = DaemonFixture::new();
    let first = fixture.index_named("doc.txt", b"needle-alpha v1 payload");
    let revision_a1 = field(&first, "revision_id").to_owned();
    let second = fixture.index_named("doc.txt", b"unrelated-beta v2 payload");
    let revision_b = field(&second, "revision_id").to_owned();
    let source_b = field(&second, "source_id").to_owned();
    let third = fixture.index_named("doc.txt", b"needle-alpha v1 payload");
    let revision_a2 = field(&third, "revision_id").to_owned();
    // Same bytes reuse the same immutable revision; changed bytes differ.
    assert_eq!(revision_a1, revision_a2);
    assert_ne!(revision_a1, revision_b);

    assert_eq!(match_lines(&fixture.search("needle-alpha")).len(), 1);
    assert!(match_lines(&fixture.search("unrelated-beta")).is_empty());

    let retire = DaemonFixture::ok(&[
        "--retire-source",
        fixture.data.to_str().expect("data path is UTF-8"),
        &source_b,
    ]);
    assert!(retire.contains("\"active\":false"), "{retire}");

    // Historical reads stay exact after change and retirement.
    for (revision, expected) in [
        (revision_a1.as_str(), b"needle-alpha v1 payload".as_slice()),
        (revision_b.as_str(), b"unrelated-beta v2 payload".as_slice()),
    ] {
        let output = fixture.read_revision(revision, 0, expected.len());
        assert!(output.contains(&hex_of(expected)), "{output}");
    }
}

#[test]
fn spine_process_restart_preserves_validated_result() {
    let fixture = DaemonFixture::new();
    fixture.index_named("doc.txt", b"restart durable needle content");
    let before = fixture.search("durable");
    assert_eq!(match_lines(&before).len(), 1, "{before}");
    // Every CLI call is a fresh process: re-running is the restart.
    let after = fixture.search("durable");
    assert_eq!(before, after, "restart preserves the validated result");
    let (status, _, _) = DaemonFixture::run(&[
        "--verify-root",
        fixture.data.to_str().expect("data path is UTF-8"),
    ]);
    assert!(status.success(), "verify-root stays green after restart");
}

#[test]
fn spine_migrated_lane_plan_deterministic_copy_denied() {
    let fixture = DaemonFixture::new();
    fixture.index_named("doc-a.txt", b"migrated legacy needle bytes alpha");
    fixture.index_named("doc-b.txt", b"migrated legacy needle bytes beta");
    let before = fixture.search("legacy");
    assert_eq!(match_lines(&before).len(), 2, "{before}");

    // Migrated lane: the supported offline control-migration plan. It must
    // stay verify-only (no authority switch, no live import) and reproduce
    // the exact plan chain for the same history and target.
    let output_a = fixture.base.join("migration-out-a");
    let output_b = fixture.base.join("migration-out-b");
    fs::create_dir(&output_a).expect("first migration output exists");
    fs::create_dir(&output_b).expect("second migration output exists");
    let plan_a = DaemonFixture::ok(&[
        "--plan-control-migration",
        fixture.data.to_str().expect("data path is UTF-8"),
        MIGRATION_TARGET,
        output_a.to_str().expect("output path is UTF-8"),
    ]);
    assert!(plan_a.contains("\"cutover_authorized\":false"), "{plan_a}");
    assert!(
        plan_a.contains("\"active_control_imported\":false"),
        "{plan_a}"
    );
    assert!(json_num(&plan_a, "events") >= 2, "{plan_a}");
    let plan_b = DaemonFixture::ok(&[
        "--plan-control-migration",
        fixture.data.to_str().expect("data path is UTF-8"),
        MIGRATION_TARGET,
        output_b.to_str().expect("output path is UTF-8"),
    ]);
    assert_eq!(
        field(&plan_b, "plan_chain_sha256"),
        field(&plan_a, "plan_chain_sha256"),
        "migration plan is deterministic"
    );

    // The dry run leaves serving state untouched: identical results.
    assert_eq!(fixture.search("legacy"), before, "plan run mutates nothing");

    // A raw relocated copy is denied by the owner guard by design (paths
    // are locators, not identity): it must never be adopted as a migrated
    // root, and the original keeps serving.
    let moved_base = ScopedTempDir {
        path: unique_tag("eliot-t30-moved"),
    };
    let moved_data = moved_base.path().join("data");
    copy_dir_recursive(&fixture.data, &moved_data);
    let (status, _, stderr) = DaemonFixture::run(&[
        "--search-root",
        moved_data.to_str().expect("data path is UTF-8"),
        "legacy",
    ]);
    assert!(!status.success(), "relocated copy must not serve");
    assert!(stderr.contains("OWNER_GUARD_MISMATCH"), "{stderr}");
    assert_eq!(fixture.search("legacy"), before, "original still serves");
}

#[test]
fn spine_denied_admission_is_typed() {
    let fixture = DaemonFixture::new();
    let empty_path = fixture.base.join("empty.txt");
    fs::write(&empty_path, b"").expect("empty probe is written");
    let (status, _, stderr) = DaemonFixture::run(&[
        "--index-file",
        fixture.data.to_str().expect("data path is UTF-8"),
        empty_path.to_str().expect("source path is UTF-8"),
    ]);
    assert!(!status.success(), "empty admission must not succeed");
    assert!(stderr.contains("SOURCE_ADMISSION_DENIED"), "{stderr}");
    let absent = fixture.search("anything");
    assert!(match_lines(&absent).is_empty(), "{absent}");
    assert!(absent.contains("\"complete\":true"), "{absent}");
    assert!(absent.contains("\"active_sources\":0"), "{absent}");
}

// ---------------------------------------------------------------------------
// Indexed half on the live server, joined with daemon-issued identities.
// ---------------------------------------------------------------------------

#[test]
fn spine_live_qdrant_ingest_query_readback() {
    let daemon = DaemonFixture::new();
    let indexed = daemon.index_named("joined.txt", b"joined spine revision payload");
    let revision = field(&indexed, "revision_id").to_owned();
    let source = field(&indexed, "source_id").to_owned();

    let qdrant = LiveQdrant::spawn();
    create_spine_collection(&qdrant);
    let start = Instant::now();
    upsert_points(
        &qdrant,
        &[
            spine_point(1, "tenant-a", &revision, &source, &[(0, 1.0), (3, 0.5)]),
            spine_point(2, "tenant-a", "00", "00", &[(0, 0.5)]),
            spine_point(3, "tenant-b", "00", "00", &[(0, 2.0)]),
        ],
    );
    let write_ms = start.elapsed().as_millis();
    assert!(write_ms <= CEIL_QDRANT_WRITE_MS, "write_ms={write_ms}");

    // Exact-proof denominator: the filtered count is the authority, and the
    // bounded top-k (8 >= 2) never narrows it.
    assert_eq!(exact_count(&qdrant, "tenant-a"), 2);
    assert_eq!(exact_count(&qdrant, "tenant-b"), 1);

    let start = Instant::now();
    let (status, body) = filtered_query(&qdrant, "tenant-a", &[0, 3]);
    let read_ms = start.elapsed().as_millis();
    assert!(read_ms <= CEIL_QDRANT_READ_MS, "read_ms={read_ms}");
    assert_eq!(status, 200, "{body}");
    assert_eq!(query_ids(&body), vec![1, 2], "{body}");

    let (status, only_b) = filtered_query(&qdrant, "tenant-b", &[0]);
    assert_eq!(status, 200, "{only_b}");
    assert_eq!(query_ids(&only_b), vec![3], "{only_b}");

    // Exact readback carries the daemon-issued identities back: the join.
    let (status, point) = qdrant
        .get(&format!("/collections/{SPINE_COLLECTION}/points/1"))
        .expect("exact GET succeeds");
    assert_eq!(status, 200, "{point}");
    assert!(point.contains(&revision), "{point}");
    assert!(point.contains(&source), "{point}");
    eprintln!(
        "T30_PERF live write_ms={write_ms} read_ms={read_ms} qdrant={QDRANT_VERSION_EVIDENCE}"
    );
}

#[test]
fn spine_live_delete_and_deny_filter() {
    let qdrant = LiveQdrant::spawn();
    create_spine_collection(&qdrant);
    upsert_points(
        &qdrant,
        &[
            spine_point(1, "tenant-a", "aa", "aa", &[(0, 1.0), (3, 0.5)]),
            spine_point(2, "tenant-a", "bb", "bb", &[(0, 0.5)]),
            spine_point(3, "tenant-b", "cc", "cc", &[(0, 2.0)]),
        ],
    );
    assert_eq!(exact_count(&qdrant, "tenant-a"), 2);

    // Explicit-ID delete only: point 2 leaves, nothing else moves.
    let (status, body) = qdrant.post(
        &format!("/collections/{SPINE_COLLECTION}/points/delete?wait=true&ordering=strong"),
        "{\"points\":[2]}",
    );
    assert_eq!(status, 200, "{body}");
    assert_eq!(exact_count(&qdrant, "tenant-a"), 1);
    let (status, body) = filtered_query(&qdrant, "tenant-a", &[0, 3]);
    assert_eq!(status, 200, "{body}");
    assert_eq!(query_ids(&body), vec![1], "{body}");
    let (status, _) = qdrant
        .get(&format!("/collections/{SPINE_COLLECTION}/points/2"))
        .expect("missing-point GET round trip succeeds");
    assert_eq!(status, 404, "deleted point is typed missing, never success");

    // Denied scope: a foreign tenant filter is a typed empty result, not an
    // error and not a narrowed denominator relabelled as success.
    let (status, body) = filtered_query(&qdrant, "tenant-denied", &[0, 3]);
    assert_eq!(status, 200, "{body}");
    assert!(query_ids(&body).is_empty(), "{body}");
    assert!(body.contains("\"status\":\"ok\""), "{body}");
}

#[test]
fn spine_live_unknown_write_resolves_by_readback() {
    let mut qdrant = LiveQdrant::spawn();
    create_spine_collection(&qdrant);
    upsert_points(
        &qdrant,
        &[spine_point(11, "tenant-a", "u11", "s11", &[(1, 2.0)])],
    );

    // Crash window: anything in flight is OUTCOME_UNKNOWN. The client must
    // observe a transport failure, never invent a receipt.
    qdrant.kill();
    assert!(
        qdrant
            .get(&format!("/collections/{SPINE_COLLECTION}/points/11"))
            .is_err(),
        "dead server fails the round trip, never a receipt"
    );

    // Exact readback after restart resolves the unknown: the committed
    // write is present, byte-identical.
    qdrant.restart();
    let (status, point) = qdrant
        .get(&format!("/collections/{SPINE_COLLECTION}/points/11"))
        .expect("post-restart readback succeeds");
    assert_eq!(status, 200, "{point}");
    assert!(point.contains("\"revision\":\"u11\""), "{point}");

    // Same identity plus same bytes converges idempotently.
    upsert_points(
        &qdrant,
        &[spine_point(11, "tenant-a", "u11", "s11", &[(1, 2.0)])],
    );
    let (status, point) = qdrant
        .get(&format!("/collections/{SPINE_COLLECTION}/points/11"))
        .expect("converged readback succeeds");
    assert_eq!(status, 200, "{point}");
    assert!(point.contains("\"revision\":\"u11\""), "{point}");
}

#[test]
fn spine_live_index_loss_and_rebuild() {
    let daemon = DaemonFixture::new();
    let retained_bytes: &[u8] = b"rebuild retained truth payload";
    let indexed = daemon.index_named("truth.txt", retained_bytes);
    let revision = field(&indexed, "revision_id").to_owned();
    let source = field(&indexed, "source_id").to_owned();

    let qdrant = LiveQdrant::spawn();
    create_spine_collection(&qdrant);
    // Retained manifest mirror: every replayed point is rebuilt from these
    // exact payloads, never inferred from backend state.
    let retained: Vec<(u64, &str)> = vec![(21, "tenant-a"), (22, "tenant-a"), (23, "tenant-b")];
    let replay: Vec<String> = retained
        .iter()
        .map(|(id, tenant)| spine_point(*id, tenant, &revision, &source, &[(0, 1.0)]))
        .collect();
    upsert_points(&qdrant, &replay);
    assert_eq!(exact_count(&qdrant, "tenant-a"), 2);
    assert_eq!(exact_count(&qdrant, "tenant-b"), 1);

    // Index loss: the whole collection is gone. Loss is typed (404), and
    // source truth on disk is untouched.
    let (status, body) = qdrant.delete(&format!("/collections/{SPINE_COLLECTION}"));
    assert_eq!(status, 200, "{body}");
    let (status, _) = qdrant
        .get(&format!("/collections/{SPINE_COLLECTION}/points/21"))
        .expect("post-loss GET round trip succeeds");
    assert_eq!(status, 404, "lost index reads typed missing");
    let lost_count = qdrant.request(
        "POST",
        &format!("/collections/{SPINE_COLLECTION}/points/count"),
        Some("{\"exact\":true}"),
    );
    assert!(
        lost_count.is_ok_and(|(status, _)| status == 404),
        "lost index counts typed missing"
    );

    // Rebuild replays the retained manifest into a fresh collection and the
    // full readback gate must pass before the route could cut over.
    create_spine_collection(&qdrant);
    upsert_points(&qdrant, &replay);
    assert_eq!(exact_count(&qdrant, "tenant-a"), 2);
    assert_eq!(exact_count(&qdrant, "tenant-b"), 1);
    for (id, _) in &retained {
        let (status, point) = qdrant
            .get(&format!("/collections/{SPINE_COLLECTION}/points/{id}"))
            .expect("rebuilt readback succeeds");
        assert_eq!(status, 200, "{point}");
        assert!(point.contains(&revision), "{point}");
    }
    let (status, body) = filtered_query(&qdrant, "tenant-a", &[0]);
    assert_eq!(status, 200, "{body}");
    assert_eq!(query_ids(&body), vec![21, 22], "{body}");

    // Source truth never moved through loss and rebuild.
    let output = daemon.read_revision(&revision, 0, retained_bytes.len());
    assert!(output.contains(&hex_of(retained_bytes)), "{output}");
}

#[test]
fn spine_joint_restart_direct_unaffected_index_durable() {
    let daemon = DaemonFixture::new();
    let indexed = daemon.index_named("joint.txt", b"joint restart needle durable");
    let revision = field(&indexed, "revision_id").to_owned();
    let source = field(&indexed, "source_id").to_owned();

    let mut qdrant = LiveQdrant::spawn();
    create_spine_collection(&qdrant);
    upsert_points(
        &qdrant,
        &[spine_point(31, "tenant-a", &revision, &source, &[(2, 1.5)])],
    );

    // Kill the index: DIRECT still serves from CAS/redb alone, proving the
    // retrieval/index separation (clients own interpretation; the index
    // only proposes).
    qdrant.kill();
    let direct = daemon.search("joint");
    assert_eq!(match_lines(&direct).len(), 1, "{direct}");

    // Respawn on the same storage: the committed point is durable.
    qdrant.restart();
    let (status, point) = qdrant
        .get(&format!("/collections/{SPINE_COLLECTION}/points/31"))
        .expect("post-restart readback succeeds");
    assert_eq!(status, 200, "{point}");
    assert!(point.contains(&revision), "{point}");
    assert!(point.contains(&source), "{point}");
    assert_eq!(exact_count(&qdrant, "tenant-a"), 1);
}

#[test]
fn spine_perf_ceilings_measured() {
    let joint_start = Instant::now();
    let daemon = DaemonFixture::new();
    let start = Instant::now();
    daemon.index_named("perf.txt", b"perf ceiling needle measurement payload");
    let daemon_index_ms = start.elapsed().as_millis();

    let start = Instant::now();
    let found = daemon.search("ceiling");
    let daemon_search_ms = start.elapsed().as_millis();
    assert_eq!(match_lines(&found).len(), 1, "{found}");

    let qdrant = LiveQdrant::spawn();
    let start = Instant::now();
    create_spine_collection(&qdrant);
    upsert_points(
        &qdrant,
        &[spine_point(41, "tenant-a", "p41", "s41", &[(0, 1.0)])],
    );
    let qdrant_write_ms = start.elapsed().as_millis();

    let start = Instant::now();
    let (status, body) = filtered_query(&qdrant, "tenant-a", &[0]);
    assert_eq!(status, 200, "{body}");
    assert_eq!(query_ids(&body), vec![41], "{body}");
    let (status, _) = qdrant
        .get(&format!("/collections/{SPINE_COLLECTION}/points/41"))
        .expect("perf readback succeeds");
    assert_eq!(status, 200);
    let qdrant_read_ms = start.elapsed().as_millis();

    let joint_ms = joint_start.elapsed().as_millis();
    eprintln!(
        "T30_PERF daemon_index_ms={daemon_index_ms} daemon_search_ms={daemon_search_ms} \
         qdrant_write_ms={qdrant_write_ms} qdrant_read_ms={qdrant_read_ms} joint_ms={joint_ms} \
         qdrant={QDRANT_VERSION_EVIDENCE}"
    );
    assert!(
        daemon_index_ms <= CEIL_DAEMON_OP_MS,
        "daemon_index_ms={daemon_index_ms}"
    );
    assert!(
        daemon_search_ms <= CEIL_DAEMON_OP_MS,
        "daemon_search_ms={daemon_search_ms}"
    );
    assert!(
        qdrant_write_ms <= CEIL_QDRANT_WRITE_MS,
        "qdrant_write_ms={qdrant_write_ms}"
    );
    assert!(
        qdrant_read_ms <= CEIL_QDRANT_READ_MS,
        "qdrant_read_ms={qdrant_read_ms}"
    );
    assert!(joint_ms <= CEIL_JOINT_PASS_MS, "joint_ms={joint_ms}");
}

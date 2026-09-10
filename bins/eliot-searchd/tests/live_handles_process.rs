//! T21 live-handle and continuation regressions through the served daemon.
//!
//! Handles and continuation tokens are opaque qualified-entropy locators bound
//! to one namespace and one session. These fixtures drive the real
//! `--serve-data-root` session and prove end to end that possession alone
//! admits nothing: cross-session, cross-root and post-restart replay fails
//! closed, source mutation invalidates live state, tampered tokens and widened
//! ranges are denied, oversized expansions stay bounded, and exhausted windows
//! release their pins without persisting query history.

use std::fmt::Write as _;
use std::fs;
use std::io::{BufRead, BufReader, BufWriter, Read as _, Write};
use std::path::{Path, PathBuf};
use std::process::{Child, ChildStdin, ChildStdout, Command, Stdio};
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::mpsc::{self, Receiver};
use std::thread::{self, JoinHandle};
use std::time::{Duration, Instant, SystemTime, UNIX_EPOCH};

// Shared Credential Manager cleanup; each harness uses a subset of it.
#[allow(dead_code)]
mod common;

const TIMEOUT: Duration = Duration::from_secs(30);
const LINE_CAP: usize = 1_048_576;
static NEXT: AtomicU64 = AtomicU64::new(0);

struct Scratch {
    base: PathBuf,
    guard: common::RevisionKeyTreeGuard,
}

impl Scratch {
    fn new() -> Self {
        let stamp = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_nanos();
        let base = std::env::temp_dir().join(format!(
            "eliot-t21-live-{}-{stamp}-{}",
            std::process::id(),
            NEXT.fetch_add(1, Ordering::Relaxed),
        ));
        fs::create_dir_all(base.join("root")).unwrap();
        fs::create_dir_all(base.join("src")).unwrap();
        let guard = common::RevisionKeyTreeGuard::for_tree(&base);
        Self { base, guard }
    }

    fn root(&self) -> PathBuf {
        self.base.join("root")
    }

    fn source(&self, name: &str, bytes: &[u8]) -> PathBuf {
        let path = self.base.join("src").join(name);
        fs::write(&path, bytes).unwrap();
        path
    }
}

impl Drop for Scratch {
    fn drop(&mut self) {
        self.guard.cleanup();
        let _ = fs::remove_dir_all(&self.base);
    }
}

struct LiveService {
    child: Child,
    stdin: BufWriter<ChildStdin>,
    lines: Receiver<String>,
    log: Vec<String>,
    _stderr: JoinHandle<String>,
}

impl LiveService {
    fn start(root: &Path) -> Self {
        let mut child = Command::new(env!("CARGO_BIN_EXE_eliot-searchd"))
            .args(["--serve-data-root"])
            .arg(root)
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .spawn()
            .expect("serve daemon");
        let stdin = BufWriter::new(child.stdin.take().expect("service stdin"));
        let stdout: ChildStdout = child.stdout.take().expect("service stdout");
        let stderr = child.stderr.take().expect("service stderr");
        let (tx, rx) = mpsc::channel();
        thread::spawn(move || {
            let mut reader = BufReader::new(stdout);
            loop {
                let mut line = String::new();
                match reader.read_line(&mut line) {
                    Ok(0) | Err(_) => break,
                    Ok(_) => {
                        assert!(line.len() <= LINE_CAP + 1, "fixture line limit");
                        if tx.send(line).is_err() {
                            break;
                        }
                    }
                }
            }
        });
        let errors = thread::spawn(move || {
            let mut bytes = Vec::new();
            let _ = stderr.take(LINE_CAP as u64 + 1).read_to_end(&mut bytes);
            String::from_utf8_lossy(&bytes).into_owned()
        });
        let mut service = Self {
            child,
            stdin,
            lines: rx,
            log: Vec::new(),
            _stderr: errors,
        };
        let ready = service.wait_for("data_root_ready", "\"event\":\"data_root_ready\"");
        assert!(ready.contains("\"paged_search_available\":true"), "{ready}");
        service
    }

    fn send(&mut self, command: &str) {
        writeln!(self.stdin, "{command}").unwrap();
        self.stdin.flush().unwrap();
    }

    fn wait_for(&mut self, what: &str, needle: &str) -> String {
        let deadline = Instant::now() + TIMEOUT;
        loop {
            let remaining = deadline.saturating_duration_since(Instant::now());
            if let Ok(line) = self.lines.recv_timeout(remaining) {
                self.log.push(line.clone());
                if line.contains(needle) {
                    return line;
                }
            } else {
                let _ = self.child.kill();
                panic!("timed out waiting for {what}; log:\n{}", self.log.join(""));
            }
        }
    }

    fn shutdown(mut self) {
        self.send("shutdown");
        let draining = self.wait_for("draining", "\"event\":\"draining\"");
        assert!(draining.contains("\"accepted\":true"), "{draining}");
        let stopped = self.wait_for("data_root_stopped", "\"event\":\"data_root_stopped\"");
        assert!(stopped.contains("\"clean\":true"), "{stopped}");
        let deadline = Instant::now() + TIMEOUT;
        loop {
            if let Some(status) = self.child.try_wait().unwrap() {
                assert!(status.success(), "log:\n{}", self.log.join(""));
                return;
            }
            if Instant::now() >= deadline {
                let _ = self.child.kill();
                panic!("service did not stop; log:\n{}", self.log.join(""));
            }
            thread::sleep(Duration::from_millis(10));
        }
    }
}

impl Drop for LiveService {
    fn drop(&mut self) {
        let _ = self.child.kill();
        let _ = self.child.wait();
    }
}

fn hex(bytes: &[u8]) -> String {
    bytes.iter().fold(String::new(), |mut output, byte| {
        let _ = write!(output, "{byte:02x}");
        output
    })
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

fn field_string(line: &str, key: &str) -> Option<String> {
    let needle = format!("\"{key}\":\"");
    let (_, rest) = line.split_once(needle.as_str())?;
    rest.split('"').next().map(str::to_owned)
}

fn field_number(line: &str, key: &str) -> Option<u64> {
    let needle = format!("\"{key}\":");
    let (_, rest) = line.split_once(needle.as_str())?;
    rest.split([',', '}']).next()?.parse().ok()
}

fn is_opaque_token(token: &str) -> bool {
    token.len() == 64 && token.bytes().all(|byte| byte.is_ascii_hexdigit())
}

fn index_file(service: &mut LiveService, path: &Path) {
    service.send(&format!("index-file\t{}", path_hex(path)));
    let indexed = service.wait_for("source_indexed", "\"event\":\"source_indexed\"");
    assert!(
        indexed.contains("\"diagnostic_internal_identifiers\":true"),
        "{indexed}"
    );
}

fn search_first_page(service: &mut LiveService, query: &str, page_size: &str) -> Vec<String> {
    service.send(&format!(
        "search-page\tsensitive\t{page_size}\t{}",
        hex(query.as_bytes())
    ));
    let deadline = Instant::now() + TIMEOUT;
    let mut lines = Vec::new();
    loop {
        let remaining = deadline.saturating_duration_since(Instant::now());
        if let Ok(line) = service.lines.recv_timeout(remaining) {
            service.log.push(line.clone());
            lines.push(line.clone());
            if line.contains("\"event\":\"search_page_complete\"") {
                return lines;
            }
        } else {
            panic!("timed out waiting for search_page_complete");
        }
    }
}

fn page_token(complete: &str) -> Option<String> {
    if complete.contains("\"continuation_token\":null") {
        None
    } else {
        field_string(complete, "continuation_token")
    }
}

fn continue_page(service: &mut LiveService, token: &str, page_size: &str) -> Vec<String> {
    service.send(&format!("continue\t{token}\t{page_size}"));
    let deadline = Instant::now() + TIMEOUT;
    let mut lines = Vec::new();
    loop {
        let remaining = deadline.saturating_duration_since(Instant::now());
        if let Ok(line) = service.lines.recv_timeout(remaining) {
            service.log.push(line.clone());
            lines.push(line.clone());
            if line.contains("\"event\":\"search_page_complete\"")
                || line.contains("\"event\":\"error\"")
            {
                return lines;
            }
        } else {
            panic!("timed out waiting for continue response");
        }
    }
}

fn match_lines(lines: &[String]) -> Vec<&String> {
    lines
        .iter()
        .filter(|line| line.contains("\"event\":\"match\""))
        .collect()
}

#[test]
fn paged_search_expand_roundtrip_releases_pins_on_exhaustion() {
    let scratch = Scratch::new();
    let path = scratch.source("needles.txt", b"needle one\nneedle two\nneedle three\n");
    let mut service = LiveService::start(&scratch.root());
    index_file(&mut service, &path);

    let mut handles = Vec::new();
    let mut pages = search_first_page(&mut service, "needle", "1");
    let mut total_matches = 0_usize;
    loop {
        let found = match_lines(&pages);
        total_matches += found.len();
        for line in &found {
            let handle = field_string(line, "source_handle").expect("opaque handle");
            assert!(is_opaque_token(&handle), "{handle}");
            let start = field_number(line, "byte_start").expect("byte_start");
            let end = field_number(line, "byte_end").expect("byte_end");
            handles.push((handle, start, end));
        }
        let complete = pages
            .iter()
            .find(|line| line.contains("\"event\":\"search_page_complete\""))
            .expect("page complete")
            .clone();
        if let Some(token) = page_token(&complete) {
            assert!(is_opaque_token(&token), "{token}");
            pages = continue_page(&mut service, &token, "1");
        } else {
            assert!(complete.contains("\"exhausted\":true"), "{complete}");
            break;
        }
    }
    assert!(
        total_matches >= 2,
        "fixture needs several pages, saw {total_matches}"
    );

    let (handle, start, end) = handles.first().expect("at least one handle").clone();
    service.send(&format!("expand-handle\t{handle}\t{start}\t{end}"));
    let expanded = service.wait_for(
        "source_handle_expanded",
        "\"event\":\"source_handle_expanded\"",
    );
    let bytes = field_string(&expanded, "bytes").expect("expansion bytes");
    let content = fs::read(&path).unwrap();
    let start = usize::try_from(start).expect("fixture byte offset fits");
    let end = usize::try_from(end).expect("fixture byte offset fits");
    assert_eq!(bytes, hex(&content[start..end]), "{expanded}");

    service.send("health");
    let health = service.wait_for("health", "\"event\":\"health\"");
    let live_continuations = field_number(&health, "live_continuations").expect("live count");
    let live_handles = field_number(&health, "live_source_handles").expect("handle count");
    assert_eq!(
        live_continuations, 0,
        "exhaustion releases every pin: {health}"
    );
    assert_eq!(
        live_handles, total_matches as u64,
        "handles stay TTL-bound: {health}"
    );
    service.shutdown();
}

#[test]
fn source_mutation_invalidates_continuation_and_handles() {
    let scratch = Scratch::new();
    let path = scratch.source("needles.txt", b"needle one\nneedle two\nneedle three\n");
    let mut service = LiveService::start(&scratch.root());
    index_file(&mut service, &path);

    let pages = search_first_page(&mut service, "needle", "1");
    let complete = pages
        .iter()
        .find(|line| line.contains("\"event\":\"search_page_complete\""))
        .expect("page complete")
        .clone();
    let token = page_token(&complete).expect("continuation expected");
    let first_match = match_lines(&pages)
        .into_iter()
        .next()
        .expect("match")
        .clone();
    let handle = field_string(&first_match, "source_handle").expect("handle");
    let start = field_number(&first_match, "byte_start").expect("byte_start");
    let end = field_number(&first_match, "byte_end").expect("byte_end");

    service.send("list-sources");
    let source_line = service.wait_for("source_listed", "\"event\":\"source\"");
    let source_id = field_string(&source_line, "source_id").expect("source id");
    service.wait_for("source_list_complete", "\"event\":\"source_list_complete\"");

    service.send(&format!("retire\t{source_id}"));
    let retired = service.wait_for("source_retired", "\"event\":\"source_retired\"");
    let invalidated_continuations =
        field_number(&retired, "invalidated_continuations").expect("count");
    let invalidated_handles = field_number(&retired, "invalidated_handles").expect("count");
    assert_eq!(invalidated_continuations, 1, "{retired}");
    assert!(invalidated_handles >= 1, "{retired}");

    service.send(&format!("continue\t{token}\t1"));
    let denied = service.wait_for("continuation_denied", "\"event\":\"error\"");
    assert!(denied.contains("DIRECT_CONTINUATION_NOT_FOUND"), "{denied}");

    service.send(&format!("expand-handle\t{handle}\t{start}\t{end}"));
    let denied = service.wait_for("handle_denied", "\"event\":\"error\"");
    assert!(
        denied.contains("DIRECT_RESULT_HANDLE_NOT_FOUND"),
        "{denied}"
    );
    service.shutdown();
}

#[test]
fn restart_forgets_session_tokens_and_roots_do_not_share_them() {
    let scratch = Scratch::new();
    let path = scratch.source("needles.txt", b"needle one\nneedle two\nneedle three\n");
    let other_root = scratch.base.join("other");
    fs::create_dir_all(&other_root).unwrap();

    let mut first = LiveService::start(&scratch.root());
    index_file(&mut first, &path);
    let pages = search_first_page(&mut first, "needle", "1");
    let complete = pages
        .iter()
        .find(|line| line.contains("\"event\":\"search_page_complete\""))
        .expect("page complete")
        .clone();
    let token = page_token(&complete).expect("continuation expected");
    let first_match = match_lines(&pages)
        .into_iter()
        .next()
        .expect("match")
        .clone();
    let handle = field_string(&first_match, "source_handle").expect("handle");
    let start = field_number(&first_match, "byte_start").expect("byte_start");
    let end = field_number(&first_match, "byte_end").expect("byte_end");
    first.shutdown();

    let mut second = LiveService::start(&scratch.root());
    second.send(&format!("continue\t{token}\t1"));
    let denied = second.wait_for("restart_denied", "\"event\":\"error\"");
    assert!(denied.contains("DIRECT_CONTINUATION_NOT_FOUND"), "{denied}");
    second.send(&format!("expand-handle\t{handle}\t{start}\t{end}"));
    let denied = second.wait_for("restart_handle_denied", "\"event\":\"error\"");
    assert!(
        denied.contains("DIRECT_RESULT_HANDLE_NOT_FOUND"),
        "{denied}"
    );
    second.shutdown();

    let mut foreign = LiveService::start(&other_root);
    foreign.send(&format!("continue\t{token}\t1"));
    let denied = foreign.wait_for("cross_root_denied", "\"event\":\"error\"");
    assert!(denied.contains("DIRECT_CONTINUATION_NOT_FOUND"), "{denied}");
    foreign.shutdown();
}

#[test]
fn tampered_token_and_bad_ranges_are_denied_without_killing_state() {
    let scratch = Scratch::new();
    let path = scratch.source("needles.txt", b"needle one\nneedle two\nneedle three\n");
    let mut service = LiveService::start(&scratch.root());
    index_file(&mut service, &path);

    let pages = search_first_page(&mut service, "needle", "1");
    let complete = pages
        .iter()
        .find(|line| line.contains("\"event\":\"search_page_complete\""))
        .expect("page complete")
        .clone();
    let raw = page_token(&complete).expect("continuation expected");
    assert!(is_opaque_token(&raw), "{raw}");
    let mut token = raw.clone();
    let first = token.remove(0);
    token.insert(0, if first == '0' { '1' } else { '0' });
    assert_ne!(token, raw, "tamper must change the token");
    let first_match = match_lines(&pages)
        .into_iter()
        .next()
        .expect("match")
        .clone();
    let handle = field_string(&first_match, "source_handle").expect("handle");
    let start = field_number(&first_match, "byte_start").expect("byte_start");
    let end = field_number(&first_match, "byte_end").expect("byte_end");

    service.send(&format!("continue\t{token}\t1"));
    let denied = service.wait_for("tamper_denied", "\"event\":\"error\"");
    assert!(denied.contains("DIRECT_CONTINUATION_NOT_FOUND"), "{denied}");

    service.send(&format!("expand-handle\t{handle}\t{start}\t99999999"));
    let denied = service.wait_for("widen_denied", "\"event\":\"error\"");
    assert!(
        denied.contains("DIRECT_RESULT_HANDLE_RANGE_INVALID"),
        "{denied}"
    );

    service.send(&format!("expand-handle\t{handle}\t{end}\t{end}"));
    let denied = service.wait_for("empty_denied", "\"event\":\"error\"");
    assert!(
        denied.contains("DIRECT_RESULT_HANDLE_RANGE_INVALID"),
        "{denied}"
    );

    service.send(&format!("expand-handle\t{handle}\t{start}\t{end}"));
    let expanded = service.wait_for("still_live", "\"event\":\"source_handle_expanded\"");
    assert!(
        expanded.contains(&format!("\"source_handle\":\"{handle}\"")),
        "{expanded}"
    );
    service.shutdown();
}

#[test]
fn oversized_expansion_stays_bounded() {
    let scratch = Scratch::new();
    let wide = vec![b'x'; 30 * 1024];
    let path = scratch.source("wide.txt", &wide);
    let mut service = LiveService::start(&scratch.root());
    index_file(&mut service, &path);

    let pages = search_first_page(&mut service, "xxx", "1");
    let first_match = match_lines(&pages)
        .into_iter()
        .next()
        .expect("match")
        .clone();
    let handle = field_string(&first_match, "source_handle").expect("handle");

    service.send(&format!("expand-handle\t{handle}\t0\t{}", 30 * 1024));
    let denied = service.wait_for("oversize_denied", "\"event\":\"error\"");
    assert!(
        denied.contains("DIRECT_RESULT_HANDLE_EXPANSION_TOO_LARGE"),
        "{denied}"
    );

    service.send(&format!("expand-handle\t{handle}\t0\t100"));
    let expanded = service.wait_for("bounded_ok", "\"event\":\"source_handle_expanded\"");
    let bytes = field_string(&expanded, "bytes").expect("expansion bytes");
    assert_eq!(bytes, "78".repeat(100), "{expanded}");
    service.shutdown();
}

#[test]
fn exact_final_page_exhausts_without_history() {
    let scratch = Scratch::new();
    let path = scratch.source("two.txt", b"needle alpha\nneedle beta\n");
    let mut service = LiveService::start(&scratch.root());
    index_file(&mut service, &path);

    let pages = search_first_page(&mut service, "needle", "100");
    let complete = pages
        .iter()
        .find(|line| line.contains("\"event\":\"search_page_complete\""))
        .expect("page complete")
        .clone();
    assert!(complete.contains("\"exhausted\":true"), "{complete}");
    assert!(
        complete.contains("\"continuation_token\":null"),
        "{complete}"
    );

    service.send("health");
    let health = service.wait_for("health", "\"event\":\"health\"");
    let live_continuations = field_number(&health, "live_continuations").expect("live count");
    assert_eq!(
        live_continuations, 0,
        "ordinary queries persist no history: {health}"
    );
    service.shutdown();
}

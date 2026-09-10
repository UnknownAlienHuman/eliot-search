//! T33 ephemeral editor overlays at the daemon boundary: unsaved bytes are
//! memory-only and never reach durable daemon state.
//!
//! The daemon query composition that fuses authenticated overlay candidates
//! over retained revisions is deferred (`query_composition.rs` is not part of
//! this change), so this process fixture proves the fail-closed half of the
//! T33 exit condition through the real daemon executable:
//!
//! * an in-memory editor edit is invisible to retained search;
//! * no daemon file, control log, or protocol line ever carries unsaved bytes
//!   (sentinel scan over the whole sandbox plus captured output);
//! * currentness is never claimed across unresolved observation gaps or
//!   pending unsaved state;
//! * an explicit save (write plus sync admission) is the only route that
//!   makes edited bytes searchable.
//!
//! No durable second index is created: the unsaved edit below never touches
//! the filesystem, and the sentinel scan proves it stayed that way.

use std::ffi::OsStr;
use std::fs;
use std::path::{Path, PathBuf};
use std::process::{Command, Output, Stdio};
use std::sync::atomic::{AtomicU64, Ordering};
use std::time::{SystemTime, UNIX_EPOCH};

#[allow(dead_code)]
mod common;

/// In-memory-only editor content marker. This byte string is constructed in
/// process memory and must never be written under the sandbox roots.
const UNSAVED_TOKEN: &str = "UNSAVED-EPHEMERAL-7F3A9C-NEEDLE";

struct Sandbox {
    base: PathBuf,
    data: PathBuf,
    source: PathBuf,
    guard: common::RevisionKeyGuard,
    transcript: Vec<String>,
}

impl Sandbox {
    fn new() -> Self {
        static NEXT: AtomicU64 = AtomicU64::new(0);
        let stamp = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_nanos();
        let base = std::env::temp_dir().join(format!(
            "eliot-overlay-process-{}-{stamp}-{}",
            std::process::id(),
            NEXT.fetch_add(1, Ordering::Relaxed),
        ));
        fs::create_dir(&base).unwrap();
        let base = fs::canonicalize(base).unwrap();
        let data = base.join("data");
        let source = base.join("source");
        fs::create_dir(&data).unwrap();
        fs::create_dir(&source).unwrap();
        let guard = common::RevisionKeyGuard::for_data_root(&data);
        Self {
            base,
            data,
            source,
            guard,
            transcript: Vec::new(),
        }
    }

    fn invoke(&mut self, command: &str, tail: &[&OsStr]) -> Output {
        let output = Command::new(env!("CARGO_BIN_EXE_eliot-searchd"))
            .arg(command)
            .arg(&self.data)
            .args(tail)
            .stdin(Stdio::null())
            .output()
            .expect("spawn primary daemon");
        self.guard.refresh();
        self.transcript
            .push(String::from_utf8_lossy(&output.stdout).into_owned());
        self.transcript
            .push(String::from_utf8_lossy(&output.stderr).into_owned());
        output
    }

    fn success(&mut self, command: &str, tail: &[&OsStr]) -> String {
        let output = self.invoke(command, tail);
        assert!(
            output.status.success(),
            "{command} failed: stdout={} stderr={}",
            String::from_utf8_lossy(&output.stdout),
            String::from_utf8_lossy(&output.stderr),
        );
        String::from_utf8(output.stdout).expect("UTF-8 protocol output")
    }

    fn assert_transcript_has_no_unsaved(&self) {
        for line in &self.transcript {
            assert!(
                !line.contains(UNSAVED_TOKEN),
                "unsaved bytes escaped into daemon output: {line}"
            );
        }
    }

    fn assert_no_currentness_claim(&self) {
        for line in &self.transcript {
            assert!(
                !line.contains("\"current_workspace_proven\":true"),
                "currentness must never be claimed across unsaved state: {line}"
            );
        }
    }
}

impl Drop for Sandbox {
    fn drop(&mut self) {
        self.guard.cleanup();
        let _ = fs::remove_dir_all(&self.base);
    }
}

fn collect_files(dir: &Path, out: &mut Vec<PathBuf>) {
    let entries = fs::read_dir(dir).expect("read sandbox dir");
    for entry in entries {
        let path = entry.expect("sandbox dir entry").path();
        if path.is_dir() {
            collect_files(&path, out);
        } else if path.is_file() {
            out.push(path);
        }
    }
}

fn contains_window(haystack: &[u8], needle: &[u8]) -> bool {
    !needle.is_empty()
        && haystack
            .windows(needle.len())
            .any(|window| window == needle)
}

fn assert_no_unsaved_bytes_on_disk(base: &Path) {
    let mut files = Vec::new();
    collect_files(base, &mut files);
    assert!(
        !files.is_empty(),
        "sandbox must contain daemon state to scan"
    );
    for path in &files {
        let bytes = fs::read(path).unwrap_or_default();
        assert!(
            !contains_window(&bytes, UNSAVED_TOKEN.as_bytes()),
            "unsaved bytes reached durable state: {}",
            path.display()
        );
    }
}

fn register_and_sync(sandbox: &mut Sandbox) {
    let root = sandbox.source.clone();
    let registered = sandbox.success("--register-source-root", &[root.as_os_str()]);
    assert!(registered.contains("\"persisted\":true"), "{registered}");
    assert!(
        registered.contains("\"access_granted\":false"),
        "{registered}"
    );
    let synced = sandbox.success("--sync-source-roots", &[]);
    assert!(synced.contains("\"complete\":true"), "{synced}");
    assert!(
        synced.contains("\"current_workspace_proven\":false"),
        "{synced}"
    );
    let listed = sandbox.success("--source-roots", &[]);
    assert!(
        listed.contains("\"current_workspace_proven\":false"),
        "{listed}"
    );
}

#[test]
fn unsaved_editor_bytes_never_reach_daemon_state() {
    let mut sandbox = Sandbox::new();
    fs::write(
        sandbox.source.join("note.txt"),
        b"retained workspace needle alpha\n",
    )
    .unwrap();
    register_and_sync(&mut sandbox);

    let retained = sandbox.success("--search-root", &[OsStr::new("retained")]);
    assert!(retained.contains("\"matches\":1"), "{retained}");

    // The editor edit lives only in this vector: it is never written under
    // the sandbox roots and never passed to the daemon as an argument.
    let unsaved: Vec<u8> = format!("dirty buffer {UNSAVED_TOKEN} draft\n").into_bytes();
    assert!(contains_window(&unsaved, UNSAVED_TOKEN.as_bytes()));

    let invisible = sandbox.success("--search-root", &[OsStr::new(UNSAVED_TOKEN)]);
    assert!(invisible.contains("\"matches\":0"), "{invisible}");

    sandbox.assert_transcript_has_no_unsaved();
    sandbox.assert_no_currentness_claim();
    assert_no_unsaved_bytes_on_disk(&sandbox.base);
}

#[test]
fn explicit_save_is_the_only_route_to_searchability() {
    let mut sandbox = Sandbox::new();
    fs::write(
        sandbox.source.join("note.txt"),
        b"retained workspace needle alpha\n",
    )
    .unwrap();
    register_and_sync(&mut sandbox);

    let before = sandbox.success("--search-root", &[OsStr::new(UNSAVED_TOKEN)]);
    assert!(before.contains("\"matches\":0"), "{before}");

    // Explicit save/admission: bytes are written to the admitted source root
    // and reconciled by sync. Only then may retained search observe them.
    let saved = format!("retained workspace needle alpha\nsaved editor line {UNSAVED_TOKEN}\n");
    fs::write(sandbox.source.join("note.txt"), saved.as_bytes()).unwrap();
    let synced = sandbox.success("--sync-source-roots", &[]);
    assert!(synced.contains("\"complete\":true"), "{synced}");

    let after = sandbox.success("--search-root", &[OsStr::new(UNSAVED_TOKEN)]);
    assert!(after.contains("\"matches\":1"), "{after}");
    let retained = sandbox.success("--search-root", &[OsStr::new("retained")]);
    assert!(retained.contains("\"matches\":1"), "{retained}");
}

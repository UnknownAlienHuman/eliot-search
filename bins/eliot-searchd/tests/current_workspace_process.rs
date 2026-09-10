//! T31 current-workspace truth: live register/list/sync/unregister with
//! explicit gaps. Each invocation holds the daemon owner exclusively;
//! a live holder denies a second owner instead of sharing the lock.

use std::ffi::OsStr;
use std::fs;
use std::path::PathBuf;
use std::process::{Command, Output, Stdio};
use std::sync::atomic::{AtomicU64, Ordering};
use std::time::{SystemTime, UNIX_EPOCH};

#[allow(dead_code)]
mod common;

struct Sandbox {
    base: PathBuf,
    data: PathBuf,
    source: PathBuf,
    guard: common::RevisionKeyGuard,
}

impl Sandbox {
    fn new() -> Self {
        static NEXT: AtomicU64 = AtomicU64::new(0);
        let stamp = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_nanos();
        let base = std::env::temp_dir().join(format!(
            "eliot-current-workspace-{}-{stamp}-{}",
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
        }
    }

    fn invoke(&self, command: &str, tail: &[&OsStr]) -> Output {
        let output = Command::new(env!("CARGO_BIN_EXE_eliot-searchd"))
            .arg(command)
            .arg(&self.data)
            .args(tail)
            .stdin(Stdio::null())
            .output()
            .expect("spawn primary daemon");
        self.guard.refresh();
        output
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
}

impl Drop for Sandbox {
    fn drop(&mut self) {
        self.guard.cleanup();
        let _ = fs::remove_dir_all(&self.base);
    }
}

#[test]
fn register_list_sync_unregister_gap_blocks_currentness() {
    let sandbox = Sandbox::new();
    fs::write(sandbox.source.join("note.txt"), b"current workspace needle\n").unwrap();

    // Register through the daemon owner: observation only, never an access grant.
    let registered = sandbox.success("--register-source-root", &[sandbox.source.as_os_str()]);
    assert!(registered.contains("\"persisted\":true"));
    assert!(registered.contains("\"access_granted\":false"));

    // List is live under the same owner; currentness is never claimed here.
    let listed = sandbox.success("--source-roots", &[]);
    assert!(listed.contains("\"configured\":1"));
    assert!(listed.contains("\"available\":1"));
    assert!(listed.contains("\"current_workspace_proven\":false"));

    // Explicit sync reconciles the one available root.
    let synced = sandbox.success("--sync-source-roots", &[]);
    assert!(synced.contains("\"complete\":true"));
    assert!(synced.contains("\"current_workspace_proven\":false"));
    assert!(synced.contains("\"qdrant_available\":false"));

    // Unregistration removes observation only; retained revisions stay readable.
    let unregistered =
        sandbox.success("--unregister-source-root", &[sandbox.source.as_os_str()]);
    assert!(unregistered.contains("\"persisted\":true"));
    assert!(unregistered.contains("\"retained_revisions_revoked\":false"));
    let listed = sandbox.success("--source-roots", &[]);
    assert!(listed.contains("\"configured\":0"));
    assert!(listed.contains("\"current_workspace_proven\":false"));
    let retained = sandbox.success("--search-root", &[OsStr::new("current")]);
    assert!(
        retained.contains("\"matches\":1"),
        "unregistration must not revoke retained revisions: {retained}"
    );

    // Re-register, then make the root disappear: a missing root is an explicit
    // gap, never an empty inventory, and it blocks currentness and sync.
    sandbox.success("--register-source-root", &[sandbox.source.as_os_str()]);
    fs::remove_dir_all(&sandbox.source).unwrap();
    let listed = sandbox.success("--source-roots", &[]);
    assert!(listed.contains("\"configured\":1"));
    assert!(listed.contains("\"unavailable\":1"));
    assert!(listed.contains("\"current_workspace_proven\":false"));
    assert!(
        listed.contains("\"event\":\"source_gap\""),
        "missing root must surface an explicit observation gap: {listed}"
    );
    let failed = sandbox.invoke("--sync-source-roots", &[]);
    assert!(!failed.status.success());
    assert!(
        String::from_utf8_lossy(&failed.stderr).contains("SOURCE_ROOTS_UNAVAILABLE"),
        "missing root must fail closed, not sync empty"
    );
    // No mass-retire: the previously retained revision still searches.
    let retained = sandbox.success("--search-root", &[OsStr::new("current")]);
    assert!(retained.contains("\"matches\":1"));
}

#[test]
fn replaced_path_is_gap_not_empty_and_never_retires_on_failed_sync() {
    let sandbox = Sandbox::new();
    fs::write(sandbox.source.join("note.txt"), b"replacement needle\n").unwrap();
    sandbox.success("--register-source-root", &[sandbox.source.as_os_str()]);
    sandbox.success("--sync-source-roots", &[]);
    let log_path = sandbox.data.join("control/source-events.log");
    let log_before = fs::read(&log_path).unwrap();

    // Replace the directory with a regular file: NotDirectory is a gap.
    fs::remove_dir_all(&sandbox.source).unwrap();
    fs::write(&sandbox.source, b"not a directory").unwrap();
    let listed = sandbox.success("--source-roots", &[]);
    assert!(listed.contains("\"configured\":1"));
    assert!(listed.contains("\"unavailable\":1"));
    assert!(listed.contains("\"current_workspace_proven\":false"));
    assert!(
        listed.contains("\"event\":\"source_gap\""),
        "replaced path must surface an explicit gap: {listed}"
    );
    let failed = sandbox.invoke("--sync-source-roots", &[]);
    assert!(!failed.status.success());
    assert!(
        String::from_utf8_lossy(&failed.stderr).contains("SOURCE_ROOTS_UNAVAILABLE"),
        "replaced path must fail closed"
    );
    assert_eq!(
        fs::read(&log_path).unwrap(),
        log_before,
        "failed sync must not retire or append"
    );
    // Explicit unregistration of the replaced locator still works.
    sandbox.success("--unregister-source-root", &[sandbox.source.as_os_str()]);
    let listed = sandbox.success("--source-roots", &[]);
    assert!(listed.contains("\"configured\":0"));
}

#[test]
fn multi_root_gap_fails_before_partial_sync_without_claiming_current() {
    let sandbox = Sandbox::new();
    let second = sandbox.base.join("second");
    fs::create_dir(&second).unwrap();
    let second = fs::canonicalize(second).unwrap();
    fs::write(sandbox.source.join("a.txt"), b"first needle\n").unwrap();
    fs::write(second.join("b.txt"), b"second needle\n").unwrap();
    sandbox.success("--register-source-root", &[sandbox.source.as_os_str()]);
    sandbox.success("--register-source-root", &[second.as_os_str()]);
    let listed = sandbox.success("--source-roots", &[]);
    assert!(listed.contains("\"configured\":2"));

    // One root disappears: the whole multi-root sync refuses to start partial work.
    fs::remove_dir_all(&second).unwrap();
    let listed = sandbox.success("--source-roots", &[]);
    assert!(listed.contains("\"unavailable\":1"));
    assert!(listed.contains("\"current_workspace_proven\":false"));
    assert!(
        listed.contains("\"event\":\"source_gap\""),
        "multi-root gap must be explicit: {listed}"
    );
    let failed = sandbox.invoke("--sync-source-roots", &[]);
    assert!(!failed.status.success());
    assert!(
        String::from_utf8_lossy(&failed.stderr).contains("SOURCE_ROOTS_UNAVAILABLE"),
        "multi-root gap must fail closed before partial sync"
    );
    assert!(
        !String::from_utf8_lossy(&failed.stdout).contains("\"current_workspace_proven\":true"),
        "no current claim on a gapped sync"
    );
}

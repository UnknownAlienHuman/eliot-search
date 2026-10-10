//! Focused native existing-only exclusion proofs for the #266 cutover.

#![cfg(windows)]

use std::collections::BTreeMap;
use std::fs;
use std::io::{BufRead, BufReader, Write};
use std::path::PathBuf;
use std::process::{Command, Output, Stdio};
use std::sync::atomic::{AtomicU64, Ordering};
use std::time::{Duration, Instant};

#[allow(dead_code)]
#[path = "../src/sealed_root_lock.rs"]
mod sealed_root_lock;

use sealed_root_lock::{SealedRootLease, SealedRootLockError};

const LOCK: &str = ".eliot-search-sealed-owner.lock";
static NEXT: AtomicU64 = AtomicU64::new(0);

struct Scratch(PathBuf);

impl Scratch {
    fn new() -> Self {
        let root = std::env::temp_dir().join(format!(
            "eliot-root-open-266-{}-{}",
            std::process::id(),
            NEXT.fetch_add(1, Ordering::Relaxed),
        ));
        fs::create_dir(&root).unwrap();
        Self(root)
    }
}

impl Drop for Scratch {
    fn drop(&mut self) {
        assert!(self.0.starts_with(std::env::temp_dir()));
        let _ = fs::remove_dir_all(&self.0);
    }
}

#[test]
fn existing_only_missing_lock_performs_no_creation() {
    let root = Scratch::new();
    assert!(SealedRootLease::acquire_existing(&root.0).is_err());
    assert_eq!(fs::read_dir(&root.0).unwrap().count(), 0);
}

#[test]
fn existing_only_lock_keeps_exact_marker_on_drop() {
    let root = Scratch::new();
    let bytes = b"retained-lock-marker";
    fs::write(root.0.join(LOCK), bytes).unwrap();
    let lease = SealedRootLease::acquire_existing(&root.0).unwrap();
    assert!(lease.is_held());
    // Windows denies a separate read handle while the exclusive byte lock is
    // held. Readback belongs after release, never through a second opener.
    assert!(matches!(
        SealedRootLease::acquire_existing(&root.0),
        Err(SealedRootLockError::AlreadyOwned)
    ));
    drop(lease);
    assert_eq!(fs::read(root.0.join(LOCK)).unwrap(), bytes);
}

#[test]
fn existing_only_lock_keeps_marker_on_unwind() {
    let root = Scratch::new();
    fs::write(root.0.join(LOCK), b"unwind-marker").unwrap();
    let result = std::panic::catch_unwind(|| {
        let _lease = SealedRootLease::acquire_existing(&root.0).unwrap();
        panic!("fixture unwind");
    });
    assert!(result.is_err());
    assert_eq!(fs::read(root.0.join(LOCK)).unwrap(), b"unwind-marker");
    assert!(SealedRootLease::acquire_existing(&root.0).is_ok());
}

#[test]
fn legacy_and_existing_exclusion_conflict_in_both_directions() {
    let root = Scratch::new();
    let legacy = SealedRootLease::acquire(&root.0).unwrap();
    assert!(matches!(
        SealedRootLease::acquire_existing(&root.0),
        Err(SealedRootLockError::AlreadyOwned)
    ));
    drop(legacy);
    let existing = SealedRootLease::acquire_existing(&root.0).unwrap();
    assert!(matches!(
        SealedRootLease::acquire(&root.0),
        Err(SealedRootLockError::AlreadyOwned)
    ));
    drop(existing);
}

const OPERATION: &str = "26600000000000000000000000000001";
const OTHER_OPERATION: &str = "26600000000000000000000000000002";

fn command(root: &std::path::Path, flag: &str, arguments: &[&str]) -> Output {
    Command::new(env!("CARGO_BIN_EXE_eliot-searchd"))
        .arg(flag)
        .arg(root)
        .args(arguments)
        .stdin(Stdio::null())
        .output()
        .unwrap()
}

fn field<'a>(text: &'a str, key: &str) -> &'a str {
    text.lines()
        .find_map(|line| line.strip_prefix(&format!("{key}=")))
        .unwrap()
}

fn parse_hex32(value: &str) -> [u8; 32] {
    assert_eq!(value.len(), 64);
    std::array::from_fn(|i| u8::from_str_radix(&value[i * 2..i * 2 + 2], 16).unwrap())
}

struct InitializedScratch {
    root: Scratch,
    namespace: [u8; 32],
}

impl InitializedScratch {
    fn new() -> Self {
        let root = Scratch::new();
        let output = command(&root.0, "--initialize-data-root", &[OPERATION]);
        assert!(
            output.status.success(),
            "{}",
            String::from_utf8_lossy(&output.stderr)
        );
        let namespace = fs::read_to_string(root.0.join("control/namespace.id")).unwrap();
        Self {
            root,
            namespace: parse_hex32(namespace.trim_end()),
        }
    }

    fn intent(&self) -> Vec<u8> {
        // Retained exact fixture inputs from the real initializer's records.
        // The fixture does not construct a capability or repeat any creates.
        let installation =
            fs::read_to_string(self.root.0.join(".eliot-search-installation.v1")).unwrap();
        let owner = fs::read_to_string(self.root.0.join(".eliot-search-owner-state-a.v1")).unwrap();
        format!(
            "ELIOT-SEARCH-INITIALIZATION-V1\nformat_version=1\noperation_id={OPERATION}\ninstallation_id={}\ninstallation_incarnation_id={}\ndata_root_id={}\nexecutable_digest={}\nnamespace_id={}\nowner_token={}\nowner_pid={}\n",
            field(&installation, "installation_id"), field(&installation, "installation_incarnation_id"),
            field(&owner, "data_root_id"), field(&owner, "executable_digest"),
            fs::read_to_string(self.root.0.join("control/namespace.id")).unwrap().trim_end(),
            field(&owner, "owner_token"), field(&owner, "owner_pid"),
        ).into_bytes()
    }
}

impl Drop for InitializedScratch {
    fn drop(&mut self) {
        search_os_secrets_windows::delete_legacy_revision_root_secret_for_test(&self.namespace)
            .unwrap();
    }
}

type TreeSnapshot = BTreeMap<PathBuf, (bool, Vec<u8>, Option<std::time::SystemTime>)>;

fn snapshot(root: &std::path::Path) -> TreeSnapshot {
    fn collect(root: &std::path::Path, directory: &std::path::Path, result: &mut TreeSnapshot) {
        for entry in fs::read_dir(directory).unwrap() {
            let entry = entry.unwrap();
            let path = entry.path();
            let metadata = fs::symlink_metadata(&path).unwrap();
            assert!(!metadata.file_type().is_symlink());
            if metadata.is_dir() {
                result.insert(
                    path.strip_prefix(root).unwrap().to_owned(),
                    (true, Vec::new(), None),
                );
                collect(root, &path, result);
            } else {
                assert!(metadata.len() < 16 * 1024);
                result.insert(
                    path.strip_prefix(root).unwrap().to_owned(),
                    (
                        false,
                        fs::read(&path).unwrap(),
                        Some(metadata.modified().unwrap()),
                    ),
                );
            }
        }
    }
    let mut result = BTreeMap::new();
    collect(root, root, &mut result);
    result
}

#[test]
fn product_reads_and_mutations_on_missing_or_empty_roots_create_nothing() {
    let scratch = Scratch::new();
    let missing = scratch.0.join("absent");
    let commands = [
        ("--health-data-root", vec![]),
        ("--list-sources", vec![]),
        ("--verify-root", vec![]),
        ("--search-root", vec!["needle"]),
        ("--read-revision", vec!["00", "0", "1"]),
        ("--index-file", vec!["unused-source"]),
        ("--index-directory", vec!["unused-directory"]),
        ("--gc-root", vec!["--dry-run"]),
        ("--gc-root", vec!["--apply"]),
        ("--source-roots", vec![]),
        ("--sync-source-roots", vec![]),
        ("--register-source-root", vec!["unused-source-root"]),
        ("--serve-data-root", vec![]),
    ];
    for root in [&missing, &scratch.0] {
        for (flag, tail) in &commands {
            let output = command(root, flag, tail);
            assert!(!output.status.success(), "{flag} unexpectedly admitted");
            assert!(output.stdout.is_empty(), "{flag} emitted a success frame");
            let error = String::from_utf8(output.stderr).unwrap();
            assert!(
                !error.contains(&root.display().to_string()),
                "{flag} disclosed root"
            );
            assert!(!missing.exists());
            assert_eq!(fs::read_dir(&scratch.0).unwrap().count(), 0);
        }
    }
}

#[test]
fn native_non_unicode_root_arguments_fail_without_parser_panic_or_creation() {
    use std::ffi::OsString;
    use std::os::windows::ffi::OsStringExt;
    let scratch = Scratch::new();
    let root = scratch
        .0
        .join(OsString::from_wide(&[0xd800, u16::from(b'x')]));
    for flag in [
        "--source-roots",
        "--health-data-root",
        "--prepare-root",
        "--serve-data-root",
    ] {
        let output = command(&root, flag, &[]);
        assert!(!output.status.success());
        assert!(output.stdout.is_empty());
        let error = String::from_utf8(output.stderr).unwrap();
        assert!(error.starts_with("{\"error\":"), "{flag}: {error}");
        assert!(!error.contains("panicked"));
        assert!(!error.contains("\\uFFFD"));
        assert_eq!(fs::read_dir(&scratch.0).unwrap().count(), 0);
    }
}

#[test]
fn explicit_initialize_replay_readonly_and_exact_recovery_preserve_objects() {
    let initialized = InitializedScratch::new();
    let root = &initialized.root.0;
    let before = snapshot(root);
    for flag in [
        "--health-data-root",
        "--list-sources",
        "--verify-root",
        "--source-roots",
    ] {
        let output = command(root, flag, &[]);
        assert!(
            output.status.success(),
            "{flag}: {}",
            String::from_utf8_lossy(&output.stderr)
        );
        assert_eq!(snapshot(root), before, "{flag} modified existing state");
    }
    let replay = command(root, "--initialize-data-root", &[OPERATION]);
    assert!(
        replay.status.success(),
        "{}",
        String::from_utf8_lossy(&replay.stderr)
    );
    assert!(
        String::from_utf8(replay.stdout)
            .unwrap()
            .contains("\"replayed\":true")
    );
    assert_eq!(snapshot(root), before);
    assert!(
        !command(root, "--initialize-data-root", &[OTHER_OPERATION])
            .status
            .success()
    );
    assert_eq!(snapshot(root), before);

    fs::write(
        root.join(".eliot-search-initialization-intent.v1"),
        initialized.intent(),
    )
    .unwrap();
    let held = snapshot(root);
    assert!(!command(root, "--health-data-root", &[]).status.success());
    assert!(
        !command(root, "--recover-initialization", &[OTHER_OPERATION])
            .status
            .success()
    );
    assert_eq!(snapshot(root), held);
    let recovery = command(root, "--recover-initialization", &[OPERATION]);
    assert!(
        recovery.status.success(),
        "{}",
        String::from_utf8_lossy(&recovery.stderr)
    );
    assert_eq!(snapshot(root), before);
}

#[test]
fn named_initialization_recovery_never_recreates_missing_state() {
    let initialized = InitializedScratch::new();
    let root = &initialized.root.0;
    fs::write(
        root.join(".eliot-search-initialization-intent.v1"),
        initialized.intent(),
    )
    .unwrap();
    fs::rename(
        root.join("control/source-events.log"),
        root.join("control/retained-log"),
    )
    .unwrap();
    let before = snapshot(root);
    for flag in ["--recover-initialization", "--initialize-data-root"] {
        assert!(!command(root, flag, &[OPERATION]).status.success());
        assert!(!root.join("control/source-events.log").exists());
        assert_eq!(snapshot(root), before);
    }
}

#[test]
fn quarantine_and_registration_debt_refuse_all_entrypoints_without_writes() {
    let initialized = InitializedScratch::new();
    let root = &initialized.root.0;
    fs::write(
        root.join("control/catalog-quarantine.marker"),
        b"retained-unknown-outcome",
    )
    .unwrap();
    let before = snapshot(root);
    for (flag, tail) in [
        ("--health-data-root", vec![]),
        ("--list-sources", vec![]),
        ("--search-root", vec!["needle"]),
        ("--index-file", vec!["unused-source"]),
        ("--index-directory", vec!["unused-source"]),
        ("--verify-root", vec![]),
        ("--read-revision", vec!["00", "0", "1"]),
        ("--retire-source", vec!["00"]),
        ("--gc-root", vec!["--dry-run"]),
        ("--gc-root", vec!["--apply"]),
        ("--source-roots", vec![]),
        ("--register-source-root", vec!["unused-source"]),
        ("--unregister-source-root", vec!["unused-source"]),
        ("--sync-source-roots", vec![]),
        ("--serve-data-root", vec![]),
        ("--prepare-root", vec![]),
        ("--repair-root", vec![]),
    ] {
        let output = command(root, flag, &tail);
        assert!(!output.status.success(), "{flag} bypassed quarantine");
        assert!(output.stdout.is_empty());
        let code = String::from_utf8(output.stderr).unwrap();
        let expected = if flag == "--repair-root" {
            "DATA_ROOT_NAMED_RECOVERY_REQUIRED"
        } else {
            "SERVICE_CATALOG_QUARANTINED"
        };
        assert!(
            code.contains(expected),
            "{flag} failed before quarantine admission: {code}"
        );
        assert_eq!(snapshot(root), before, "{flag} changed quarantine evidence");
    }
    for flag in ["--list-sources", "--index-file"] {
        let mut cmd = Command::new(env!("CARGO_BIN_EXE_eliot-searchd"));
        cmd.args(["--set", "query.limit=2"])
            .arg(flag)
            .arg(root)
            .stdin(Stdio::null());
        if flag == "--index-file" {
            cmd.arg("unused-source");
        }
        let output = cmd.output().unwrap();
        assert!(!output.status.success());
        assert!(
            String::from_utf8(output.stderr)
                .unwrap()
                .contains("SERVICE_CATALOG_QUARANTINED")
        );
        assert_eq!(snapshot(root), before);
    }
    fs::remove_file(root.join("control/catalog-quarantine.marker")).unwrap();
    for name in ["source-roots.tmp", "source-roots.bak", "source-roots.v1"] {
        fs::write(
            root.join("control").join(name),
            b"retained-incomplete-registration",
        )
        .unwrap();
        let before = snapshot(root);
        for flag in [
            "--source-roots",
            "--health-data-root",
            "--sync-source-roots",
        ] {
            assert!(!command(root, flag, &[]).status.success());
            assert_eq!(
                snapshot(root),
                before,
                "{flag} advanced owner/repaired registration"
            );
        }
        fs::remove_file(root.join("control").join(name)).unwrap();
    }
}

#[test]
fn damaged_sibling_or_replaced_control_object_cannot_advance_owner() {
    let initialized = InitializedScratch::new();
    let root = &initialized.root.0;
    for name in [
        ".eliot-search-owner-state-a.v1",
        ".eliot-search-owner-state-b.v1",
    ] {
        let original = fs::read(root.join(name)).unwrap();
        fs::write(root.join(name), b"torn-authority-record").unwrap();
        let before = snapshot(root);
        for flag in ["--health-data-root", "--sync-source-roots"] {
            assert!(!command(root, flag, &[]).status.success());
            assert_eq!(snapshot(root), before);
        }
        fs::write(root.join(name), original).unwrap();
    }
    fs::rename(
        root.join("control/namespace.id"),
        root.join("control/original-namespace.id"),
    )
    .unwrap();
    fs::write(
        root.join("control/namespace.id"),
        fs::read(root.join("control/original-namespace.id")).unwrap(),
    )
    .unwrap();
    let before = snapshot(root);
    let refused = command(root, "--health-data-root", &[]);
    assert!(!refused.status.success());
    assert!(
        String::from_utf8(refused.stderr)
            .unwrap()
            .contains("OWNER_GUARD_MISMATCH")
    );
    assert_eq!(snapshot(root), before);
}

#[test]
fn metadata_inspection_does_not_resolve_or_recreate_a_missing_credential() {
    let initialized = InitializedScratch::new();
    let root = &initialized.root.0;
    search_os_secrets_windows::delete_legacy_revision_root_secret_for_test(&initialized.namespace)
        .unwrap();
    let before = snapshot(root);
    for flag in ["--health-data-root", "--list-sources"] {
        let output = command(root, flag, &[]);
        assert!(
            output.status.success(),
            "{flag}: {}",
            String::from_utf8_lossy(&output.stderr)
        );
        assert_eq!(snapshot(root), before);
        assert!(
            search_os_secrets_windows::load_existing_legacy_revision_root_secret(
                &initialized.namespace
            )
            .unwrap()
            .is_none()
        );
    }
    assert!(!command(root, "--verify-root", &[]).status.success());
    fs::write(
        root.join(".eliot-search-initialization-intent.v1"),
        initialized.intent(),
    )
    .unwrap();
    let held = snapshot(root);
    assert!(
        !command(root, "--recover-initialization", &[OPERATION])
            .status
            .success()
    );
    assert_eq!(snapshot(root), held);
    assert!(
        search_os_secrets_windows::load_existing_legacy_revision_root_secret(
            &initialized.namespace
        )
        .unwrap()
        .is_none()
    );
}

#[test]
fn corrupt_existing_catalog_is_refused_before_owner_succession() {
    let initialized = InitializedScratch::new();
    let root = &initialized.root.0;
    fs::write(root.join("control/source-events.log"), b"malformed-history").unwrap();
    let before = snapshot(root);
    for flag in ["--health-data-root", "--sync-source-roots"] {
        assert!(!command(root, flag, &[]).status.success());
        assert_eq!(
            snapshot(root),
            before,
            "{flag} mutated owner before catalogue validation"
        );
    }
}

struct ServiceChild(
    std::process::Child,
    Option<BufReader<std::process::ChildStdout>>,
);

impl ServiceChild {
    fn wait_bounded(&mut self) -> std::process::ExitStatus {
        let deadline = Instant::now() + Duration::from_secs(20);
        loop {
            if let Some(status) = self.0.try_wait().unwrap() {
                return status;
            }
            assert!(Instant::now() < deadline, "service did not exit");
            std::thread::sleep(Duration::from_millis(10));
        }
    }
}

impl Drop for ServiceChild {
    fn drop(&mut self) {
        // A failed fixture must not leave a live root owner behind.
        let _ = self.0.kill();
        let _ = self.0.wait();
    }
}

fn live_service(root: &std::path::Path) -> ServiceChild {
    let mut child = ServiceChild(
        Command::new(env!("CARGO_BIN_EXE_eliot-searchd"))
            .arg("--serve-data-root")
            .arg(root)
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .spawn()
            .unwrap(),
        None,
    );
    let stdout = child.0.stdout.take().unwrap();
    let (send, receive) = std::sync::mpsc::channel();
    let reader = std::thread::spawn(move || {
        let mut reader = BufReader::new(stdout);
        let mut line = String::new();
        let result = reader.read_line(&mut line);
        let _ = send.send((result, line, reader));
    });
    let (read, ready, stdout) = receive.recv_timeout(Duration::from_secs(20)).unwrap();
    assert!(read.unwrap() > 0);
    assert!(ready.contains("\"event\":\"data_root_ready\""), "{ready}");
    reader.join().unwrap();
    child.1 = Some(stdout);
    child
}

#[test]
fn administrative_pages_share_the_live_owner_and_preserve_catalog_objects() {
    use std::io::Read;
    let initialized = InitializedScratch::new();
    let root = &initialized.root.0;
    let mut before = snapshot(root);
    let mut service = live_service(root);
    service
        .0
        .stdin
        .as_mut()
        .unwrap()
        .write_all(
            b"control-migration-page\ncontrol-migration-revisions\ncontrol-migration-revisions\torphans\ncontrol-migration-revisions\tpreparation-files\ncontrol-migration-directories\nshutdown\n",
        )
        .unwrap();
    drop(service.0.stdin.take());
    assert!(service.wait_bounded().success());
    let mut output = String::new();
    service
        .1
        .as_mut()
        .unwrap()
        .read_to_string(&mut output)
        .unwrap();
    for event in [
        "control_migration_page",
        "control_migration_revisions",
        "control_migration_orphans",
        "control_migration_preparation_files",
        "control_migration_directories",
        "data_root_stopped",
    ] {
        assert!(
            output.contains(&format!("\"event\":\"{event}\"")),
            "{output}"
        );
    }
    assert!(!output.contains("\"error\""), "{output}");
    drop(service);
    let mut after = snapshot(root);
    for slot in [
        ".eliot-search-owner-state-a.v1",
        ".eliot-search-owner-state-b.v1",
    ] {
        before.remove(&PathBuf::from(slot));
        after.remove(&PathBuf::from(slot));
    }
    assert_eq!(
        after, before,
        "administrative pages changed a catalog object"
    );
}

#[test]
fn clean_service_shutdown_releases_before_an_existing_inspection_reopens() {
    use std::io::Read;
    let initialized = InitializedScratch::new();
    let root = &initialized.root.0;
    let mut before = snapshot(root);
    let mut service = live_service(root);
    service
        .0
        .stdin
        .as_mut()
        .unwrap()
        .write_all(b"shutdown\n")
        .unwrap();
    drop(service.0.stdin.take());
    assert!(service.wait_bounded().success());
    let mut stopped = String::new();
    service
        .1
        .as_mut()
        .unwrap()
        .read_to_string(&mut stopped)
        .unwrap();
    assert!(stopped.contains("\"event\":\"data_root_stopped\",\"clean\":true"));
    drop(service);
    let released = snapshot(root);
    assert_ne!(released, before, "service never advanced durable ownership");
    assert!(command(root, "--health-data-root", &[]).status.success());
    assert_eq!(
        snapshot(root),
        released,
        "inspection succeeded by rewriting ownership"
    );
    let mut after = released;
    for name in [
        ".eliot-search-owner-state-a.v1",
        ".eliot-search-owner-state-b.v1",
    ] {
        before.remove(&PathBuf::from(name));
        after.remove(&PathBuf::from(name));
    }
    assert_eq!(after, before, "shutdown modified another layout object");
}

#[test]
fn live_owner_pins_layout_objects_and_abrupt_exit_stays_recovery_required() {
    let initialized = InitializedScratch::new();
    let root = &initialized.root.0;
    let mut service = live_service(root);
    for (index, name) in [
        "control",
        "revisions",
        "control/namespace.id",
        "control/source-events.log",
        ".eliot-search-installation.v1",
        ".eliot-search-owner-state-a.v1",
        ".eliot-search-owner-state-b.v1",
        ".eliot-search-owner.lock",
        ".eliot-search-sealed-owner.lock",
    ]
    .into_iter()
    .enumerate()
    {
        assert!(
            fs::rename(root.join(name), root.join(format!("replacement-{index}"))).is_err(),
            "live owner did not retain {name}",
        );
    }
    assert!(!command(root, "--health-data-root", &[]).status.success());
    service.0.kill().unwrap();
    assert!(!service.wait_bounded().success());
    drop(service);
    let retained = snapshot(root);
    for flag in ["--health-data-root", "--list-sources", "--serve-data-root"] {
        let output = command(root, flag, &[]);
        assert!(!output.status.success());
        assert!(
            String::from_utf8(output.stderr)
                .unwrap()
                .contains("OWNER_RECOVERY_QUARANTINED")
        );
        assert_eq!(
            snapshot(root),
            retained,
            "{flag} cleaned abandoned ownership"
        );
    }
}

#[test]
fn one_shot_failure_emits_only_closed_reason_and_retains_uncertain_state() {
    let initialized = InitializedScratch::new();
    let root = &initialized.root.0;
    let source = root.join("absent-private-source-locator");
    let output = command(root, "--index-directory", &[source.to_str().unwrap()]);
    assert!(!output.status.success());
    assert!(output.stdout.is_empty());
    assert_eq!(
        String::from_utf8(output.stderr).unwrap().trim(),
        "{\"error\":\"DIRECT_DIRECTORY_METADATA_ERROR\"}"
    );
    assert!(root.join("control/catalog-quarantine.marker").exists());
    let retained = snapshot(root);
    assert!(!command(root, "--health-data-root", &[]).status.success());
    assert_eq!(snapshot(root), retained);
}

fn stage_in_service(root: &std::path::Path) -> String {
    use std::io::Read;
    let mut service = live_service(root);
    service
        .0
        .stdin
        .as_mut()
        .unwrap()
        .write_all(b"control-migration-plan\t26600000-0000-0000-0000-000000000003\nshutdown\n")
        .unwrap();
    drop(service.0.stdin.take());
    let status = service.wait_bounded();
    let mut output = String::new();
    service
        .1
        .as_mut()
        .unwrap()
        .read_to_string(&mut output)
        .unwrap();
    let mut stderr = String::new();
    service
        .0
        .stderr
        .take()
        .unwrap()
        .read_to_string(&mut stderr)
        .unwrap();
    assert!(status.success(), "{output}\n{stderr}");
    assert!(!output.contains("\"error\""), "{output}\n{stderr}");
    assert!(
        output.contains("\"event\":\"data_root_stopped\",\"clean\":true"),
        "{output}"
    );
    let stages = output
        .lines()
        .filter(|line| line.starts_with("{\"event\":\"source_migration_plan_staged\","))
        .collect::<Vec<_>>();
    assert_eq!(stages.len(), 1, "{output}");
    let stage = stages[0].to_owned();
    drop(service);
    stage
}

// The renderer emits flat, quoted ASCII artifact locators. Validate their
// closed basename grammar before using any returned locator as a path.
fn staged_receipt_string<'a>(receipt: &'a str, key: &str) -> &'a str {
    let prefix = format!("\"{key}\":\"");
    let (_, value) = receipt.split_once(&prefix).expect("receipt field missing");
    value
        .split_once('"')
        .expect("receipt string unterminated")
        .0
}

fn staged_relative_paths(
    root: &std::path::Path,
    directory: &std::path::Path,
    paths: &mut std::collections::BTreeSet<PathBuf>,
) {
    for entry in fs::read_dir(directory).unwrap() {
        let path = entry.unwrap().path();
        let metadata = fs::symlink_metadata(&path).unwrap();
        assert!(!metadata.file_type().is_symlink());
        assert!(metadata.is_dir() || metadata.is_file());
        paths.insert(path.strip_prefix(root).unwrap().to_owned());
        if metadata.is_dir() {
            staged_relative_paths(root, &path, paths);
        }
    }
}

fn assert_staged_catalog_unchanged(
    root: &std::path::Path,
    before: &TreeSnapshot,
    artifacts: &[PathBuf],
) {
    let mut expected_paths = before
        .keys()
        .cloned()
        .collect::<std::collections::BTreeSet<_>>();
    expected_paths.insert(PathBuf::from("control/migration-plans"));
    expected_paths.extend(artifacts.iter().cloned());
    let mut actual_paths = std::collections::BTreeSet::new();
    staged_relative_paths(root, root, &mut actual_paths);
    assert_eq!(
        actual_paths, expected_paths,
        "unexpected staging side effects"
    );

    for (relative, expected) in before {
        if relative == &PathBuf::from(".eliot-search-owner-state-a.v1")
            || relative == &PathBuf::from(".eliot-search-owner-state-b.v1")
        {
            continue;
        }
        let path = root.join(relative);
        let metadata = fs::symlink_metadata(&path).unwrap();
        assert_eq!(metadata.is_dir(), expected.0, "{}", relative.display());
        if !expected.0 {
            assert_eq!(
                fs::read(&path).unwrap(),
                expected.1,
                "{}",
                relative.display()
            );
            assert_eq!(
                Some(metadata.modified().unwrap()),
                expected.2,
                "{} was rewritten",
                relative.display()
            );
        }
    }
}

fn assert_new_staging_receipt(first: &str) {
    for expected in [
        "\"schema\":\"eliot.source-mapping.v1\"",
        "\"target_namespace_id\":\"26600000-0000-0000-0000-000000000003\"",
        "\"digest_scheme\":\"sha256-record-chain-v1\"",
        "\"plan_location\":\"data_root\"",
        "\"staged_database_schema\":\"source-map-content-v2\"",
        "\"events\":0,",
        "\"sources\":0,",
        "\"revision_occurrences\":0,",
        "\"retained_revision_events\":0,",
        "\"retirements\":0,",
        "\"content_objects_verified\":0,",
        "\"content_bytes_verified\":0,",
        "\"all_source_events_mapped\":true",
        "\"canonical_records_materialized\":false",
        "\"source_mapping_imported_to_redb\":true",
        "\"staged_database_verified\":true",
        "\"content_manifest_bound_to_redb\":true",
        "\"content_blake3_verified\":true",
        "\"redb_imported\":false",
        "\"active_control_imported\":false",
        "\"cutover_authorized\":false",
        "\"reused\":false",
        "\"staged_database_reused\":false",
    ] {
        assert!(first.contains(expected), "missing {expected}: {first}");
    }
}

type StagedArtifactSnapshot = (Vec<u8>, std::time::SystemTime, (u32, u64, u64));

fn assert_reused_staging_artifact(
    root: &std::path::Path,
    relative: &std::path::Path,
    expected: &StagedArtifactSnapshot,
) {
    let path = root.join(relative);
    let metadata = fs::symlink_metadata(&path).unwrap();
    let file = fs::File::open(&path).unwrap();
    let native = eliot_searchd::native_file::observe(&file).unwrap();
    assert!(
        fs::read(&path).unwrap() == expected.0,
        "reused artifact bytes changed: {}",
        relative.display()
    );
    assert_eq!(
        (
            native.volume_serial,
            native.file_index,
            native.creation_time
        ),
        expected.2,
        "reused artifact identity changed: {}",
        relative.display()
    );
    // SourceMappingReadback explicitly permits native redb target recovery.
    // Mutating staging does not promise metadata write absence for that DB.
    // The immutable text artifacts must still retain their original mtime.
    if relative
        .extension()
        .is_none_or(|extension| extension != "redb")
    {
        assert_eq!(
            metadata.modified().unwrap(),
            expected.1,
            "reused text artifact was rewritten: {}",
            relative.display()
        );
    }
}

#[test]
fn native_service_stages_inactive_plan_and_reuses_exact_artifacts() {
    let initialized = InitializedScratch::new();
    let root = &initialized.root.0;
    let before = snapshot(root);
    let first = stage_in_service(root);
    assert_new_staging_receipt(&first);

    let mut artifacts = Vec::new();
    let mut staged = BTreeMap::new();
    for (key, suffix) in [
        ("plan_locator", ".source-map.v1"),
        ("staged_database_locator", ".source-map.v2.redb"),
        ("content_manifest_locator", ".source-content.v1"),
    ] {
        let locator = staged_receipt_string(&first, key);
        let basename = locator
            .strip_prefix("control/migration-plans/")
            .expect("artifact outside inactive staging directory");
        let digest = basename.strip_suffix(suffix).expect("artifact suffix");
        assert_eq!(digest.len(), 64);
        assert!(
            digest
                .bytes()
                .all(|byte| byte.is_ascii_digit() || (b'a'..=b'f').contains(&byte))
        );
        let relative = PathBuf::from(locator);
        let path = root.join(&relative);
        let metadata = fs::symlink_metadata(&path).unwrap();
        assert!(metadata.is_file() && !metadata.file_type().is_symlink());
        assert!(metadata.len() > 0, "empty staged artifact: {relative:?}");
        let file = fs::File::open(&path).unwrap();
        let native = eliot_searchd::native_file::observe(&file).unwrap();
        let observed = (
            fs::read(&path).unwrap(),
            metadata.modified().unwrap(),
            (
                native.volume_serial,
                native.file_index,
                native.creation_time,
            ),
        );
        assert!(staged.insert(relative.clone(), observed).is_none());
        artifacts.push(relative);
    }
    assert_eq!(artifacts.len(), 3);
    // The inactive database owner keeps one empty technical exclusion object.
    // It is not an evidence artifact and must not be removed on successful reuse.
    let database_locator = staged_receipt_string(&first, "staged_database_locator");
    let database_name = database_locator
        .strip_prefix("control/migration-plans/")
        .unwrap();
    let output_lock = PathBuf::from(format!("control/migration-plans/.{database_name}.lock"));
    assert!(fs::read(root.join(&output_lock)).unwrap().is_empty());
    artifacts.push(output_lock);
    assert_staged_catalog_unchanged(root, &before, &artifacts);

    let second = stage_in_service(root);
    let expected_second = first
        .replacen("\"reused\":false", "\"reused\":true", 1)
        .replacen(
            "\"staged_database_reused\":false",
            "\"staged_database_reused\":true",
            1,
        );
    assert_eq!(second, expected_second, "staging receipt changed on reuse");
    assert_staged_catalog_unchanged(root, &before, &artifacts);
    for (relative, expected) in staged {
        assert_reused_staging_artifact(root, &relative, &expected);
    }
}

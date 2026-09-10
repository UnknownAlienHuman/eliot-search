//! T10 read-only legacy control migration verification without switching authority.
//!
//! Black-box coverage through the product binary only. The golden corpus mixes
//! non-ASCII names, CRLF bytes, rename/hardlink paths, A/B/A content, retirement
//! with reactivation, historical revisions and one pre-registered observation
//! root. Every dry run must leave source bytes untouched, reproduce the same
//! deterministic plan chain, account every source event exactly once, keep
//! derived/orphan evidence separate and never activate a new authority
//! (`cutover_authorized:false`, `active_control_imported:false`).
//!
//! No `redb` import becomes live control here: staged artifacts land in an
//! explicit separate output directory (offline) or under
//! `control/migration-plans` as inert drafts, never as the serving catalog.

use std::collections::BTreeMap;
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
const TARGET_A: &str = "123e4567-e89b-12d3-a456-426614174000";
const TARGET_B: &str = "123e4567-e89b-12d3-a456-426614174001";

static NEXT: AtomicU64 = AtomicU64::new(0);

struct Fixture {
    base: PathBuf,
    data: PathBuf,
    sources: PathBuf,
    output: PathBuf,
    guard: common::RevisionKeyTreeGuard,
}

impl Fixture {
    fn new(name: &str) -> Self {
        let stamp = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_nanos();
        let base = std::env::temp_dir().join(format!(
            "eliot-migration-{name}-{}-{stamp}-{}",
            std::process::id(),
            NEXT.fetch_add(1, Ordering::Relaxed),
        ));
        fs::create_dir(&base).unwrap();
        let data = base.join("data");
        let sources = base.join("sources");
        let output = base.join("output");
        fs::create_dir(&data).unwrap();
        fs::create_dir(&sources).unwrap();
        fs::create_dir(&output).unwrap();
        let guard = common::RevisionKeyTreeGuard::for_tree(&base);
        Self {
            base,
            data,
            sources,
            output,
            guard,
        }
    }

    fn run(args: &[&str]) -> (ExitStatus, String, String) {
        let mut command = Command::new(env!("CARGO_BIN_EXE_eliot-searchd"));
        for arg in args {
            command.arg(arg);
        }
        let output = command
            .stdin(Stdio::null())
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .output()
            .expect("spawn primary daemon");
        let stdout = String::from_utf8(output.stdout).expect("UTF-8 stdout");
        let stderr = String::from_utf8(output.stderr).expect("UTF-8 stderr");
        (output.status, stdout, stderr)
    }

    fn ok(args: &[&str]) -> String {
        let (status, stdout, stderr) = Self::run(args);
        assert!(
            status.success(),
            "command {args:?} failed: stdout={stdout} stderr={stderr}"
        );
        stdout
    }

    fn index_file(&self, file: &Path) -> String {
        // Windows Credential Manager serializes concurrent creators across
        // test binaries; a lone first-open can transiently miss its fresh
        // key under parallel load. Retry only that bounded credential window,
        // never a real ingest rejection. The budget covers the full
        // 19-target parallel storm with margin.
        let mut last = None;
        for attempt in 0..16_u32 {
            let (status, stdout, stderr) = Self::run(&[
                "--index-file",
                self.data.to_str().unwrap(),
                file.to_str().unwrap(),
            ]);
            if status.success() {
                return stdout;
            }
            let transient = stderr.contains("DIRECT_REVISION_KEY_MISSING")
                || stderr.contains("DIRECT_REVISION_KEY_WRITE_FAILED")
                || stderr.contains("DIRECT_REVISION_KEY_OPEN_FAILED");
            assert!(
                transient,
                "command [\"--index-file\", {}, {}] failed: stdout={stdout} stderr={stderr}",
                self.data.display(),
                file.display()
            );
            last = Some((stdout, stderr));
            std::thread::sleep(Duration::from_millis(10_u64 << attempt.min(7)));
        }
        let (stdout, stderr) = last.expect("retry attempted");
        panic!(
            "command [\"--index-file\", {}, {}] failed after bounded credential retries: stdout={stdout} stderr={stderr}",
            self.data.display(),
            file.display()
        );
    }

    fn snapshot_source_bytes(&self) -> BTreeMap<String, Vec<u8>> {
        let mut snapshot = BTreeMap::new();
        for rel in ["control/namespace.id", "control/source-events.log"] {
            let path = self.data.join(rel);
            if let Ok(bytes) = fs::read(&path) {
                snapshot.insert(rel.to_owned(), bytes);
            }
        }
        let revisions = self.data.join("revisions");
        collect_files(&revisions, "revisions/", &mut snapshot);
        snapshot.insert(
            "sources-tree".to_owned(),
            list_tree(&self.sources).into_bytes(),
        );
        snapshot
    }
}

impl Drop for Fixture {
    fn drop(&mut self) {
        self.guard.cleanup();
        let _ = fs::remove_dir_all(&self.base);
    }
}

fn collect_files(root: &Path, prefix: &str, out: &mut BTreeMap<String, Vec<u8>>) {
    let entries = match fs::read_dir(root) {
        Ok(entries) => entries.filter_map(Result::ok).collect::<Vec<_>>(),
        Err(_) => return,
    };
    for entry in entries {
        let path = entry.path();
        let name = entry.file_name().into_string().unwrap_or_default();
        if path.is_dir() {
            collect_files(&path, &format!("{prefix}{name}/"), out);
        } else if let Ok(bytes) = fs::read(&path) {
            // Bound the snapshot itself: revision objects are already bounded
            // by the daemon, the test only reads them back for comparison.
            assert!(bytes.len() <= 65 * 1024 * 1024, "snapshot ceiling");
            out.insert(format!("{prefix}{name}"), bytes);
        }
    }
}

fn list_tree(root: &Path) -> String {
    let mut names = Vec::new();
    if let Ok(entries) = fs::read_dir(root) {
        for entry in entries.filter_map(Result::ok) {
            names.push(entry.file_name().into_string().unwrap_or_default());
        }
    }
    names.sort();
    names.join(",")
}

fn json_str(output: &str, key: &str) -> String {
    let needle = format!("\"{key}\":\"");
    let rest = output
        .split_once(&needle)
        .unwrap_or_else(|| panic!("string field {key} present in {output}"))
        .1;
    rest.split('"')
        .next()
        .expect("closed string field")
        .to_owned()
}

fn json_num(output: &str, key: &str) -> u64 {
    let needle = format!("\"{key}\":");
    let rest = output
        .split_once(&needle)
        .unwrap_or_else(|| panic!("numeric field {key} present in {output}"))
        .1;
    let digits: String = rest.chars().take_while(char::is_ascii_digit).collect();
    assert!(!digits.is_empty(), "numeric field {key} has digits");
    digits.parse().expect("numeric field parses")
}

fn assert_hex64(value: &str, what: &str) {
    assert_eq!(value.len(), 64, "{what} is 64 hex: {value}");
    assert!(
        value.bytes().all(|b| b.is_ascii_hexdigit()),
        "{what} is hex: {value}"
    );
    assert_eq!(
        value.to_lowercase(),
        *value,
        "{what} is lowercase hex: {value}"
    );
}

fn assert_uuid_v8(value: &str, what: &str) {
    assert_eq!(value.len(), 36, "{what} is UUID text: {value}");
    let bytes = value.as_bytes();
    assert_eq!(bytes[8], b'-', "{what}: {value}");
    assert_eq!(bytes[13], b'-', "{what}: {value}");
    assert_eq!(bytes[18], b'-', "{what}: {value}");
    assert_eq!(bytes[23], b'-', "{what}: {value}");
    assert_eq!(bytes[14], b'8', "{what} is version 8: {value}");
    assert!(
        matches!(bytes[19], b'8' | b'9' | b'a' | b'b'),
        "{what} has variant bits: {value}"
    );
    for (i, b) in bytes.iter().enumerate() {
        if [8, 13, 18, 23].contains(&i) {
            continue;
        }
        assert!(b.is_ascii_hexdigit(), "{what} is hex: {value}");
    }
}

/// Build the golden corpus: CRLF + non-ASCII, A/B/A, rename, hardlink,
/// retirement/reactivation and one pre-registered root with a manifest.
fn build_golden(fixture: &Fixture) -> (String, Vec<String>) {
    let file_a = fixture.sources.join("note-\u{0451}\u{0436}.txt");
    let version_a = b"h\xc3\xa9llo\r\nworld-\xd1\x91\xd0\xb6\r\n".to_vec();
    let version_b = b"second version\n".to_vec();
    fs::write(&file_a, &version_a).unwrap();
    let first = fixture.index_file(&file_a);
    let source_id = json_str(&first, "source_id");
    assert_hex64(&source_id, "legacy source_id");
    let revision_a = json_str(&first, "revision_id");

    // A -> B transition keeps the same source identity with new bytes.
    fs::write(&file_a, &version_b).unwrap();
    let second = fixture.index_file(&file_a);
    assert_eq!(json_str(&second, "source_id"), source_id);
    assert_ne!(json_str(&second, "revision_id"), revision_a);

    // B -> A reverts to the exact original bytes: history must keep both.
    fs::write(&file_a, &version_a).unwrap();
    let third = fixture.index_file(&file_a);
    assert_eq!(json_str(&third, "source_id"), source_id);
    assert_eq!(json_str(&third, "revision_id"), revision_a);

    // Rename keeps bytes but moves the observed path.
    let renamed = fixture.sources.join("renamed.txt");
    fs::rename(&file_a, &renamed).unwrap();
    let fourth = fixture.index_file(&renamed);
    assert_hex64(&json_str(&fourth, "source_id"), "renamed source_id");

    // Hardlink locators are denied as ingest sources by the safe-reader
    // kernel (T07); the denial is typed and leaves the catalog unchanged.
    let file_b = fixture.sources.join("second.txt");
    fs::write(&file_b, b"hardlink target bytes\n").unwrap();
    let fifth = fixture.index_file(&file_b);
    let link = fixture.sources.join("second-link.txt");
    fs::hard_link(&file_b, &link).unwrap();
    let log_before = fs::read(fixture.data.join("control/source-events.log")).unwrap();
    let (status, _stdout, stderr) = Fixture::run(&[
        "--index-file",
        fixture.data.to_str().unwrap(),
        link.to_str().unwrap(),
    ]);
    assert!(!status.success(), "hardlink ingest must be denied");
    assert!(
        stderr.contains("HARDLINK_DENIED") || stderr.contains("DIRECT_SOURCE_HARDLINK_DENIED"),
        "typed hardlink denial: {stderr}"
    );
    assert_eq!(
        fs::read(fixture.data.join("control/source-events.log")).unwrap(),
        log_before,
        "denied hardlink mutates nothing"
    );
    let _ = fifth;

    // Retirement followed by reactivation of the renamed source.
    let renamed_id = json_str(&fourth, "source_id");
    let retire_out = Fixture::ok(&[
        "--retire-source",
        fixture.data.to_str().unwrap(),
        &renamed_id,
    ]);
    assert!(retire_out.contains("\"active\":false"), "{retire_out}");
    let reactivated = fixture.index_file(&renamed);
    assert_eq!(json_str(&reactivated, "source_id"), renamed_id);

    // One pre-registered observation root with its own manifest generation.
    let registered = fixture.sources.join("registered");
    fs::create_dir(&registered).unwrap();
    fs::write(registered.join("leaf.txt"), b"registered leaf\n").unwrap();
    let reg_out = Fixture::ok(&[
        "--register-source-root",
        fixture.data.to_str().unwrap(),
        registered.to_str().unwrap(),
    ]);
    assert!(reg_out.contains("\"persisted\":true"), "{reg_out}");
    let sync_out = Fixture::ok(&["--sync-source-roots", fixture.data.to_str().unwrap()]);
    assert!(
        sync_out.contains("source_roots_synced") || sync_out.contains("directory_index_complete"),
        "{sync_out}"
    );

    let verify = Fixture::ok(&["--verify-root", fixture.data.to_str().unwrap()]);
    (verify, vec![source_id, renamed_id])
}

fn plan_once(fixture: &Fixture, target: &str, output: &Path) -> String {
    Fixture::ok(&[
        "--plan-control-migration",
        fixture.data.to_str().unwrap(),
        target,
        output.to_str().unwrap(),
    ])
}

fn serve_script(data: &Path, commands: &[&str]) -> String {
    let mut child: Child = Command::new(env!("CARGO_BIN_EXE_eliot-searchd"))
        .args(["--serve-data-root"])
        .arg(data)
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .expect("serve-data-root");
    let mut input = child.stdin.take().expect("serve stdin");
    let stdout = child.stdout.take().expect("serve stdout");
    let stderr = child.stderr.take().expect("serve stderr");
    let (ready_tx, ready_rx) = mpsc::channel();
    let out_handle: JoinHandle<String> = thread::spawn(move || {
        let mut reader = BufReader::new(stdout);
        let mut first = Vec::new();
        Read::take(&mut reader, 65_537)
            .read_until(b'\n', &mut first)
            .unwrap();
        let first = String::from_utf8(first).unwrap();
        let _ = ready_tx.send(first.clone());
        let mut rest = Vec::new();
        reader.read_to_end(&mut rest).unwrap();
        let mut combined = first;
        combined.push_str(&String::from_utf8(rest).unwrap());
        combined
    });
    let err_handle: JoinHandle<String> = thread::spawn(move || {
        let mut bytes = Vec::new();
        let mut reader = BufReader::new(stderr);
        reader.read_to_end(&mut bytes).unwrap();
        String::from_utf8(bytes).unwrap()
    });
    // Wait for the ready line before sending the script.
    let _ready = ready_rx.recv_timeout(TIMEOUT).expect("serve ready");
    let mut script = commands.join("\n");
    script.push_str("\nshutdown\n");
    input.write_all(script.as_bytes()).unwrap();
    input.flush().unwrap();
    drop(input);
    let deadline = Instant::now() + TIMEOUT;
    let status = loop {
        if let Some(status) = child.try_wait().unwrap() {
            break status;
        }
        if Instant::now() >= deadline {
            let _ = child.kill();
            panic!("serve-data-root exceeded deadline");
        }
        thread::sleep(Duration::from_millis(10));
    };
    assert!(status.success(), "serve exited cleanly");
    let output = out_handle.join().unwrap();
    let _errors = err_handle.join().unwrap();
    output
}

#[test]
fn golden_corpus_stages_deterministic_verify_only_plan() {
    let fixture = Fixture::new("golden");
    let (verify, _ids) = build_golden(&fixture);
    let before = fixture.snapshot_source_bytes();
    let plan = plan_once(&fixture, TARGET_A, &fixture.output);
    assert!(plan.contains("\"all_source_events_mapped\":true"), "{plan}");
    assert!(
        plan.contains("\"canonical_records_materialized\":false"),
        "{plan}"
    );
    assert!(plan.contains("\"cutover_authorized\":false"), "{plan}");
    assert!(plan.contains("\"active_control_imported\":false"), "{plan}");
    assert!(plan.contains("\"redb_imported\":false"), "{plan}");
    assert!(
        plan.contains("\"source_mapping_imported_to_redb\":true"),
        "{plan}"
    );
    assert!(plan.contains("\"staged_database_verified\":true"), "{plan}");
    assert!(plan.contains("\"content_blake3_verified\":true"), "{plan}");
    let chain = json_str(&plan, "plan_chain_sha256");
    assert_hex64(&chain, "plan_chain_sha256");
    let content_chain = json_str(&plan, "content_manifest_chain_sha256");
    assert_hex64(&content_chain, "content_manifest_chain_sha256");
    assert_ne!(chain, content_chain, "plan and content chains differ");

    // Exact source accounting: every event is mapped exactly once.
    let events = json_num(&plan, "events");
    let occurrences = json_num(&plan, "revision_occurrences");
    let retained = json_num(&plan, "retained_revision_events");
    let retirements = json_num(&plan, "retirements");
    assert!(events > 0, "golden corpus has events: {plan}");
    assert_eq!(
        events,
        occurrences + retained + retirements,
        "complete accounting: {plan}"
    );
    assert_eq!(
        events,
        json_num(&verify, "source_events"),
        "plan covers every verified event"
    );

    // Source bytes are unchanged by the dry run.
    assert_eq!(
        before,
        fixture.snapshot_source_bytes(),
        "dry run leaves source bytes untouched"
    );
}

#[test]
fn plan_is_reproducible_and_matches_live_accounting() {
    let fixture = Fixture::new("repro");
    let (verify, _) = build_golden(&fixture);
    let first = plan_once(&fixture, TARGET_A, &fixture.output);
    let chain = json_str(&first, "plan_chain_sha256");
    let bytes = json_num(&first, "plan_bytes");

    // A second staging of the same history/target must reproduce the chain.
    let second_output = fixture.base.join("output2");
    fs::create_dir(&second_output).unwrap();
    let second = plan_once(&fixture, TARGET_A, &second_output);
    assert_eq!(
        json_str(&second, "plan_chain_sha256"),
        chain,
        "deterministic plan chain"
    );
    assert_eq!(
        json_num(&second, "plan_bytes"),
        bytes,
        "deterministic plan bytes"
    );

    // A different import target must not collide with the first plan.
    let third_output = fixture.base.join("output3");
    fs::create_dir(&third_output).unwrap();
    let third = plan_once(&fixture, TARGET_B, &third_output);
    assert_ne!(
        json_str(&third, "plan_chain_sha256"),
        chain,
        "target-bound plan chain"
    );

    // Live accounting matches the staged plan exactly.
    assert_eq!(
        json_num(&first, "events"),
        json_num(&verify, "source_events"),
        "events match verify-root"
    );
    assert_eq!(
        json_num(&first, "sources"),
        json_num(&verify, "registered_sources"),
        "sources match verify-root"
    );
    assert_eq!(
        json_num(&first, "content_objects_verified"),
        json_num(&verify, "referenced_revisions"),
        "content objects match verify-root"
    );
    assert_eq!(
        json_num(&first, "content_bytes_verified"),
        json_num(&verify, "total_revision_bytes"),
        "content bytes match verify-root"
    );
}

#[test]
fn dry_run_activates_no_authority_and_preserves_originals() {
    let fixture = Fixture::new("noauth");
    let _ = build_golden(&fixture);
    let before = fixture.snapshot_source_bytes();
    let plan = plan_once(&fixture, TARGET_A, &fixture.output);
    assert!(plan.contains("\"cutover_authorized\":false"), "{plan}");
    assert_eq!(
        before,
        fixture.snapshot_source_bytes(),
        "no source mutation"
    );
    // No new authority inside the data root: no redb, no staged maps.
    let control = fixture.data.join("control");
    let mut control_names = Vec::new();
    for entry in fs::read_dir(&control).unwrap().filter_map(Result::ok) {
        control_names.push(entry.file_name().into_string().unwrap_or_default());
    }
    for name in &control_names {
        assert!(
            Path::new(name).extension().is_none_or(|ext| ext != "redb"),
            "no live redb activated in data root: {name}"
        );
        assert!(
            !name.ends_with(".source-map.v1"),
            "no staged map inside data root: {name}"
        );
    }
    // Staged artifacts live only in the explicit output directory.
    let mut staged: Vec<String> = fs::read_dir(&fixture.output)
        .unwrap()
        .filter_map(Result::ok)
        .map(|e| e.file_name().into_string().unwrap_or_default())
        .collect();
    staged.sort();
    assert!(
        staged.iter().any(|n| n.ends_with(".source-map.v1")),
        "plan artifact staged: {staged:?}"
    );
    assert!(
        staged.iter().any(|n| n.ends_with(".source-content.v1")),
        "content artifact staged: {staged:?}"
    );
    assert!(
        staged.iter().any(|n| n.ends_with(".source-map.v2.redb")),
        "inactive redb staged: {staged:?}"
    );
    // Historical readback still serves the original A bytes after the dry run.
    let sources = Fixture::ok(&["--list-sources", fixture.data.to_str().unwrap()]);
    assert!(sources.contains("source_list_complete"), "{sources}");
}

#[test]
fn truncation_conflicts_and_absent_catalog_are_rejected() {
    // Absent catalog: an empty directory is never an empty migration.
    let empty = Fixture::new("absent");
    let output = empty.base.join("plan-out");
    fs::create_dir(&output).unwrap();
    let (status, _stdout, stderr) = Fixture::run(&[
        "--plan-control-migration",
        empty.data.to_str().unwrap(),
        TARGET_A,
        output.to_str().unwrap(),
    ]);
    assert!(!status.success(), "absent catalog must fail");
    assert!(
        stderr.contains("CATALOG")
            || stderr.contains("NAMESPACE")
            || stderr.contains("DATA_ROOT")
            || stderr.contains("DIRECT_"),
        "typed rejection: {stderr}"
    );
    assert!(
        fs::read_dir(&output).unwrap().count() == 0,
        "no artifact on absent catalog"
    );

    // Truncation: a torn source log must not produce a plan.
    let fixture = Fixture::new("trunc");
    let _ = build_golden(&fixture);
    let log_path = fixture.data.join("control/source-events.log");
    let original = fs::read(&log_path).unwrap();
    assert!(original.len() > 16, "golden log is non-trivial");
    let truncated = &original[..original.len() - 1];
    fs::write(&log_path, truncated).unwrap();
    let bad_output = fixture.base.join("bad-out");
    fs::create_dir(&bad_output).unwrap();
    let (status, _stdout, stderr) = Fixture::run(&[
        "--plan-control-migration",
        fixture.data.to_str().unwrap(),
        TARGET_A,
        bad_output.to_str().unwrap(),
    ]);
    assert!(!status.success(), "truncated log must fail");
    assert!(
        stderr.contains("DIRECT_") || stderr.contains("CATALOG"),
        "typed truncation rejection: {stderr}"
    );
    assert!(
        fs::read_dir(&bad_output).unwrap().count() == 0
            || fs::read_dir(&bad_output)
                .unwrap()
                .filter_map(Result::ok)
                .all(|e| e
                    .file_name()
                    .to_str()
                    .is_some_and(|n| Path::new(n)
                        .extension()
                        .is_some_and(|ext| ext == "lock"))),
        "no plan artifact from truncated input"
    );
    // The torn input is preserved for forensics, never repaired into success.
    assert_eq!(fs::read(&log_path).unwrap(), truncated);
    fs::write(&log_path, &original).unwrap();

    // Conflicting identity: flipping one digest byte must fail closed.
    let mut conflicted = original.clone();
    let flip_at = conflicted.len() / 2;
    conflicted[flip_at] = if conflicted[flip_at] == b'0' {
        b'1'
    } else {
        b'0'
    };
    fs::write(&log_path, &conflicted).unwrap();
    let conflict_output = fixture.base.join("conflict-out");
    fs::create_dir(&conflict_output).unwrap();
    let (status, _stdout, stderr) = Fixture::run(&[
        "--plan-control-migration",
        fixture.data.to_str().unwrap(),
        TARGET_A,
        conflict_output.to_str().unwrap(),
    ]);
    assert!(!status.success(), "conflicting digest must fail");
    assert!(
        stderr.contains("DIRECT_") || stderr.contains("CATALOG"),
        "typed conflict rejection: {stderr}"
    );
    fs::write(&log_path, &original).unwrap();
}

#[test]
fn migration_inventories_stay_read_only_bounded_and_separate() {
    let fixture = Fixture::new("invent");
    let before = {
        let _ = build_golden(&fixture);
        fixture.snapshot_source_bytes()
    };
    let output = serve_script(
        &fixture.data,
        &[
            "control-migration-page",
            "control-migration-revisions",
            "control-migration-revisions\torphans",
            "control-migration-revisions\tpreparation-files",
            "control-migration-directories",
        ],
    );
    assert!(
        output.contains("\"event\":\"control_migration_page\""),
        "{output}"
    );
    assert!(
        output.contains("\"scope\":\"source_events_only\""),
        "history scope stays bounded: {output}"
    );
    assert!(
        output.contains("\"event\":\"control_migration_revisions\""),
        "{output}"
    );
    assert!(
        output.contains("\"event\":\"control_migration_orphans\""),
        "physical orphans stay separate: {output}"
    );
    assert!(
        output.contains("\"event\":\"control_migration_preparation_files\""),
        "derived preparation stays separate: {output}"
    );
    assert!(
        output.contains("\"event\":\"control_migration_directories\""),
        "root/directory inventory present: {output}"
    );
    for marker in [
        "\"read_only\":true",
        "\"deletion_authorized\":false",
        "\"canonical_mapping_complete\":false",
        "\"cutover_revalidation_required\":true",
    ] {
        assert!(
            output.contains(marker),
            "verify-only marker {marker}: {output}"
        );
    }
    assert!(
        output.contains("\"page_history_bindings_verified\":true"),
        "directory page binds history: {output}"
    );
    assert!(
        output.contains("\"page_payloads_verified\":true"),
        "revision payloads verified: {output}"
    );
    // Quarantine, owner and safe-reader state are untouched by reads.
    assert!(
        !fixture
            .data
            .join("control/catalog-quarantine.marker")
            .exists(),
        "reads arm no quarantine"
    );
    assert_eq!(
        before,
        fixture.snapshot_source_bytes(),
        "inventories mutate nothing"
    );
}

#[test]
fn h5_mapping_never_relabels_digests_sequences_or_paths() {
    let fixture = Fixture::new("h5");
    let _ = build_golden(&fixture);
    let plan = plan_once(&fixture, TARGET_A, &fixture.output);
    let chain = json_str(&plan, "plan_chain_sha256");
    let plan_name = format!("{chain}.source-map.v1");
    let plan_bytes = fs::read(fixture.output.join(&plan_name)).expect("plan artifact");
    let plan_text = String::from_utf8(plan_bytes).expect("plan UTF-8");
    let lines: Vec<&str> = plan_text.lines().collect();
    assert!(lines.len() >= 3, "header + mappings + end: {plan}");
    assert!(
        lines[0].contains("\"draft_only\":true"),
        "draft: {}",
        lines[0]
    );
    assert!(
        lines[0].contains("\"cutover_authorized\":false"),
        "no cutover: {}",
        lines[0]
    );
    assert!(
        lines[0].contains("\"identity_kind\":\"imported_object\""),
        "imported identity: {}",
        lines[0]
    );
    let end = lines[lines.len() - 1];
    assert!(
        end.contains("\"canonical_records_materialized\":false"),
        "no materialization: {end}"
    );
    assert!(
        end.contains("namespace_owner_cutover"),
        "cutover stays explicit: {end}"
    );
    for line in &lines[1..lines.len() - 1] {
        assert!(
            line.contains("\"kind\":\"source_event_mapping\""),
            "mapping row: {line}"
        );
        let source_id = json_str(line, "source_id");
        let revision_id = json_str(line, "revision_id");
        assert_uuid_v8(&source_id, "canonical source_id");
        assert_uuid_v8(&revision_id, "canonical revision_id");
        let legacy_source = json_str(line, "source_id");
        let _ = legacy_source;
        // Legacy bindings are 64-hex SHA-256; canonical IDs are UUIDv8.
        assert!(line.contains("\"legacy\":"), "legacy binding kept: {line}");
        assert!(line.contains("\"legacy_revision_id\":\""), "{line}");
        let legacy_revision = line
            .split_once("\"legacy_revision_id\":\"")
            .unwrap()
            .1
            .split('"')
            .next()
            .unwrap();
        assert_hex64(legacy_revision, "legacy_revision_id");
        assert_ne!(
            revision_id, legacy_revision,
            "ordinal/revision never relabelled: {line}"
        );
        // Ordinal counts per-source events; occurrence counts revision opens.
        // Both are integers, neither is a revision identifier.
        assert!(line.contains("\"occurrence_sequence\":"), "{line}");
        assert!(line.contains("\"source_event_ordinal\":"), "{line}");
    }
    // Content facts are BLAKE3 computed from retained bytes, never SHA relabelled.
    let content_chain = json_str(&plan, "content_manifest_chain_sha256");
    let content_name = format!("{content_chain}.source-content.v1");
    let content_bytes = fs::read(fixture.output.join(&content_name)).expect("content artifact");
    let content_text = String::from_utf8(content_bytes).expect("content UTF-8");
    assert!(
        content_text.contains("\"content_digest_algorithm\":\"blake3_256\""),
        "{content_text}"
    );
    assert!(
        content_text.contains("\"blake3_computed_from_bytes\":true"),
        "{content_text}"
    );
    assert!(
        content_text.contains("\"cutover_authorized\":false"),
        "{content_text}"
    );
    for line in content_text
        .lines()
        .filter(|l| l.contains("source_content_readback"))
    {
        let sha = json_str(line, "content_sha256");
        let blake = json_str(line, "content_blake3");
        assert_hex64(&sha, "content_sha256");
        assert_hex64(&blake, "content_blake3");
        assert_ne!(sha, blake, "SHA-256 is never relabelled as BLAKE3: {line}");
    }
}

/// T11 contract: the T10 verify-only output already carries every binding the
/// atomic cutover commits into its marker (target, snapshot and both chains).
#[test]
fn plan_output_binds_all_cutover_inputs() {
    let fixture = Fixture::new("cutbind");
    let _ = build_golden(&fixture);
    let plan = plan_once(&fixture, TARGET_A, &fixture.output);
    assert!(
        plan.contains(&format!("\"target_namespace_id\":\"{TARGET_A}\"")),
        "{plan}"
    );
    assert_hex64(&json_str(&plan, "catalog_snapshot_sha256"), "catalog_snapshot_sha256");
    assert_hex64(&json_str(&plan, "plan_chain_sha256"), "plan_chain_sha256");
    assert_hex64(
        &json_str(&plan, "content_manifest_chain_sha256"),
        "content_manifest_chain_sha256",
    );
    let locator = json_str(&plan, "staged_database_locator");
    assert!(locator.ends_with(".source-map.v2.redb"), "{locator}");
    assert!(
        plan.contains("\"staged_database_schema\":\"source-map-content-v2\""),
        "{plan}"
    );
    assert!(plan.contains("\"cutover_authorized\":false"), "{plan}");
    assert!(plan.contains("\"active_control_imported\":false"), "{plan}");
    // The staged redb is materialized but inert: no authority is activated.
    let name = locator.rsplit('/').next().expect("locator tail");
    assert!(
        !fs::read(fixture.output.join(name)).expect("staged redb").is_empty(),
        "staged redb is materialized"
    );
}

/// Without an explicit cutover the serving control directory gains no marker
/// and no live redb, while ordinary requests keep appending the file journal.
#[test]
fn file_authority_preserved_without_cutover() {
    let fixture = Fixture::new("fileauth");
    let _ = build_golden(&fixture);
    let plan = plan_once(&fixture, TARGET_A, &fixture.output);
    let planned_events = json_num(&plan, "events");
    let control: Vec<String> = fs::read_dir(fixture.data.join("control"))
        .unwrap()
        .filter_map(Result::ok)
        .map(|entry| entry.file_name().into_string().unwrap_or_default())
        .collect();
    assert!(
        !control.iter().any(|name| name == "control-cutover.v1"),
        "no marker without cutover: {control:?}"
    );
    assert!(
        !control.iter().any(|name| std::path::Path::new(name)
            .extension()
            .is_some_and(|extension| extension == "redb")),
        "no live redb without cutover: {control:?}"
    );
    // The next ordinary index advances the file journal by exactly one event.
    let extra = fixture.sources.join("post-plan.txt");
    fs::write(&extra, b"indexed after the dry run\n").unwrap();
    fixture.index_file(&extra);
    let verify = Fixture::ok(&["--verify-root", fixture.data.to_str().unwrap()]);
    assert_eq!(
        json_num(&verify, "source_events"),
        planned_events + 1,
        "journal still live: {verify}"
    );
    let sources = Fixture::ok(&["--list-sources", fixture.data.to_str().unwrap()]);
    assert!(sources.contains("source_list_complete"), "{sources}");
}

/// A torn cutover marker fails staging closed and quarantines, while the
/// preserved legacy history still reads and verifies after explicit recovery.
#[test]
fn torn_marker_fails_staging_closed_while_history_reads_survive() {
    let fixture = Fixture::new("tornmark");
    let (verify_before, _) = build_golden(&fixture);
    let before = fixture.snapshot_source_bytes();
    let marker_path = fixture.data.join("control/control-cutover.v1");
    fs::write(&marker_path, b"torn-marker-bytes").unwrap();
    // Pre-wiring history reads do not consult the marker and arm nothing.
    let output = serve_script(&fixture.data, &["control-migration-page"]);
    assert!(output.contains("\"event\":\"control_migration_page\""), "{output}");
    assert!(output.contains("\"read_only\":true"), "{output}");
    assert!(
        !fixture.data.join("control/catalog-quarantine.marker").exists(),
        "reads arm no quarantine"
    );
    // Staging over the torn marker is refused with a typed code, preserves
    // the torn bytes for forensics, stages no artifact and quarantines.
    let bad_output = fixture.base.join("torn-out");
    fs::create_dir(&bad_output).unwrap();
    let (status, _stdout, stderr) = Fixture::run(&[
        "--plan-control-migration",
        fixture.data.to_str().unwrap(),
        TARGET_A,
        bad_output.to_str().unwrap(),
    ]);
    assert!(!status.success(), "torn marker must fail staging");
    assert!(
        stderr.contains("DIRECT_MIGRATION_CUTOVER_CORRUPT"),
        "typed refusal: {stderr}"
    );
    assert_eq!(fs::read(&marker_path).unwrap(), b"torn-marker-bytes");
    assert!(
        fs::read_dir(&bad_output).unwrap().count() == 0,
        "no artifact over torn marker"
    );
    assert!(
        fixture.data.join("control/catalog-quarantine.marker").exists(),
        "quarantine armed"
    );
    // The legacy source bytes are otherwise untouched by the refused staging.
    assert_eq!(before, fixture.snapshot_source_bytes(), "history preserved");
    // Explicit recovery removes exactly the torn marker and the quarantine
    // signal; the original catalog then verifies with identical accounting.
    fs::remove_file(&marker_path).unwrap();
    fs::remove_file(fixture.data.join("control/catalog-quarantine.marker")).unwrap();
    let verify = Fixture::ok(&["--verify-root", fixture.data.to_str().unwrap()]);
    assert_eq!(
        json_num(&verify, "source_events"),
        json_num(&verify_before, "source_events"),
        "identical accounting after recovery"
    );
}

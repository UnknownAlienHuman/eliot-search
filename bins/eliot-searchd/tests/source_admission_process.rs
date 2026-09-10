//! T13 canonical admission process regressions: primary ingestion consumes
//! admitted source identities and registry state instead of an independent
//! development catalog.
//!
//! Every test drives the real `eliot-searchd` binary (`--index-file`,
//! `--index-directory`, `--list-sources`, `--verify-root`). Denied sources
//! never publish, equal-content files stay distinct, rename preserves stable
//! identity, replacement is a new transition, hardlink/escape/VCS/system
//! faults fail closed, and restart preserves memberships, revision
//! occurrences and lineage bindings. No mock, no fabricated receipt.

use std::fs;
use std::io::Read;
use std::path::{Path, PathBuf};
use std::process::{Command, ExitStatus, Stdio};
use std::sync::atomic::{AtomicU64, Ordering};
use std::thread;
use std::time::{Duration, Instant, SystemTime, UNIX_EPOCH};

#[allow(dead_code)]
mod common;

static NEXT: AtomicU64 = AtomicU64::new(0);

struct Fixture {
    base: PathBuf,
    data: PathBuf,
    files: PathBuf,
    guard: common::RevisionKeyTreeGuard,
}

impl Fixture {
    fn new() -> Self {
        let stamp = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_nanos();
        let base = std::env::temp_dir().join(format!(
            "eliot-source-admission-{}-{stamp}-{}",
            std::process::id(),
            NEXT.fetch_add(1, Ordering::Relaxed)
        ));
        let data = base.join("data");
        let files = base.join("files");
        fs::create_dir_all(&data).unwrap();
        fs::create_dir_all(&files).unwrap();
        let guard = common::RevisionKeyTreeGuard::for_tree(&base);
        Self {
            base,
            data,
            files,
            guard,
        }
    }

    fn run(args: &[&str]) -> (ExitStatus, String, String) {
        let mut attempt = 0_u32;
        loop {
            let outcome = Self::run_once(args);
            if outcome.0.success() || attempt >= 7 {
                return outcome;
            }
            let noisy = outcome.2.contains("DIRECT_REVISION_KEY_");
            if !noisy {
                return outcome;
            }
            std::thread::sleep(Duration::from_millis(25_u64 << attempt.min(5)));
            attempt += 1;
        }
    }

    fn run_once(args: &[&str]) -> (ExitStatus, String, String) {
        let mut child = Command::new(env!("CARGO_BIN_EXE_eliot-searchd"))
            .args(args)
            .stdin(Stdio::null())
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .spawn()
            .expect("primary daemon");
        let stdout = child.stdout.take().unwrap();
        let stderr = child.stderr.take().unwrap();
        let out = thread::spawn(move || read_output(stdout));
        let err = thread::spawn(move || read_output(stderr));
        let deadline = Instant::now() + Duration::from_secs(30);
        let status = loop {
            if let Some(status) = child.try_wait().unwrap() {
                break status;
            }
            if Instant::now() >= deadline {
                let _ = child.kill();
                let _ = child.wait();
                panic!("primary daemon exceeded the test deadline");
            }
            thread::sleep(Duration::from_millis(10));
        };
        (status, out.join().unwrap(), err.join().unwrap())
    }

    fn ok(args: &[&str]) -> String {
        let (status, stdout, stderr) = Self::run(args);
        assert!(
            status.success(),
            "status={status} stdout={stdout} stderr={stderr}"
        );
        stdout
    }

    fn err(args: &[&str]) -> String {
        let (status, stdout, stderr) = Self::run(args);
        assert!(
            !status.success(),
            "expected failure stdout={stdout} stderr={stderr}"
        );
        assert!(
            stdout.is_empty(),
            "failures must not emit products: {stdout}"
        );
        stderr
    }

    fn write(&self, name: &str, bytes: &[u8]) -> PathBuf {
        let path = self.files.join(name);
        if let Some(parent) = path.parent() {
            fs::create_dir_all(parent).unwrap();
        }
        fs::write(&path, bytes).unwrap();
        path
    }

    fn source_events(&self) -> usize {
        let output = Self::ok(&["--verify-root", self.data.to_str().unwrap()]);
        parse_event_usize(&output, "\"source_events\":")
    }

    fn listed_sources(&self) -> Vec<(String, String)> {
        let output = Self::ok(&["--list-sources", self.data.to_str().unwrap()]);
        output
            .lines()
            .filter(|line| line.contains("\"event\":\"source\","))
            .map(|line| {
                (
                    parse_event_string(line, "\"source_id\":\""),
                    parse_event_string(line, "\"revision_id\":\""),
                )
            })
            .collect()
    }
}

impl Drop for Fixture {
    fn drop(&mut self) {
        self.guard.cleanup();
        let _ = fs::remove_dir_all(&self.base);
    }
}

fn read_output(reader: impl Read) -> String {
    const MAX_OUTPUT: u64 = 1024 * 1024;
    let mut bytes = Vec::new();
    reader.take(MAX_OUTPUT + 1).read_to_end(&mut bytes).unwrap();
    assert!(
        bytes.len() <= usize::try_from(MAX_OUTPUT).expect("ceiling fits"),
        "test output ceiling exceeded"
    );
    String::from_utf8(bytes).expect("UTF-8 output")
}

fn parse_event_usize(output: &str, key: &str) -> usize {
    let start = output
        .find(key)
        .unwrap_or_else(|| panic!("missing {key} in {output}"));
    let rest = &output[start + key.len()..];
    let end = rest
        .find(|character: char| !character.is_ascii_digit())
        .unwrap_or(rest.len());
    rest[..end].parse().expect("usize event field")
}

fn parse_event_string(line: &str, key: &str) -> String {
    let start = line
        .find(key)
        .unwrap_or_else(|| panic!("missing {key} in {line}"));
    let rest = &line[start + key.len()..];
    let end = rest.find('"').expect("closing quote");
    rest[..end].to_owned()
}

fn symlink_file(target: &Path, link: &Path) {
    #[cfg(unix)]
    std::os::unix::fs::symlink(target, link).unwrap();
    #[cfg(windows)]
    std::os::windows::fs::symlink_file(target, link).unwrap();
}

#[test]
fn denied_sources_never_publish_and_regular_sources_still_admit() {
    let fixture = Fixture::new();
    // Baseline deny-by-default: credential, secret-candidate, generated,
    // vendor, VCS/system, cache and build classes never reach CAS.
    for (name, bytes) in [
        ("id_rsa", b"candidate bytes".as_slice()),
        ("tls.pem", b"candidate bytes".as_slice()),
        ("secret_notes.txt", b"candidate bytes".as_slice()),
        ("app.generated.js", b"candidate bytes".as_slice()),
        ("vendor/lib.js", b"candidate bytes".as_slice()),
    ] {
        let path = fixture.write(name, bytes);
        let stderr = Fixture::err(&[
            "--index-file",
            fixture.data.to_str().unwrap(),
            path.to_str().unwrap(),
        ]);
        assert!(
            stderr.contains("SOURCE_ADMISSION_DENIED")
                || stderr.contains("SENSITIVE_SOURCE_DENIED"),
            "name={name} stderr={stderr}"
        );
        assert!(!stderr.contains("source_indexed"));
    }
    assert!(fixture.listed_sources().is_empty());
    assert_eq!(fixture.source_events(), 0);
    // A regular source in the same root still admits afterwards.
    let allowed = fixture.write("notes.txt", b"allowed bytes");
    let output = Fixture::ok(&[
        "--index-file",
        fixture.data.to_str().unwrap(),
        allowed.to_str().unwrap(),
    ]);
    assert!(output.contains("\"event\":\"source_indexed\""), "{output}");
    assert_eq!(fixture.listed_sources().len(), 1);
    assert_eq!(fixture.source_events(), 1);
}

#[test]
fn empty_source_is_denied_without_publication() {
    let fixture = Fixture::new();
    let path = fixture.write("empty.txt", b"");
    let stderr = Fixture::err(&[
        "--index-file",
        fixture.data.to_str().unwrap(),
        path.to_str().unwrap(),
    ]);
    assert!(
        stderr.contains("SOURCE_ADMISSION_DENIED"),
        "stderr={stderr}"
    );
    assert!(fixture.listed_sources().is_empty());
    assert_eq!(fixture.source_events(), 0);
}

#[test]
fn equal_content_files_keep_distinct_stable_identities() {
    let fixture = Fixture::new();
    let first = fixture.write("first.txt", b"same bytes");
    let second = fixture.write("second.txt", b"same bytes");
    let one = Fixture::ok(&[
        "--index-file",
        fixture.data.to_str().unwrap(),
        first.to_str().unwrap(),
    ]);
    let two = Fixture::ok(&[
        "--index-file",
        fixture.data.to_str().unwrap(),
        second.to_str().unwrap(),
    ]);
    let first_id = parse_event_string(&one, "\"source_id\":\"");
    let second_id = parse_event_string(&two, "\"source_id\":\"");
    assert_ne!(first_id, second_id);
    let first_revision = parse_event_string(&one, "\"revision_id\":\"");
    let second_revision = parse_event_string(&two, "\"revision_id\":\"");
    assert_ne!(first_revision, second_revision);
    assert_eq!(fixture.listed_sources().len(), 2);
    let verify = Fixture::ok(&["--verify-root", fixture.data.to_str().unwrap()]);
    assert!(verify.contains("\"referenced_revisions\":2"), "{verify}");
}

#[test]
fn rename_preserves_stable_identity_with_new_locator_binding() {
    let fixture = Fixture::new();
    let before = fixture.write("before.txt", b"renamed bytes");
    let one = Fixture::ok(&[
        "--index-file",
        fixture.data.to_str().unwrap(),
        before.to_str().unwrap(),
    ]);
    let first_id = parse_event_string(&one, "\"source_id\":\"");
    let after = fixture.files.join("after.txt");
    fs::rename(&before, &after).unwrap();
    let two = Fixture::ok(&[
        "--index-file",
        fixture.data.to_str().unwrap(),
        after.to_str().unwrap(),
    ]);
    let second_id = parse_event_string(&two, "\"source_id\":\"");
    assert_eq!(second_id, first_id);
    assert!(two.contains("\"changed\":true"), "{two}");
    // The old locator no longer resolves; no bytes may be returned.
    let stderr = Fixture::err(&[
        "--index-file",
        fixture.data.to_str().unwrap(),
        before.to_str().unwrap(),
    ]);
    assert!(!stderr.contains("source_indexed"), "{stderr}");
}

#[test]
fn replacement_at_same_path_is_a_new_transition() {
    let fixture = Fixture::new();
    let path = fixture.write("victim.txt", b"original bytes");
    let one = Fixture::ok(&[
        "--index-file",
        fixture.data.to_str().unwrap(),
        path.to_str().unwrap(),
    ]);
    let first_revision = parse_event_string(&one, "\"revision_id\":\"");
    fs::write(&path, b"substituted bytes").unwrap();
    let two = Fixture::ok(&[
        "--index-file",
        fixture.data.to_str().unwrap(),
        path.to_str().unwrap(),
    ]);
    let second_revision = parse_event_string(&two, "\"revision_id\":\"");
    assert_ne!(second_revision, first_revision);
    assert!(two.contains("\"changed\":true"), "{two}");
}

#[test]
fn hardlink_and_symlink_are_denied_without_publication() {
    let fixture = Fixture::new();
    let outside_dir = std::env::temp_dir().join(format!(
        "eliot-admission-out-{}-{}",
        std::process::id(),
        SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_nanos()
    ));
    fs::create_dir_all(&outside_dir).unwrap();
    let outside = outside_dir.join("shared.txt");
    fs::write(&outside, b"shared bytes").unwrap();
    let alias = fixture.files.join("alias.txt");
    fs::hard_link(&outside, &alias).unwrap();
    let stderr = Fixture::err(&[
        "--index-file",
        fixture.data.to_str().unwrap(),
        alias.to_str().unwrap(),
    ]);
    assert!(
        stderr.contains("HARDLINK_DENIED") || stderr.contains("SOURCE_"),
        "stderr={stderr}"
    );
    let _ = fs::remove_file(&alias);
    // Symlink final objects are denied before any open.
    let target = fixture.write("target.txt", b"target bytes");
    let link = fixture.files.join("link.txt");
    symlink_file(&target, &link);
    let stderr = Fixture::err(&[
        "--index-file",
        fixture.data.to_str().unwrap(),
        link.to_str().unwrap(),
    ]);
    assert!(stderr.contains("LINK_DENIED"), "stderr={stderr}");
    let _ = fs::remove_file(&link);
    assert!(fixture.listed_sources().is_empty());
    let _ = fs::remove_dir_all(&outside_dir);
}

#[test]
fn vcs_and_build_locations_are_denied_without_publication() {
    let fixture = Fixture::new();
    for name in [".git/config", "target/debug/app.txt", "__pycache__/mod.txt"] {
        let path = fixture.write(name, b"location bytes");
        let stderr = Fixture::err(&[
            "--index-file",
            fixture.data.to_str().unwrap(),
            path.to_str().unwrap(),
        ]);
        assert!(
            stderr.contains("SOURCE_ADMISSION_DENIED"),
            "name={name} stderr={stderr}"
        );
    }
    assert!(fixture.listed_sources().is_empty());
    assert_eq!(fixture.source_events(), 0);
}

#[test]
fn restart_preserves_memberships_revision_occurrences_and_lineage() {
    let fixture = Fixture::new();
    let path = fixture.write("notes.txt", b"lineage bytes");
    let one = Fixture::ok(&[
        "--index-file",
        fixture.data.to_str().unwrap(),
        path.to_str().unwrap(),
    ]);
    let first_id = parse_event_string(&one, "\"source_id\":\"");
    let first_revision = parse_event_string(&one, "\"revision_id\":\"");
    // A new process replays the same log: memberships, occurrences and
    // lineage bindings survive without re-admission side effects.
    let listed = fixture.listed_sources();
    assert_eq!(listed.len(), 1);
    assert_eq!(listed[0].0, first_id);
    assert_eq!(listed[0].1, first_revision);
    assert_eq!(fixture.source_events(), 1);
    // Re-indexing unchanged bytes is a no-op without a new event.
    let two = Fixture::ok(&[
        "--index-file",
        fixture.data.to_str().unwrap(),
        path.to_str().unwrap(),
    ]);
    assert!(two.contains("\"changed\":false"), "{two}");
    assert_eq!(fixture.source_events(), 1);
}

#[test]
fn directory_batch_denies_without_partial_publication() {
    let fixture = Fixture::new();
    fixture.write("good.txt", b"good bytes");
    fixture.write("id_rsa", b"candidate bytes");
    let before = fixture.source_events();
    assert_eq!(before, 0);
    let stderr = Fixture::err(&[
        "--index-directory",
        fixture.data.to_str().unwrap(),
        fixture.files.to_str().unwrap(),
    ]);
    assert!(
        stderr.contains("SOURCE_ADMISSION_DENIED"),
        "stderr={stderr}"
    );
    // No partial batch member may publish when one member is denied.
    assert!(fixture.listed_sources().is_empty());
    assert_eq!(fixture.source_events(), 0);
}

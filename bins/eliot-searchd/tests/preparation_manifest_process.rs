//! Canonical preparation binding: durable manifests bound to source
//! revision, representation, materializer/unitizer profiles and exact digest
//! algorithms. No `ReceiptRef` substitution; real provenance only.

use std::fs;
use std::io::Read;
use std::path::PathBuf;
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
    source: PathBuf,
    guard: common::RevisionKeyGuard,
}

impl Fixture {
    fn new() -> Self {
        let stamp = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_nanos();
        let base = std::env::temp_dir().join(format!(
            "eliot-prep-manifest-{}-{stamp}-{}",
            std::process::id(),
            NEXT.fetch_add(1, Ordering::Relaxed)
        ));
        let data = base.join("data");
        let source = base.join("source.txt");
        fs::create_dir_all(&data).unwrap();
        let guard = common::RevisionKeyGuard::for_data_root(&data);
        Self {
            base,
            data,
            source,
            guard,
        }
    }

    fn run(args: &[&str]) -> (ExitStatus, String, String) {
        Self::run_with_stdin(args, Stdio::null())
    }

    fn run_with_stdin(args: &[&str], input: Stdio) -> (ExitStatus, String, String) {
        let mut child = Command::new(env!("CARGO_BIN_EXE_eliot-searchd"))
            .args(args)
            .stdin(input)
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

    fn index(&self, bytes: &[u8]) -> String {
        fs::write(&self.source, bytes).unwrap();
        let output = Self::ok(&[
            "--index-file",
            self.data.to_str().unwrap(),
            self.source.to_str().unwrap(),
        ]);
        self.guard.refresh();
        output
    }

    fn search(&self, query: &str) -> String {
        Self::ok(&["--search-root", self.data.to_str().unwrap(), query])
    }

    fn prepare_revision(&self, revision: &str) -> String {
        Self::ok(&["--prepare-revision", self.data.to_str().unwrap(), revision])
    }

    fn count_preparation_files(&self) -> usize {
        let base = self.data.join("preparation");
        if !base.exists() {
            return 0;
        }
        let mut count = 0;
        let mut stack = vec![base];
        while let Some(dir) = stack.pop() {
            let entries = fs::read_dir(&dir).unwrap();
            for entry in entries {
                let entry = entry.unwrap();
                let path = entry.path();
                if path.is_dir() {
                    stack.push(path);
                } else {
                    count += 1;
                }
            }
        }
        count
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

fn field<'a>(output: &'a str, key: &str) -> &'a str {
    let needle = format!("\"{key}\":\"");
    output
        .split_once(&needle)
        .expect("field")
        .1
        .split('"')
        .next()
        .unwrap()
}

fn matches(output: &str) -> Vec<&str> {
    output
        .lines()
        .filter(|line| line.contains("\"event\":\"match\""))
        .collect()
}

fn hex_len(value: &str, len: usize) -> bool {
    value.len() == len
        && value
            .bytes()
            .all(|b| b.is_ascii_digit() || (b'a'..=b'f').contains(&b))
}

#[test]
fn preparation_binds_canonical_representation_and_profiles() {
    let fixture = Fixture::new();
    let first = fixture.index(b"canonical binding needle");
    let revision = field(&first, "revision_id").to_owned();
    let output = fixture.prepare_revision(&revision);
    assert!(
        output.contains("\"event\":\"revision_preparation_stored\""),
        "{output}"
    );
    // Canonical binding: real representation and profile digests, exact algorithms.
    for key in [
        "representation_id",
        "materializer_profile_digest",
        "unitizer_profile_digest",
        "materializer_profile_revision",
        "unitizer_profile_revision",
        "content_digest_algorithm",
        "representation_digest_algorithm",
        "manifest_digest_algorithm",
    ] {
        assert!(output.contains(&format!("\"{key}\":")), "{output}");
    }
    let representation = field(&output, "representation_id");
    assert!(hex_len(representation, 64), "{output}");
    let materializer = field(&output, "materializer_profile_digest");
    assert!(hex_len(materializer, 64), "{output}");
    let unitizer = field(&output, "unitizer_profile_digest");
    assert!(hex_len(unitizer, 64), "{output}");
    assert_ne!(representation, materializer, "{output}");
    assert_ne!(representation, unitizer, "{output}");
    // No ReceiptRef substitution: no receipt fields in preparation binding.
    assert!(!output.contains("receipt"), "{output}");
}

#[test]
fn reopen_uses_identical_manifest_identities_after_restart() {
    let fixture = Fixture::new();
    let first = fixture.index(b"restart identical needle");
    let revision = field(&first, "revision_id").to_owned();
    let first_prepare = fixture.prepare_revision(&revision);
    let first_representation = field(&first_prepare, "representation_id").to_owned();
    // Second explicit prepare is idempotent and binds the same representation.
    let second_prepare = fixture.prepare_revision(&revision);
    let second_representation = field(&second_prepare, "representation_id").to_owned();
    assert_eq!(
        first_representation, second_representation,
        "{second_prepare}"
    );
    // Search after restart uses the same durable preparation.
    let output = fixture.search("needle");
    let rows = matches(&output);
    assert_eq!(rows.len(), 1, "{output}");
    assert!(output.contains("\"complete\":true"), "{output}");
    let again = fixture.search("needle");
    assert_eq!(matches(&again).len(), 1, "{again}");
}

#[test]
fn tampered_manifest_is_detected_not_silent_success() {
    let fixture = Fixture::new();
    fixture.index(b"tamper detection needle");
    let before = fixture.search("needle");
    assert_eq!(matches(&before).len(), 1, "{before}");
    // Flip one byte in the first preparation object.
    let objects = fixture.data.join("preparation").join("objects");
    let mut tampered = false;
    let mut stack = vec![objects];
    while let Some(dir) = stack.pop() {
        if !dir.exists() {
            continue;
        }
        for entry in fs::read_dir(&dir).unwrap() {
            let entry = entry.unwrap();
            let path = entry.path();
            if path.is_dir() {
                stack.push(path);
            } else if path.extension().is_some_and(|e| e == "bin" || e == "dpapi") {
                let mut bytes = fs::read(&path).unwrap();
                assert!(!bytes.is_empty(), "empty preparation object");
                let last = bytes.len() - 1;
                bytes[last] ^= 0x01;
                fs::write(&path, &bytes).unwrap();
                tampered = true;
                break;
            }
        }
        if tampered {
            break;
        }
    }
    assert!(tampered, "no preparation object to tamper");
    let output = fixture.search("needle");
    // Tamper must surface as an explicit gap, never silent success.
    assert!(matches(&output).is_empty(), "{output}");
    assert!(output.contains("\"complete\":false"), "{output}");
}

#[test]
fn missing_preparation_reports_unavailable_without_query_write() {
    let fixture = Fixture::new();
    fixture.index(b"missing preparation needle");
    let before = fixture.count_preparation_files();
    assert!(before > 0, "expected durable preparation files");
    let good = fixture.search("needle");
    assert_eq!(matches(&good).len(), 1, "{good}");
    // Delete durable preparation; query must report unavailable, not rebuild.
    let _ = fs::remove_dir_all(fixture.data.join("preparation"));
    let files_before_search = fixture.count_preparation_files();
    assert_eq!(files_before_search, 0);
    let output = fixture.search("needle");
    assert!(matches(&output).is_empty(), "{output}");
    assert!(output.contains("\"complete\":false"), "{output}");
    assert!(
        output.contains("DIRECT_PREPARATION_UNAVAILABLE")
            || output.contains("DIRECT_PREPARATION_REFERENCE_READ_FAILED")
            || output.contains("DIRECT_PREPARATION_OBJECT"),
        "{output}"
    );
    // Query path performs zero durable writes.
    assert_eq!(fixture.count_preparation_files(), 0, "{output}");
    // Explicit repair rebuilds without touching source state.
    let index_output = fixture.index(b"missing preparation needle");
    let revision = field(&index_output, "revision_id").to_owned();
    let repaired = fixture.prepare_revision(&revision);
    assert!(
        repaired.contains("\"event\":\"revision_preparation_stored\""),
        "{repaired}"
    );
    let after = fixture.search("needle");
    assert_eq!(matches(&after).len(), 1, "{after}");
}

#[test]
fn query_counts_zero_durable_writes_and_keeps_cross_unit_matches() {
    let fixture = Fixture::new();
    let start = 16 * 1024 - 2;
    let text = format!("{}ABCDEFGH{}", "x".repeat(start), "z".repeat(64 * 1024));
    fixture.index(text.as_bytes());
    let before = fixture.count_preparation_files();
    assert!(before > 0);
    let output = fixture.search("ABCDEFGH");
    let rows = matches(&output);
    assert_eq!(rows.len(), 1, "{output}");
    assert!(rows[0].contains(&format!("\"byte_start\":{start},")));
    assert!(output.contains("\"complete\":true"), "{output}");
    assert_eq!(
        fixture.count_preparation_files(),
        before,
        "query must not write preparation"
    );
    // Overlapping matches still exact with durable preparation.
    fixture.index(b"aaaaa");
    let before_second = fixture.count_preparation_files();
    assert!(before_second >= before);
    let overlapping = fixture.search("aaa");
    assert_eq!(matches(&overlapping).len(), 3, "{overlapping}");
    assert_eq!(
        fixture.count_preparation_files(),
        before_second,
        "second query must not write preparation"
    );
}

#[test]
fn truncated_reference_is_detected_not_reinterpreted() {
    let fixture = Fixture::new();
    fixture.index(b"truncated reference needle");
    let good = fixture.search("needle");
    assert_eq!(matches(&good).len(), 1, "{good}");
    // Truncate the first reference file to trigger a bounded failure.
    let refs = fixture.data.join("preparation").join("refs");
    let mut truncated = false;
    let mut stack = vec![refs];
    while let Some(dir) = stack.pop() {
        if !dir.exists() {
            continue;
        }
        for entry in fs::read_dir(&dir).unwrap() {
            let entry = entry.unwrap();
            let path = entry.path();
            if path.is_dir() {
                stack.push(path);
            } else if path.extension().is_some_and(|e| e == "ref") {
                let bytes = fs::read(&path).unwrap();
                assert!(bytes.len() > 1, "reference too small to truncate");
                fs::write(&path, &bytes[..bytes.len() - 1]).unwrap();
                truncated = true;
                break;
            }
        }
        if truncated {
            break;
        }
    }
    assert!(truncated, "no reference to truncate");
    let output = fixture.search("needle");
    assert!(matches(&output).is_empty(), "{output}");
    assert!(output.contains("\"complete\":false"), "{output}");
}

//! Destructive-setup regressions use disposable roots and the primary binary.

use std::fs;
use std::io::Read;
use std::path::{Path, PathBuf};
use std::process::{Command, ExitStatus, Stdio};
use std::sync::atomic::{AtomicU64, Ordering};
use std::thread;
use std::time::{Duration, Instant, SystemTime, UNIX_EPOCH};

static NEXT: AtomicU64 = AtomicU64::new(0);
struct Fixture { base: PathBuf, root: PathBuf, source: PathBuf }
impl Fixture {
    fn new() -> Self {
        let stamp = SystemTime::now().duration_since(UNIX_EPOCH).unwrap().as_nanos();
        let base = std::env::temp_dir().join(format!("eliot-catalog-loss-{}-{stamp}-{}",
            std::process::id(), NEXT.fetch_add(1, Ordering::Relaxed)));
        let root = base.join("data");
        fs::create_dir_all(&root).unwrap();
        let source = base.join("source.txt");
        fs::write(&source, b"irreplaceable historical bytes").unwrap();
        Self { base, root, source }
    }
    fn run(&self, command: &str, extra: &[&Path]) -> (ExitStatus, String) {
        let mut child = Command::new(env!("CARGO_BIN_EXE_eliot-searchd"))
            .arg(command).arg(&self.root).args(extra)
            .stdin(Stdio::null()).stdout(Stdio::piped()).stderr(Stdio::piped())
            .spawn().unwrap();
        let out = child.stdout.take().unwrap();
        let err = child.stderr.take().unwrap();
        let stdout = thread::spawn(move || drain(out));
        let stderr = thread::spawn(move || drain(err));
        let deadline = Instant::now() + Duration::from_secs(30);
        let status = loop {
            if let Some(status) = child.try_wait().unwrap() { break status; }
            if Instant::now() >= deadline {
                let _ = child.kill(); let _ = child.wait();
                panic!("primary daemon did not exit within the test deadline");
            }
            thread::sleep(Duration::from_millis(10));
        };
        (status, format!("{}{}", stdout.join().unwrap(), stderr.join().unwrap()))
    }
    fn index(&self) {
        let (status, output) = self.run("--index-file", &[&self.source]);
        assert!(status.success(), "{output}");
    }
    fn catalog(&self, name: &str) -> PathBuf { self.root.join("control").join(name) }
    fn payloads(&self) -> Vec<(PathBuf, Vec<u8>)> {
        fn walk(path: &Path, files: &mut Vec<(PathBuf, Vec<u8>)>) {
            for entry in fs::read_dir(path).unwrap() {
                let path = entry.unwrap().path();
                if path.is_dir() { walk(&path, files); } else { files.push((path.clone(), fs::read(path).unwrap())); }
            }
        }
        let mut files = Vec::new();
        walk(&self.root.join("revisions"), &mut files);
        files.sort_by(|left, right| left.0.cmp(&right.0));
        files
    }
}
impl Drop for Fixture { fn drop(&mut self) { let _ = fs::remove_dir_all(&self.base); } }
fn drain(reader: impl Read) -> String {
    let mut bytes = Vec::new();
    reader.take(1024 * 1024 + 1).read_to_end(&mut bytes).unwrap();
    assert!(bytes.len() <= 1024 * 1024, "output budget exceeded");
    String::from_utf8(bytes).unwrap()
}

#[test]
fn lost_log_never_creates_an_empty_catalog_or_authorizes_gc() {
    let fixture = Fixture::new(); fixture.index();
    let payloads = fixture.payloads(); assert!(!payloads.is_empty());
    let namespace = fs::read(fixture.catalog("namespace.id")).unwrap();
    fs::remove_file(fixture.catalog("source-events.log")).unwrap();
    for (command, extra) in [
        ("--verify-root", vec![]),
        ("--gc-root", vec![Path::new("--apply")]),
        ("--index-file", vec![fixture.source.as_path()]),
    ] {
        let (status, output) = fixture.run(command, &extra);
        assert!(!status.success(), "{output}");
        assert!(output.contains("RECOVERY_REQUIRED"), "{output}");
        assert!(!fixture.catalog("source-events.log").exists());
        assert_eq!(fs::read(fixture.catalog("namespace.id")).unwrap(), namespace);
        assert_eq!(fixture.payloads(), payloads);
    }
}

#[test]
fn missing_namespace_does_not_rebind_the_existing_corpus() {
    let fixture = Fixture::new(); fixture.index();
    let payloads = fixture.payloads();
    let log = fs::read(fixture.catalog("source-events.log")).unwrap();
    fs::remove_file(fixture.catalog("namespace.id")).unwrap();
    let (status, output) = fixture.run("--verify-root", &[]);
    assert!(!status.success(), "{output}");
    assert!(output.contains("RECOVERY_REQUIRED"), "{output}");
    assert!(!fixture.catalog("namespace.id").exists());
    assert_eq!(fs::read(fixture.catalog("source-events.log")).unwrap(), log);
    assert_eq!(fixture.payloads(), payloads);
}

#[test]
fn lost_catalog_pair_with_surviving_payloads_is_not_a_fresh_installation() {
    let fixture = Fixture::new(); fixture.index();
    let payloads = fixture.payloads();
    for file in ["namespace.id", "source-events.log"] { fs::remove_file(fixture.catalog(file)).unwrap(); }
    let (status, output) = fixture.run("--index-file", &[&fixture.source]);
    assert!(!status.success(), "{output}");
    assert!(output.contains("RECOVERY_REQUIRED"), "{output}");
    for file in ["namespace.id", "source-events.log"] { assert!(!fixture.catalog(file).exists()); }
    assert_eq!(fixture.payloads(), payloads);
}

#[test]
fn pre_registration_can_still_precede_first_corpus_creation() {
    let fixture = Fixture::new();
    let sources = fixture.base.join("registered"); fs::create_dir(&sources).unwrap();
    let (status, output) = fixture.run("--register-source-root", &[&sources]);
    assert!(status.success(), "{output}");
    fixture.index();
    let (status, output) = fixture.run("--verify-root", &[]);
    assert!(status.success(), "{output}");
}

#[test]
fn malformed_existing_log_is_not_silently_reset() {
    let fixture = Fixture::new(); fixture.index();
    let payloads = fixture.payloads();
    fs::write(fixture.catalog("source-events.log"), b"corrupt-and-preserved").unwrap();
    let (status, output) = fixture.run("--verify-root", &[]);
    assert!(!status.success(), "{output}");
    assert_eq!(fs::read(fixture.catalog("source-events.log")).unwrap(), b"corrupt-and-preserved");
    assert_eq!(fixture.payloads(), payloads);
}

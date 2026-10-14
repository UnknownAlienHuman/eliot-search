//! The primary daemon refuses legacy effects before creating a root owner.

use std::ffi::OsString;
use std::fs;
use std::path::{Path, PathBuf};
use std::process::{Command, Output};
use std::time::{SystemTime, UNIX_EPOCH};

const CASES: [(&str, Option<&str>); 6] = [
    ("--index-file", Some("missing-source")),
    ("--index-directory", Some("missing-directory")),
    ("--retire-source", Some("missing-source-id")),
    ("--repair-root", None),
    ("--gc-root", Some("--dry-run")),
    ("--gc-root", Some("--apply")),
];

struct Fixture(PathBuf);

impl Fixture {
    fn new() -> Self {
        let nonce = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_nanos();
        let root = std::env::temp_dir().join(format!(
            "eliot-legacy-refusal-{}-{nonce}",
            std::process::id()
        ));
        fs::create_dir(&root).expect("exclusive fixture directory");
        Self(root)
    }
}

impl Drop for Fixture {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.0);
    }
}

fn invoke(root: &Path, flag: &str, tail: Option<&str>, config: Option<&Path>) -> Output {
    let mut command = Command::new(env!("CARGO_BIN_EXE_eliot-searchd"));
    if let Some(config) = config {
        command.arg("--config-file").arg(config);
    }
    command.arg(flag).arg(root);
    if let Some(tail) = tail {
        command.arg(tail);
    }
    command.output().expect("daemon refusal process")
}

fn assert_refusal(output: &Output) {
    assert_eq!(output.status.code(), Some(2));
    assert!(output.stdout.is_empty());
    assert_eq!(
        String::from_utf8_lossy(&output.stderr).trim(),
        "{\"error\":\"LEGACY_EFFECTS_UNAVAILABLE\"}"
    );
}

#[test]
fn effects_and_gc_preview_refuse_without_creating_or_changing_roots() {
    let fixture = Fixture::new();
    let missing = fixture.0.join("missing-root");
    let existing = fixture.0.join("existing-root");
    fs::create_dir(&existing).unwrap();
    fs::write(existing.join("sentinel"), b"unchanged root bytes").unwrap();
    let missing_config = fixture.0.join("missing-config.toml");
    for (flag, tail) in CASES {
        assert_refusal(&invoke(&missing, flag, tail, None));
        assert!(!missing.exists(), "{flag} created a root before refusing");
        assert_refusal(&invoke(&existing, flag, tail, None));
        assert_refusal(&invoke(&existing, flag, tail, Some(&missing_config)));
        assert_eq!(
            fs::read(existing.join("sentinel")).unwrap(),
            b"unchanged root bytes"
        );
        let entries = fs::read_dir(&existing)
            .unwrap()
            .map(|entry| entry.unwrap().file_name())
            .collect::<Vec<_>>();
        assert_eq!(
            entries,
            vec![OsString::from("sentinel")],
            "{flag} created owner/store state"
        );
    }
}

#[test]
fn incomplete_effect_arguments_still_refuse_and_help_preserves_read_routes() {
    for (flag, _) in CASES {
        assert_refusal(
            &Command::new(env!("CARGO_BIN_EXE_eliot-searchd"))
                .arg(flag)
                .output()
                .unwrap(),
        );
    }
    let help = Command::new(env!("CARGO_BIN_EXE_eliot-searchd"))
        .arg("--help")
        .output()
        .unwrap();
    assert!(help.status.success());
    let text = String::from_utf8(help.stdout).unwrap();
    for (flag, _) in CASES {
        assert!(!text.contains(flag), "help advertises {flag}");
    }
    for flag in [
        "--read-revision",
        "--list-sources",
        "--verify-root",
        "--source-roots",
    ] {
        assert!(text.contains(flag), "help lost {flag}");
    }
}

//! Disabled CLI effects must refuse locally, before daemon selection or spawn.

use std::process::Command;
use std::time::{SystemTime, UNIX_EPOCH};

#[test]
fn every_legacy_effect_refuses_without_spawning_the_selected_daemon() {
    let nonce = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap()
        .as_nanos();
    let missing =
        std::env::temp_dir().join(format!("eliot-cli-refusal-{}-{nonce}", std::process::id()));
    let daemon = missing.join("absent-daemon.exe");
    for (name, tail) in [
        ("index-file", Some("missing-source")),
        ("index-directory", Some("missing-directory")),
        ("retire-source", Some("missing-source-id")),
        ("repair-root", None),
        ("gc-root", Some("--dry-run")),
        ("gc-root", Some("--apply")),
    ] {
        let mut command = Command::new(env!("CARGO_BIN_EXE_eliot-search"));
        command.arg(name).arg(&missing);
        if let Some(tail) = tail {
            command.arg(tail);
        }
        let output = command
            .arg("--daemon")
            .arg(&daemon)
            .env("ELIOT_SEARCHD_BIN", &daemon)
            .output()
            .unwrap();
        assert_eq!(output.status.code(), Some(2), "{name}");
        assert!(output.stdout.is_empty());
        assert_eq!(
            String::from_utf8_lossy(&output.stderr).trim(),
            "{\"error\":\"LEGACY_EFFECTS_UNAVAILABLE\"}",
            "{name} reached daemon forwarding"
        );
        assert!(!missing.exists());
    }
}

#[test]
fn primary_cli_help_exposes_reads_but_no_legacy_effects() {
    let output = Command::new(env!("CARGO_BIN_EXE_eliot-search"))
        .arg("--help")
        .output()
        .unwrap();
    assert!(output.status.success());
    let text = String::from_utf8(output.stdout).unwrap();
    for name in [
        " index-file ",
        " index-directory ",
        " retire-source ",
        " repair-root ",
        " gc-root ",
    ] {
        assert!(!text.contains(name), "help advertises {name}");
    }
    for name in [
        " read-revision ",
        " list-sources ",
        " verify-root ",
        " serve-data-root ",
    ] {
        assert!(text.contains(name), "help lost {name}");
    }
}
